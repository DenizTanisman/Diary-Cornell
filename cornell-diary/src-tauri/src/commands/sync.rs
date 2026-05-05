//! Sync-surface Tauri commands.
//!
//! `connect_cloud` is the only command that touches the user's password —
//! the value lives only on the wire and inside the SyncEngine.connect call;
//! it is never written to a log or to the database.

use std::sync::Arc;

use tauri::State;
use tokio::sync::OnceCell;

use crate::commands::entries::AppState;
use crate::error::DomainError;
use crate::sync::{
    AutoSyncHandle, ConnectReport, NetworkMonitor, SyncEngine, SyncReport, SyncStatus,
};

/// Held in `tauri::State` alongside `AppState`. The engine is `Arc`-wrapped
/// so the scheduler / network monitor (FAZ 2.2) can clone it into
/// background tasks.
#[derive(Clone)]
pub struct SyncState {
    pub engine: Arc<SyncEngine>,
    pub network: NetworkMonitor,
}

/// Wraps the auto-sync scheduler handle in a `OnceCell` because the
/// scheduler is started lazily off the setup hook (the macOS app
/// delegate can't tolerate a nested block_on). The UI commands wait
/// briefly for it to populate, then read/write the active flag.
#[derive(Clone)]
pub struct AutoSyncState {
    pub handle: Arc<OnceCell<AutoSyncHandle>>,
}

#[tauri::command]
pub async fn connect_cloud(
    state: State<'_, SyncState>,
    username: String,
    password: String,
    device_label: String,
) -> Result<ConnectReport, DomainError> {
    state
        .engine
        .connect(&username, &password, &device_label)
        .await
}

#[tauri::command]
pub async fn disconnect_cloud(state: State<'_, SyncState>) -> Result<(), DomainError> {
    state.engine.disconnect().await
}

#[tauri::command]
pub async fn trigger_sync(state: State<'_, SyncState>) -> Result<SyncReport, DomainError> {
    state.engine.run_full_cycle().await
}

#[tauri::command]
pub async fn get_sync_status(state: State<'_, SyncState>) -> Result<SyncStatus, DomainError> {
    state.engine.status(state.network.is_online()).await
}

#[tauri::command]
pub async fn forgot_password_cloud(
    state: State<'_, SyncState>,
    email: String,
) -> Result<(), DomainError> {
    state.engine.forgot_password(&email).await
}

#[tauri::command]
pub async fn reset_password_cloud(
    state: State<'_, SyncState>,
    token: String,
    new_password: String,
) -> Result<(), DomainError> {
    state.engine.reset_password(&token, &new_password).await
}

#[tauri::command]
pub async fn get_auto_sync_enabled(
    auto: State<'_, AutoSyncState>,
    app: State<'_, AppState>,
) -> Result<bool, DomainError> {
    // Prefer the live scheduler flag — it's the source of truth at
    // runtime. Fall back to the persisted setting when the scheduler
    // hasn't booted yet (rare, only the first ~ms after launch).
    if let Some(handle) = auto.handle.get() {
        return Ok(handle.is_active());
    }
    let val = app
        .repo
        .get_setting("auto_sync_enabled")
        .await?
        .map(|v| v != "false")
        .unwrap_or(true);
    Ok(val)
}

#[tauri::command]
pub async fn set_auto_sync_enabled(
    enabled: bool,
    auto: State<'_, AutoSyncState>,
    app: State<'_, AppState>,
) -> Result<(), DomainError> {
    app.repo
        .set_setting("auto_sync_enabled", if enabled { "true" } else { "false" })
        .await?;
    if let Some(handle) = auto.handle.get() {
        handle.set_active(enabled);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_auto_start_cloud(
    app: State<'_, AppState>,
) -> Result<bool, DomainError> {
    Ok(app
        .repo
        .get_setting("auto_start_cloud_on_launch")
        .await?
        .map(|v| v == "true")
        .unwrap_or(false))
}

#[tauri::command]
pub async fn set_auto_start_cloud(
    enabled: bool,
    app: State<'_, AppState>,
) -> Result<(), DomainError> {
    app.repo
        .set_setting(
            "auto_start_cloud_on_launch",
            if enabled { "true" } else { "false" },
        )
        .await?;
    Ok(())
}
