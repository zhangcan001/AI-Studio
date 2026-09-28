use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

pub const REPAIR_JOB_RUNNING: &str = "RUNNING";
pub const REPAIR_JOB_COMPLETED: &str = "COMPLETED";
pub const REPAIR_JOB_FAILED: &str = "FAILED";
pub const REPAIR_JOB_SKIPPED: &str = "SKIPPED";

/// One persisted application-level repair job run marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepairJobRecord {
    pub job_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub summary_json: String,
}

/// Persistence boundary for `app_repair_jobs`. A job whose status is
/// COMPLETED or SKIPPED is never run again unless it is explicitly reset.
#[async_trait]
pub trait RepairJobRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<RepairJobRecord>, RepositoryError>;

    async fn find(&self, job_id: &str) -> Result<Option<RepairJobRecord>, RepositoryError>;

    async fn mark_started(
        &self,
        job_id: &str,
        started_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;

    async fn mark_finished(
        &self,
        job_id: &str,
        status: &str,
        completed_at: DateTime<Utc>,
        summary_json: &str,
    ) -> Result<(), RepositoryError>;

    /// Forget a job marker so the job runs again on the next pass (used after
    /// importing packages or backups that may contain legacy recipes).
    async fn reset(&self, job_id: &str) -> Result<(), RepositoryError>;
}
