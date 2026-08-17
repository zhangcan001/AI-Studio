use super::validate_project_id;
use crate::{
    app_state::AppState,
    application::{
        generation_input_preparer::GenerationInputValue,
        shot_bulk_service::{
            BulkPromptAssignmentRequest, BulkPromptSource, BulkStageConfigRequest,
            ShotBulkImportRequest, ShotBulkInputFormat, ShotBulkServiceError,
        },
    },
    domain::ShotStage,
    error::AppError,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotBulkImportRequestDto {
    pub project_id: String,
    pub format: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BulkPromptSourceDto {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "promptLibraryVersion")]
    PromptLibraryVersion {
        prompt_entry_id: String,
        prompt_version_id: String,
    },
    #[serde(rename = "clearProvenance")]
    ClearProvenance,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkPromptAssignmentRequestDto {
    pub project_id: String,
    pub stage: String,
    pub shot_ids: Vec<String>,
    pub source: BulkPromptSourceDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkStageConfigRequestDto {
    pub project_id: String,
    pub stage: String,
    pub shot_ids: Vec<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, super::generation::InputValueDto>,
    pub prompt: Option<BulkPromptSourceDto>,
}

#[tauri::command]
pub async fn preview_shot_bulk_import(
    state: State<'_, AppState>,
    request: ShotBulkImportRequestDto,
) -> Result<crate::application::shot_bulk_service::ShotBulkImportPreview, AppError> {
    validate_project_id(&request.project_id)?;
    state
        .shot_bulk_service
        .preview_import(&into_import_request(request)?)
        .await
        .map_err(map_bulk_error)
}

#[tauri::command]
pub async fn commit_shot_bulk_import(
    state: State<'_, AppState>,
    request: ShotBulkImportRequestDto,
) -> Result<crate::application::shot_bulk_service::ShotBulkImportResult, AppError> {
    validate_project_id(&request.project_id)?;
    state
        .shot_bulk_service
        .commit_import(&into_import_request(request)?)
        .await
        .map_err(map_bulk_error)
}

#[tauri::command]
pub async fn bulk_assign_shot_prompt(
    state: State<'_, AppState>,
    request: BulkPromptAssignmentRequestDto,
) -> Result<crate::application::shot_bulk_service::BulkAssignmentResult, AppError> {
    validate_project_id(&request.project_id)?;
    let stage = parse_stage(&request.stage)?;
    state
        .shot_bulk_service
        .assign_prompt(BulkPromptAssignmentRequest {
            project_id: request.project_id,
            stage,
            shot_ids: request.shot_ids,
            source: into_prompt_source(request.source),
        })
        .await
        .map_err(map_bulk_error)
}

#[tauri::command]
pub async fn bulk_set_shot_stage_config(
    state: State<'_, AppState>,
    request: BulkStageConfigRequestDto,
) -> Result<crate::application::shot_bulk_service::BulkStageConfigResult, AppError> {
    validate_project_id(&request.project_id)?;
    let stage = parse_stage(&request.stage)?;
    let values = request
        .values
        .into_iter()
        .map(|(key, value)| Ok((key.clone(), value.into_application(&key)?)))
        .collect::<Result<BTreeMap<String, GenerationInputValue>, AppError>>()?;
    state
        .shot_bulk_service
        .set_stage_config(BulkStageConfigRequest {
            project_id: request.project_id,
            stage,
            shot_ids: request.shot_ids,
            workflow_version_id: request.workflow_version_id,
            recipe_id: request.recipe_id,
            values,
            prompt: request.prompt.map(into_prompt_source),
        })
        .await
        .map_err(map_bulk_error)
}

fn into_import_request(
    request: ShotBulkImportRequestDto,
) -> Result<ShotBulkImportRequest, AppError> {
    let format = match request.format.trim().to_ascii_lowercase().as_str() {
        "json" => ShotBulkInputFormat::Json,
        "tsv" => ShotBulkInputFormat::Tsv,
        other => {
            return Err(AppError::invalid_input(format!(
                "BULK_IMPORT_INVALID_FORMAT: unsupported format {other}"
            )))
        }
    };
    Ok(ShotBulkImportRequest {
        project_id: request.project_id,
        format,
        contents: request.content,
    })
}

fn into_prompt_source(source: BulkPromptSourceDto) -> BulkPromptSource {
    match source {
        BulkPromptSourceDto::Text { text } => BulkPromptSource::Text(text),
        BulkPromptSourceDto::PromptLibraryVersion {
            prompt_entry_id,
            prompt_version_id,
        } => BulkPromptSource::PromptLibraryVersion {
            prompt_entry_id,
            prompt_version_id,
        },
        BulkPromptSourceDto::ClearProvenance => BulkPromptSource::ClearProvenance,
    }
}

fn parse_stage(value: &str) -> Result<ShotStage, AppError> {
    ShotStage::try_from_str(value).map_err(|error| AppError::invalid_input(error.to_string()))
}

fn map_bulk_error(error: ShotBulkServiceError) -> AppError {
    let code = error.code().to_owned();
    let issues = error.issues();
    let mut app_error = AppError::invalid_input(error.to_string());
    app_error.details = Some(json!({ "code": code, "issues": issues }));
    app_error
}
