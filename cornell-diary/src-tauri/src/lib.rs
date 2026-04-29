use std::sync::Arc;

use tauri::Manager;
use tauri_plugin_sql::{Migration, MigrationKind};

use crate::commands::entries::{
    diary_bulk_upsert, diary_delete, diary_entry_count, diary_get_by_date, diary_get_setting,
    diary_last_updated_at, diary_list_all, diary_list_dates, diary_list_range, diary_search,
    diary_set_setting, diary_upsert, AppState,
};
use crate::db::{EntryRepository, SqliteEntryRepository};

mod commands;
mod db;
mod error;

const DB_FILENAME: &str = "cornell_diary.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Subscribe to tracing exactly once per process. RUST_LOG controls the
    // filter; default is `info` for our crate, `warn` for sqlx/rusqlite noise.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cornell_diary=info,sqlx=warn,rusqlite=warn".into()),
        )
        .with_target(false)
        .try_init();

    // tauri-plugin-sql migrations stay so the existing front-end QR/JSON sync
    // code path keeps working in transition. Removed in FAZ 1.3 once the
    // Repository takes over completely.
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

            // Resolve the per-OS app data directory and the DB file path
            // inside it. Tauri creates the directory on demand.
            let app_data_dir = app_handle
                .path()
                .app_data_dir()
                .map_err(|e| anyhow::anyhow!("app_data_dir: {e}"))?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join(DB_FILENAME);

            tracing::info!(
                target: "cornell_diary",
                path = %db_path.display(),
                "initialising sqlite repository"
            );

            // Build the repo synchronously here, then init it on a Tokio
            // task. We block-on so failures show up at startup rather than
            // surfacing as the first IPC error.
            let repo = Arc::new(SqliteEntryRepository::new(db_path));
            let repo_for_init: Arc<dyn EntryRepository> = repo.clone();
            tauri::async_runtime::block_on(async move {
                repo_for_init
                    .init()
                    .await
                    .map_err(|e| anyhow::anyhow!("repo init: {e:?}"))
            })?;

            app.manage(AppState {
                repo: repo as Arc<dyn EntryRepository>,
            });
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
