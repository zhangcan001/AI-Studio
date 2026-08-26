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
    /// Live semantic relations.  These are kept as IDs so the repository
    /// port remains independent from UI wording while the application layer
    /// can produce concrete, readable blocker messages.
    pub reference_set_ids: Vec<String>,
    pub reference_anchor_ids: Vec<String>,
    pub shot_reference_ids: Vec<String>,
    pub selected_by_shot_ids: Vec<String>,
    pub selected_image_by_shot_ids: Vec<String>,
    pub selected_video_by_shot_ids: Vec<String>,
}

#[async_trait]
pub trait AssetDeletionRepository: Send + Sync {
    async fn references_for(
        &self,
        project_id: &str,
        asset_ids: &[AssetId],
    ) -> Result<Vec<AssetDeletionReferences>, RepositoryError>;
}
