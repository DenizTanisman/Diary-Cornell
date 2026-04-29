//! `sqlx::PgPool` factory + once-per-process migration runner.
//!
//! The pool is small (5 connections) — Diary is single-tenant and almost
//! always issues one query at a time. We pin `acquire_timeout` to 5s so a
//! transient Postgres restart surfaces as `Storage` instead of silently
//! hanging IPC calls.

use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;

use crate::error::DomainError;

const POOL_MAX_CONNECTIONS: u32 = 5;
const POOL_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn build_pool(database_url: &str) -> Result<PgPool, DomainError> {
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

/// Run the bundled SQL migrations under `postgres_migrations/`.
///
/// `sqlx::migrate!` reads the directory at compile time and embeds every
/// `*.sql` file in the binary. Re-running is safe — sqlx tracks state in
/// the `_sqlx_migrations` table.
pub async fn run_migrations(pool: &PgPool) -> Result<(), DomainError> {
    sqlx::migrate!("./postgres_migrations")
        .run(pool)
        .await
        .map_err(|e| DomainError::Storage(format!("postgres migrate: {e}")))
}
