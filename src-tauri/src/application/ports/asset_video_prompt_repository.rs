use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::RepositoryError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetVideoPromptRecord {
    pub asset_id: String,
    pub project_id: String,
    pub prompt_text: String,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait AssetVideoPromptRepository: Send + Sync {
    async fn find(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<Option<AssetVideoPromptRecord>, RepositoryError>;

    async fn list(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<Vec<AssetVideoPromptRecord>, RepositoryError>;

    async fn upsert(&self, record: &AssetVideoPromptRecord) -> Result<(), RepositoryError>;
}
