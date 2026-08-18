use crate::{
    app_state::AppState,
    application::scene_production_service::{
        SceneProductionError, SceneProductionPlan, SceneProductionPrepareResult,
    },
    commands::production_queue::ProductionBatchDetailView,
    domain::ShotStage,
    error::AppError,
};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneProductionPrepareRequest {
    pub project_id: String,
    pub scene_id: String,
    pub stage: String,
    #[serde(default)]
    pub allow_partial: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneProductionPlanRequest {
    pub project_id: String,
    pub scene_id: String,
    pub stage: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneProductionPrepareView {
    pub project_id: String,
    pub scene_id: String,
    pub stage: String,
    pub created: bool,
    pub created_count: usize,
    pub already_prepared: bool,
    pub existing_batch_ids: Vec<String>,
    pub detail: Option<ProductionBatchDetailView>,
}

#[tauri::command]
pub async fn scene_production_plan(
    state: State<'_, AppState>,
    request: SceneProductionPlanRequest,
) -> Result<SceneProductionPlan, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .scene_production_service
        .plan(&request.project_id, &request.scene_id, stage)
        .await
        .map_err(map_error)
}

#[tauri::command]
pub async fn scene_production_prepare(
    state: State<'_, AppState>,
    request: SceneProductionPrepareRequest,
) -> Result<SceneProductionPrepareView, AppError> {
    let stage = parse_stage(&request.stage)?;
    let result = state
        .scene_production_service
        .prepare(
            &request.project_id,
            &request.scene_id,
            stage,
            request.allow_partial,
        )
        .await
        .map_err(map_error)?;
    Ok(prepare_view(result))
}

fn prepare_view(result: SceneProductionPrepareResult) -> SceneProductionPrepareView {
    SceneProductionPrepareView {
        project_id: result.project_id,
        scene_id: result.scene_id,
        stage: result.stage.as_str().to_owned(),
        created: result.created,
        created_count: result.created_count,
        already_prepared: result.already_prepared,
        existing_batch_ids: result.existing_batch_ids,
        detail: result.detail.map(Into::into),
    }
}

fn parse_stage(value: &str) -> Result<ShotStage, AppError> {
    ShotStage::try_from_str(value).map_err(|error| AppError::invalid_input(error.to_string()))
}

fn map_error(error: SceneProductionError) -> AppError {
    match error {
        SceneProductionError::Blocked(plan) => AppError::invalid_input(format!(
            "SCENE_PRODUCTION_BLOCKED: {}",
            serde_json::to_string(&plan).unwrap_or_else(|_| "plan serialization failed".to_owned())
        )),
        SceneProductionError::TooLarge { eligible, max } => AppError::invalid_input(format!(
            "SCENE_PRODUCTION_TOO_LARGE: {eligible} eligible shots exceeds {max}"
        )),
        SceneProductionError::SceneNotFound(scene_id) => {
            AppError::invalid_input(format!("SCENE_NOT_FOUND: {scene_id}"))
        }
        SceneProductionError::Structure(error) => AppError::invalid_input(error.to_string()),
        SceneProductionError::ShotBatch(error) => AppError::invalid_input(error.to_string()),
    }
}
