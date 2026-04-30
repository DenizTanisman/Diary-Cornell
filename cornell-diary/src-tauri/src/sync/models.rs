//! Wire-format DTOs shared between Cloud REST calls, the SyncEngine, and the
//! Tauri commands the frontend invokes.
//!
//! All field names use camelCase via `#[serde(rename_all = "camelCase")]` for
//! the data crossing the IPC boundary; the Cloud REST envelopes use
//! snake_case (Cloud is a Python service) so request structs override that
//! per-field with `rename = ...` where needed.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Cloud REST request / response envelopes
// ---------------------------------------------------------------------------

/// Cloud's `/auth/login` validates `{ username, password }`. The frontend
/// labels the field "Kullanıcı adı" so the user types the same handle they
/// registered with; we pass it through verbatim.
#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest<'a> {
    pub username: &'a str,
    pub password: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    /// Cloud may include the parsed expiry as a convenience; we recompute it
    /// from the JWT itself if the field is missing.
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    /// First-time logins also return user/peer so we can persist them in
    /// sync_metadata.
    #[serde(default)]
    pub user_id: Option<Uuid>,
    #[serde(default)]
    pub peer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshRequest<'a> {
    pub refresh_token: &'a str,
}

/// Cloud's `JournalOut` shape. Cloud calls it `title`; we surface it as
/// `name` to the frontend through ConnectReport for consistency with the
/// Diary domain vocabulary, but the wire field is `title`.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudJournal {
    pub id: Uuid,
    pub title: String,
    /// Cloud also returns owner_id, created_at, updated_at, role — we
    /// don't need them on the client but keep the struct lenient via
    /// `serde(default)` on optional fields and ignore unknown ones.
    #[serde(default)]
    #[allow(dead_code)]
    pub owner_id: Option<Uuid>,
    #[serde(default)]
    #[allow(dead_code)]
    pub role: Option<String>,
}

/// Wrapper Cloud uses for list endpoints — `{ "items": [...] }`. Without
/// this wrapper our `Vec<CloudJournal>` deserialise fails and the connect
/// flow silently abandons journal selection (cloud_journal_id stays NULL,
/// next sync errors with "cloud journal not selected").
#[derive(Debug, Clone, Deserialize)]
pub struct JournalListResponse {
    pub items: Vec<CloudJournal>,
}

/// Cloud's `JournalCreate` request body — single field `title` (not `name`).
#[derive(Debug, Clone, Serialize)]
pub struct JournalCreateRequest<'a> {
    pub title: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PullResponse {
    pub entries: Vec<CloudEntry>,
    /// Server-side cursor. We currently re-stamp `last_pull_at = now()`
    /// after a successful pull rather than reading this; field is kept so
    /// future Cloud versions can hand us a more precise watermark without a
    /// schema change.
    #[serde(default)]
    #[allow(dead_code)]
    pub cursor: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PushRequest {
    pub journal_id: Uuid,
    pub entries: Vec<PushEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PushResponse {
    pub merged: Vec<MergedEntry>,
    #[serde(default)]
    pub rejected: Vec<RejectedEntry>,
}

/// One entry as Cloud sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudEntry {
    pub id: Uuid,
    pub journal_id: Uuid,
    /// Local Diary uses ISO yyyy-mm-dd; Cloud accepts the same.
    pub entry_date: String,
    pub diary: String,
    pub title_1: Option<String>,
    pub content_1: Option<String>,
    pub title_2: Option<String>,
    pub content_2: Option<String>,
    pub title_3: Option<String>,
    pub content_3: Option<String>,
    pub title_4: Option<String>,
    pub content_4: Option<String>,
    pub title_5: Option<String>,
    pub content_5: Option<String>,
    pub title_6: Option<String>,
    pub content_6: Option<String>,
    pub title_7: Option<String>,
    pub content_7: Option<String>,
    pub summary: String,
    pub quote: String,
    pub version: i64,
    pub last_modified_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Same shape we push up — the Cloud-side id (`cloud_entry_id`) is optional
/// because brand-new local rows haven't been assigned one yet.
#[derive(Debug, Clone, Serialize)]
pub struct PushEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub entry_date: String,
    pub diary: String,
    pub title_1: Option<String>,
    pub content_1: Option<String>,
    pub title_2: Option<String>,
    pub content_2: Option<String>,
    pub title_3: Option<String>,
    pub content_3: Option<String>,
    pub title_4: Option<String>,
    pub content_4: Option<String>,
    pub title_5: Option<String>,
    pub content_5: Option<String>,
    pub title_6: Option<String>,
    pub content_6: Option<String>,
    pub title_7: Option<String>,
    pub content_7: Option<String>,
    pub summary: String,
    pub quote: String,
    pub version: i64,
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergedEntry {
    /// Local PK (the date string) so the engine knows which local row to
    /// mark as synced.
    pub local_date: String,
    pub cloud_id: Uuid,
    pub version: i64,
    /// Echoed from Cloud's authoritative timestamp. Engine doesn't read it
    /// today (mark_synced uses `now()` for last_synced_at), but the column
    /// is part of the Cloud contract so we keep it deserialised.
    #[allow(dead_code)]
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // engine logs the count; full contents reserved for future debug surface
pub struct RejectedEntry {
    pub local_date: String,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Tauri command DTOs (frontend-facing)
// ---------------------------------------------------------------------------

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
