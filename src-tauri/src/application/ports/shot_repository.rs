use super::{shot_bulk_repository::ShotStagePromptRecord, RepositoryError};
use crate::domain::ShotStage;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotRecord {
    pub id: String,
    pub project_id: String,
    pub ordinal: i64,
    pub name: String,
    pub prompt_text: String,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
    pub selected_image_asset_id: Option<String>,
    pub selected_video_asset_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShotStageConfigRecord {
    pub shot_id: String,
    pub stage: ShotStage,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub scalar_values: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotReferenceAssetRecord {
    pub shot_id: String,
    pub stage: ShotStage,
    pub asset_id: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotGenerationLinkRecord {
    pub id: String,
    pub shot_id: String,
    pub stage: ShotStage,
    pub task_id: Option<String>,
    pub production_batch_item_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShotData {
    pub shot: ShotRecord,
    pub stage_configs: Vec<ShotStageConfigRecord>,
    pub stage_prompts: Vec<ShotStagePromptRecord>,
    pub reference_assets: Vec<ShotReferenceAssetRecord>,
    pub generation_links: Vec<ShotGenerationLinkRecord>,
}

#[async_trait]
pub trait ShotRepository: Send + Sync {
    async fn list(&self, project_id: &str) -> Result<Vec<ShotData>, RepositoryError>;

    async fn find(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<Option<ShotData>, RepositoryError>;

    async fn insert(&self, shot: &ShotRecord) -> Result<(), RepositoryError>;

    async fn update(&self, shot: &ShotRecord) -> Result<bool, RepositoryError>;

    async fn delete(&self, project_id: &str, shot_id: &str) -> Result<bool, RepositoryError>;

    async fn reorder(
        &self,
        project_id: &str,
        ordered_ids: &[String],
        updated_at: DateTime<Utc>,
    ) -> Result<Vec<ShotRecord>, RepositoryError>;

    async fn upsert_stage_config(
        &self,
        project_id: &str,
        config: &ShotStageConfigRecord,
    ) -> Result<(), RepositoryError>;

    async fn replace_reference_assets(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
        asset_ids: &[String],
    ) -> Result<(), RepositoryError>;

    async fn select_image(
        &self,
        project_id: &str,
        shot_id: &str,
        asset_id: &str,
    ) -> Result<(), RepositoryError>;

    async fn select_video(
        &self,
        project_id: &str,
        shot_id: &str,
        asset_id: &str,
    ) -> Result<(), RepositoryError>;

    async fn link_generation(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
        task_id: &str,
        production_batch_item_id: Option<&str>,
        created_at: DateTime<Utc>,
    ) -> Result<ShotGenerationLinkRecord, RepositoryError>;
}
