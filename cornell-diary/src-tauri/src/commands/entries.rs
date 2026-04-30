//! Entry-CRUD Tauri commands.
//!
//! Every command is `async`, takes a `tauri::State<AppState>`, and returns
//! `Result<T, DomainError>`. The frontend sees the `DomainError` envelope
//! (`{ code, message }`) on failure — never a stack trace.

use std::sync::Arc;

use sqlx::PgPool;
use tauri::State;

use crate::db::{BulkResult, DiaryEntry, EntryRepository};
use crate::error::DomainError;

/// Shared application state; held by Tauri once `manage`d in `lib::run()`.
/// `Arc<dyn EntryRepository>` lets us swap the concrete implementation at
/// boot. `pg_pool` is held so FAZ 2 (sync engine, scheduler) can reach the
/// same pool the repository uses without round-tripping through invoke.
pub struct AppState {
    pub repo: Arc<dyn EntryRepository>,
    /// Currently no command reads it directly — clippy flags it as dead
    /// until the sync surface lands.
    #[allow(dead_code)]
    pub pg_pool: Option<PgPool>,
}

#[tauri::command]
pub async fn diary_get_by_date(
    state: State<'_, AppState>,
    date: String,
) -> Result<Option<DiaryEntry>, DomainError> {
    state.repo.get_by_date(&date).await
}

#[tauri::command]
pub async fn diary_upsert(
    state: State<'_, AppState>,
    entry: DiaryEntry,
) -> Result<DiaryEntry, DomainError> {
    state.repo.upsert(entry).await
}

#[tauri::command]
pub async fn diary_delete(state: State<'_, AppState>, date: String) -> Result<(), DomainError> {
    state.repo.delete(&date).await
}

#[tauri::command]
pub async fn diary_list_dates(state: State<'_, AppState>) -> Result<Vec<String>, DomainError> {
    state.repo.list_dates().await
}

#[tauri::command]
pub async fn diary_list_range(
    state: State<'_, AppState>,
    start_date: String,
    end_date: String,
) -> Result<Vec<DiaryEntry>, DomainError> {
    state.repo.list_range(&start_date, &end_date).await
}

#[tauri::command]
pub async fn diary_list_all(state: State<'_, AppState>) -> Result<Vec<DiaryEntry>, DomainError> {
    state.repo.list_all().await
}

#[tauri::command]
pub async fn diary_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<DiaryEntry>, DomainError> {
    state.repo.search(&query, limit.unwrap_or(50)).await
}

#[tauri::command]
pub async fn diary_entry_count(state: State<'_, AppState>) -> Result<i64, DomainError> {
    state.repo.entry_count().await
}

#[tauri::command]
pub async fn diary_last_updated_at(
    state: State<'_, AppState>,
) -> Result<Option<String>, DomainError> {
    state.repo.last_updated_at().await
}

#[tauri::command]
pub async fn diary_bulk_upsert(
    state: State<'_, AppState>,
    entries: Vec<DiaryEntry>,
) -> Result<BulkResult, DomainError> {
    state.repo.bulk_upsert(entries).await
}

#[tauri::command]
pub async fn diary_get_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, DomainError> {
    state.repo.get_setting(&key).await
}

#[tauri::command]
pub async fn diary_set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), DomainError> {
    state.repo.set_setting(&key, &value).await
}
