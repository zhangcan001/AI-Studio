use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectWorkflowBindingRecord {
    pub project_id: String,
    pub stage: String,
    pub mode: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ProjectWorkflowBindingRepository: Send + Sync {
    async fn list_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError>;

    async fn replace_for_project(
        &self,
        project_id: &str,
        bindings: &[ProjectWorkflowBindingRecord],
    ) -> Result<(), RepositoryError>;

    async fn list_for_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError>;

    async fn clear_by_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<u64, RepositoryError>;
}
