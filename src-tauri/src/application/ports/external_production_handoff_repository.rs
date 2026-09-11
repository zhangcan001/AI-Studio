use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::RepositoryError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffRecord {
    pub id: String,
    pub project_id: String,
    pub schema_version: u32,
    pub source_agent: String,
    pub source_revision: Option<String>,
    pub document_sha256: String,
    pub imported_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffEntityMapping {
    pub handoff_id: String,
    pub entity_kind: String,
    pub external_id: String,
    pub formal_entity_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffSeries {
    pub id: String,
    pub project_id: String,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffEpisode {
    pub id: String,
    pub series_id: String,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffScene {
    pub id: String,
    pub episode_id: String,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffShot {
    pub id: String,
    pub project_id: String,
    pub ordinal: i64,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffPrompt {
    pub shot_id: String,
    pub stage: String,
    pub prompt_text: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalProductionHandoffStageConfig {
    pub shot_id: String,
    pub stage: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub scalar_values: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffAssetReference {
    pub shot_id: String,
    pub stage: String,
    pub asset_id: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffSceneAssignment {
    pub shot_id: String,
    pub scene_id: String,
    pub ordinal: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalProductionHandoffImportPlan {
    pub handoff: ExternalProductionHandoffRecord,
    pub series: Vec<ExternalProductionHandoffSeries>,
    pub episodes: Vec<ExternalProductionHandoffEpisode>,
    pub scenes: Vec<ExternalProductionHandoffScene>,
    pub shots: Vec<ExternalProductionHandoffShot>,
    pub prompts: Vec<ExternalProductionHandoffPrompt>,
    pub stage_configs: Vec<ExternalProductionHandoffStageConfig>,
    pub asset_references: Vec<ExternalProductionHandoffAssetReference>,
    pub assignments: Vec<ExternalProductionHandoffSceneAssignment>,
    pub mappings: Vec<ExternalProductionHandoffEntityMapping>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProductionHandoffImportResult {
    pub handoff: ExternalProductionHandoffRecord,
    pub mappings: Vec<ExternalProductionHandoffEntityMapping>,
    pub replayed: bool,
}

#[async_trait]
pub trait ExternalProductionHandoffRepository: Send + Sync {
    async fn find_by_document_hash(
        &self,
        project_id: &str,
        document_sha256: &str,
    ) -> Result<Option<ExternalProductionHandoffImportResult>, RepositoryError>;

    async fn find_by_source_revision(
        &self,
        project_id: &str,
        source_agent: &str,
        source_revision: &str,
    ) -> Result<Option<ExternalProductionHandoffRecord>, RepositoryError>;

    async fn list(
        &self,
        project_id: &str,
    ) -> Result<Vec<ExternalProductionHandoffRecord>, RepositoryError>;

    async fn mappings(
        &self,
        project_id: &str,
        handoff_id: &str,
    ) -> Result<Vec<ExternalProductionHandoffEntityMapping>, RepositoryError>;

    async fn import_atomic(
        &self,
        plan: &ExternalProductionHandoffImportPlan,
    ) -> Result<ExternalProductionHandoffImportResult, RepositoryError>;
}
