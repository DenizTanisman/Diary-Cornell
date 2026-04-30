use std::sync::Arc;

use tauri::Manager;

use crate::commands::entries::{
    diary_bulk_upsert, diary_delete, diary_entry_count, diary_get_by_date, diary_get_setting,
    diary_last_updated_at, diary_list_all, diary_list_dates, diary_list_range, diary_search,
    diary_set_setting, diary_upsert, AppState,
};
use crate::commands::sync::{
    connect_cloud, disconnect_cloud, get_sync_status, trigger_sync, SyncState,
};
use crate::db::{build_pool, run_migrations, EntryRepository, PostgresEntryRepository};
use crate::sync::auth::AuthManager;
use crate::sync::{network, scheduler, CloudClient, SyncEngine};

mod commands;
mod db;
mod error;
mod sync;

const DEFAULT_CLOUD_URL: &str = "http://127.0.0.1:5000";

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
                repo: repo.clone(),
                pg_pool: Some(pool.clone()),
            });

            // Sync surface (FAZ 2). CloudClient is built up-front; the
            // engine reaches it via the AuthManager + SyncEngine wrapper.
            let cloud_url = std::env::var("CLOUD_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_CLOUD_URL.to_string());
            let cloud =
                CloudClient::new(&cloud_url).map_err(|e| anyhow::anyhow!("cloud client: {e:?}"))?;
            let auth = AuthManager::new(pool.clone());
            let engine = Arc::new(SyncEngine::new(repo, cloud, auth, pool));

            // FAZ 2.2 background tasks. The scheduler runs the hourly cron;
            // the network monitor probes /health every 30s and triggers a
            // sync on offline → online transitions. Both are fire-and-forget;
            // Tauri tearing down its async runtime stops them at app quit.
            tauri::async_runtime::block_on(async {
                if let Err(e) = scheduler::start(engine.clone()).await {
                    tracing::warn!(target: "cornell_diary::sync", error = %e, "scheduler disabled");
                }
            });
            let network = network::start(cloud_url.clone(), engine.clone());

            app.manage(SyncState { engine, network });

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
            connect_cloud,
            disconnect_cloud,
            trigger_sync,
            get_sync_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
