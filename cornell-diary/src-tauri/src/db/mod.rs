//! Data access layer — `EntryRepository` trait + Postgres implementation.
//!
//! As of FAZ 1.3 the SQLite implementation is gone; the contract still
//! lives as a trait so future backends (in-memory test doubles, a remote
//! sync target, etc.) can plug in without touching call sites.

pub mod models;
pub mod pool;
pub mod postgres_impl;
pub mod repository;

pub use models::{BulkResult, DiaryEntry};
pub use pool::{build_pool, run_migrations};
pub use postgres_impl::PostgresEntryRepository;
pub use repository::EntryRepository;
