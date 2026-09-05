use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

pub const RUNTIME_ARTIFACT_SOURCE_PRODUCT: &str = "PRODUCT";
pub const RUNTIME_ARTIFACT_SOURCE_USER: &str = "USER";
pub const RUNTIME_ARTIFACT_CONFLICT: &str = "RUNTIME_ARTIFACT_CONFLICT";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRuntimeArtifactRecord {
    pub id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub package_name: String,
    pub source_kind: String,
    pub package_source_path: Option<String>,
    pub workflow_sha256: String,
    pub recipe_sha256: String,
    pub created_at: DateTime<Utc>,
}

/// Exact runtime package provenance. A workflow-version/recipe pair has one
/// canonical runtime package; legacy package columns are not used to resolve
/// that pair.
#[async_trait]
pub trait WorkflowRuntimeArtifactRepository: Send + Sync {
    async fn find_exact(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        package_name: &str,
    ) -> Result<Option<WorkflowRuntimeArtifactRecord>, RepositoryError>;

    async fn list(&self) -> Result<Vec<WorkflowRuntimeArtifactRecord>, RepositoryError>;

    async fn list_for_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<Vec<WorkflowRuntimeArtifactRecord>, RepositoryError>;

    async fn list_for_recipe(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Vec<WorkflowRuntimeArtifactRecord>, RepositoryError>;

    async fn upsert(&self, artifact: &WorkflowRuntimeArtifactRecord)
        -> Result<(), RepositoryError>;
}
