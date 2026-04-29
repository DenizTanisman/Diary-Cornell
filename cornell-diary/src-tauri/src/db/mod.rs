//! Data access layer — Repository trait + SQLite implementation.
//!
//! Trait-first design: FAZ 1.0 ships a SQLite implementation that reads
//! the same file the existing tauri-plugin-sql frontend touches. FAZ 1.1
//! adds a Postgres implementation behind the same trait. FAZ 1.3 deletes
//! the SQLite implementation entirely.

pub mod models;
pub mod repository;
pub mod sqlite_impl;

pub use models::{BulkResult, DiaryEntry};
pub use repository::EntryRepository;
pub use sqlite_impl::SqliteEntryRepository;
