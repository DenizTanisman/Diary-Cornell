use std::sync::Arc;

use tauri::Manager;

use crate::commands::entries::{
    diary_bulk_upsert, diary_delete, diary_entry_count, diary_get_by_date, diary_get_setting,
    diary_last_updated_at, diary_list_all, diary_list_dates, diary_list_range, diary_search,
    diary_set_setting, diary_upsert, AppState,
};
use crate::db::{build_pool, run_migrations, EntryRepository, PostgresEntryRepository};

mod commands;
mod db;
mod error;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cornell_diary=info,sqlx=warn".into()),
        )
        .with_target(false)
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // Postgres is the sole storage backend (FAZ 1.3). DATABASE_URL is
            // required at boot — there is no SQLite fallback any more.
            let database_url = std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set (e.g. via .env)"))?;
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
            app.manage(AppState {
                repo,
                pg_pool: Some(pool),
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
