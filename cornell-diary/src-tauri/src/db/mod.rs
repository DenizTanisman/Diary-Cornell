//! Data access layer — `EntryRepository` trait + Postgres implementation.
//!
//! As of FAZ 1.3 the SQLite implementation is gone; the contract still
//! lives as a trait so future backends (in-memory test doubles, a remote
//! sync target, etc.) can plug in without touching call sites.

pub mod models;
pub mod pool;
#[cfg(not(diary_sqlite))]
pub mod postgres_impl;
pub mod repository;
#[cfg(diary_sqlite)]
pub mod sqlite_impl;
#[cfg(test)]
pub mod test_helpers;

pub use models::{BulkResult, DiaryEntry};
pub use pool::{build_pool, run_migrations, DbPool};
#[cfg(not(diary_sqlite))]
pub use postgres_impl::PostgresEntryRepository;
pub use repository::EntryRepository;
#[cfg(diary_sqlite)]
pub use sqlite_impl::SqliteEntryRepository;
