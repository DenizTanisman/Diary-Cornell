//! `EntryRepository` is the only data-access contract the rest of the app
//! depends on. FAZ 1.0 provides a SQLite implementation; FAZ 1.1 adds a
//! Postgres implementation that obeys the same trait so handlers don't notice
//! the swap. FAZ 1.3 deletes the SQLite implementation entirely.

use async_trait::async_trait;

use crate::db::models::{BulkResult, DiaryEntry};
use crate::error::DomainError;

#[async_trait]
pub trait EntryRepository: Send + Sync {
    /// One-time bootstrap: open the connection / pool, apply any required
    /// pragmas. Idempotent. Today the production Postgres impl makes this
    /// a no-op (the pool is built in lib.rs setup), but the trait keeps it
    /// for future backends with their own lazy-init paths.
    #[allow(dead_code)]
    async fn init(&self) -> Result<(), DomainError>;

    async fn get_by_date(&self, date: &str) -> Result<Option<DiaryEntry>, DomainError>;
    async fn upsert(&self, entry: DiaryEntry) -> Result<DiaryEntry, DomainError>;
    async fn delete(&self, date: &str) -> Result<(), DomainError>;

    async fn list_dates(&self) -> Result<Vec<String>, DomainError>;
    async fn list_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<DiaryEntry>, DomainError>;
    async fn list_all(&self) -> Result<Vec<DiaryEntry>, DomainError>;

    async fn search(&self, query: &str, limit: u32) -> Result<Vec<DiaryEntry>, DomainError>;

    async fn entry_count(&self) -> Result<i64, DomainError>;
    async fn last_updated_at(&self) -> Result<Option<String>, DomainError>;

    /// Protective bulk insert: existing dates are NEVER overwritten — only
    /// brand-new dates are inserted. Mirrors the existing TS behaviour
    /// (handoff Part I §3.5 / SQLiteRepository.bulkUpsert).
    async fn bulk_upsert(&self, entries: Vec<DiaryEntry>) -> Result<BulkResult, DomainError>;

    async fn get_setting(&self, key: &str) -> Result<Option<String>, DomainError>;
    async fn set_setting(&self, key: &str, value: &str) -> Result<(), DomainError>;
}
