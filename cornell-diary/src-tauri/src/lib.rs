use std::sync::Arc;

use tauri::Manager;
use tauri_plugin_sql::{Migration, MigrationKind};

use crate::commands::entries::{
    diary_bulk_upsert, diary_delete, diary_entry_count, diary_get_by_date, diary_get_setting,
    diary_last_updated_at, diary_list_all, diary_list_dates, diary_list_range, diary_search,
    diary_set_setting, diary_upsert, AppState,
};
use crate::db::{
    build_pool, run_migrations, EntryRepository, PostgresEntryRepository, SqliteEntryRepository,
};
use crate::migration::migrate_sqlite_to_postgres;

mod commands;
mod db;
mod error;
mod migration;

const DB_FILENAME: &str = "cornell_diary.db";

/// Picks `STORAGE_BACKEND` from environment. Default is `sqlite` so an empty
/// `.env` keeps existing users on the previous backend; flipping to
/// `STORAGE_BACKEND=postgres` exercises the new path. FAZ 1.3 deletes this
/// flag and forces `postgres`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageBackend {
    Sqlite,
    Postgres,
}

impl StorageBackend {
    fn from_env() -> Self {
        match std::env::var("STORAGE_BACKEND")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("postgres") | Some("pg") => Self::Postgres,
            _ => Self::Sqlite,
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cornell_diary=info,sqlx=warn,rusqlite=warn".into()),
        )
        .with_target(false)
        .try_init();

    // tauri-plugin-sql migrations stay so the existing front-end QR/JSON sync
    // code path keeps working in transition. Removed in FAZ 1.3 once Postgres
    // is the sole backend.
    let migrations = vec![Migration {
        version: 1,
        description: "create_initial_tables",
        sql: include_str!("../migrations/001_initial.sql"),
        kind: MigrationKind::Up,
    }];

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(&format!("sqlite:{DB_FILENAME}"), migrations)
                .build(),
        )
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let backend = StorageBackend::from_env();
            tracing::info!(target: "cornell_diary", ?backend, "selecting storage backend");

            let (repo, pg_pool): (Arc<dyn EntryRepository>, Option<sqlx::PgPool>) = match backend {
                StorageBackend::Sqlite => {
                    let app_data_dir = app_handle
                        .path()
                        .app_data_dir()
                        .map_err(|e| anyhow::anyhow!("app_data_dir: {e}"))?;
                    std::fs::create_dir_all(&app_data_dir)?;
                    let db_path = app_data_dir.join(DB_FILENAME);
                    tracing::info!(target: "cornell_diary", path = %db_path.display(), "sqlite backend");

                    let r = Arc::new(SqliteEntryRepository::new(db_path));
                    let init_clone: Arc<dyn EntryRepository> = r.clone();
                    tauri::async_runtime::block_on(async move {
                        init_clone
                            .init()
                            .await
                            .map_err(|e| anyhow::anyhow!("repo init: {e:?}"))
                    })?;
                    (r, None)
                }
                StorageBackend::Postgres => {
                    let database_url = std::env::var("DATABASE_URL").map_err(|_| {
                        anyhow::anyhow!(
                            "STORAGE_BACKEND=postgres requires DATABASE_URL in the environment"
                        )
                    })?;
                    tracing::info!(target: "cornell_diary", "postgres backend");

                    let pool = tauri::async_runtime::block_on(async {
                        let pool = build_pool(&database_url)
                            .await
                            .map_err(|e| anyhow::anyhow!("postgres pool: {e:?}"))?;
                        run_migrations(&pool)
                            .await
                            .map_err(|e| anyhow::anyhow!("postgres migrate: {e:?}"))?;
                        Ok::<_, anyhow::Error>(pool)
                    })?;

                    let repo: Arc<dyn EntryRepository> =
                        Arc::new(PostgresEntryRepository::new(pool.clone()));
                    (repo, Some(pool))
                }
            };

            app.manage(AppState { repo, pg_pool });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            diary_get_by_date,
            diary_upsert,
            diary_delete,
            diary_list_dates,
            diary_list_range,
            diary_list_all,
            diary_search,
            diary_entry_count,
            diary_last_updated_at,
            diary_bulk_upsert,
            diary_get_setting,
            diary_set_setting,
            migrate_sqlite_to_postgres,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
