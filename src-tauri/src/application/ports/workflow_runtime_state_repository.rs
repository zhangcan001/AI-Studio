use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRuntimeState {
    pub workflow_version_id: String,
    pub enabled: bool,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait WorkflowRuntimeStateRepository: Send + Sync {
    /// A missing row is intentionally treated as enabled for backward compatibility.
    async fn is_enabled(&self, workflow_version_id: &str) -> Result<bool, RepositoryError>;

    async fn set_enabled(
        &self,
        workflow_version_id: &str,
        enabled: bool,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;

    async fn list_states(&self) -> Result<Vec<WorkflowRuntimeState>, RepositoryError>;
}
