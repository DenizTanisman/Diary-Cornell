use std::sync::Arc;

use tauri::Manager;

use crate::commands::crdt::{apply_local_op, subscribe_crdt, unsubscribe_crdt, CrdtState};
use crate::commands::entries::{
    diary_bulk_upsert, diary_delete, diary_entry_count, diary_get_by_date, diary_get_setting,
    diary_last_updated_at, diary_list_all, diary_list_dates, diary_list_range, diary_search,
    diary_set_setting, diary_upsert, AppState,
};
use crate::commands::sync::{
    connect_cloud, disconnect_cloud, get_sync_status, trigger_sync, SyncState,
};
use crate::crdt::{PendingOpRepo, WsClient};
use crate::db::{build_pool, run_migrations, EntryRepository, PostgresEntryRepository};
use crate::sync::auth::AuthManager;
use crate::sync::{network, CloudClient, SyncEngine};
// `scheduler` module exists (sync::scheduler) but is currently NOT wired into
// setup. Calling block_on(scheduler::start(...)) inside Tauri's macOS
// did_finish_launching panics with panic_cannot_unwind — tokio-cron-scheduler
// spawns its own tokio runtime and Cocoa's main thread can't unwind through
// the nested runtime. Hourly sync re-lands as a separate FAZ once the
// scheduler is moved to a managed JobScheduler in app state, started via
// tauri::async_runtime::spawn after the window is up. Manual "Şimdi
// Senkronize Et" + the network monitor's offline→online auto-trigger
// already cover every functional sync path for now.

mod commands;
mod crdt;
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
            let engine = Arc::new(SyncEngine::new(
                repo,
                cloud.clone(),
                auth.clone(),
                pool.clone(),
            ));

            // FAZ 2.2 background tasks. The scheduler runs the hourly cron;
            // The network monitor probes /health every 30s and triggers a
            // sync on offline → online transitions. Fire-and-forget; Tauri
            // tearing down its async runtime stops it at app quit.
            //
            // The hourly cron scheduler is intentionally NOT started from
            // here — see the file-top comment. Manual sync still works via
            // the trigger_sync command.
            let network = network::start(cloud_url.clone(), engine.clone());

            app.manage(SyncState { engine, network });

            // FAZ 3.2: WS client for live multi-user CRDT exchange. The
            // engine is initialised lazily on the first subscribe_crdt
            // call (it needs a tauri::AppHandle for emit), so we just
            // stash the dependencies here.
            let ws = WsClient::new(
                Arc::new(cloud),
                auth,
                PendingOpRepo::new(pool),
                app.handle().clone(),
            );
            app.manage(CrdtState { ws });

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
            subscribe_crdt,
            apply_local_op,
            unsubscribe_crdt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
