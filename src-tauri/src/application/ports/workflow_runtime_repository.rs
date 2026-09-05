use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeRecipeRecord {
    pub recipe_id: String,
    pub version: String,
    pub schema_version: u32,
    pub recipe_yaml: String,
    pub recipe_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeWorkflowVersionRecord {
    pub workflow_version_id: String,
    pub workflow_id: String,
    pub name: String,
    pub category: String,
    pub mode: String,
    pub workflow_version: String,
    pub workflow_sha256: String,
    /// The immutable API graph snapshot stored with the version. This is the
    /// Registry identity source; package files are only runtime artifacts.
    pub api_workflow_json: String,
    pub package_name: Option<String>,
    pub is_current: bool,
    pub recipes: Vec<RuntimeRecipeRecord>,
    pub active_tasks: u64,
    pub total_tasks: u64,
    pub has_successful_run: bool,
    pub latest_success_at: Option<String>,
    pub latest_failure_at: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowDeletionCounts {
    pub active_task_count: u64,
    pub active_queue_item_count: u64,
    pub historical_task_count: u64,
    pub production_batch_item_count: u64,
    pub other_reference_count: u64,
    pub benchmark_reference_count: u64,
}

#[async_trait]
pub trait WorkflowRuntimeRepository: Send + Sync {
    async fn list_versions(&self) -> Result<Vec<RuntimeWorkflowVersionRecord>, RepositoryError>;

    async fn find_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<Option<RuntimeWorkflowVersionRecord>, RepositoryError>;

    async fn inspect_deletion(
        &self,
        workflow_version_id: &str,
    ) -> Result<Option<WorkflowDeletionCounts>, RepositoryError>;

    async fn delete_version(
        &self,
        workflow_version_id: &str,
        workflow_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;
}
