use crate::{
    app_state::AppState,
    application::series_production_service::{
        SeriesProductionError, SeriesProductionPlan, SeriesProductionPrepareResult,
        SeriesProductionReadinessSummary,
    },
    domain::ShotStage,
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesProductionPlanRequest {
    pub project_id: String,
    pub series_id: String,
    pub stage: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesProductionPrepareRequest {
    pub project_id: String,
    pub series_id: String,
    pub stage: String,
    #[serde(default)]
    pub episode_ids: Vec<String>,
    #[serde(default)]
    pub allow_partial: bool,
}

#[tauri::command]
pub async fn series_production_plan(
    state: State<'_, AppState>,
    request: SeriesProductionPlanRequest,
) -> Result<SeriesProductionPlan, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .series_production_service
        .plan(&request.project_id, &request.series_id, stage)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn series_production_readiness_summary(
    state: State<'_, AppState>,
    request: SeriesProductionPlanRequest,
) -> Result<SeriesProductionReadinessSummary, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .series_production_service
        .readiness_summary(
            &request.project_id,
            &request.series_id,
            stage,
            &state.shot_readiness_service,
        )
        .await
        .map_err(map_error)
}

#[tauri::command]
pub async fn series_production_prepare(
    state: State<'_, AppState>,
    request: SeriesProductionPrepareRequest,
) -> Result<SeriesProductionPrepareResult, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .series_production_service
        .prepare(
            &request.project_id,
            &request.series_id,
            stage,
            &request.episode_ids,
            request.allow_partial,
        )
        .await
        .map_err(map_error)
}

fn parse_stage(value: &str) -> Result<ShotStage, AppError> {
    ShotStage::try_from_str(value.trim())
        .map_err(|error| AppError::invalid_input(error.to_string()))
}

fn map_error(error: SeriesProductionError) -> AppError {
    match error {
        SeriesProductionError::Blocked(plan) => AppError::invalid_input(format!(
            "SERIES_PRODUCTION_BLOCKED: {}",
            serde_json::to_string(&plan).unwrap_or_else(|_| "plan serialization failed".to_owned())
        )),
        SeriesProductionError::Partial(result) => AppError::invalid_input(format!(
            "SERIES_PRODUCTION_PARTIAL: {}",
            serde_json::to_string(&result)
                .unwrap_or_else(|_| "partial result serialization failed".to_owned())
        )),
        other => AppError::invalid_input(other.to_string()),
    }
}
