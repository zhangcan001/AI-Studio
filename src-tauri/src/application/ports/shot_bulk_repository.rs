use super::RepositoryError;
use crate::application::ports::{ShotRecord, ShotStageConfigRecord};
use crate::domain::ShotStage;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// The stage-owned prompt snapshot that belongs beside a shot stage config.
///
/// This is deliberately separate from `ShotRecord::prompt_text`.  The latter
/// is the legacy shot-level field and cannot represent different image and
/// video prompts. Migration 019 persists these snapshots in the dedicated
/// `shot_stage_prompts` table so prompts can exist before a workflow/Recipe is
/// configured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotStagePromptRecord {
    pub shot_id: String,
    pub stage: ShotStage,
    pub prompt_text: String,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShotBulkData {
    pub shot: ShotRecord,
    pub stage_configs: Vec<ShotStageConfigRecord>,
    pub stage_prompts: Vec<ShotStagePromptRecord>,
}

#[async_trait]
pub trait ShotBulkRepository: Send + Sync {
    /// Read the shot plus stage-owned prompt snapshots for validation and
    /// create-only duplicate checks.
    async fn list_bulk_data(&self, project_id: &str) -> Result<Vec<ShotBulkData>, RepositoryError>;

    async fn find_bulk_data(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<Option<ShotBulkData>, RepositoryError> {
        Ok(self
            .list_bulk_data(project_id)
            .await?
            .into_iter()
            .find(|data| data.shot.id == shot_id))
    }

    /// Insert all imported shots and their stage prompt snapshots in one
    /// database transaction.  Implementations must roll back the complete
    /// operation when any row or relation fails.
    async fn insert_shots_atomic(
        &self,
        project_id: &str,
        shots: &[ShotRecord],
        stage_prompts: &[ShotStagePromptRecord],
    ) -> Result<(), RepositoryError>;

    /// Update all stage prompt snapshots in one database transaction.
    async fn update_stage_prompts_atomic(
        &self,
        project_id: &str,
        updates: &[ShotStagePromptRecord],
    ) -> Result<(), RepositoryError>;

    /// Upsert all stage configs and any accompanying prompt snapshots in one
    /// database transaction.  The prompt slice is intentionally optional at
    /// the call site so a config-only operation cannot clear a prompt.
    async fn upsert_stage_configs_atomic(
        &self,
        project_id: &str,
        configs: &[ShotStageConfigRecord],
        prompt_updates: &[ShotStagePromptRecord],
    ) -> Result<(), RepositoryError>;
}
