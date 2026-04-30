//! Shared helpers for Postgres-backed integration tests.
//!
//! Every site that historically reached for `DATABASE_URL` now goes
//! through `test_database_url()` so we can swap in a dedicated test DB
//! without editing five call sites. The convention:
//!
//! 1. `TEST_DATABASE_URL` if set — points at a separate `_test` DB
//!    that's safe to TRUNCATE between tests. Recommended for any
//!    environment where `cargo test` and a running Diary share a
//!    machine.
//! 2. Otherwise fall back to `DATABASE_URL` — keeps zero-config CI
//!    workable, with the documented downside that tests will TRUNCATE
//!    the dev DB.
//! 3. Otherwise the test skips with an `eprintln!`.
//!
//! Day-2 setup is one line:
//! ```sh
//! createdb -h 127.0.0.1 -p 5435 -U diary_user diary_db_test
//! export TEST_DATABASE_URL='postgres://diary_user:...@127.0.0.1:5435/diary_db_test'
//! ```

#![allow(dead_code)] // every consumer is `#[cfg(test)]` only

/// Returns the URL the next `build_pool` call should use, preferring an
/// explicitly-set `TEST_DATABASE_URL`.
pub fn test_database_url() -> Option<String> {
    if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
        if !url.is_empty() {
            return Some(url);
        }
    }
    std::env::var("DATABASE_URL").ok().filter(|s| !s.is_empty())
}
