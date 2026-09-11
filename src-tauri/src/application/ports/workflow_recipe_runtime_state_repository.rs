use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRecipeRuntimeState {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait WorkflowRecipeRuntimeStateRepository: Send + Sync {
    /// Missing rows intentionally mean that the immutable recipe is active.
    async fn find_state(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Option<WorkflowRecipeRuntimeState>, RepositoryError>;

    async fn set_archived(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        archived: bool,
        archived_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;

    async fn list_states(&self) -> Result<Vec<WorkflowRecipeRuntimeState>, RepositoryError>;
}
