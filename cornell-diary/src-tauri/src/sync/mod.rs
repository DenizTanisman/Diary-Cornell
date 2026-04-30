//! Cloud sync surface (FAZ 2).
//!
//! All HTTP and conflict-resolution code lives here, isolated from `db/` so
//! `EntryRepository` stays storage-only and the sync engine can be unit
//! tested against a `mockito` server without touching Postgres at all.

pub mod auth;
pub mod client;
pub mod conflict;
pub mod engine;
pub mod meta;
pub mod models;
pub mod network;
// `scheduler` is built but not wired into setup yet — see lib.rs note.
// `#[allow(dead_code)]` keeps clippy quiet; module is re-enabled when the
// hourly cron is moved to a managed JobScheduler in app state.
#[allow(dead_code)]
pub mod scheduler;

pub use client::CloudClient;
pub use engine::SyncEngine;
pub use models::{ConnectReport, SyncReport, SyncStatus};
pub use network::NetworkMonitor;
