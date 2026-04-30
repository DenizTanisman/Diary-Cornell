//! SQLite implementation of `EntryRepository` (Android, future iOS).
//!
//! Behaviour matches `PostgresEntryRepository` — same protective
//! `bulk_upsert`, same version bump on conflict, same UTC `updated_at`
//! stamp. The wire shape (`DiaryEntry` JSON) is byte-for-byte identical
//! across both backends, so the React frontend never branches on which
//! storage layer is underneath.
//!
//! Dialect notes vs Postgres:
//! - Placeholders use `$N` — sqlx-sqlite supports them as well as `?`.
//! - `ILIKE` doesn't exist; SQLite's `LIKE` is case-insensitive for
//!   ASCII by default, which is what the search command was after.
//! - `now()` doesn't exist; we precompute the timestamp in Rust and
//!   bind it.
//! - `INSERT OR IGNORE` replaces `ON CONFLICT DO NOTHING`.
//! - `is_dirty` is INTEGER 0/1 in SQLite; we bind / read as `bool`
//!   and sqlx-sqlite handles the round-trip.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::db::models::{BulkResult, CueItem, DiaryEntry, MAX_CUE_ITEMS};
use crate::db::repository::EntryRepository;
use crate::error::DomainError;

pub struct SqliteEntryRepository {
    pool: SqlitePool,
}

impl SqliteEntryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    #[allow(dead_code)] // mirrors PostgresEntryRepository::pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl EntryRepository for SqliteEntryRepository {
    async fn init(&self) -> Result<(), DomainError> {
        Ok(())
    }

    async fn get_by_date(&self, date: &str) -> Result<Option<DiaryEntry>, DomainError> {
        validate_date(date)?;
        let row = sqlx::query(SELECT_ALL_BY_DATE_SQL)
            .bind(date)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(row.as_ref().map(row_to_entry).transpose()?)
    }

    async fn upsert(&self, entry: DiaryEntry) -> Result<DiaryEntry, DomainError> {
        validate_date(&entry.date)?;
        validate_cue_items(&entry.cue_items)?;

        let mut entry = entry;
        entry.updated_at = Utc::now().to_rfc3339();
        if entry.created_at.is_empty() {
            entry.created_at = entry.updated_at.clone();
        }
        let cells = expand_cue_cells(&entry.cue_items);

        sqlx::query(UPSERT_BUMP_VERSION_SQL)
            .bind(&entry.date)
            .bind(&entry.diary)
            .bind(cells.0)
            .bind(cells.1)
            .bind(cells.2)
            .bind(cells.3)
            .bind(cells.4)
            .bind(cells.5)
            .bind(cells.6)
            .bind(cells.7)
            .bind(cells.8)
            .bind(cells.9)
            .bind(cells.10)
            .bind(cells.11)
            .bind(cells.12)
            .bind(cells.13)
            .bind(&entry.summary)
            .bind(&entry.quote)
            .bind(&entry.created_at)
            .bind(&entry.updated_at)
            .bind(&entry.device_id)
            .bind(entry.version)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;

        match self.get_by_date(&entry.date).await? {
            Some(saved) => Ok(saved),
            None => Err(DomainError::Storage(format!(
                "upsert succeeded but read failed for {}",
                entry.date
            ))),
        }
    }

    async fn delete(&self, date: &str) -> Result<(), DomainError> {
        validate_date(date)?;
        sqlx::query("DELETE FROM diary_entries WHERE date = $1")
            .bind(date)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn list_dates(&self) -> Result<Vec<String>, DomainError> {
        let rows = sqlx::query("SELECT date FROM diary_entries ORDER BY date DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx)?;
        rows.iter()
            .map(|r| r.try_get::<String, _>("date").map_err(map_sqlx))
            .collect()
    }

    async fn list_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<DiaryEntry>, DomainError> {
        validate_date(start_date)?;
        validate_date(end_date)?;
        let sql =
            format!("{SELECT_ALL_COLUMNS} WHERE date >= $1 AND date <= $2 ORDER BY date DESC");
        let rows = sqlx::query(&sql)
            .bind(start_date)
            .bind(end_date)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx)?;
        rows.iter().map(row_to_entry).collect()
    }

    async fn list_all(&self) -> Result<Vec<DiaryEntry>, DomainError> {
        let sql = format!("{SELECT_ALL_COLUMNS} ORDER BY date DESC");
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx)?;
        rows.iter().map(row_to_entry).collect()
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Vec<DiaryEntry>, DomainError> {
        // SQLite's LIKE is ASCII-case-insensitive by default. For Turkish
        // case folding the user would need either a custom collation or
        // FTS5 — out of scope for FAZ Android.
        let q = format!("%{query}%");
        let limit = limit.max(1) as i64;
        let sql = format!(
            "{SELECT_ALL_COLUMNS} \
             WHERE diary LIKE $1 OR summary LIKE $1 OR quote LIKE $1 \
                OR content_1 LIKE $1 OR content_2 LIKE $1 OR content_3 LIKE $1 \
                OR content_4 LIKE $1 OR content_5 LIKE $1 OR content_6 LIKE $1 \
                OR content_7 LIKE $1 \
                OR title_1 LIKE $1 OR title_2 LIKE $1 OR title_3 LIKE $1 \
                OR title_4 LIKE $1 OR title_5 LIKE $1 OR title_6 LIKE $1 \
                OR title_7 LIKE $1 \
             ORDER BY date DESC LIMIT $2"
        );
        let rows = sqlx::query(&sql)
            .bind(&q)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx)?;
        rows.iter().map(row_to_entry).collect()
    }

    async fn entry_count(&self) -> Result<i64, DomainError> {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM diary_entries")
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx)?;
        row.try_get::<i64, _>("n").map_err(map_sqlx)
    }

    async fn last_updated_at(&self) -> Result<Option<String>, DomainError> {
        let row =
            sqlx::query("SELECT updated_at FROM diary_entries ORDER BY updated_at DESC LIMIT 1")
                .fetch_optional(&self.pool)
                .await
                .map_err(map_sqlx)?;
        row.as_ref()
            .map(|r| r.try_get::<String, _>("updated_at").map_err(map_sqlx))
            .transpose()
    }

    async fn bulk_upsert(&self, entries: Vec<DiaryEntry>) -> Result<BulkResult, DomainError> {
        for e in &entries {
            validate_date(&e.date)?;
            validate_cue_items(&e.cue_items)?;
        }

        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let mut inserted = 0u32;
        let mut skipped = 0u32;

        for entry in entries {
            // Same protective semantics as Postgres impl: existing dates
            // are NEVER overwritten by a foreign import.
            let exists: Option<i64> =
                sqlx::query("SELECT 1 AS one FROM diary_entries WHERE date = $1")
                    .bind(&entry.date)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(map_sqlx)?
                    .and_then(|r| r.try_get::<i64, _>(0).ok());
            if exists.is_some() {
                skipped += 1;
                continue;
            }
            let cells = expand_cue_cells(&entry.cue_items);
            sqlx::query(INSERT_RAW_SQL)
                .bind(&entry.date)
                .bind(&entry.diary)
                .bind(cells.0)
                .bind(cells.1)
                .bind(cells.2)
                .bind(cells.3)
                .bind(cells.4)
                .bind(cells.5)
                .bind(cells.6)
                .bind(cells.7)
                .bind(cells.8)
                .bind(cells.9)
                .bind(cells.10)
                .bind(cells.11)
                .bind(cells.12)
                .bind(cells.13)
                .bind(&entry.summary)
                .bind(&entry.quote)
                .bind(&entry.created_at)
                .bind(&entry.updated_at)
                .bind(&entry.device_id)
                .bind(entry.version)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
            inserted += 1;
        }

        tx.commit().await.map_err(map_sqlx)?;
        Ok(BulkResult {
            inserted,
            updated: 0,
            skipped,
        })
    }

    async fn get_setting(&self, key: &str) -> Result<Option<String>, DomainError> {
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx)?;
        row.as_ref()
            .map(|r| r.try_get::<String, _>("value").map_err(map_sqlx))
            .transpose()
    }

    async fn set_setting(&self, key: &str, value: &str) -> Result<(), DomainError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (key) DO UPDATE \
                SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers (mirror postgres_impl, dialect-adapted)
// ---------------------------------------------------------------------------

const SELECT_ALL_COLUMNS: &str = "SELECT date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version FROM diary_entries";

const SELECT_ALL_BY_DATE_SQL: &str = "SELECT date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version FROM diary_entries WHERE date = $1";

// SQLite's `ON CONFLICT (date) DO UPDATE` mirrors Postgres exactly
// (since 3.24). `version + 1` works on both. `is_dirty = 1` instead of
// `TRUE` because SQLite stores the column as INTEGER per migration.
const UPSERT_BUMP_VERSION_SQL: &str = "INSERT INTO diary_entries ( \
    date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version) VALUES ( \
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
    $15, $16, $17, $18, $19, $20, $21, $22) \
    ON CONFLICT (date) DO UPDATE SET \
    diary      = excluded.diary, \
    title_1    = excluded.title_1, content_1 = excluded.content_1, \
    title_2    = excluded.title_2, content_2 = excluded.content_2, \
    title_3    = excluded.title_3, content_3 = excluded.content_3, \
    title_4    = excluded.title_4, content_4 = excluded.content_4, \
    title_5    = excluded.title_5, content_5 = excluded.content_5, \
    title_6    = excluded.title_6, content_6 = excluded.content_6, \
    title_7    = excluded.title_7, content_7 = excluded.content_7, \
    summary    = excluded.summary, \
    quote      = excluded.quote, \
    updated_at = excluded.updated_at, \
    device_id  = excluded.device_id, \
    version    = diary_entries.version + 1, \
    is_dirty   = 1";

const INSERT_RAW_SQL: &str = "INSERT INTO diary_entries ( \
    date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version) VALUES ( \
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
    $15, $16, $17, $18, $19, $20, $21, $22)";

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

fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<DiaryEntry, DomainError> {
    let mut cue_items = Vec::with_capacity(MAX_CUE_ITEMS as usize);
    for i in 1..=MAX_CUE_ITEMS {
        let title: Option<String> = row.try_get(&*format!("title_{i}")).map_err(map_sqlx)?;
        let content: Option<String> = row.try_get(&*format!("content_{i}")).map_err(map_sqlx)?;
        if let Some(title) = title {
            cue_items.push(CueItem {
                position: i,
                title,
                content: content.unwrap_or_default(),
            });
        }
    }
    cue_items.sort_by_key(|c| c.position);

    Ok(DiaryEntry {
        date: row.try_get("date").map_err(map_sqlx)?,
        diary: row.try_get("diary").map_err(map_sqlx)?,
        cue_items,
        summary: row.try_get("summary").map_err(map_sqlx)?,
        quote: row.try_get("quote").map_err(map_sqlx)?,
        created_at: row.try_get("created_at").map_err(map_sqlx)?,
        updated_at: row.try_get("updated_at").map_err(map_sqlx)?,
        device_id: row.try_get("device_id").map_err(map_sqlx)?,
        version: row.try_get("version").map_err(map_sqlx)?,
    })
}

fn map_sqlx(err: sqlx::Error) -> DomainError {
    DomainError::Storage(err.to_string())
}

#[cfg(test)]
mod tests {
    //! Tests run against an in-memory SQLite (`sqlite::memory:`) so they
    //! need no external service. Each test gets a fresh DB with the
    //! sqlite_migrations applied.

    use super::*;
    use crate::db::{build_pool, run_migrations};

    async fn fresh_repo() -> SqliteEntryRepository {
        // sqlite::memory: with cache=shared so all pool connections see
        // the same in-memory DB. Without cache=shared sqlx-sqlite spawns
        // independent :memory: instances per connection and migrations
        // applied on one don't show up on the other.
        let pool = build_pool("sqlite::memory:?cache=shared").await.unwrap();
        run_migrations(&pool).await.unwrap();
        SqliteEntryRepository::new(pool)
    }

    fn sample_entry(date: &str) -> DiaryEntry {
        DiaryEntry {
            date: date.to_string(),
            diary: "ben uğurböceğinden korkarım".into(),
            cue_items: vec![
                CueItem {
                    position: 1,
                    title: "Reflection".into(),
                    content: "deep".into(),
                },
                CueItem {
                    position: 3,
                    title: "Goal".into(),
                    content: "ship phase android".into(),
                },
            ],
            summary: "good day".into(),
            quote: "carpe diem".into(),
            created_at: String::new(),
            updated_at: String::new(),
            device_id: Some("phone-1".into()),
            version: 1,
        }
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
        assert_eq!(fetched.device_id.as_deref(), Some("phone-1"));
    }

    #[tokio::test]
    async fn upsert_bumps_version_on_conflict() {
        let repo = fresh_repo().await;
        let mut e = sample_entry("2026-04-29");
        repo.upsert(e.clone()).await.unwrap();
        e.diary = "second take".into();
        let saved = repo.upsert(e).await.unwrap();
        assert!(saved.version >= 2, "version should bump, got {}", saved.version);
    }

    #[tokio::test]
    async fn missing_date_returns_none() {
        let repo = fresh_repo().await;
        assert!(repo.get_by_date("2099-01-01").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn invalid_date_rejected() {
        let repo = fresh_repo().await;
        assert!(matches!(
            repo.get_by_date("not-a-date").await,
            Err(DomainError::InvalidDate(_))
        ));
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let repo = fresh_repo().await;
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        repo.delete("2026-04-29").await.unwrap();
        assert!(repo.get_by_date("2026-04-29").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_range_filters_by_date_inclusive() {
        let repo = fresh_repo().await;
        repo.upsert(sample_entry("2026-04-27")).await.unwrap();
        repo.upsert(sample_entry("2026-04-28")).await.unwrap();
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        let v = repo.list_range("2026-04-27", "2026-04-28").await.unwrap();
        assert_eq!(v.len(), 2);
    }

    #[tokio::test]
    async fn search_matches_diary_and_cue_content() {
        let repo = fresh_repo().await;
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        let v = repo.search("uğurböce", 10).await.unwrap();
        assert_eq!(v.len(), 1);
        let v2 = repo.search("ship phase", 10).await.unwrap();
        assert_eq!(v2.len(), 1, "search must hit cue content");
    }

    #[tokio::test]
    async fn settings_round_trip() {
        let repo = fresh_repo().await;
        // Default seed
        assert_eq!(repo.get_setting("theme").await.unwrap().as_deref(), Some("auto"));
        repo.set_setting("theme", "dark").await.unwrap();
        assert_eq!(repo.get_setting("theme").await.unwrap().as_deref(), Some("dark"));
    }

    #[tokio::test]
    async fn bulk_upsert_protects_existing_dates() {
        let repo = fresh_repo().await;
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        let mut foreign = sample_entry("2026-04-29");
        foreign.diary = "should not overwrite".into();
        let new_one = sample_entry("2026-04-30");

        let result = repo.bulk_upsert(vec![foreign, new_one]).await.unwrap();
        assert_eq!(result.inserted, 1);
        assert_eq!(result.skipped, 1);
        let kept = repo.get_by_date("2026-04-29").await.unwrap().unwrap();
        assert!(kept.diary.contains("uğurböce"), "existing row preserved");
    }
}
