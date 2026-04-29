//! SQLite implementation of `EntryRepository`. Bridges the existing on-disk
//! Cornell Diary SQLite file (`~/Library/Application Support/com.deniz.cornelldiary/cornell_diary.db`
//! on macOS) until FAZ 1.1 ships a Postgres implementation that lives behind
//! the same trait.
//!
//! Concurrency model: rusqlite is blocking, so every method wraps its work
//! in `tokio::task::spawn_blocking`. The connection is shared behind a
//! `parking_lot::Mutex`; SQLite serialises writes inside the database engine
//! anyway, so the user-space mutex is just a borrow checker.
//!
//! The schema is identical to `migrations/001_initial.sql`; `init` runs the
//! same `CREATE TABLE IF NOT EXISTS` statements so a fresh DB is usable
//! out of the box.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};

use crate::db::models::{BulkResult, CueItem, DiaryEntry, MAX_CUE_ITEMS};
use crate::db::repository::EntryRepository;
use crate::error::DomainError;

const SCHEMA_SQL: &str = include_str!("../../migrations/001_initial.sql");

pub struct SqliteEntryRepository {
    db_path: PathBuf,
    conn: Arc<Mutex<Option<Connection>>>,
}

impl SqliteEntryRepository {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            conn: Arc::new(Mutex::new(None)),
        }
    }

    fn run_blocking<F, T>(&self, f: F) -> tokio::task::JoinHandle<Result<T, DomainError>>
    where
        F: FnOnce(&mut Connection) -> Result<T, DomainError> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = conn
                .lock()
                .map_err(|e| DomainError::Internal(format!("mutex poisoned: {e}")))?;
            let conn = guard.as_mut().ok_or(DomainError::NotInitialized)?;
            f(conn)
        })
    }
}

#[async_trait]
impl EntryRepository for SqliteEntryRepository {
    async fn init(&self) -> Result<(), DomainError> {
        let path = self.db_path.clone();
        let slot = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<(), DomainError> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| DomainError::Storage(format!("mkdir parent: {e}")))?;
            }
            let conn = Connection::open(&path)?;
            // WAL gives us readers + one writer concurrently; safer than the
            // default DELETE journal mode under Tauri's async runtime.
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "foreign_keys", "ON")?;
            conn.execute_batch(SCHEMA_SQL)?;
            let mut guard = slot
                .lock()
                .map_err(|e| DomainError::Internal(format!("mutex poisoned: {e}")))?;
            *guard = Some(conn);
            Ok(())
        })
        .await??;
        Ok(())
    }

    async fn get_by_date(&self, date: &str) -> Result<Option<DiaryEntry>, DomainError> {
        validate_date(date)?;
        let date = date.to_string();
        self.run_blocking(move |conn| {
            let mut stmt = conn.prepare(SELECT_ALL_COLUMNS_BY_DATE)?;
            let row = stmt.query_row(params![date], row_to_entry).optional()?;
            Ok(row)
        })
        .await?
    }

    async fn upsert(&self, entry: DiaryEntry) -> Result<DiaryEntry, DomainError> {
        validate_date(&entry.date)?;
        validate_cue_items(&entry.cue_items)?;
        let date = entry.date.clone();
        let entry_clone = entry.clone();

        // Mirror SQLiteRepository.upsert: bumps version on conflict.
        self.run_blocking(move |conn| {
            let mut entry = entry_clone;
            entry.updated_at = Utc::now().to_rfc3339();
            // SQLiteRepository sets created_at on first insert; honour the
            // input value if present (sync paths supply their own).
            if entry.created_at.is_empty() {
                entry.created_at = entry.updated_at.clone();
            }
            let cells = expand_cue_cells(&entry.cue_items);

            conn.execute(
                UPSERT_BUMP_VERSION_SQL,
                params![
                    entry.date,
                    entry.diary,
                    cells.0,
                    cells.1,
                    cells.2,
                    cells.3,
                    cells.4,
                    cells.5,
                    cells.6,
                    cells.7,
                    cells.8,
                    cells.9,
                    cells.10,
                    cells.11,
                    cells.12,
                    cells.13,
                    entry.summary,
                    entry.quote,
                    entry.created_at,
                    entry.updated_at,
                    entry.device_id,
                    entry.version,
                ],
            )?;
            Ok(())
        })
        .await??;

        match self.get_by_date(&date).await? {
            Some(saved) => Ok(saved),
            None => Err(DomainError::Storage(format!(
                "upsert succeeded but read failed for {date}"
            ))),
        }
    }

    async fn delete(&self, date: &str) -> Result<(), DomainError> {
        validate_date(date)?;
        let date = date.to_string();
        self.run_blocking(move |conn| {
            conn.execute("DELETE FROM diary_entries WHERE date = ?1", params![date])?;
            Ok(())
        })
        .await?
    }

    async fn list_dates(&self) -> Result<Vec<String>, DomainError> {
        self.run_blocking(|conn| {
            let mut stmt = conn.prepare("SELECT date FROM diary_entries ORDER BY date DESC")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?
    }

    async fn list_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<DiaryEntry>, DomainError> {
        validate_date(start_date)?;
        validate_date(end_date)?;
        let start = start_date.to_string();
        let end = end_date.to_string();
        self.run_blocking(move |conn| {
            let sql =
                format!("{SELECT_ALL_COLUMNS} WHERE date >= ?1 AND date <= ?2 ORDER BY date DESC");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![start, end], row_to_entry)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?
    }

    async fn list_all(&self) -> Result<Vec<DiaryEntry>, DomainError> {
        self.run_blocking(|conn| {
            let sql = format!("{SELECT_ALL_COLUMNS} ORDER BY date DESC");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], row_to_entry)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Vec<DiaryEntry>, DomainError> {
        let q = format!("%{query}%");
        let limit = limit.max(1) as i64;
        self.run_blocking(move |conn| {
            let sql = format!(
                "{SELECT_ALL_COLUMNS} \
                 WHERE diary LIKE ?1 OR summary LIKE ?1 OR quote LIKE ?1 \
                    OR content_1 LIKE ?1 OR content_2 LIKE ?1 OR content_3 LIKE ?1 \
                    OR content_4 LIKE ?1 OR content_5 LIKE ?1 OR content_6 LIKE ?1 \
                    OR content_7 LIKE ?1 \
                    OR title_1 LIKE ?1 OR title_2 LIKE ?1 OR title_3 LIKE ?1 \
                    OR title_4 LIKE ?1 OR title_5 LIKE ?1 OR title_6 LIKE ?1 \
                    OR title_7 LIKE ?1 \
                 ORDER BY date DESC LIMIT ?2"
            );
            let mut stmt = conn.prepare(&sql)?;
            let params_iter = params_from_iter::<[&dyn rusqlite::ToSql; 2]>([&q, &limit]);
            let rows = stmt
                .query_map(params_iter, row_to_entry)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?
    }

    async fn entry_count(&self) -> Result<i64, DomainError> {
        self.run_blocking(|conn| {
            let n: i64 =
                conn.query_row("SELECT COUNT(*) FROM diary_entries", [], |row| row.get(0))?;
            Ok(n)
        })
        .await?
    }

    async fn last_updated_at(&self) -> Result<Option<String>, DomainError> {
        self.run_blocking(|conn| {
            let v: Option<String> = conn
                .query_row(
                    "SELECT updated_at FROM diary_entries ORDER BY updated_at DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(v)
        })
        .await?
    }

    async fn bulk_upsert(&self, entries: Vec<DiaryEntry>) -> Result<BulkResult, DomainError> {
        for e in &entries {
            validate_date(&e.date)?;
            validate_cue_items(&e.cue_items)?;
        }
        self.run_blocking(move |conn| {
            let mut inserted = 0u32;
            let mut skipped = 0u32;
            let tx = conn.transaction()?;
            for entry in entries {
                let exists: bool = tx
                    .query_row(
                        "SELECT 1 FROM diary_entries WHERE date = ?1",
                        params![entry.date],
                        |_| Ok(true),
                    )
                    .optional()?
                    .unwrap_or(false);
                if exists {
                    skipped += 1;
                    continue;
                }
                let cells = expand_cue_cells(&entry.cue_items);
                tx.execute(
                    INSERT_RAW_SQL,
                    params![
                        entry.date,
                        entry.diary,
                        cells.0,
                        cells.1,
                        cells.2,
                        cells.3,
                        cells.4,
                        cells.5,
                        cells.6,
                        cells.7,
                        cells.8,
                        cells.9,
                        cells.10,
                        cells.11,
                        cells.12,
                        cells.13,
                        entry.summary,
                        entry.quote,
                        entry.created_at,
                        entry.updated_at,
                        entry.device_id,
                        entry.version,
                    ],
                )?;
                inserted += 1;
            }
            tx.commit()?;
            Ok(BulkResult {
                inserted,
                updated: 0,
                skipped,
            })
        })
        .await?
    }

    async fn get_setting(&self, key: &str) -> Result<Option<String>, DomainError> {
        let key = key.to_string();
        self.run_blocking(move |conn| {
            let v: Option<String> = conn
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(v)
        })
        .await?
    }

    async fn set_setting(&self, key: &str, value: &str) -> Result<(), DomainError> {
        let key = key.to_string();
        let value = value.to_string();
        let now = Utc::now().to_rfc3339();
        self.run_blocking(move |conn| {
            conn.execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value, now],
            )?;
            Ok(())
        })
        .await?
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SELECT_ALL_COLUMNS: &str = "SELECT date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version FROM diary_entries";

const SELECT_ALL_COLUMNS_BY_DATE: &str = "SELECT date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version FROM diary_entries WHERE date = ?1";

const UPSERT_BUMP_VERSION_SQL: &str = "INSERT INTO diary_entries ( \
    date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version) VALUES ( \
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
    ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22) \
    ON CONFLICT(date) DO UPDATE SET \
    diary = excluded.diary, \
    title_1 = excluded.title_1, content_1 = excluded.content_1, \
    title_2 = excluded.title_2, content_2 = excluded.content_2, \
    title_3 = excluded.title_3, content_3 = excluded.content_3, \
    title_4 = excluded.title_4, content_4 = excluded.content_4, \
    title_5 = excluded.title_5, content_5 = excluded.content_5, \
    title_6 = excluded.title_6, content_6 = excluded.content_6, \
    title_7 = excluded.title_7, content_7 = excluded.content_7, \
    summary = excluded.summary, \
    quote = excluded.quote, \
    updated_at = excluded.updated_at, \
    device_id = excluded.device_id, \
    version = diary_entries.version + 1";

const INSERT_RAW_SQL: &str = "INSERT INTO diary_entries ( \
    date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version) VALUES ( \
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
    ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)";

fn validate_date(date: &str) -> Result<(), DomainError> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| DomainError::InvalidDate(date.to_string()))
}

fn validate_cue_items(items: &[CueItem]) -> Result<(), DomainError> {
    if items.len() > MAX_CUE_ITEMS as usize {
        return Err(DomainError::Validation(format!(
            "too many cue items: {} (max {})",
            items.len(),
            MAX_CUE_ITEMS
        )));
    }
    let mut seen = [false; MAX_CUE_ITEMS as usize];
    for item in items {
        if item.position < 1 || item.position > MAX_CUE_ITEMS {
            return Err(DomainError::Validation(format!(
                "invalid cue position: {}",
                item.position
            )));
        }
        let idx = (item.position - 1) as usize;
        if seen[idx] {
            return Err(DomainError::Validation(format!(
                "duplicate cue position: {}",
                item.position
            )));
        }
        seen[idx] = true;
    }
    Ok(())
}

/// Spread `cue_items` into the 14 column slots `(title_1..7, content_1..7)`.
/// Each slot is `Option<String>`: `None` means the column stays NULL, matching
/// the original "title null = unused" semantics.
type CueCells = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn expand_cue_cells(items: &[CueItem]) -> CueCells {
    let mut titles: [Option<String>; MAX_CUE_ITEMS as usize] = Default::default();
    let mut contents: [Option<String>; MAX_CUE_ITEMS as usize] = Default::default();
    for item in items {
        let idx = (item.position - 1) as usize;
        titles[idx] = Some(item.title.clone());
        contents[idx] = Some(item.content.clone());
    }
    (
        titles[0].clone(),
        contents[0].clone(),
        titles[1].clone(),
        contents[1].clone(),
        titles[2].clone(),
        contents[2].clone(),
        titles[3].clone(),
        contents[3].clone(),
        titles[4].clone(),
        contents[4].clone(),
        titles[5].clone(),
        contents[5].clone(),
        titles[6].clone(),
        contents[6].clone(),
    )
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<DiaryEntry> {
    let mut cue_items = Vec::with_capacity(MAX_CUE_ITEMS as usize);
    for i in 0..MAX_CUE_ITEMS {
        // Columns: 0=date, 1=diary, then alternating title/content from index 2.
        let title_col = 2 + (i * 2);
        let content_col = title_col + 1;
        let title: Option<String> = row.get(title_col as usize)?;
        let content: Option<String> = row.get(content_col as usize)?;
        if let Some(title) = title {
            cue_items.push(CueItem {
                position: i + 1,
                title,
                content: content.unwrap_or_default(),
            });
        }
    }
    cue_items.sort_by_key(|c| c.position);

    Ok(DiaryEntry {
        date: row.get(0)?,
        diary: row.get(1)?,
        cue_items,
        summary: row.get::<_, Option<String>>(16)?.unwrap_or_default(),
        quote: row.get::<_, Option<String>>(17)?.unwrap_or_default(),
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
        device_id: row.get(20)?,
        version: row.get(21)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_entry(date: &str) -> DiaryEntry {
        DiaryEntry {
            date: date.to_string(),
            diary: "ben uğurböceğinden korkarım".to_string(),
            cue_items: vec![
                CueItem {
                    position: 1,
                    title: "Reflection".to_string(),
                    content: "deep thoughts".to_string(),
                },
                CueItem {
                    position: 3,
                    title: "Goal".to_string(),
                    content: "ship phase 1".to_string(),
                },
            ],
            summary: "good day".to_string(),
            quote: "carpe diem".to_string(),
            created_at: "2026-04-29T10:00:00Z".to_string(),
            updated_at: "2026-04-29T10:00:00Z".to_string(),
            device_id: Some("test-device".to_string()),
            version: 1,
        }
    }

    async fn fresh_repo() -> SqliteEntryRepository {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        // Leak the tempdir so the file survives until the test ends.
        std::mem::forget(dir);
        let repo = SqliteEntryRepository::new(path);
        repo.init().await.unwrap();
        repo
    }

    #[tokio::test]
    async fn upsert_then_get_round_trips_all_fields() {
        let repo = fresh_repo().await;
        let entry = sample_entry("2026-04-29");
        repo.upsert(entry.clone()).await.unwrap();

        let fetched = repo.get_by_date("2026-04-29").await.unwrap().unwrap();
        assert_eq!(fetched.diary, entry.diary);
        assert_eq!(fetched.cue_items.len(), 2);
        assert_eq!(fetched.cue_items[0].position, 1);
        assert_eq!(fetched.cue_items[1].position, 3);
        assert_eq!(fetched.summary, "good day");
        assert_eq!(fetched.quote, "carpe diem");
        assert_eq!(fetched.device_id.as_deref(), Some("test-device"));
    }

    #[tokio::test]
    async fn upsert_bumps_version_on_conflict() {
        let repo = fresh_repo().await;
        let mut entry = sample_entry("2026-04-29");
        repo.upsert(entry.clone()).await.unwrap();
        entry.diary = "edited".to_string();
        let saved = repo.upsert(entry).await.unwrap();
        assert_eq!(saved.version, 2);
        assert_eq!(saved.diary, "edited");
    }

    #[tokio::test]
    async fn missing_date_returns_none() {
        let repo = fresh_repo().await;
        let got = repo.get_by_date("2030-01-01").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn list_range_filters_by_date_inclusive() {
        let repo = fresh_repo().await;
        repo.upsert(sample_entry("2026-04-27")).await.unwrap();
        repo.upsert(sample_entry("2026-04-28")).await.unwrap();
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        let in_range = repo.list_range("2026-04-28", "2026-04-29").await.unwrap();
        assert_eq!(in_range.len(), 2);
    }

    #[tokio::test]
    async fn search_matches_diary_and_cue_content() {
        let repo = fresh_repo().await;
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        let hits_diary = repo.search("uğurböceğ", 50).await.unwrap();
        assert_eq!(hits_diary.len(), 1);
        let hits_cue = repo.search("ship phase", 50).await.unwrap();
        assert_eq!(hits_cue.len(), 1);
        let no_hits = repo.search("nonexistent", 50).await.unwrap();
        assert!(no_hits.is_empty());
    }

    #[tokio::test]
    async fn bulk_upsert_protects_existing_dates() {
        let repo = fresh_repo().await;
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();

        let mut stale = sample_entry("2026-04-29");
        stale.diary = "stale import".to_string();
        let new_day = sample_entry("2026-04-28");

        let result = repo.bulk_upsert(vec![stale, new_day]).await.unwrap();
        assert_eq!(result.inserted, 1);
        assert_eq!(result.skipped, 1);

        // Existing date should NOT have been overwritten.
        let kept = repo.get_by_date("2026-04-29").await.unwrap().unwrap();
        assert_eq!(kept.diary, "ben uğurböceğinden korkarım");
    }

    #[tokio::test]
    async fn settings_round_trip() {
        let repo = fresh_repo().await;
        assert!(
            repo.get_setting("theme").await.unwrap().is_none()
                || repo.get_setting("theme").await.unwrap() == Some("auto".to_string())
        );
        repo.set_setting("custom_key", "custom_value")
            .await
            .unwrap();
        assert_eq!(
            repo.get_setting("custom_key").await.unwrap(),
            Some("custom_value".to_string())
        );
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let repo = fresh_repo().await;
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        repo.delete("2026-04-29").await.unwrap();
        assert!(repo.get_by_date("2026-04-29").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn invalid_date_rejected() {
        let repo = fresh_repo().await;
        let err = repo.get_by_date("not-a-date").await.unwrap_err();
        assert!(matches!(err, DomainError::InvalidDate(_)));
    }
}
