use super::RepositoryError;
use crate::domain::{ReferenceAnchor, ReferenceAnchorAsset, ReferenceAnchorId};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceAnchorRecord {
    pub anchor: ReferenceAnchor,
    pub assets: Vec<ReferenceAnchorAsset>,
}

#[async_trait]
pub trait ReferenceAnchorRepository: Send + Sync {
    async fn list(&self, project_id: &str) -> Result<Vec<ReferenceAnchorRecord>, RepositoryError>;

    async fn find(
        &self,
        project_id: &str,
        anchor_id: &ReferenceAnchorId,
    ) -> Result<Option<ReferenceAnchorRecord>, RepositoryError>;

    async fn create_atomic(
        &self,
        anchor: &ReferenceAnchor,
        asset_ids: &[crate::domain::AssetId],
    ) -> Result<ReferenceAnchorRecord, RepositoryError>;

    async fn update_atomic(
        &self,
        anchor: &ReferenceAnchor,
        asset_ids: &[crate::domain::AssetId],
    ) -> Result<ReferenceAnchorRecord, RepositoryError>;

    async fn delete(
        &self,
        project_id: &str,
        anchor_id: &ReferenceAnchorId,
    ) -> Result<bool, RepositoryError>;
}
