use std::sync::Arc;

use tauri::Manager;

use crate::commands::crdt::{
    apply_local_op, apply_local_text, subscribe_crdt, unsubscribe_crdt, CrdtState,
};
use crate::commands::entries::{
    diary_bulk_upsert, diary_delete, diary_entry_count, diary_get_by_date, diary_get_setting,
    diary_last_updated_at, diary_list_all, diary_list_dates, diary_list_range, diary_search,
    diary_set_setting, diary_upsert, AppState,
};
use crate::commands::cloud_service::{
    cloud_service_status, get_lan_addresses, start_cloud_service, stop_cloud_service,
    CloudServiceState,
};
use crate::commands::mdns::{discover_cloud_servers, MdnsState};
use crate::commands::profile::{
    delete_cloud_profile, get_active_cloud_profile, list_cloud_profiles, set_active_cloud_profile,
    upsert_cloud_profile, ProfileState,
};
use crate::commands::sync::{
    connect_cloud, disconnect_cloud, forgot_password_cloud, get_auto_start_cloud,
    get_auto_sync_enabled, get_sync_status, reset_password_cloud, set_auto_start_cloud,
    set_auto_sync_enabled, trigger_sync, AutoSyncState, SyncState,
};
use crate::crdt::{PendingOpRepo, WsClient};
use crate::db::cloud_profile;
use crate::db::{build_pool, run_migrations, EntryRepository};
#[cfg(not(diary_sqlite))]
use crate::db::PostgresEntryRepository;
#[cfg(diary_sqlite)]
use crate::db::SqliteEntryRepository;
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

// Compile-time fallback. Mobile builds inherit `DIARY_CLOUD_URL` from
// build.rs (laptop LAN IP); desktop without an override falls through
// to loopback. The runtime CLOUD_URL env var still wins everywhere.
const DEFAULT_CLOUD_URL: &str = match option_env!("DIARY_CLOUD_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:5001",
};

#[cfg(not(diary_sqlite))]
const STORAGE_BACKEND: &str = "postgres";
#[cfg(diary_sqlite)]
const STORAGE_BACKEND: &str = "sqlite";

/// Where the local DB lives.
///
/// - Postgres: must come from the `DATABASE_URL` env var. There's no
///   sensible default — you don't want a fresh checkout silently
///   pointing at a previous user's prod DB.
/// - SQLite: a single file under the app's data directory (Android:
///   `/data/data/com.deniz.cornelldiary/files/cornell_diary.db`,
///   resolved by `tauri::Manager::path()`). Override with
///   `DATABASE_URL` if you want to point at a custom file (used by
///   integration tests that drive a temp file).
#[cfg(not(diary_sqlite))]
fn resolve_database_url(_app: &tauri::App) -> anyhow::Result<String> {
    std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set (e.g. via .env)"))
}

#[cfg(diary_sqlite)]
fn resolve_database_url(app: &tauri::App) -> anyhow::Result<String> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.is_empty() {
            return Ok(url);
        }
    }
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| anyhow::anyhow!("app data dir: {e:?}"))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| anyhow::anyhow!("create app data dir: {e:?}"))?;
    let path = dir.join("cornell_diary.db");
    // sqlite uri scheme — sqlx parses `sqlite://path?...` and our
    // build_pool sets create_if_missing so the first launch self-seeds.
    Ok(format!("sqlite://{}", path.display()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // H-1 — explicit panic hook that fans out to BOTH tracing::error
    // (so logcat picks it up via tracing-android on Android) AND stderr
    // (so desktop terminal sees it the way it always has). Earlier
    // sprint hooks used eprintln only, which silently disappeared on
    // Android because Tauri's stderr is /dev/null on mobile. Wraps the
    // default hook so the backtrace still goes through the usual path.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<no-location>".into());
        tracing::error!(target: "cornell_diary::panic", at = %location, "{}", info);
        eprintln!("=== cornell_diary PANIC ===");
        eprintln!("{info}");
        eprintln!("at {location}");
        eprintln!("===========================");
        prev_hook(info);
    }));

    // Auto-load .env so `cargo run` from src-tauri/ inherits the
    // sibling `cornell-diary/.env` (DATABASE_URL, CLOUD_URL, etc.)
    // without the developer having to `set -a; source .env; set +a`
    // every time. dotenvy walks the cwd up to the first hit, so it
    // finds cornell-diary/.env from inside src-tauri/. .ok() means
    // production bundles (no .env present) silently skip — vars are
    // expected to come from the OS environment in that case.
    //
    // Skip on Android — the executable's cwd there is `/` (init's
    // default), and walking up looking for .env hits permission
    // errors on system mount points. Android setup uses SQLite + the
    // app data dir, no env-driven config needed.
    #[cfg(not(target_os = "android"))]
    let _ = dotenvy::dotenv();

    // Faz 1.3: Sentry. Empty DSN → no init, no overhead, no network.
    // The `_guard` keeps the client alive for the process lifetime;
    // dropping it flushes pending events.
    let _sentry_guard = std::env::var("SENTRY_DSN").ok().filter(|s| !s.is_empty()).map(|dsn| {
        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                environment: std::env::var("APP_ENV")
                    .ok()
                    .map(Into::into)
                    .or(Some("development".into())),
                traces_sample_rate: 0.1,
                send_default_pii: false,
                ..Default::default()
            },
        ))
    });

    // H-1 — tracing init forks per platform. Desktop keeps the existing
    // stderr fmt subscriber. Android wires `tracing-android`, which
    // forwards every event through __android_log_print to logcat under
    // the `cornell_diary` tag — without it, our tracing::info! lines
    // and the panic hook below disappear into Tauri's nulled stderr.
    // Same env filter on both sides so RUST_LOG behaves identically.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "cornell_diary=info,sqlx=warn".into());

    #[cfg(target_os = "android")]
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        let android_layer = tracing_android::layer("cornell_diary")
            .expect("init tracing-android logcat layer");
        let _ = tracing_subscriber::registry()
            .with(env_filter)
            .with(android_layer)
            .try_init();
    }

    #[cfg(not(target_os = "android"))]
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // Storage backend is picked at compile time via Cargo
            // features. On desktop the `postgres` feature is on and
            // DATABASE_URL must point at a reachable Postgres. On
            // Android (and future iOS) the `sqlite` feature is on and
            // we resolve the SQLite file inside the app's data dir —
            // there's no way for the user to misconfigure it.
            let database_url = resolve_database_url(app)?;
            tracing::info!(target: "cornell_diary", backend = STORAGE_BACKEND, %database_url, "storage backend");

            let pool = tauri::async_runtime::block_on(async {
                let pool = build_pool(&database_url)
                    .await
                    .map_err(|e| anyhow::anyhow!("db pool: {e:?}"))?;
                run_migrations(&pool)
                    .await
                    .map_err(|e| anyhow::anyhow!("db migrate: {e:?}"))?;
                Ok::<_, anyhow::Error>(pool)
            })?;

            #[cfg(not(diary_sqlite))]
            let repo: Arc<dyn EntryRepository> =
                Arc::new(PostgresEntryRepository::new(pool.clone()));
            #[cfg(diary_sqlite)]
            let repo: Arc<dyn EntryRepository> =
                Arc::new(SqliteEntryRepository::new(pool.clone()));
            app.manage(AppState {
                repo: repo.clone(),
                pg_pool: Some(pool.clone()),
            });

            // MD 03 / Faz 3.1 — read the active cloud profile from the
            // freshly migrated DB. Precedence: CLOUD_URL env (developer
            // override) > active_profile.base_url > DEFAULT_CLOUD_URL.
            let profile_repo = cloud_profile::create_repo(pool.clone());
            let active_profile_url = tauri::async_runtime::block_on(async {
                profile_repo
                    .get_active()
                    .await
                    .ok()
                    .map(|p| p.base_url)
                    .filter(|s| !s.is_empty())
            });
            let cloud_url = std::env::var("CLOUD_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .or(active_profile_url)
                .unwrap_or_else(|| DEFAULT_CLOUD_URL.to_string());
            tracing::info!(
                target: "cornell_diary",
                cloud_url = %cloud_url,
                "active cloud profile resolved"
            );
            let cloud =
                CloudClient::new(&cloud_url).map_err(|e| anyhow::anyhow!("cloud client: {e:?}"))?;
            let auth = AuthManager::new(pool.clone());
            let engine = Arc::new(SyncEngine::new(
                repo,
                cloud.clone(),
                auth.clone(),
                pool.clone(),
            ));

            // FAZ 2.2 + auto-sync. The network monitor probes /health every
            // 30s and triggers a sync on offline → online transitions; the
            // tokio-cron scheduler fires `engine.run_full_cycle()` every
            // two minutes when its toggle is on. Both are fire-and-forget
            // and Tauri's async runtime cleans them up at quit.
            let network = network::start(cloud_url.clone(), engine.clone());

            // Read the saved auto-sync preference; default ON the first
            // time the app launches.
            let auto_sync_initial = tauri::async_runtime::block_on(async {
                engine
                    .pool()
                    .acquire()
                    .await
                    .ok();
                // Use the existing settings table (key/value) — see
                // entries::diary_get_setting / diary_set_setting.
                use sqlx::Row;
                #[cfg(not(diary_sqlite))]
                let row: Result<Option<sqlx::postgres::PgRow>, _> = sqlx::query(
                    "SELECT value FROM app_settings WHERE key = 'auto_sync_enabled'",
                )
                .fetch_optional(&pool)
                .await;
                #[cfg(diary_sqlite)]
                let row: Result<Option<sqlx::sqlite::SqliteRow>, _> = sqlx::query(
                    "SELECT value FROM app_settings WHERE key = 'auto_sync_enabled'",
                )
                .fetch_optional(&pool)
                .await;
                row.ok()
                    .flatten()
                    .and_then(|r| r.try_get::<String, _>("value").ok())
                    .map(|v| v != "false")
                    .unwrap_or(true)
            });

            // Spawn the scheduler outside the setup hook so the macOS
            // app delegate doesn't see a nested tokio runtime block_on
            // (which is what made the 2.2 attempt panic). After the
            // window is up, the runtime is fully ours.
            let auto_sync_handle: std::sync::Arc<tokio::sync::OnceCell<sync::AutoSyncHandle>> =
                std::sync::Arc::new(tokio::sync::OnceCell::new());
            {
                let cell = auto_sync_handle.clone();
                let engine_for_sched = engine.clone();
                tauri::async_runtime::spawn(async move {
                    match sync::scheduler::start(engine_for_sched, auto_sync_initial).await {
                        Ok(handle) => {
                            tracing::info!(
                                target: "cornell_diary::sync",
                                active = auto_sync_initial,
                                "auto-sync scheduler ready"
                            );
                            let _ = cell.set(handle);
                        }
                        Err(e) => tracing::warn!(
                            target: "cornell_diary::sync",
                            error = %e,
                            "auto-sync scheduler failed to start"
                        ),
                    }
                });
            }

            app.manage(SyncState { engine, network });
            app.manage(ProfileState { repo: profile_repo });
            app.manage(AutoSyncState { handle: auto_sync_handle });

            let cloud_service_state = CloudServiceState::default();

            // Tier 3 follow-up: read the "auto-start cloud on Diary
            // launch" toggle and, if on, spawn the Cloud uvicorn from a
            // managed child. Default OFF — only opt-in users pay the
            // ~250 MB RAM cost. Mobile targets skip the read entirely
            // since the spawn path below is also gated out for them.
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let auto_start_cloud = tauri::async_runtime::block_on(async {
                use sqlx::Row;
                #[cfg(not(diary_sqlite))]
                let row: Result<Option<sqlx::postgres::PgRow>, _> = sqlx::query(
                    "SELECT value FROM app_settings WHERE key = 'auto_start_cloud_on_launch'",
                )
                .fetch_optional(&pool)
                .await;
                #[cfg(diary_sqlite)]
                let row: Result<Option<sqlx::sqlite::SqliteRow>, _> = sqlx::query(
                    "SELECT value FROM app_settings WHERE key = 'auto_start_cloud_on_launch'",
                )
                .fetch_optional(&pool)
                .await;
                row.ok()
                    .flatten()
                    .and_then(|r| r.try_get::<String, _>("value").ok())
                    .map(|v| v == "true")
                    .unwrap_or(false)
            });

            // mDNS state — Mac advertises Cloud here, phone discovers
            // it from the same crate via discover_cloud_servers. The
            // advertise lifecycle is hooked into start/stop_cloud_service
            // so it automatically tracks whether Cloud is up.
            let mdns_state = MdnsState::default();

            // Mobile platforms can't host Cloud (~/Projects/Cloud
            // doesn't exist on Android/iOS, and even if it did the
            // app sandbox blocks spawning external processes). Skip
            // the auto-start path entirely there so we don't pollute
            // last_error with a "klasör bulunamadı" the panel would
            // happily display next to the unrelated login form.
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if auto_start_cloud {
                let state_for_spawn = cloud_service_state.clone();
                let mdns_for_spawn = mdns_state.clone();
                tauri::async_runtime::spawn(async move {
                    match commands::cloud_service::start_cloud_service_internal(
                        &state_for_spawn,
                        &mdns_for_spawn,
                    )
                    .await
                    {
                        Ok(s) => tracing::info!(
                            target: "cornell_diary::cloud_service",
                            state = %s.state,
                            "auto-started Cloud on Diary launch"
                        ),
                        Err(e) => tracing::warn!(
                            target: "cornell_diary::cloud_service",
                            error = %e,
                            "auto-start Cloud failed"
                        ),
                    }
                });
            }

            app.manage(cloud_service_state);
            app.manage(mdns_state);

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
            forgot_password_cloud,
            reset_password_cloud,
            subscribe_crdt,
            apply_local_op,
            apply_local_text,
            unsubscribe_crdt,
            list_cloud_profiles,
            get_active_cloud_profile,
            set_active_cloud_profile,
            upsert_cloud_profile,
            delete_cloud_profile,
            get_auto_sync_enabled,
            set_auto_sync_enabled,
            get_auto_start_cloud,
            set_auto_start_cloud,
            start_cloud_service,
            stop_cloud_service,
            cloud_service_status,
            get_lan_addresses,
            discover_cloud_servers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
