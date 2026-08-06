use super::RepositoryError;
use async_trait::async_trait;

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
    pub is_current: bool,
    pub recipes: Vec<RuntimeRecipeRecord>,
    pub active_tasks: u64,
    pub total_tasks: u64,
    pub has_successful_run: bool,
    pub latest_success_at: Option<String>,
    pub latest_failure_at: Option<String>,
}

#[async_trait]
pub trait WorkflowRuntimeRepository: Send + Sync {
    async fn list_versions(&self) -> Result<Vec<RuntimeWorkflowVersionRecord>, RepositoryError>;

    async fn find_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<Option<RuntimeWorkflowVersionRecord>, RepositoryError>;
}
