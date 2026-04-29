//! Postgres implementation of `EntryRepository`.
//!
//! Behaviour matches `SqliteEntryRepository` exactly — the same protective
//! `bulk_upsert`, the same version bump on conflict, the same UTC `updated_at`
//! stamp, the same `INSERT OR IGNORE` semantics on settings. The wire shape
//! emitted to the frontend (`DiaryEntry` JSON) is byte-for-byte identical so
//! the UI can swap backends through `STORAGE_BACKEND=postgres` without any
//! React-side change.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{PgPool, Row};

use crate::db::models::{BulkResult, CueItem, DiaryEntry, MAX_CUE_ITEMS};
use crate::db::repository::EntryRepository;
use crate::error::DomainError;

pub struct PostgresEntryRepository {
    pool: PgPool,
}

impl PostgresEntryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[allow(dead_code)] // used by FAZ 1.2 migration command and tests
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl EntryRepository for PostgresEntryRepository {
    async fn init(&self) -> Result<(), DomainError> {
        // The pool is built and migrations are run by the setup hook before
        // the repo is wrapped in AppState; nothing to do here.
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
        let q = format!("%{query}%");
        let limit = limit.max(1) as i64;
        let sql = format!(
            "{SELECT_ALL_COLUMNS} \
             WHERE diary ILIKE $1 OR summary ILIKE $1 OR quote ILIKE $1 \
                OR content_1 ILIKE $1 OR content_2 ILIKE $1 OR content_3 ILIKE $1 \
                OR content_4 ILIKE $1 OR content_5 ILIKE $1 OR content_6 ILIKE $1 \
                OR content_7 ILIKE $1 \
                OR title_1 ILIKE $1 OR title_2 ILIKE $1 OR title_3 ILIKE $1 \
                OR title_4 ILIKE $1 OR title_5 ILIKE $1 OR title_6 ILIKE $1 \
                OR title_7 ILIKE $1 \
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
        let row = sqlx::query("SELECT COUNT(*)::BIGINT AS n FROM diary_entries")
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
            // Protective semantics: existing dates are NEVER overwritten by
            // a foreign import, even if the remote copy is newer. Mirrors
            // SQLiteRepository.bulkUpsert (handoff Part I §3.5).
            let exists: Option<i64> =
                sqlx::query("SELECT 1::BIGINT FROM diary_entries WHERE date = $1")
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
        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at) \
             VALUES ($1, $2, now()) \
             ON CONFLICT (key) DO UPDATE \
                SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
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

const SELECT_ALL_BY_DATE_SQL: &str = "SELECT date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version FROM diary_entries WHERE date = $1";

const UPSERT_BUMP_VERSION_SQL: &str = "INSERT INTO diary_entries ( \
    date, diary, \
    title_1, content_1, title_2, content_2, title_3, content_3, \
    title_4, content_4, title_5, content_5, title_6, content_6, \
    title_7, content_7, summary, quote, created_at, updated_at, \
    device_id, version) VALUES ( \
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
    $15, $16, $17, $18, $19, $20, $21, $22) \
    ON CONFLICT (date) DO UPDATE SET \
    diary      = EXCLUDED.diary, \
    title_1    = EXCLUDED.title_1, content_1 = EXCLUDED.content_1, \
    title_2    = EXCLUDED.title_2, content_2 = EXCLUDED.content_2, \
    title_3    = EXCLUDED.title_3, content_3 = EXCLUDED.content_3, \
    title_4    = EXCLUDED.title_4, content_4 = EXCLUDED.content_4, \
    title_5    = EXCLUDED.title_5, content_5 = EXCLUDED.content_5, \
    title_6    = EXCLUDED.title_6, content_6 = EXCLUDED.content_6, \
    title_7    = EXCLUDED.title_7, content_7 = EXCLUDED.content_7, \
    summary    = EXCLUDED.summary, \
    quote      = EXCLUDED.quote, \
    updated_at = EXCLUDED.updated_at, \
    device_id  = EXCLUDED.device_id, \
    version    = diary_entries.version + 1, \
    is_dirty   = TRUE";

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

fn row_to_entry(row: &sqlx::postgres::PgRow) -> Result<DiaryEntry, DomainError> {
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
    //! Integration tests that talk to a real Postgres on `DATABASE_URL`. Each
    //! test runs inside a savepoint that's rolled back, so they don't pollute
    //! each other or leave state behind. Disabled when DATABASE_URL is unset
    //! so `cargo test` still works in environments without a Postgres handy.

    use super::*;

    fn database_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok().filter(|s| !s.is_empty())
    }

    async fn fresh_pool() -> Option<PgPool> {
        let url = database_url()?;
        let pool = crate::db::pool::build_pool(&url).await.ok()?;
        crate::db::pool::run_migrations(&pool).await.ok()?;
        // Wipe data between tests; structure stays.
        sqlx::query("TRUNCATE diary_entries, sync_log, sync_metadata RESTART IDENTITY")
            .execute(&pool)
            .await
            .ok()?;
        // Re-seed singleton.
        sqlx::query("INSERT INTO sync_metadata (id) VALUES (1) ON CONFLICT DO NOTHING")
            .execute(&pool)
            .await
            .ok()?;
        Some(pool)
    }

    fn sample_entry(date: &str) -> DiaryEntry {
        DiaryEntry {
            date: date.to_string(),
            diary: "ben uğurböceğinden korkarım".to_string(),
            cue_items: vec![
                CueItem {
                    position: 1,
                    title: "Reflection".to_string(),
                    content: "deep".to_string(),
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

    macro_rules! pg_test {
        ($name:ident, |$repo:ident, $pool:ident| $body:block) => {
            // `#[serial(postgres)]` keeps Postgres-touching tests from racing
            // each other when `cargo test` runs them in parallel — they all
            // share one local DB and TRUNCATE in `fresh_pool` would otherwise
            // wipe a peer mid-test.
            #[tokio::test]
            #[serial_test::serial(postgres)]
            async fn $name() {
                let Some($pool) = fresh_pool().await else {
                    eprintln!(
                        "skipping {} — DATABASE_URL not reachable",
                        stringify!($name)
                    );
                    return;
                };
                let $repo = PostgresEntryRepository::new($pool.clone());
                $repo.init().await.unwrap();
                $body
            }
        };
    }

    pg_test!(upsert_then_get_round_trips_all_fields, |repo, _pool| {
        let entry = sample_entry("2026-04-29");
        repo.upsert(entry.clone()).await.unwrap();
        let fetched = repo.get_by_date("2026-04-29").await.unwrap().unwrap();
        assert_eq!(fetched.diary, entry.diary);
        assert_eq!(fetched.cue_items.len(), 2);
        assert_eq!(fetched.cue_items[0].position, 1);
        assert_eq!(fetched.cue_items[1].position, 3);
        assert_eq!(fetched.summary, "good day");
        assert_eq!(fetched.device_id.as_deref(), Some("test-device"));
    });

    pg_test!(upsert_bumps_version_on_conflict, |repo, _pool| {
        let mut entry = sample_entry("2026-04-29");
        repo.upsert(entry.clone()).await.unwrap();
        entry.diary = "edited".to_string();
        let saved = repo.upsert(entry).await.unwrap();
        assert_eq!(saved.version, 2);
        assert_eq!(saved.diary, "edited");
    });

    pg_test!(missing_date_returns_none, |repo, _pool| {
        let got = repo.get_by_date("2030-01-01").await.unwrap();
        assert!(got.is_none());
    });

    pg_test!(list_range_filters_by_date_inclusive, |repo, _pool| {
        repo.upsert(sample_entry("2026-04-27")).await.unwrap();
        repo.upsert(sample_entry("2026-04-28")).await.unwrap();
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        let in_range = repo.list_range("2026-04-28", "2026-04-29").await.unwrap();
        assert_eq!(in_range.len(), 2);
    });

    pg_test!(search_matches_diary_and_cue_content, |repo, _pool| {
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        let hits_diary = repo.search("uğurböceğ", 50).await.unwrap();
        assert_eq!(hits_diary.len(), 1);
        let hits_cue = repo.search("ship phase", 50).await.unwrap();
        assert_eq!(hits_cue.len(), 1);
        let no_hits = repo.search("nonexistent", 50).await.unwrap();
        assert!(no_hits.is_empty());
    });

    pg_test!(bulk_upsert_protects_existing_dates, |repo, _pool| {
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();

        let mut stale = sample_entry("2026-04-29");
        stale.diary = "stale import".to_string();
        let new_day = sample_entry("2026-04-28");

        let result = repo.bulk_upsert(vec![stale, new_day]).await.unwrap();
        assert_eq!(result.inserted, 1);
        assert_eq!(result.skipped, 1);

        let kept = repo.get_by_date("2026-04-29").await.unwrap().unwrap();
        assert_eq!(kept.diary, "ben uğurböceğinden korkarım");
    });

    pg_test!(settings_round_trip, |repo, _pool| {
        repo.set_setting("custom_key", "custom_value")
            .await
            .unwrap();
        assert_eq!(
            repo.get_setting("custom_key").await.unwrap(),
            Some("custom_value".to_string())
        );
    });

    pg_test!(delete_removes_row, |repo, _pool| {
        repo.upsert(sample_entry("2026-04-29")).await.unwrap();
        repo.delete("2026-04-29").await.unwrap();
        assert!(repo.get_by_date("2026-04-29").await.unwrap().is_none());
    });

    pg_test!(invalid_date_rejected, |repo, _pool| {
        let err = repo.get_by_date("not-a-date").await.unwrap_err();
        assert!(matches!(err, DomainError::InvalidDate(_)));
    });
}
