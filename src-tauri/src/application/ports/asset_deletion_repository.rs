use super::RepositoryError;
use crate::domain::{AssetId, TaskId};
use async_trait::async_trait;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetDeletionReferences {
    pub asset_id: AssetId,
    pub active_production_item_ids: Vec<String>,
    pub active_task_ids: Vec<TaskId>,
    pub historical_task_ids: Vec<TaskId>,
    /// Immutable execution inputs, independent of Task status.
    pub snapshot_input_task_ids: Vec<TaskId>,
    /// Terminal source outputs are warnings, not immutable input blockers.
    pub historical_source_task_ids: Vec<TaskId>,
    pub historical_production_output_ids: Vec<String>,
    pub historical_review_ids: Vec<String>,
    /// Actual audit facts must survive schema-038 cascade deletion.
    pub meaningful_artifact_review_ids: Vec<String>,
    /// Automatic pending rows contain no human review fact.
    pub placeholder_artifact_review_ids: Vec<String>,
    /// Immutable provenance edges that point at one of this asset's versions.
    /// These are hard blockers: removing the asset would destroy lineage.
    pub generation_asset_version_ids: Vec<String>,
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

/// Shared application policy, including whitespace-only comments. Unknown states fail closed.
pub fn artifact_review_is_placeholder(decision: &str, revision: i64, comment: &str) -> bool {
    decision == "PENDING" && revision == 0 && comment.trim().is_empty()
}
