//! Wire-format DTOs shared between Cloud REST calls, the SyncEngine, and the
//! Tauri commands the frontend invokes.
//!
//! Two namespaces live here:
//! - **Cloud** REST envelopes — match the live OpenAPI spec exposed by the
//!   sync server. Snake_case Python defaults; `serde(default)` and
//!   `Option<…>` keep us forward-compatible with new optional fields.
//! - **Frontend** IPC payloads — camelCase via `#[serde(rename_all)]` so the
//!   existing TypeScript types in `src/types/cloudSync.ts` keep working.
//!
//! The Cloud entry shape (`cue_column`, `notes_column`, `summary`, `planlar`)
//! is the same projection the Reporter sidecar produces. Diary stores
//! `cue_items[]` natively; the engine flattens to / from that string in
//! `engine.rs::push_entry_from` / `entry_from_cloud`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ===========================================================================
// Auth
// ===========================================================================

/// `POST /auth/login` body.
#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest<'a> {
    pub username: &'a str,
    pub password: &'a str,
}

/// `POST /auth/refresh` body.
#[derive(Debug, Clone, Serialize)]
pub struct RefreshRequest<'a> {
    pub refresh_token: &'a str,
}

/// `TokenResponse`. Cloud does NOT include `peer_id` or `user_id` here —
/// peer_id is generated locally and persisted in sync_metadata; the user id
/// can be fetched from `/auth/me` when needed. We keep the optional fields
/// in case a future Cloud version starts returning them.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    /// Cloud sends `access_ttl_seconds`; consumers compute the expiry from
    /// JWT exp instead. Field kept in case Cloud ever sends an absolute
    /// timestamp.
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub access_ttl_seconds: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub user_id: Option<Uuid>,
    #[serde(default)]
    pub peer_id: Option<String>,
}

// ===========================================================================
// Journals
// ===========================================================================

/// `JournalOut` — one journal entry from Cloud.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudJournal {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub owner_id: Option<Uuid>,
    #[serde(default)]
    #[allow(dead_code)]
    pub role: Option<String>,
}

/// `JournalListResponse` — Cloud wraps lists in `{ "items": [...] }`.
#[derive(Debug, Clone, Deserialize)]
pub struct JournalListResponse {
    pub items: Vec<CloudJournal>,
}

/// `JournalCreate` — single field `title` (not `name`).
#[derive(Debug, Clone, Serialize)]
pub struct JournalCreateRequest<'a> {
    pub title: &'a str,
}

// ===========================================================================
// Sync — Pull
// ===========================================================================

/// `PullResponse`.
#[derive(Debug, Clone, Deserialize)]
pub struct PullResponse {
    pub entries: Vec<CloudEntry>,
    /// Char-level CRDT ops Cloud accumulated since `since`. Phase 3 will
    /// apply these; Phase 2 ignores them and relies on the full-entry rows.
    #[serde(default)]
    #[allow(dead_code)]
    pub crdt_ops: Vec<serde_json::Value>,
    /// Authoritative server clock at the moment the response was assembled.
    /// We reuse this as the next pull's `since` cursor when available;
    /// falls back to `Utc::now()` otherwise.
    #[serde(default)]
    pub server_time: Option<DateTime<Utc>>,
}

// ===========================================================================
// Sync — Push
// ===========================================================================

/// `PushRequest`. peer_id is mandatory (mints a 422 from Cloud if missing
/// or empty). idempotency_key lets Cloud dedupe on retry; we send a fresh
/// uuid each call.
#[derive(Debug, Clone, Serialize)]
pub struct PushRequest {
    pub journal_id: Uuid,
    pub peer_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub entries: Vec<PushEntry>,
    /// Char-level ops; Phase 2 always sends an empty list.
    pub crdt_ops: Vec<serde_json::Value>,
}

/// `PushResponse`.
#[derive(Debug, Clone, Deserialize)]
pub struct PushResponse {
    pub merged_entries: Vec<CloudEntry>,
    #[serde(default)]
    #[allow(dead_code)]
    pub crdt_ops_applied: i64,
    #[serde(default)]
    #[allow(dead_code)]
    pub crdt_ops_skipped: i64,
    #[serde(default)]
    #[allow(dead_code)]
    pub duplicate: bool,
    #[serde(default)]
    #[allow(dead_code)]
    pub server_time: Option<DateTime<Utc>>,
}

// ===========================================================================
// Entry shape — Cloud-side projection (cue_column, notes_column, summary, planlar)
// ===========================================================================

/// `EntryOut` — the read shape Cloud returns from pull / push merge.
/// Diary stores `cue_items[]` natively; engine.rs converts on the way in.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudEntry {
    pub id: Uuid,
    #[allow(dead_code)]
    pub journal_id: Uuid,
    pub entry_date: String,
    #[serde(default)]
    pub cue_column: String,
    #[serde(default)]
    pub notes_column: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub planlar: String,
    pub version: i64,
    /// Cloud uses a `last_modified_at` field name we still want for
    /// conflict resolution; some entry payloads also carry it as
    /// `updated_at`, so we accept both via Optionals + a helper.
    #[serde(default, alias = "updated_at")]
    pub last_modified_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub last_modified_by: Option<Uuid>,
}

impl CloudEntry {
    pub fn modified_at_or_now(&self) -> DateTime<Utc> {
        self.last_modified_at.unwrap_or_else(Utc::now)
    }
}

/// `PushEntryDTO` — the write shape Cloud expects in PushRequest.entries.
/// `id` is None for brand-new local rows; once Cloud assigns one it comes
/// back in PushResponse.merged_entries and we persist it as `cloud_entry_id`.
#[derive(Debug, Clone, Serialize)]
pub struct PushEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub entry_date: String,
    pub cue_column: String,
    pub notes_column: String,
    pub summary: String,
    pub planlar: String,
    pub version: i64,
    pub last_modified_at: DateTime<Utc>,
}

// ===========================================================================
// Frontend IPC payloads
// ===========================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectReport {
    pub user_id: Option<Uuid>,
    pub peer_id: String,
    pub journal_id: Uuid,
    pub journal_name: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub enabled: bool,
    pub online: bool,
    pub last_pull_at: Option<DateTime<Utc>>,
    pub last_push_at: Option<DateTime<Utc>>,
    pub dirty_count: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub pulled: u32,
    pub pushed: u32,
    pub conflicts_cloud_won: u32,
    pub conflicts_local_won: u32,
    pub rejected: u32,
    pub duration_ms: u64,
}
