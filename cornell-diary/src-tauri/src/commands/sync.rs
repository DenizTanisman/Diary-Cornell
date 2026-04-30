//! Sync-surface Tauri commands.
//!
//! `connect_cloud` is the only command that touches the user's password —
//! the value lives only on the wire and inside the SyncEngine.connect call;
//! it is never written to a log or to the database.

use std::sync::Arc;

use tauri::State;

use crate::error::DomainError;
use crate::sync::{ConnectReport, NetworkMonitor, SyncEngine, SyncReport, SyncStatus};

/// Held in `tauri::State` alongside `AppState`. The engine is `Arc`-wrapped
/// so the scheduler / network monitor (FAZ 2.2) can clone it into
/// background tasks.
#[derive(Clone)]
pub struct SyncState {
    pub engine: Arc<SyncEngine>,
    pub network: NetworkMonitor,
}

#[tauri::command]
pub async fn connect_cloud(
    state: State<'_, SyncState>,
    email: String,
    password: String,
    device_label: String,
) -> Result<ConnectReport, DomainError> {
    state.engine.connect(&email, &password, &device_label).await
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
