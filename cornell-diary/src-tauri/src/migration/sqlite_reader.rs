//! Read-only adapter for the legacy SQLite file. Surfaces every row Diary's
//! schema cares about as plain Rust structs so the migration command can hand
//! them to the Postgres pool.
//!
//! The file is opened with `mode=ro` so we cannot accidentally write to the
//! user's data even if a bug slipped through.

use std::path::Path;

use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension};

use crate::db::models::{CueItem, DiaryEntry, MAX_CUE_ITEMS};
use crate::error::DomainError;

/// Holds every row read out of the legacy SQLite. Counts are kept alongside
/// the data so the report can be assembled even when `dry_run` is true and
/// the rows themselves are immediately discarded.
#[derive(Debug, Default, Clone)]
pub struct SqliteSnapshot {
    pub diary_entries: Vec<DiaryEntry>,
    pub sync_logs: Vec<SqliteSyncLog>,
    pub app_settings: Vec<SqliteAppSetting>,
}

#[derive(Debug, Clone)]
pub struct SqliteSyncLog {
    pub sync_type: String,
    pub method: String,
    pub device_id: String,
    pub peer_device_id: Option<String>,
    pub timestamp: String,
    pub entry_count: i64,
    pub checksum: String,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SqliteAppSetting {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

pub struct SqliteReader {
    conn: Connection,
}

impl SqliteReader {
    pub fn open(path: &Path) -> Result<Self, DomainError> {
        if !path.exists() {
            return Err(DomainError::Path(format!(
                "sqlite file not found: {}",
                path.display()
            )));
        }
        // mode=ro at the URI level is the strongest guarantee SQLite gives
        // us against accidental writes.
        let uri = format!("file:{}?mode=ro", path.display());
        let conn = Connection::open_with_flags(
            &uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?;
        Ok(Self { conn })
    }

    pub fn read_snapshot(&self) -> Result<SqliteSnapshot, DomainError> {
        Ok(SqliteSnapshot {
            diary_entries: self.read_all_diary_entries()?,
            sync_logs: self.read_all_sync_logs()?,
            app_settings: self.read_all_app_settings()?,
        })
    }

    fn read_all_diary_entries(&self) -> Result<Vec<DiaryEntry>, DomainError> {
        let mut stmt = self.conn.prepare(
            "SELECT date, diary, \
                title_1, content_1, title_2, content_2, title_3, content_3, \
                title_4, content_4, title_5, content_5, title_6, content_6, \
                title_7, content_7, summary, quote, created_at, updated_at, \
                device_id, version FROM diary_entries ORDER BY date ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let date: String = row.get(0)?;
                // Validate the date here so corrupted source data fails the
                // migration up front instead of slipping into Postgres and
                // tripping the CHECK constraint mid-transaction.
                let _ = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let diary: String = row.get(1)?;
                let mut cue_items = Vec::with_capacity(MAX_CUE_ITEMS as usize);
                for i in 0..MAX_CUE_ITEMS {
                    let title_col = 2 + (i * 2);
                    let content_col = title_col + 1;
                    let title: Option<String> = row.get(title_col as usize)?;
                    let content: Option<String> = row.get(content_col as usize)?;
                    if let Some(title) = title {
                        cue_items.push(CueItem {
                            position: i + 1,
                            title,
                            content: content.unwrap_or_default(),
                        });
                    }
                }
                cue_items.sort_by_key(|c| c.position);
                Ok(DiaryEntry {
                    date,
                    diary,
                    cue_items,
                    summary: row.get::<_, Option<String>>(16)?.unwrap_or_default(),
                    quote: row.get::<_, Option<String>>(17)?.unwrap_or_default(),
                    created_at: row.get(18)?,
                    updated_at: row.get(19)?,
                    device_id: row.get(20)?,
                    version: row.get(21)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn read_all_sync_logs(&self) -> Result<Vec<SqliteSyncLog>, DomainError> {
        // The table may be empty / absent in early installs; treat a missing
        // table as zero logs rather than a hard error.
        if !self.table_exists("sync_log")? {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare(
            "SELECT sync_type, method, device_id, peer_device_id, timestamp, \
                entry_count, checksum, status, error_message FROM sync_log",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SqliteSyncLog {
                    sync_type: row.get(0)?,
                    method: row.get(1)?,
                    device_id: row.get(2)?,
                    peer_device_id: row.get(3)?,
                    timestamp: row.get(4)?,
                    entry_count: row.get(5)?,
                    checksum: row.get(6)?,
                    status: row.get(7)?,
                    error_message: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn read_all_app_settings(&self) -> Result<Vec<SqliteAppSetting>, DomainError> {
        if !self.table_exists("app_settings")? {
            return Ok(Vec::new());
        }
        let mut stmt = self
            .conn
            .prepare("SELECT key, value, updated_at FROM app_settings")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SqliteAppSetting {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn table_exists(&self, name: &str) -> Result<bool, DomainError> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name = ?1",
                [name],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }
}
