use super::{map_repository_error, validate_project_id};
use crate::{
    app_state::AppState,
    application::{
        generation_input_preparer::GenerationInputValue,
        shot_service::{
            ShotGenerationRequest, ShotServiceError, ShotStageConfigRequest, ShotUpdateRequest,
            ShotView,
        },
        task_query_service::TaskView,
    },
    domain::ShotStage,
    error::AppError,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotUpdateRequestDto {
    pub project_id: String,
    pub shot_id: String,
    pub name: String,
    pub prompt_text: String,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotReorderRequestDto {
    pub project_id: String,
    pub ordered_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotStageConfigRequestDto {
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, super::generation::InputValueDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotReferencesReplaceRequestDto {
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    pub asset_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotResultSelectRequestDto {
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    pub asset_id: String,
    #[serde(default)]
    pub from_linked_task: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotGenerateRequestDto {
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    #[serde(default)]
    pub values: BTreeMap<String, super::generation::InputValueDto>,
    pub production_batch_item_id: Option<String>,
    pub retry_task_id: Option<String>,
}

#[tauri::command]
pub async fn shot_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ShotView>, AppError> {
    validate_project_id(&project_id)?;
    state
        .shot_service
        .list(&project_id)
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_get(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: String,
) -> Result<ShotView, AppError> {
    validate_project_id(&project_id)?;
    state
        .shot_service
        .get(&project_id, &shot_id)
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_create(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<ShotView, AppError> {
    validate_project_id(&project_id)?;
    state
        .shot_service
        .create(&project_id)
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_update(
    state: State<'_, AppState>,
    request: ShotUpdateRequestDto,
) -> Result<ShotView, AppError> {
    state
        .shot_service
        .update(ShotUpdateRequest {
            project_id: request.project_id,
            shot_id: request.shot_id,
            name: request.name,
            prompt_text: request.prompt_text,
            prompt_entry_id: request.prompt_entry_id,
            prompt_version_id: request.prompt_version_id,
        })
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_delete(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: String,
) -> Result<(), AppError> {
    validate_project_id(&project_id)?;
    state
        .shot_service
        .delete(&project_id, &shot_id)
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_reorder(
    state: State<'_, AppState>,
    request: ShotReorderRequestDto,
) -> Result<Vec<ShotView>, AppError> {
    state
        .shot_service
        .reorder(&request.project_id, request.ordered_ids)
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_stage_config_set(
    state: State<'_, AppState>,
    request: ShotStageConfigRequestDto,
) -> Result<ShotView, AppError> {
    let stage = parse_stage(&request.stage)?;
    let values = into_values(request.values)?;
    state
        .shot_service
        .set_stage_config(ShotStageConfigRequest {
            project_id: request.project_id,
            shot_id: request.shot_id,
            stage,
            workflow_version_id: request.workflow_version_id,
            recipe_id: request.recipe_id,
            values,
        })
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_references_replace(
    state: State<'_, AppState>,
    request: ShotReferencesReplaceRequestDto,
) -> Result<ShotView, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .shot_service
        .replace_references(
            &request.project_id,
            &request.shot_id,
            stage,
            request.asset_ids,
        )
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_result_select(
    state: State<'_, AppState>,
    request: ShotResultSelectRequestDto,
) -> Result<ShotView, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .shot_service
        .select_result(
            &request.project_id,
            &request.shot_id,
            stage,
            &request.asset_id,
            request.from_linked_task,
        )
        .await
        .map_err(map_shot_error)
}

#[tauri::command]
pub async fn shot_generate(
    state: State<'_, AppState>,
    request: ShotGenerateRequestDto,
) -> Result<TaskView, AppError> {
    let stage = parse_stage(&request.stage)?;
    let values = into_values(request.values)?;
    let _admission = state
        .production_queue_service
        .acquire_interactive_admission()
        .await
        .map_err(super::production_queue::map_queue_error)?;
    state
        .shot_service
        .generate(ShotGenerationRequest {
            project_id: request.project_id,
            shot_id: request.shot_id,
            stage,
            values,
            production_batch_item_id: request.production_batch_item_id,
            retry_task_id: request.retry_task_id,
        })
        .await
        .map_err(map_shot_error)
}

fn parse_stage(value: &str) -> Result<ShotStage, AppError> {
    ShotStage::try_from_str(value).map_err(|error| AppError::invalid_input(error.to_string()))
}

fn into_values(
    values: BTreeMap<String, super::generation::InputValueDto>,
) -> Result<BTreeMap<String, GenerationInputValue>, AppError> {
    values
        .into_iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                super::generation::input_value_into_application(value, &key)?,
            ))
        })
        .collect()
}

fn map_shot_error(error: ShotServiceError) -> AppError {
    match error {
        ShotServiceError::InvalidInput(message) => AppError::invalid_input(message),
        ShotServiceError::NotFound(id) => AppError::database(format!("shot {id} was not found")),
        ShotServiceError::Repository(error) => map_repository_error(&error),
        ShotServiceError::Generation(error) => super::generation::map_generation_error(error),
        ShotServiceError::TaskView(message) => AppError::internal(message),
    }
}
