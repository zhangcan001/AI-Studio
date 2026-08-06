use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowPackageRecord {
    pub workflow_id: String,
    pub name: String,
    pub category: String,
    pub mode: String,
    pub workflow_version: String,
    pub workflow_json: Value,
    pub workflow_sha256: String,
    pub recipe_version: String,
    pub recipe_schema_version: u32,
    pub recipe_yaml: String,
    pub recipe_sha256: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowPackageRegistration {
    Inserted,
    Reused,
}

#[async_trait]
pub trait WorkflowLibraryRepository: Send + Sync {
    async fn register_package(
        &self,
        package: &WorkflowPackageRecord,
    ) -> Result<WorkflowPackageRegistration, RepositoryError>;
}
