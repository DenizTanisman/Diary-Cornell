//! Background auto-sync.
//!
//! Runs `engine.run_full_cycle()` every two minutes. The cron expression
//! is `0 */2 * * * *` — fire at second 0 of every minute that is a
//! multiple of two. Tokio-cron-scheduler is preferred over a hand-rolled
//! `tokio::time::interval` because it sleeps wall-clock-aware: a Mac
//! coming out of sleep doesn't catch up by replaying every missed
//! interval.
//!
//! The scheduler is **paused** when the user flips the auto-sync toggle
//! off. Internally that's a single `AtomicBool` the cron job consults
//! before each cycle — so toggling doesn't tear down / rebuild the
//! scheduler, which would race with an in-flight job.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio_cron_scheduler::{Job, JobScheduler};

use crate::error::DomainError;
use crate::sync::SyncEngine;

/// Cron expression for "every two minutes". The leading `0` is the
/// seconds field that tokio-cron-scheduler requires (cron(6) format),
/// not a minute spec.
const AUTO_SYNC_CRON: &str = "0 */2 * * * *";

/// Handle exposed to the rest of the app. Holding it keeps the cron
/// scheduler alive (drop = stop) and lets the UI flip the active flag
/// without rebuilding the job.
#[derive(Clone)]
pub struct AutoSyncHandle {
    active: Arc<AtomicBool>,
    // Held so the scheduler isn't dropped while the handle lives. Wrapped
    // in Arc<Mutex<_>> because JobScheduler isn't `Sync` and we want
    // multiple clones of the handle to coexist.
    _scheduler: Arc<tokio::sync::Mutex<JobScheduler>>,
}

impl AutoSyncHandle {
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
    }
}

pub async fn start(
    engine: Arc<SyncEngine>,
    initial_active: bool,
) -> Result<AutoSyncHandle, DomainError> {
    let scheduler = JobScheduler::new()
        .await
        .map_err(|e| DomainError::Internal(format!("scheduler init: {e}")))?;

    let active = Arc::new(AtomicBool::new(initial_active));
    let active_for_job = active.clone();
    let engine_for_job = engine.clone();

    let job = Job::new_async(AUTO_SYNC_CRON, move |_uuid, _l| {
        let engine = engine_for_job.clone();
        let active = active_for_job.clone();
        Box::pin(async move {
            if !active.load(Ordering::Relaxed) {
                // Toggle is off — don't even probe. Cheap.
                return;
            }
            match engine.run_full_cycle().await {
                Ok(report) => tracing::info!(
                    target: "cornell_diary::sync",
                    pulled = report.pulled,
                    pushed = report.pushed,
                    duration_ms = report.duration_ms,
                    "auto sync completed"
                ),
                Err(e) => tracing::warn!(
                    target: "cornell_diary::sync",
                    error = %e,
                    "auto sync skipped"
                ),
            }
        })
    })
    .map_err(|e| DomainError::Internal(format!("scheduler job: {e}")))?;

    scheduler
        .add(job)
        .await
        .map_err(|e| DomainError::Internal(format!("scheduler add: {e}")))?;
    scheduler
        .start()
        .await
        .map_err(|e| DomainError::Internal(format!("scheduler start: {e}")))?;

    Ok(AutoSyncHandle {
        active,
        _scheduler: Arc::new(tokio::sync::Mutex::new(scheduler)),
    })
}
