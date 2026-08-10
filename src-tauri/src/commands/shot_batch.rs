use super::{
    map_repository_error, production_queue::ProductionBatchDetailView, validate_project_id,
};
use crate::{
    app_state::AppState,
    application::shot_batch_service::{
        CreateShotBatchRequest, ShotBatchPlanView, ShotBatchServiceError,
    },
    domain::ShotStage,
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotBatchCreateRequestDto {
    pub project_id: String,
    pub stage: String,
    pub shot_ids: Vec<String>,
}

#[tauri::command]
pub async fn shot_batch_plan(
    state: State<'_, AppState>,
    project_id: String,
    stage: String,
) -> Result<ShotBatchPlanView, AppError> {
    validate_project_id(&project_id)?;
    let stage = parse_stage(&stage)?;
    state
        .shot_batch_service
        .plan(&project_id, stage)
        .await
        .map_err(map_shot_batch_error)
}

#[tauri::command]
pub async fn shot_batch_create(
    state: State<'_, AppState>,
    request: ShotBatchCreateRequestDto,
) -> Result<ProductionBatchDetailView, AppError> {
    validate_project_id(&request.project_id)?;
    let stage = parse_stage(&request.stage)?;
    state
        .shot_batch_service
        .create(CreateShotBatchRequest {
            project_id: request.project_id,
            stage,
            shot_ids: request.shot_ids,
        })
        .await
        .map(Into::into)
        .map_err(map_shot_batch_error)
}

fn parse_stage(value: &str) -> Result<ShotStage, AppError> {
    ShotStage::try_from_str(value).map_err(|error| AppError::invalid_input(error.to_string()))
}

fn map_shot_batch_error(error: ShotBatchServiceError) -> AppError {
    match error {
        ShotBatchServiceError::InvalidInput(message) => AppError::invalid_input(message),
        ShotBatchServiceError::NotFound(id) => {
            AppError::database(format!("Shot batch project or record {id} was not found"))
        }
        ShotBatchServiceError::Repository(error) => map_repository_error(&error),
    }
}
