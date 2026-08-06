use async_trait::async_trait;
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

    async fn read_runtime(
        &self,
        package_name: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError>;

    async fn list_staging_ids(&self) -> Result<Vec<String>, WorkflowPackageStoreError>;
}
