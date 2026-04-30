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
use sqlx::PgPool;
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
    pool: PgPool,
    /// Single-instance gate. The hourly scheduler and a network-up trigger
    /// must not race on dirty rows — the second caller waits.
    cycle_lock: Mutex<()>,
}

impl SyncEngine {
    pub fn new(
        repo: Arc<dyn EntryRepository>,
        client: CloudClient,
        auth: Arc<AuthManager>,
        pool: PgPool,
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
    pub fn pool(&self) -> &PgPool {
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
        let token = self.auth.get_or_refresh(&self.client).await?;

        // ---------- PULL ----------
        let pull = self
            .client
            .pull(&token, journal_id, metadata.last_pull_at)
            .await?;
        for cloud_entry in pull.entries {
            self.merge_remote(cloud_entry, &mut report).await?;
        }
        meta::save_pull_at(&self.pool, Utc::now()).await?;

        // ---------- PUSH ----------
        let dirty = self.list_dirty_entries().await?;
        if !dirty.is_empty() {
            let body = PushRequest {
                journal_id,
                entries: dirty.iter().map(push_entry_from).collect(),
            };
            let resp = self.client.push(&token, &body).await?;
            for merged in &resp.merged {
                self.mark_synced(&merged.local_date, merged.cloud_id, merged.version)
                    .await?;
                report.pushed += 1;
            }
            report.rejected += resp.rejected.len() as u32;
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
                title_7, content_7, summary, quote, version, updated_at, cloud_entry_id \
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
        sqlx::query(
            "UPDATE diary_entries SET cloud_entry_id = $1, version = $2, is_dirty = FALSE, \
                last_synced_at = now() WHERE date = $3",
        )
        .bind(cloud_id)
        .bind(version)
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
            updated_at,
            cloud_id: r.cloud_entry_id,
        }
    }
}

fn push_entry_from(d: &DirtyEntry) -> PushEntry {
    PushEntry {
        id: d.cloud_id,
        entry_date: d.date.clone(),
        diary: d.diary.clone(),
        title_1: d.titles[0].clone(),
        content_1: d.contents[0].clone(),
        title_2: d.titles[1].clone(),
        content_2: d.contents[1].clone(),
        title_3: d.titles[2].clone(),
        content_3: d.contents[2].clone(),
        title_4: d.titles[3].clone(),
        content_4: d.contents[3].clone(),
        title_5: d.titles[4].clone(),
        content_5: d.contents[4].clone(),
        title_6: d.titles[5].clone(),
        content_6: d.contents[5].clone(),
        title_7: d.titles[6].clone(),
        content_7: d.contents[6].clone(),
        summary: d.summary.clone(),
        quote: d.quote.clone(),
        version: d.version,
        last_modified_at: d.updated_at,
    }
}

fn entry_from_cloud(c: &CloudEntry) -> DiaryEntry {
    let mut cue_items = Vec::with_capacity(7);
    let titles = [
        &c.title_1, &c.title_2, &c.title_3, &c.title_4, &c.title_5, &c.title_6, &c.title_7,
    ];
    let contents = [
        &c.content_1,
        &c.content_2,
        &c.content_3,
        &c.content_4,
        &c.content_5,
        &c.content_6,
        &c.content_7,
    ];
    for (i, title) in titles.iter().enumerate() {
        if let Some(title) = title.as_ref() {
            cue_items.push(CueItem {
                position: (i as u32) + 1,
                title: title.clone(),
                content: contents[i].clone().unwrap_or_default(),
            });
        }
    }

    DiaryEntry {
        date: c.entry_date.clone(),
        diary: c.diary.clone(),
        cue_items,
        summary: c.summary.clone(),
        quote: c.quote.clone(),
        created_at: c.created_at.to_rfc3339(),
        updated_at: c.last_modified_at.to_rfc3339(),
        device_id: None,
        version: c.version,
    }
}
