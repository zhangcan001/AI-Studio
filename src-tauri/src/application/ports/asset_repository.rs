use super::RepositoryError;
use crate::domain::{Asset, AssetId, TaskId};
use async_trait::async_trait;

#[async_trait]
pub trait AssetRepository: Send + Sync {
    async fn insert_many(&self, assets: &[Asset]) -> Result<(), RepositoryError>;

    async fn find_by_id(&self, asset_id: &AssetId) -> Result<Option<Asset>, RepositoryError>;

    async fn list_by_source_task(&self, task_id: &TaskId) -> Result<Vec<Asset>, RepositoryError>;

    async fn list_recent(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<Asset>, RepositoryError>;
}
