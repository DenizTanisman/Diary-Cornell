//! One-shot SQLite → Postgres migration.
//!
//! This module exists only for the duration of FAZ 1.2; once every user has
//! run the migration once, FAZ 1.3 deletes it together with `tauri-plugin-sql`.
//! Intentionally kept self-contained: nothing in `db/` depends on it.

pub mod migrate_command;
pub mod sqlite_reader;

pub use migrate_command::migrate_sqlite_to_postgres;
