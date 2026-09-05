use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowPackageBytes {
    pub manifest_yaml: Vec<u8>,
    pub recipe_yaml: Vec<u8>,
    pub workflow_api_json: Vec<u8>,
}

impl WorkflowPackageBytes {
    pub fn new(manifest_yaml: Vec<u8>, recipe_yaml: Vec<u8>, workflow_api_json: Vec<u8>) -> Self {
        Self {
            manifest_yaml,
            recipe_yaml,
            workflow_api_json,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowPackageStoreError {
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowPackageQuarantineResult {
    Quarantined,
    AlreadyMissing,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WorkflowPurgeOperationRecord {
    pub schema_version: u32,
    pub operation_id: String,
    pub workflow_id: String,
    pub package_names: Vec<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowPurgeOperationEntry {
    Journal(WorkflowPurgeOperationRecord),
    Legacy {
        operation_id: String,
    },
    Malformed {
        operation_id: String,
        message: String,
    },
}

impl WorkflowPurgeOperationEntry {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::Journal(record) => &record.operation_id,
            Self::Legacy { operation_id } | Self::Malformed { operation_id, .. } => operation_id,
        }
    }
}

impl fmt::Display for WorkflowPackageStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for WorkflowPackageStoreError {}

#[async_trait]
pub trait WorkflowPackageStore: Send + Sync {
    async fn stage(
        &self,
        staging_id: &str,
        package: &WorkflowPackageBytes,
    ) -> Result<(), WorkflowPackageStoreError>;

    async fn read_staging(
        &self,
        staging_id: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError>;

    async fn publish_atomic(
        &self,
        staging_id: &str,
        package_name: &str,
    ) -> Result<(), WorkflowPackageStoreError>;

    async fn remove_staging(&self, staging_id: &str) -> Result<(), WorkflowPackageStoreError>;

    /// Only used to compensate a publication that failed registration in the same request.
    async fn remove_published(&self, package_name: &str) -> Result<(), WorkflowPackageStoreError>;

    /// Persist the operation record before any package directory is moved.
    async fn prepare_purge_operation(
        &self,
        operation: &WorkflowPurgeOperationRecord,
    ) -> Result<(), WorkflowPackageStoreError>;

    /// Enumerate both current journals and pre-journal quarantine directories.
    async fn list_purge_operations(
        &self,
    ) -> Result<Vec<WorkflowPurgeOperationEntry>, WorkflowPackageStoreError>;

    /// List the package directories that were actually moved for an operation.
    async fn list_quarantined_packages(
        &self,
        operation_id: &str,
    ) -> Result<Vec<String>, WorkflowPackageStoreError>;

    async fn read_quarantined(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError>;

    /// Move an installed runtime package into an operation-scoped quarantine.
    /// Implementations must use an atomic same-filesystem rename so a failed
    /// purge can restore the package without reconstructing its bytes.
    async fn quarantine_published(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<WorkflowPackageQuarantineResult, WorkflowPackageStoreError>;

    /// Restore one package from an operation-scoped quarantine.
    async fn restore_quarantined(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<(), WorkflowPackageStoreError>;

    /// Remove an operation-scoped quarantine after the database transaction
    /// has committed successfully.
    async fn remove_quarantine(&self, operation_id: &str) -> Result<(), WorkflowPackageStoreError>;

    /// List published package directory names. Internal quarantine directories
    /// are not part of the published package namespace.
    async fn list_published(&self) -> Result<Vec<String>, WorkflowPackageStoreError>;

    async fn read_runtime(
        &self,
        package_name: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError>;

    async fn list_staging_ids(&self) -> Result<Vec<String>, WorkflowPackageStoreError>;
}
