use crate::{
    app_state::AppState,
    application::episode_production_service::{
        EpisodeProductionError, EpisodeProductionPlan, EpisodeProductionPrepareResult,
        EpisodeProductionReadinessSummary,
    },
    domain::ShotStage,
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeProductionPlanRequest {
    pub project_id: String,
    pub episode_id: String,
    pub stage: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeProductionPrepareRequest {
    pub project_id: String,
    pub episode_id: String,
    pub stage: String,
    #[serde(default)]
    pub scene_ids: Vec<String>,
    #[serde(default)]
    pub allow_partial: bool,
}

#[tauri::command]
pub async fn episode_production_plan(
    state: State<'_, AppState>,
    request: EpisodeProductionPlanRequest,
) -> Result<EpisodeProductionPlan, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .episode_production_service
        .plan(&request.project_id, &request.episode_id, stage)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn episode_production_readiness_summary(
    state: State<'_, AppState>,
    request: EpisodeProductionPlanRequest,
) -> Result<EpisodeProductionReadinessSummary, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .episode_production_service
        .readiness_summary(
            &request.project_id,
            &request.episode_id,
            stage,
            &state.shot_readiness_service,
        )
        .await
        .map_err(map_error)
}

#[tauri::command]
pub async fn episode_production_prepare(
    state: State<'_, AppState>,
    request: EpisodeProductionPrepareRequest,
) -> Result<EpisodeProductionPrepareResult, AppError> {
    let stage = parse_stage(&request.stage)?;
    state
        .episode_production_service
        .prepare(
            &request.project_id,
            &request.episode_id,
            stage,
            &request.scene_ids,
            request.allow_partial,
        )
        .await
        .map_err(map_error)
}

fn parse_stage(value: &str) -> Result<ShotStage, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    ShotStage::try_from_str(&normalized).map_err(|error| AppError::invalid_input(error.to_string()))
}

fn map_error(error: EpisodeProductionError) -> AppError {
    match error {
        EpisodeProductionError::Blocked(plan) => AppError::invalid_input(format!(
            "EPISODE_PRODUCTION_BLOCKED: {}",
            serde_json::to_string(&plan).unwrap_or_else(|_| "plan serialization failed".to_owned())
        )),
        EpisodeProductionError::Partial(result) => AppError::invalid_input(format!(
            "EPISODE_PRODUCTION_PARTIAL: {}",
            serde_json::to_string(&result)
                .unwrap_or_else(|_| "partial result serialization failed".to_owned())
        )),
        other => AppError::invalid_input(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_stage;
    use crate::domain::ShotStage;

    #[test]
    fn parse_stage_accepts_wire_case_without_adding_a_second_stage_type() {
        assert_eq!(parse_stage("IMAGE").unwrap(), ShotStage::Image);
        assert_eq!(parse_stage(" video ").unwrap(), ShotStage::Video);
    }
}
