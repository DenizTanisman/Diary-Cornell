//! CRDT-pipeline Tauri commands (FAZ 3.2).
//!
//! Frontend invokes:
//! - `subscribe_crdt(entryDate, fieldName, seedText)` — open the WS
//!   socket if needed, mirror this entry on it, return whatever the
//!   document already materialises (so a reconnect-after-typing still
//!   shows the user's text).
//! - `apply_local_op(entryDate, fieldName, op)` — broadcast one
//!   keystroke. Falls back to `pending_ops` if the socket is down.
//! - `unsubscribe_crdt(entryDate, fieldName)` — stop mirroring this
//!   entry; keeps the socket open for the rest.

use std::sync::Arc;

use tauri::State;

use crate::crdt::operations::CharOp;
use crate::crdt::WsClient;
use crate::error::DomainError;

#[derive(Clone)]
pub struct CrdtState {
    pub ws: Arc<WsClient>,
}

#[tauri::command]
pub async fn subscribe_crdt(
    state: State<'_, CrdtState>,
    entry_date: String,
    field_name: String,
    seed_text: Option<String>,
) -> Result<String, DomainError> {
    state
        .ws
        .subscribe(&entry_date, &field_name, seed_text.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn apply_local_op(
    state: State<'_, CrdtState>,
    entry_date: String,
    field_name: String,
    op: CharOp,
) -> Result<String, DomainError> {
    state.ws.apply_local_op(&entry_date, &field_name, op).await
}

#[tauri::command]
pub async fn unsubscribe_crdt(
    state: State<'_, CrdtState>,
    entry_date: String,
    field_name: String,
) -> Result<(), DomainError> {
    state.ws.unsubscribe(&entry_date, &field_name).await
}
