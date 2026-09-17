use crate::domain::{Artifact, ArtifactReview, AssetId, TaskId};
use async_trait::async_trait;

use super::RepositoryError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactReviewQueueFilter {
    #[default]
    Pending,
    Completed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArtifactRecord {
    pub artifact: Artifact,
    pub review: Option<ArtifactReview>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArtifactReviewQueuePage {
    pub items: Vec<ArtifactRecord>,
    pub total: usize,
}

#[async_trait]
pub trait ArtifactRepository: Send + Sync {
    async fn list_for_tasks(
        &self,
        project_id: &str,
        task_ids: &[TaskId],
    ) -> Result<Vec<ArtifactRecord>, RepositoryError>;

    async fn find_artifact(
        &self,
        project_id: &str,
        artifact_id: &AssetId,
    ) -> Result<Option<Artifact>, RepositoryError>;

    async fn find_artifact_by_id(
        &self,
        artifact_id: &AssetId,
    ) -> Result<Option<Artifact>, RepositoryError>;

    async fn list_review_queue(
        &self,
        project_id: &str,
        filter: ArtifactReviewQueueFilter,
        limit: usize,
        offset: usize,
    ) -> Result<ArtifactReviewQueuePage, RepositoryError>;

    async fn find_review(
        &self,
        project_id: &str,
        artifact_id: &AssetId,
    ) -> Result<Option<ArtifactReview>, RepositoryError>;

    /// Compare-and-swap update. `false` means the persisted revision changed.
    async fn update_review_if_revision(
        &self,
        review: &ArtifactReview,
        expected_revision: i64,
    ) -> Result<bool, RepositoryError>;
}
