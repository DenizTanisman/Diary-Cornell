//! DTOs that travel between Rust and the React frontend over Tauri's IPC.
//!
//! Field names use camelCase via `#[serde(rename_all = "camelCase")]` so the
//! existing TypeScript `DiaryEntry` interface stays valid without any change
//! on the React side. Snake_case is preserved on the Rust side per Rust style.

use serde::{Deserialize, Serialize};

/// One Cornell-cue cell. `position` is 1..=7 (matches `title_N` / `content_N`
/// columns). Empty content is represented as `""`, not `null`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CueItem {
    pub position: u32,
    pub title: String,
    pub content: String,
}

/// One day's full Diary entry. The wide schema (`title_1..7` + `content_1..7`)
/// is normalised here into a single `cue_items` array — every consumer prefers
/// the array shape; the wide columns only exist at the storage boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiaryEntry {
    pub date: String,
    pub diary: String,
    pub cue_items: Vec<CueItem>,
    pub summary: String,
    pub quote: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub device_id: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BulkResult {
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
}

pub const MAX_CUE_ITEMS: u32 = 7;
