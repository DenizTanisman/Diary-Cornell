//! Hourly background sync.
//!
//! Uses `tokio-cron-scheduler` so the schedule is declarative
//! (`0 0 * * * *` = top of every hour). We never rebuild the scheduler
//! after startup — there's only one job and its job_id is the same for
//! the lifetime of the process.

use std::sync::Arc;

use tokio_cron_scheduler::{Job, JobScheduler};

use crate::error::DomainError;
use crate::sync::SyncEngine;

const HOURLY_CRON: &str = "0 0 * * * *"; // top of every hour, all days

pub async fn start(engine: Arc<SyncEngine>) -> Result<JobScheduler, DomainError> {
    let scheduler = JobScheduler::new()
        .await
        .map_err(|e| DomainError::Internal(format!("scheduler init: {e}")))?;

    let engine_for_job = engine.clone();
    let job = Job::new_async(HOURLY_CRON, move |_uuid, _l| {
        let engine = engine_for_job.clone();
        Box::pin(async move {
            match engine.run_full_cycle().await {
                Ok(report) => tracing::info!(
                    target: "cornell_diary::sync",
                    pulled = report.pulled,
                    pushed = report.pushed,
                    "hourly sync completed"
                ),
                Err(e) => tracing::warn!(
                    target: "cornell_diary::sync",
                    error = %e,
                    "hourly sync skipped"
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
    Ok(scheduler)
}
