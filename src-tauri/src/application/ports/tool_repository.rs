use super::RepositoryError;
use crate::domain::{
    Capability, Tool, ToolId, ToolInstance, ToolInstanceId, ToolVersion, ToolVersionId,
};
use async_trait::async_trait;

#[async_trait]
pub trait ToolRepository: Send + Sync {
    async fn list_tools(&self) -> Result<Vec<Tool>, RepositoryError>;

    async fn find_tool(&self, tool_id: &ToolId) -> Result<Option<Tool>, RepositoryError>;

    async fn create_tool(&self, tool: &Tool) -> Result<(), RepositoryError>;

    async fn update_tool(&self, tool: &Tool) -> Result<Option<Tool>, RepositoryError>;

    async fn delete_tool(&self, tool_id: &ToolId) -> Result<bool, RepositoryError>;

    async fn list_instances(&self, tool_id: &ToolId) -> Result<Vec<ToolInstance>, RepositoryError>;

    async fn find_instance(
        &self,
        instance_id: &ToolInstanceId,
    ) -> Result<Option<ToolInstance>, RepositoryError>;

    async fn create_instance(&self, instance: &ToolInstance) -> Result<(), RepositoryError>;

    async fn update_instance(
        &self,
        instance: &ToolInstance,
    ) -> Result<Option<ToolInstance>, RepositoryError>;

    async fn list_versions(&self, tool_id: &ToolId) -> Result<Vec<ToolVersion>, RepositoryError>;

    async fn find_version(
        &self,
        version_id: &ToolVersionId,
    ) -> Result<Option<ToolVersion>, RepositoryError>;

    async fn create_version(&self, version: &ToolVersion) -> Result<(), RepositoryError>;

    async fn list_capabilities(&self, tool_id: &ToolId)
        -> Result<Vec<Capability>, RepositoryError>;

    async fn create_capability(&self, capability: &Capability) -> Result<(), RepositoryError>;
}
