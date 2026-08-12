use super::RepositoryError;
use crate::domain::{AssetId, TaskId};
use async_trait::async_trait;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetDeletionReferences {
    pub asset_id: AssetId,
    pub active_production_item_ids: Vec<String>,
    pub active_task_ids: Vec<TaskId>,
    pub historical_task_ids: Vec<TaskId>,
    pub historical_review_ids: Vec<String>,
}

#[async_trait]
pub trait AssetDeletionRepository: Send + Sync {
    async fn references_for(
        &self,
        project_id: &str,
        asset_ids: &[AssetId],
    ) -> Result<Vec<AssetDeletionReferences>, RepositoryError>;
}
