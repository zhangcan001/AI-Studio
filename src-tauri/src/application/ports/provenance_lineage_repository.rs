use super::RepositoryError;
use crate::domain::{GenerationAssetVersion, GenerationToolUsage, TaskId};
use async_trait::async_trait;

/// Persistence boundary for explicit edges around the existing Task and
/// result authorities. This port does not own Task, GenerationSnapshot,
/// Result, Asset, or Tool lifecycle.
#[async_trait]
pub trait ProvenanceLineageRepository: Send + Sync {
    async fn insert_tool_usage(&self, usage: &GenerationToolUsage) -> Result<(), RepositoryError>;

    async fn list_tool_usages(
        &self,
        project_id: &str,
        generation_id: &TaskId,
    ) -> Result<Vec<GenerationToolUsage>, RepositoryError>;

    async fn insert_asset_version_link(
        &self,
        link: &GenerationAssetVersion,
    ) -> Result<(), RepositoryError>;

    async fn list_asset_version_links(
        &self,
        project_id: &str,
        generation_id: &TaskId,
    ) -> Result<Vec<GenerationAssetVersion>, RepositoryError>;
}
