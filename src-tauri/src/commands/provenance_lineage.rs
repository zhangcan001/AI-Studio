use crate::app_state::AppState;
use crate::application::provenance_lineage_service::{
    CreateGenerationAssetVersionRequest, CreateGenerationToolUsageRequest,
    GenerationAssetVersionView, GenerationToolUsageView, ProvenanceLineageError,
};
use crate::error::AppError;
use serde::Deserialize;
use serde_json::Value;
use tauri::State;

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationToolUsageCreateRequest {
    pub project_id: String,
    pub generation_id: String,
    pub tool_instance_id: String,
    #[serde(default)]
    pub tool_version_id: Option<String>,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationAssetVersionLinkCreateRequest {
    pub project_id: String,
    pub generation_id: String,
    pub output_id: String,
    pub ordinal: u32,
    pub asset_version_id: String,
    pub relation_type: String,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn generation_tool_usage_create(
    state: State<'_, AppState>,
    request: GenerationToolUsageCreateRequest,
) -> Result<GenerationToolUsageView, AppError> {
    state
        .tasks
        .provenance_lineage
        .create_tool_usage(CreateGenerationToolUsageRequest {
            project_id: request.project_id,
            generation_id: request.generation_id,
            tool_instance_id: request.tool_instance_id,
            tool_version_id: request.tool_version_id,
            metadata: request.metadata,
        })
        .await
        .map_err(map_provenance_lineage_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn generation_tool_usage_list(
    state: State<'_, AppState>,
    project_id: String,
    generation_id: String,
) -> Result<Vec<GenerationToolUsageView>, AppError> {
    state
        .tasks
        .provenance_lineage
        .list_tool_usages(&project_id, &generation_id)
        .await
        .map_err(map_provenance_lineage_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn generation_asset_version_link_create(
    state: State<'_, AppState>,
    request: GenerationAssetVersionLinkCreateRequest,
) -> Result<GenerationAssetVersionView, AppError> {
    state
        .tasks
        .provenance_lineage
        .create_asset_version_link(CreateGenerationAssetVersionRequest {
            project_id: request.project_id,
            generation_id: request.generation_id,
            output_id: request.output_id,
            ordinal: request.ordinal,
            asset_version_id: request.asset_version_id,
            relation_type: request.relation_type,
        })
        .await
        .map_err(map_provenance_lineage_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn generation_asset_version_link_list(
    state: State<'_, AppState>,
    project_id: String,
    generation_id: String,
) -> Result<Vec<GenerationAssetVersionView>, AppError> {
    state
        .tasks
        .provenance_lineage
        .list_asset_version_links(&project_id, &generation_id)
        .await
        .map_err(map_provenance_lineage_error)
}

fn map_provenance_lineage_error(error: ProvenanceLineageError) -> AppError {
    match error {
        ProvenanceLineageError::InvalidInput(_) | ProvenanceLineageError::NotFound(_) => {
            AppError::invalid_input(error.to_string())
        }
        ProvenanceLineageError::Repository(repository) => super::map_repository_error(&repository),
    }
}
