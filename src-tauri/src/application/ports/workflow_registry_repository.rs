use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

pub const WORKFLOW_SOURCE_PRODUCT: &str = "PRODUCT";
pub const WORKFLOW_SOURCE_USER: &str = "USER";
pub const WORKFLOW_STATE_ACTIVE: &str = "ACTIVE";
pub const WORKFLOW_STATE_REMOVED: &str = "REMOVED";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRegistryRecord {
    pub id: String,
    pub name: String,
    pub category: String,
    pub mode: String,
    pub source_kind: String,
    pub library_state: String,
    pub current_version_id: Option<String>,
    pub removed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Persistence boundary for the logical Workflow entity.
///
/// Versions, recipes, and runtime artifacts retain their own immutable IDs;
/// this port only owns the logical workflow metadata and lifecycle state.
#[async_trait]
pub trait WorkflowRegistryRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<WorkflowRegistryRecord>, RepositoryError>;

    async fn get(
        &self,
        workflow_id: &str,
    ) -> Result<Option<WorkflowRegistryRecord>, RepositoryError>;

    async fn rename(
        &self,
        workflow_id: &str,
        name: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowRegistryRecord>, RepositoryError>;

    async fn set_current_version(
        &self,
        workflow_id: &str,
        workflow_version_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;

    async fn remove(
        &self,
        workflow_id: &str,
        removed_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowRegistryRecord>, RepositoryError>;

    async fn restore(
        &self,
        workflow_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowRegistryRecord>, RepositoryError>;

    async fn purge(&self, workflow_id: &str) -> Result<bool, RepositoryError>;
}
