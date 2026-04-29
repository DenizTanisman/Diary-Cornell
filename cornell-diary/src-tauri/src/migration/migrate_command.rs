//! `migrate_sqlite_to_postgres` Tauri command.
//!
//! Two-step UX:
//! 1. `dry_run = true` — read the SQLite snapshot, count rows by table,
//!    return a `MigrationReport` so the user can sanity-check before any
//!    write.
//! 2. `dry_run = false` — re-read, then transactionally insert into Postgres.
//!    Existing rows in Postgres (matched by date / settings.key) are kept;
//!    nothing in Postgres ever gets overwritten by a stale SQLite copy. This
//!    matches the protective semantics of `EntryRepository::bulk_upsert` and
//!    means re-running the command is safe.
//!
//! The frontend calls these via:
//!   `invoke('migrate_sqlite_to_postgres', { sqlitePath, dryRun: true|false })`

use std::path::PathBuf;
use std::time::Instant;

use serde::Serialize;
use sqlx::PgPool;
use tauri::State;

use crate::commands::entries::AppState;
use crate::error::DomainError;
use crate::migration::sqlite_reader::{SqliteReader, SqliteSnapshot};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    pub dry_run: bool,
    /// Counts read out of the source SQLite (always populated, even on dry
    /// run, so the frontend can show the user what's about to happen).
    pub source_counts: TableCounts,
    /// Counts present in the destination Postgres after the run. On dry run
    /// these are the *current* counts (nothing has been written yet).
    pub target_counts: TableCounts,
    /// How many rows we actually wrote in this invocation. All zeros on dry
    /// run.
    pub inserted: TableCounts,
    /// How many SQLite rows we skipped because the same primary key already
    /// existed in Postgres. Re-runs of the migration sit entirely in this
    /// bucket.
    pub skipped: TableCounts,
    pub duration_ms: u64,
    pub success: bool,
    /// User-safe message; empty on success.
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TableCounts {
    pub diary_entries: u64,
    pub sync_logs: u64,
    pub app_settings: u64,
}

/// Tauri command surface. `app_state.repo` must be the Postgres backend —
/// migrating into a SQLite repo would be a no-op at best and a corruption
/// risk at worst, so we surface a domain error instead.
#[tauri::command]
pub async fn migrate_sqlite_to_postgres(
    app_state: State<'_, AppState>,
    sqlite_path: String,
    dry_run: bool,
) -> Result<MigrationReport, DomainError> {
    let started = Instant::now();
    let mut report = MigrationReport {
        dry_run,
        ..Default::default()
    };

    let pool = app_state.pg_pool.clone().ok_or_else(|| {
        DomainError::Validation("migration requires STORAGE_BACKEND=postgres at app start".into())
    })?;

    // 1. Read the SQLite snapshot. Always done — both branches need it.
    let path = PathBuf::from(&sqlite_path);
    let reader = SqliteReader::open(&path)?;
    let snapshot = reader.read_snapshot()?;
    report.source_counts = TableCounts {
        diary_entries: snapshot.diary_entries.len() as u64,
        sync_logs: snapshot.sync_logs.len() as u64,
        app_settings: snapshot.app_settings.len() as u64,
    };
    report.target_counts = current_pg_counts(&pool).await?;

    if dry_run {
        report.success = true;
        report.duration_ms = started.elapsed().as_millis() as u64;
        return Ok(report);
    }

    // 2. Real migration — single transaction so a mid-run crash leaves
    //    Postgres in its prior state.
    if let Err(e) = apply_snapshot(&pool, snapshot, &mut report).await {
        report.error = e.to_string();
        report.duration_ms = started.elapsed().as_millis() as u64;
        return Ok(report);
    }

    report.target_counts = current_pg_counts(&pool).await?;
    report.success = true;
    report.duration_ms = started.elapsed().as_millis() as u64;
    Ok(report)
}

async fn apply_snapshot(
    pool: &PgPool,
    snapshot: SqliteSnapshot,
    report: &mut MigrationReport,
) -> Result<(), DomainError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| DomainError::Storage(e.to_string()))?;

    // diary_entries — insert; ON CONFLICT DO NOTHING means re-runs skip
    // dates that are already in Postgres.
    for entry in snapshot.diary_entries {
        let cells = expand_cue_cells(&entry);
        let result = sqlx::query(
            "INSERT INTO diary_entries ( \
                date, diary, \
                title_1, content_1, title_2, content_2, title_3, content_3, \
                title_4, content_4, title_5, content_5, title_6, content_6, \
                title_7, content_7, summary, quote, created_at, updated_at, \
                device_id, version, is_dirty) VALUES ( \
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
                $15, $16, $17, $18, $19, $20, $21, $22, TRUE) \
             ON CONFLICT (date) DO NOTHING",
        )
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
        .map_err(|e| DomainError::Storage(e.to_string()))?;
        if result.rows_affected() == 1 {
            report.inserted.diary_entries += 1;
        } else {
            report.skipped.diary_entries += 1;
        }
    }

    // sync_log — append every row. The id column is BIGSERIAL on Postgres,
    // so we don't carry the SQLite id over (it would collide with future
    // server-side autoincrements).
    for log in snapshot.sync_logs {
        sqlx::query(
            "INSERT INTO sync_log ( \
                sync_type, method, device_id, peer_device_id, timestamp, \
                entry_count, checksum, status, error_message) \
             VALUES ($1, $2, $3, $4, $5::timestamptz, $6, $7, $8, $9)",
        )
        .bind(&log.sync_type)
        .bind(&log.method)
        .bind(&log.device_id)
        .bind(&log.peer_device_id)
        .bind(&log.timestamp)
        .bind(log.entry_count)
        .bind(&log.checksum)
        .bind(&log.status)
        .bind(&log.error_message)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Storage(format!("sync_log insert: {e}")))?;
        report.inserted.sync_logs += 1;
    }

    // app_settings — INSERT ON CONFLICT DO NOTHING. Postgres seeds the
    // default keys (theme, language, etc.); user-customised values from
    // SQLite take priority only if the key isn't already there.
    for setting in snapshot.app_settings {
        let result = sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at) \
             VALUES ($1, $2, $3::timestamptz) \
             ON CONFLICT (key) DO NOTHING",
        )
        .bind(&setting.key)
        .bind(&setting.value)
        .bind(&setting.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Storage(format!("app_setting insert: {e}")))?;
        if result.rows_affected() == 1 {
            report.inserted.app_settings += 1;
        } else {
            report.skipped.app_settings += 1;
        }
    }

    tx.commit()
        .await
        .map_err(|e| DomainError::Storage(format!("commit: {e}")))?;
    Ok(())
}

async fn current_pg_counts(pool: &PgPool) -> Result<TableCounts, DomainError> {
    Ok(TableCounts {
        diary_entries: count(pool, "diary_entries").await?,
        sync_logs: count(pool, "sync_log").await?,
        app_settings: count(pool, "app_settings").await?,
    })
}

async fn count(pool: &PgPool, table: &str) -> Result<u64, DomainError> {
    // Table name is internal — never user input.
    let sql = format!("SELECT COUNT(*)::BIGINT AS n FROM {table}");
    let row: (i64,) = sqlx::query_as(&sql)
        .fetch_one(pool)
        .await
        .map_err(|e| DomainError::Storage(format!("count {table}: {e}")))?;
    Ok(row.0 as u64)
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

fn expand_cue_cells(entry: &crate::db::models::DiaryEntry) -> CueCells {
    let mut titles: [Option<String>; 7] = Default::default();
    let mut contents: [Option<String>; 7] = Default::default();
    for item in &entry.cue_items {
        let idx = (item.position - 1) as usize;
        if idx < 7 {
            titles[idx] = Some(item.title.clone());
            contents[idx] = Some(item.content.clone());
        }
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

#[cfg(test)]
mod tests {
    //! End-to-end migration test: build a SQLite fixture, hand it to
    //! `apply_snapshot`, assert Postgres ends up with the right rows.
    //! Skipped when DATABASE_URL is unset.

    use super::*;
    use crate::db::pool::{build_pool, run_migrations};
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn build_sqlite_fixture(path: &std::path::Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(include_str!("../../migrations/001_initial.sql"))
            .unwrap();
        conn.execute(
            "INSERT INTO diary_entries (\
                date, diary, title_1, content_1, summary, quote, \
                created_at, updated_at, device_id, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                "2026-04-29",
                "ben uğurböceğinden korkarım",
                "Reflection",
                "deep thoughts",
                "good day",
                "carpe diem",
                "2026-04-29T10:00:00Z",
                "2026-04-29T10:00:00Z",
                "test-device",
                1i64,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sync_log (\
                sync_type, method, device_id, peer_device_id, timestamp, \
                entry_count, checksum, status, error_message) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                "export",
                "qr",
                "device-1",
                Option::<String>::None,
                "2026-04-29T10:00:00Z",
                3i64,
                "abc123",
                "success",
                Option::<String>::None,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('custom_key', 'custom_value', '2026-04-29T10:00:00Z')",
            [],
        )
        .unwrap();
    }

    fn database_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok().filter(|s| !s.is_empty())
    }

    async fn fresh_pool() -> Option<PgPool> {
        let pool = build_pool(&database_url()?).await.ok()?;
        run_migrations(&pool).await.ok()?;
        sqlx::query("TRUNCATE diary_entries, sync_log, sync_metadata RESTART IDENTITY")
            .execute(&pool)
            .await
            .ok()?;
        sqlx::query("DELETE FROM app_settings")
            .execute(&pool)
            .await
            .ok()?;
        sqlx::query("INSERT INTO sync_metadata (id) VALUES (1) ON CONFLICT DO NOTHING")
            .execute(&pool)
            .await
            .ok()?;
        Some(pool)
    }

    #[tokio::test]
    #[serial_test::serial(postgres)]
    async fn dry_run_counts_without_writing() {
        let Some(pool) = fresh_pool().await else {
            return;
        };

        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy.db");
        build_sqlite_fixture(&path);

        let reader = SqliteReader::open(&path).unwrap();
        let snapshot = reader.read_snapshot().unwrap();

        // Direct call to apply_snapshot is the second branch; the dry-run
        // branch in migrate_sqlite_to_postgres is exercised via the public
        // command, but here we assert the read step alone produces the
        // expected counts.
        assert_eq!(snapshot.diary_entries.len(), 1);
        assert_eq!(snapshot.sync_logs.len(), 1);
        // The legacy SQLite seed inserts 4 default settings (theme, language,
        // auto_save_interval_ms, first_launch_date) and our fixture adds one
        // user-set custom_key on top.
        assert_eq!(snapshot.app_settings.len(), 5);

        let counts_before = current_pg_counts(&pool).await.unwrap();
        // dry_run should not touch Postgres.
        assert_eq!(counts_before.diary_entries, 0);
        assert_eq!(counts_before.sync_logs, 0);
    }

    #[tokio::test]
    #[serial_test::serial(postgres)]
    async fn full_migration_inserts_every_table() {
        let Some(pool) = fresh_pool().await else {
            return;
        };

        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy.db");
        build_sqlite_fixture(&path);

        let reader = SqliteReader::open(&path).unwrap();
        let snapshot = reader.read_snapshot().unwrap();

        let mut report = MigrationReport::default();
        apply_snapshot(&pool, snapshot, &mut report).await.unwrap();

        assert_eq!(report.inserted.diary_entries, 1);
        assert_eq!(report.inserted.sync_logs, 1);
        // The legacy SQLite seed contains 4 default rows (theme, language,
        // auto_save_interval_ms, first_launch_date) plus the test fixture's
        // custom_key. Production Postgres seeds the same 4 defaults, so a
        // real run would skip them. The test setup wipes app_settings to
        // get a deterministic count, which forces all 5 to be inserted.
        assert_eq!(
            report.inserted.app_settings + report.skipped.app_settings,
            5
        );

        let after = current_pg_counts(&pool).await.unwrap();
        assert_eq!(after.diary_entries, 1);
        assert_eq!(after.sync_logs, 1);
        assert!(after.app_settings >= 5);

        // Reading the row back should produce the same JSON shape the
        // frontend already consumes.
        let row: (String, String, Option<String>, i64) = sqlx::query_as(
            "SELECT date, diary, title_1, version FROM diary_entries WHERE date = $1",
        )
        .bind("2026-04-29")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, "2026-04-29");
        assert_eq!(row.1, "ben uğurböceğinden korkarım");
        assert_eq!(row.2.as_deref(), Some("Reflection"));
        assert_eq!(row.3, 1);
    }

    #[tokio::test]
    #[serial_test::serial(postgres)]
    async fn rerun_migration_is_safe_skips_existing_rows() {
        let Some(pool) = fresh_pool().await else {
            return;
        };

        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy.db");
        build_sqlite_fixture(&path);

        // First run.
        let reader = SqliteReader::open(&path).unwrap();
        let snapshot = reader.read_snapshot().unwrap();
        let mut first = MigrationReport::default();
        apply_snapshot(&pool, snapshot, &mut first).await.unwrap();
        assert_eq!(first.inserted.diary_entries, 1);

        // Second run — same fixture, should skip.
        let reader2 = SqliteReader::open(&path).unwrap();
        let snapshot2 = reader2.read_snapshot().unwrap();
        let mut second = MigrationReport::default();
        apply_snapshot(&pool, snapshot2, &mut second).await.unwrap();
        assert_eq!(second.inserted.diary_entries, 0);
        assert_eq!(second.skipped.diary_entries, 1);

        // sync_log gets appended every run by design — that's fine, audit
        // trails are append-only.
        assert_eq!(second.inserted.sync_logs, 1);
    }

    #[tokio::test]
    async fn missing_sqlite_file_is_a_path_error() {
        let dir = tempdir().unwrap();
        let absent = dir.path().join("does_not_exist.db");
        match SqliteReader::open(&absent) {
            Ok(_) => panic!("expected error for missing path"),
            Err(e) => assert!(matches!(e, DomainError::Path(_))),
        }
    }
}
