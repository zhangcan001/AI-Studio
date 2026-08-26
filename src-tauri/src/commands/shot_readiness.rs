use crate::{
    app_state::AppState,
    application::shot_readiness_service::{SceneReadinessSummary, ShotReadinessServiceError},
    domain::{shot::ShotStage, shot_readiness::ShotReadiness},
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotReadinessRequest {
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneReadinessRequest {
    pub project_id: String,
    pub scene_id: String,
    pub stage: String,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn shot_readiness_cached(
    state: State<'_, AppState>,
    request: ShotReadinessRequest,
) -> Result<ShotReadiness, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .shot_readiness_service
        .readiness_cached(&request.project_id, &request.shot_id, stage)
        .await
        .map_err(map_readiness_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn shot_preflight(
    state: State<'_, AppState>,
    request: ShotReadinessRequest,
) -> Result<ShotReadiness, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .shot_readiness_service
        .preflight(&request.project_id, &request.shot_id, stage)
        .await
        .map_err(map_readiness_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn scene_readiness_cached(
    state: State<'_, AppState>,
    request: SceneReadinessRequest,
) -> Result<SceneReadinessSummary, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .shot_readiness_service
        .scene_readiness_cached(&request.project_id, &request.scene_id, stage)
        .await
        .map_err(map_readiness_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn scene_preflight(
    state: State<'_, AppState>,
    request: SceneReadinessRequest,
) -> Result<SceneReadinessSummary, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .shot_readiness_service
        .scene_preflight(&request.project_id, &request.scene_id, stage)
        .await
        .map_err(map_readiness_error)
}

fn parse_stage(value: &str) -> Result<ShotStage, AppError> {
    ShotStage::try_from_str(value)
        .map_err(|error| AppError::invalid_input(format!("invalid stage: {error}")))
}

fn map_readiness_error(error: ShotReadinessServiceError) -> AppError {
    AppError::invalid_input(error.to_string())
}
