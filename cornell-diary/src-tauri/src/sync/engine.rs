//! `SyncEngine` — the orchestration that pull/push/merges between local
//! Postgres and the Cloud.
//!
//! FAZ 2.1 ships `run_full_cycle` end to end against the existing
//! EntryRepository. FAZ 2.2 layers the scheduler + network monitor on top.
//! FAZ 3 swaps in CRDT for the live editing path; the REST cycle here
//! stays the canonical "everything settled" round-trip.

use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use crate::db::DbPool;
use tokio::sync::Mutex;

use crate::db::{models::CueItem, DiaryEntry, EntryRepository};
use crate::error::DomainError;
use crate::sync::auth::AuthManager;
use crate::sync::client::CloudClient;
use crate::sync::conflict::{decide, ConflictDecision};
use crate::sync::meta;
use crate::sync::models::{
    CloudEntry, ConnectReport, PushEntry, PushRequest, SyncReport, SyncStatus,
};

pub struct SyncEngine {
    repo: Arc<dyn EntryRepository>,
    client: CloudClient,
    auth: Arc<AuthManager>,
    pool: DbPool,
    /// Single-instance gate. The hourly scheduler and a network-up trigger
    /// must not race on dirty rows — the second caller waits.
    cycle_lock: Mutex<()>,
}

impl SyncEngine {
    pub fn new(
        repo: Arc<dyn EntryRepository>,
        client: CloudClient,
        auth: Arc<AuthManager>,
        pool: DbPool,
    ) -> Self {
        Self {
            repo,
            client,
            auth,
            pool,
            cycle_lock: Mutex::new(()),
        }
    }

    /// Convenience for FAZ 2.2 / 2.3 — exposes the same pool the engine
    /// holds so callers (Tauri commands, status polling, etc.) can issue
    /// short DB queries without reconstructing one.
    #[allow(dead_code)]
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    pub async fn connect(
        &self,
        username: &str,
        password: &str,
        device_label: &str,
    ) -> Result<ConnectReport, DomainError> {
        // Step 1: login. We do *not* persist anything until we've also
        // resolved a journal — partial state (token saved, journal_id
        // NULL) leaves the user in a broken "Çevrimiçi: Evet, but every
        // sync errors with cloud_journal_not_selected" loop.
        let tokens = self.client.login(username, password).await?;

        // Step 2: pick or create the journal.
        let journals = self.client.list_journals(&tokens.access_token).await?;
        let chosen = match journals.into_iter().next() {
            Some(j) => j,
            None => {
                self.client
                    .create_journal(&tokens.access_token, "Cornell Diary")
                    .await?
            }
        };

        // Step 3: peer_id is generated locally — Cloud's TokenResponse
        // doesn't include one. Reuse whatever was previously saved (so a
        // disconnect+reconnect from the same device keeps its identity).
        let existing = meta::read(&self.pool).await?;
        let peer_id = if existing.peer_id.is_empty() {
            uuid::Uuid::new_v4().simple().to_string()
        } else {
            existing.peer_id.clone()
        };

        // Step 4: persist atomically — tokens, peer_id, device_label, and
        // journal id all in one happy commit. Anything before this point
        // that errored leaves sync_metadata untouched.
        let exp = tokens.expires_at;
        meta::save_tokens(
            &self.pool,
            &tokens.access_token,
            &tokens.refresh_token,
            exp,
            tokens.user_id,
            Some(&peer_id),
            Some(device_label),
        )
        .await?;
        meta::save_journal(&self.pool, chosen.id).await?;

        Ok(ConnectReport {
            user_id: tokens.user_id,
            peer_id,
            journal_id: chosen.id,
            journal_name: chosen.title,
        })
    }

    pub async fn disconnect(&self) -> Result<(), DomainError> {
        meta::clear(&self.pool).await
    }

    pub async fn status(&self, online: bool) -> Result<SyncStatus, DomainError> {
        let m = meta::read(&self.pool).await?;
        let dirty = self.count_dirty().await?;
        Ok(SyncStatus {
            enabled: m.sync_enabled,
            online,
            last_pull_at: m.last_pull_at,
            last_push_at: m.last_push_at,
            dirty_count: dirty,
        })
    }

    pub async fn run_full_cycle(&self) -> Result<SyncReport, DomainError> {
        if !self.auth.is_connected().await? {
            return Err(DomainError::Validation("not connected to cloud".into()));
        }

        // Hold the single-instance gate for the entire cycle. Cheap: the
        // critical section is one round-trip per minute at most.
        let _guard = self.cycle_lock.lock().await;
        let started = Instant::now();
        let mut report = SyncReport::default();

        let metadata = meta::read(&self.pool).await?;
        let journal_id = metadata.cloud_journal_id.ok_or_else(|| {
            DomainError::Validation("cloud journal not selected — re-connect".into())
        })?;
        if metadata.peer_id.is_empty() {
            return Err(DomainError::Validation(
                "peer_id missing — re-connect".into(),
            ));
        }
        // ---------- PULL ----------
        // Both pull + push go through `with_retry`: a Cloud 401 mid-sync
        // (server restart, manual revoke) force-refreshes the token and
        // retries once. Without this, a long-idle session every few
        // hours surfaces as a "token_invalid" red box requiring manual
        // disconnect/reconnect.
        let last_pull = metadata.last_pull_at;
        let pull = crate::sync::auth::with_retry(
            &self.auth,
            self.client.clone(),
            move |c, token| async move { c.pull(&token, journal_id, last_pull).await },
        )
        .await?;
        for cloud_entry in pull.entries {
            self.merge_remote(cloud_entry, &mut report).await?;
        }
        // Use Cloud's authoritative server clock as the next watermark when
        // it provides one; otherwise fall back to local Utc::now().
        let pull_watermark = pull.server_time.unwrap_or_else(Utc::now);
        meta::save_pull_at(&self.pool, pull_watermark).await?;

        // ---------- PUSH ----------
        let dirty = self.list_dirty_entries().await?;
        if !dirty.is_empty() {
            let body = PushRequest {
                journal_id,
                peer_id: metadata.peer_id.clone(),
                device_label: metadata.device_label.clone(),
                idempotency_key: Some(uuid::Uuid::new_v4().simple().to_string()),
                entries: dirty.iter().map(push_entry_from).collect(),
                crdt_ops: Vec::new(),
            };
            let resp =
                crate::sync::auth::with_retry(&self.auth, self.client.clone(), move |c, token| {
                    let body = body.clone();
                    async move { c.push(&token, &body).await }
                })
                .await?;
            for merged in &resp.merged_entries {
                self.mark_synced(&merged.entry_date, merged.id, merged.version)
                    .await?;
                report.pushed += 1;
            }
            meta::save_push_at(&self.pool, Utc::now()).await?;
        }

        report.duration_ms = started.elapsed().as_millis() as u64;
        Ok(report)
    }

    // ------------------------------------------------------------------
    // internals
    async fn merge_remote(
        &self,
        cloud: CloudEntry,
        report: &mut SyncReport,
    ) -> Result<(), DomainError> {
        let local = self.repo.get_by_date(&cloud.entry_date).await?;
        let (is_dirty, updated_at) = self
            .read_dirty_and_updated(&cloud.entry_date)
            .await
            .unwrap_or((false, None));

        match decide(local.as_ref(), is_dirty, updated_at, &cloud) {
            ConflictDecision::InsertCloud | ConflictDecision::OverwriteWithCloud => {
                self.write_from_cloud(&cloud).await?;
                report.pulled += 1;
            }
            ConflictDecision::CloudWonOverDirtyLocal => {
                if let Some(local) = local.as_ref() {
                    self.archive_local(local).await?;
                }
                self.write_from_cloud(&cloud).await?;
                report.pulled += 1;
                report.conflicts_cloud_won += 1;
            }
            ConflictDecision::LocalWon => {
                report.conflicts_local_won += 1;
            }
            ConflictDecision::LocalAlreadyFresher => {}
        }
        Ok(())
    }

    async fn write_from_cloud(&self, cloud: &CloudEntry) -> Result<(), DomainError> {
        let entry = entry_from_cloud(cloud);
        self.repo.upsert(entry).await?;
        // Mark the freshly-written row as synced so the same content doesn't
        // get pushed back next cycle.
        self.mark_synced(&cloud.entry_date, cloud.id, cloud.version)
            .await?;
        Ok(())
    }

    async fn archive_local(&self, local: &DiaryEntry) -> Result<(), DomainError> {
        // Persist the loser into sync_log so users can recover it if the
        // last-write-wins call was wrong. Audit only — the row is JSON.
        let payload = serde_json::to_string(local)
            .map_err(|e| DomainError::Internal(format!("serialise loser: {e}")))?;
        sqlx::query(
            "INSERT INTO sync_log (sync_type, method, device_id, timestamp, entry_count, \
                checksum, status, error_message) \
             VALUES ('export', 'cloud', $1, now(), 1, $2, 'partial', $3)",
        )
        .bind(local.device_id.as_deref().unwrap_or(""))
        .bind(format!("conflict:{}", local.date))
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::Storage(format!("archive_local: {e}")))?;
        Ok(())
    }

    async fn list_dirty_entries(&self) -> Result<Vec<DirtyEntry>, DomainError> {
        let rows = sqlx::query_as::<_, DirtyEntryRow>(
            "SELECT date, diary, \
                title_1, content_1, title_2, content_2, title_3, content_3, \
                title_4, content_4, title_5, content_5, title_6, content_6, \
                title_7, content_7, summary, quote, version, baseline_version, \
                updated_at, cloud_entry_id \
             FROM diary_entries WHERE is_dirty = TRUE ORDER BY date",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Storage(format!("list_dirty: {e}")))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn count_dirty(&self) -> Result<i64, DomainError> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*)::BIGINT FROM diary_entries WHERE is_dirty = TRUE")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| DomainError::Storage(format!("count_dirty: {e}")))?;
        Ok(row.0)
    }

    async fn read_dirty_and_updated(
        &self,
        date: &str,
    ) -> Result<(bool, Option<DateTime<Utc>>), DomainError> {
        let row: Option<(bool, String)> =
            sqlx::query_as("SELECT is_dirty, updated_at FROM diary_entries WHERE date = $1")
                .bind(date)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| DomainError::Storage(format!("read dirty/updated: {e}")))?;
        Ok(match row {
            Some((dirty, updated_at)) => {
                let parsed = DateTime::parse_from_rfc3339(&updated_at)
                    .ok()
                    .map(|d| d.with_timezone(&Utc));
                (dirty, parsed)
            }
            None => (false, None),
        })
    }

    async fn mark_synced(
        &self,
        date: &str,
        cloud_id: uuid::Uuid,
        version: i64,
    ) -> Result<(), DomainError> {
        // baseline_version is pinned to the server version we just
        // observed — the next local edit's push will use this as the
        // baseline for Cloud's CRDT-aware merge.
        // last_synced_at is bound (instead of SQL `now()`) so the same
        // statement compiles under SQLite (no `now()` function) too.
        sqlx::query(
            "UPDATE diary_entries SET cloud_entry_id = $1, version = $2, \
                baseline_version = $2, is_dirty = FALSE, \
                last_synced_at = $3 WHERE date = $4",
        )
        .bind(cloud_id)
        .bind(version)
        .bind(Utc::now())
        .bind(date)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::Storage(format!("mark_synced: {e}")))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// row helpers — keep the engine free of column-name strings.
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct DirtyEntryRow {
    date: String,
    diary: String,
    title_1: Option<String>,
    content_1: Option<String>,
    title_2: Option<String>,
    content_2: Option<String>,
    title_3: Option<String>,
    content_3: Option<String>,
    title_4: Option<String>,
    content_4: Option<String>,
    title_5: Option<String>,
    content_5: Option<String>,
    title_6: Option<String>,
    content_6: Option<String>,
    title_7: Option<String>,
    content_7: Option<String>,
    summary: String,
    quote: String,
    version: i64,
    baseline_version: i64,
    updated_at: String,
    cloud_entry_id: Option<uuid::Uuid>,
}

struct DirtyEntry {
    date: String,
    diary: String,
    titles: [Option<String>; 7],
    contents: [Option<String>; 7],
    summary: String,
    quote: String,
    version: i64,
    /// Last server `version` we observed via mark_synced. Sent in
    /// PushEntry.baseline_version so Cloud's CRDT-aware merge can
    /// detect a concurrent writer that landed since this baseline.
    baseline_version: i64,
    updated_at: DateTime<Utc>,
    cloud_id: Option<uuid::Uuid>,
}

impl From<DirtyEntryRow> for DirtyEntry {
    fn from(r: DirtyEntryRow) -> Self {
        let updated_at = DateTime::parse_from_rfc3339(&r.updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        Self {
            date: r.date,
            diary: r.diary,
            titles: [
                r.title_1, r.title_2, r.title_3, r.title_4, r.title_5, r.title_6, r.title_7,
            ],
            contents: [
                r.content_1,
                r.content_2,
                r.content_3,
                r.content_4,
                r.content_5,
                r.content_6,
                r.content_7,
            ],
            summary: r.summary,
            quote: r.quote,
            version: r.version,
            baseline_version: r.baseline_version,
            updated_at,
            cloud_id: r.cloud_entry_id,
        }
    }
}

/// Local DiaryEntry → Cloud PushEntry.
///
/// Cloud uses a flat-string projection for cue items:
/// `cue_column = "{title_1}: {content_1}\n{title_2}: {content_2}\n…"` (only
/// non-empty cells, position-ordered). This matches the Reporter sidecar's
/// projection so any tool consuming Cloud directly sees the same shape.
/// `notes_column = diary`, `planlar = quote`. The mapping is lossy on
/// position when re-imported (cue_items pull back onto positions 1..N
/// densely instead of preserving sparse positions), but that's acceptable
/// for FAZ 2 — Phase 3 char-CRDT handles per-field sync directly.
fn push_entry_from(d: &DirtyEntry) -> PushEntry {
    PushEntry {
        id: d.cloud_id,
        entry_date: d.date.clone(),
        cue_column: encode_cue_column(&d.titles, &d.contents),
        notes_column: d.diary.clone(),
        summary: d.summary.clone(),
        planlar: d.quote.clone(),
        version: d.version,
        last_modified_at: d.updated_at,
        // Faz 1.1: tell Cloud which server version we last saw. None
        // when the row has never been synced — Cloud's crdt path then
        // falls through to lmw.
        baseline_version: if d.baseline_version > 0 {
            Some(d.baseline_version)
        } else {
            None
        },
    }
}

/// Cloud EntryOut → local DiaryEntry. Inverse of `push_entry_from`.
fn entry_from_cloud(c: &CloudEntry) -> DiaryEntry {
    let cue_items = decode_cue_column(&c.cue_column);
    let modified_at = c.modified_at_or_now();
    let created_at = c.created_at.unwrap_or(modified_at);

    DiaryEntry {
        date: c.entry_date.clone(),
        diary: c.notes_column.clone(),
        cue_items,
        summary: c.summary.clone(),
        quote: c.planlar.clone(),
        created_at: created_at.to_rfc3339(),
        updated_at: modified_at.to_rfc3339(),
        device_id: None,
        version: c.version,
    }
}

fn encode_cue_column(titles: &[Option<String>; 7], contents: &[Option<String>; 7]) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(7);
    for i in 0..7 {
        let title = titles[i].as_deref().unwrap_or("").trim();
        let content = contents[i].as_deref().unwrap_or("").trim();
        if title.is_empty() && content.is_empty() {
            continue;
        }
        lines.push(format!("{title}: {content}"));
    }
    lines.join("\n")
}

fn decode_cue_column(cue_column: &str) -> Vec<CueItem> {
    let mut out = Vec::with_capacity(7);
    let mut pos: u32 = 1;
    for line in cue_column.lines() {
        if pos > 7 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // "Title: content" — split on the first colon. Anything without a
        // colon becomes a content-only cue with an empty title.
        let (title, content) = match line.find(':') {
            Some(idx) => {
                let title = line[..idx].trim().to_string();
                let content = line[idx + 1..].trim().to_string();
                (title, content)
            }
            None => (String::new(), line.to_string()),
        };
        out.push(CueItem {
            position: pos,
            title,
            content,
        });
        pos += 1;
    }
    out
}

#[cfg(test)]
mod cue_codec_tests {
    use super::*;

    #[test]
    fn encode_skips_fully_empty_slots_and_keeps_positions_packed() {
        let titles = [
            Some("Plan".into()),
            None,
            Some("Mood".into()),
            None,
            None,
            None,
            None,
        ];
        let contents = [
            Some("ship sync".into()),
            None,
            Some("focused".into()),
            None,
            None,
            None,
            None,
        ];
        assert_eq!(
            encode_cue_column(&titles, &contents),
            "Plan: ship sync\nMood: focused"
        );
    }

    #[test]
    fn round_trip_preserves_title_and_content() {
        let titles = [
            Some("A".into()),
            Some("B".into()),
            None,
            None,
            None,
            None,
            None,
        ];
        let contents = [
            Some("alpha".into()),
            Some("beta".into()),
            None,
            None,
            None,
            None,
            None,
        ];
        let encoded = encode_cue_column(&titles, &contents);
        let decoded = decode_cue_column(&encoded);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].position, 1);
        assert_eq!(decoded[0].title, "A");
        assert_eq!(decoded[0].content, "alpha");
        assert_eq!(decoded[1].position, 2);
        assert_eq!(decoded[1].title, "B");
        assert_eq!(decoded[1].content, "beta");
    }

    #[test]
    fn decode_handles_lines_without_colon() {
        let decoded = decode_cue_column("just a thought\nAnother: with title");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].title, "");
        assert_eq!(decoded[0].content, "just a thought");
        assert_eq!(decoded[1].title, "Another");
        assert_eq!(decoded[1].content, "with title");
    }
}

#[cfg(test)]
mod baseline_version_tests {
    //! Faz 1.1: PushEntry must carry the last-seen server version so
    //! Cloud's CRDT-aware merge can refuse stale-baseline overwrites.

    use super::*;

    fn dirty(version: i64, baseline: i64, cloud_id: Option<uuid::Uuid>) -> DirtyEntry {
        DirtyEntry {
            date: "2026-05-01".into(),
            diary: "today's note".into(),
            titles: [None, None, None, None, None, None, None],
            contents: [None, None, None, None, None, None, None],
            summary: String::new(),
            quote: String::new(),
            version,
            baseline_version: baseline,
            updated_at: Utc::now(),
            cloud_id,
        }
    }

    #[test]
    fn push_entry_omits_baseline_for_never_synced_rows() {
        // baseline=0 → first push, no concurrent writer to detect.
        // Wire shape stays pre-1.1: `baseline_version` field absent.
        let json = serde_json::to_value(push_entry_from(&dirty(1, 0, None))).unwrap();
        assert!(json.get("baseline_version").is_none());
    }

    #[test]
    fn push_entry_includes_baseline_after_first_sync() {
        // baseline=3 (the version we last received from Cloud) → next
        // push attaches it so Cloud can compare against current.
        let id = Some(uuid::Uuid::new_v4());
        let json = serde_json::to_value(push_entry_from(&dirty(4, 3, id))).unwrap();
        assert_eq!(json["baseline_version"], 3);
        // version is the post-local-edit value (= baseline + 1 here);
        // server may bump further on its own.
        assert_eq!(json["version"], 4);
    }
}
