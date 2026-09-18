use super::{map_repository_error, validate_project_id};
use crate::{
    app_state::AppState,
    application::{
        generation_input_preparer::GenerationInputValue,
        production_queue_service::{CreateDirectGenerationRequest, CreateProductionBatchItem},
        shot_service::{
            ShotGenerationRequest, ShotServiceError, ShotStageConfigRequest, ShotUpdateRequest,
            ShotView,
        },
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
    pub retry_task_id: Option<String>,
    #[serde(default)]
    pub submission_idempotency_key: Option<String>,
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
) -> Result<super::production_queue::ProductionBatchDetailView, AppError> {
    let stage = parse_stage(&request.stage)?;
    let values = into_values(request.values)?;
    let _admission = state
        .production_queue_service
        .acquire_interactive_admission()
        .await
        .map_err(super::production_queue::map_queue_error)?;
    let prepared = state
        .shot_service
        .prepare_generation_submission(ShotGenerationRequest {
            project_id: request.project_id,
            shot_id: request.shot_id,
            stage,
            values,
            retry_task_id: request.retry_task_id,
            submission_idempotency_key: request.submission_idempotency_key,
        })
        .await
        .map_err(map_shot_error)?;
    state
        .production_queue_service
        .create_direct_generation(CreateDirectGenerationRequest {
            project_id: prepared.project_id,
            name: "Shot generation".to_owned(),
            continue_on_failure: true,
            item: CreateProductionBatchItem {
                workflow_version_id: prepared.workflow_version_id,
                recipe_id: prepared.recipe_id,
                values: prepared.values,
            },
            shot_id: Some(prepared.shot_id),
            stage: Some(prepared.stage.as_str().to_owned()),
            prompt_version_id: None,
            model_version_id: None,
            tool_instance_id: None,
            tool_version_id: None,
            submission_idempotency_key: prepared.submission_idempotency_key,
            parent_task_id: prepared.parent_task_id,
        })
        .await
        .map(Into::into)
        .map_err(super::production_queue::map_queue_error)
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
        ShotServiceError::TaskView(message) => AppError::internal(message),
    }
}
