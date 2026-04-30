//! Cargo feature picks the backend at compile time. Everywhere else in
//! the Rust crate that historically wanted `sqlx::PgPool` now imports
//! `crate::db::DbPool` and never branches on the storage layer.
//!
//! - `postgres` (default, desktop): `DbPool = sqlx::PgPool`.
//! - `sqlite` (Android, iOS): `DbPool = sqlx::SqlitePool`.

use std::time::Duration;

use crate::error::DomainError;

const POOL_MAX_CONNECTIONS: u32 = 5;
const POOL_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(all(feature = "postgres", feature = "sqlite"))]
compile_error!(
    "enable exactly one storage backend: --features postgres OR --features sqlite"
);

#[cfg(not(any(feature = "postgres", feature = "sqlite")))]
compile_error!("enable a storage backend: --features postgres OR --features sqlite");

// ----------------------------------------------------------------------
// Postgres backend (desktop)
// ----------------------------------------------------------------------

#[cfg(feature = "postgres")]
pub type DbPool = sqlx::PgPool;

#[cfg(feature = "postgres")]
pub async fn build_pool(database_url: &str) -> Result<DbPool, DomainError> {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

    let options = database_url
        .parse::<PgConnectOptions>()
        .map_err(|e| DomainError::Path(format!("invalid DATABASE_URL: {e}")))?
        .application_name("cornell-diary");

    PgPoolOptions::new()
        .max_connections(POOL_MAX_CONNECTIONS)
        .acquire_timeout(POOL_ACQUIRE_TIMEOUT)
        .connect_with(options)
        .await
        .map_err(|e| DomainError::Storage(format!("postgres connect: {e}")))
}

#[cfg(feature = "postgres")]
pub async fn run_migrations(pool: &DbPool) -> Result<(), DomainError> {
    sqlx::migrate!("./postgres_migrations")
        .run(pool)
        .await
        .map_err(|e| DomainError::Storage(format!("postgres migrate: {e}")))
}

// ----------------------------------------------------------------------
// SQLite backend (Android, iOS, future embedded targets)
// ----------------------------------------------------------------------

#[cfg(feature = "sqlite")]
pub type DbPool = sqlx::SqlitePool;

#[cfg(feature = "sqlite")]
pub async fn build_pool(database_url: &str) -> Result<DbPool, DomainError> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    // `create_if_missing` lets a fresh Android install self-bootstrap on
    // first launch — Android sandboxes the app's data dir, no other
    // process will be racing the file. WAL is the right journal mode for
    // a single-writer app: better concurrent reader performance and
    // crash safety vs the default rollback journal.
    let options = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| DomainError::Path(format!("invalid DATABASE_URL: {e}")))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(POOL_MAX_CONNECTIONS)
        .acquire_timeout(POOL_ACQUIRE_TIMEOUT)
        .connect_with(options)
        .await
        .map_err(|e| DomainError::Storage(format!("sqlite connect: {e}")))
}

#[cfg(feature = "sqlite")]
pub async fn run_migrations(pool: &DbPool) -> Result<(), DomainError> {
    sqlx::migrate!("./sqlite_migrations")
        .run(pool)
        .await
        .map_err(|e| DomainError::Storage(format!("sqlite migrate: {e}")))
}
