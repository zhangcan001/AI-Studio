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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowPurgeReferenceCounts {
    pub task_count: u64,
    pub batch_item_count: u64,
    pub preset_count: u64,
    pub template_count: u64,
    pub shot_config_count: u64,
    pub benchmark_count: u64,
    pub binding_count: u64,
    pub stage_count: u64,
    pub run_template_count: u64,
}

impl WorkflowPurgeReferenceCounts {
    pub fn total(&self) -> u64 {
        self.task_count
            + self.batch_item_count
            + self.preset_count
            + self.template_count
            + self.shot_config_count
            + self.benchmark_count
            + self.binding_count
            + self.stage_count
            + self.run_template_count
    }
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

    async fn inspect_purge(
        &self,
        workflow_id: &str,
    ) -> Result<Option<WorkflowPurgeReferenceCounts>, RepositoryError>;

    async fn purge(&self, workflow_id: &str) -> Result<bool, RepositoryError>;
}

/// W-03 repair input: one workflow version of an ACTIVE workflow together
/// with its raw runtime state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryVersionStateRecord {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub workflow_version: String,
    pub is_current: bool,
    pub archived: bool,
    pub enabled: bool,
    /// Raw stored `archived_at` text so timestamps compare exactly.
    pub archived_at: Option<String>,
}

/// Narrow repair port for `registry_stranded_versions_v1`.
#[async_trait]
pub trait RegistryRepairRepository: Send + Sync {
    /// Every version of every ACTIVE workflow with its runtime state.
    async fn list_active_version_states(
        &self,
    ) -> Result<Vec<RegistryVersionStateRecord>, RepositoryError>;

    /// Un-archive and re-enable exactly the given versions (only rows that
    /// are still archived=1 AND enabled=0). Returns the updated row count.
    async fn unarchive_versions(
        &self,
        workflow_version_ids: &[String],
        updated_at: DateTime<Utc>,
    ) -> Result<u64, RepositoryError>;
}
