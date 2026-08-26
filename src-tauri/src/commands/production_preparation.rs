//! Commands for the DEV-052 preparation boundary.
//!
//! This module is intentionally a thin transport layer. The authoritative
//! implementation lives in `ProductionPreparationService` (Agent B): it must
//! resolve the requested shots again, perform one live preflight, and only
//! then create the existing batch/item/binding/snapshot transaction. These
//! commands never receive prompts, context hashes, values, readiness reports,
//! or workflow payloads from the client.
//!
//! Agent B's landed service contract used here:
//!
//! - `plan_many(project_id, shot_ids, stage) -> Vec<ShotProductionPlan>`
//! - `admit(project_id, shot_ids, stage, allow_partial) ->
//!   ProductionPreparationAdmission`
//! - `plan_detail(project_id, shot_id, stage) -> ShotProductionPlan`
//!
//! The command returns the stable domain DTOs directly; their
//! `camelCase` serde representation is the public wire shape.

use crate::{
    app_state::AppState,
    application::production_preparation_service::ProductionPreparationService,
    domain::{
        ProductionPreparationAdmission, ScenePreparationView, ShotProductionPlan,
        ShotProductionPlanSummary, ShotStage,
    },
    error::AppError,
};
use chrono::Utc;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneProductionPreflightRequest {
    pub project_id: String,
    pub scene_id: String,
    pub stage: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneProductionAdmitRequest {
    pub project_id: String,
    pub scene_id: String,
    pub stage: String,
    pub shot_ids: Vec<String>,
    #[serde(default)]
    pub allow_partial: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShotProductionPlanDetailRequest {
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
}

/// Read-only scene preparation. This command must not create a batch, task,
/// snapshot, or generation request.
#[tauri::command(rename_all = "camelCase")]
pub async fn scene_production_preflight(
    state: State<'_, AppState>,
    request: SceneProductionPreflightRequest,
) -> Result<ScenePreparationView, AppError> {
    let stage = parse_stage(&request.stage)?;
    let (scene_name, shot_ids) =
        scene_scope(&state, &request.project_id, &request.scene_id).await?;
    let plans = state
        .production_preparation_service
        .plan_many(&request.project_id, &shot_ids, stage)
        .await
        .map_err(map_preparation_error)?;
    let items = plans.iter().map(ShotProductionPlanSummary::from).collect();
    Ok(ProductionPreparationService::scene_view(
        request.project_id,
        request.scene_id,
        scene_name,
        stage,
        items,
        Utc::now(),
    ))
}

/// Explicit admission of the selected READY shots. The service re-resolves
/// and live-preflights every shot; the client cannot submit frozen values.
/// Admission creates the prepared batch and snapshot only. Queue start remains
/// a separate, existing user action.
#[tauri::command(rename_all = "camelCase")]
pub async fn scene_production_admit(
    state: State<'_, AppState>,
    request: SceneProductionAdmitRequest,
) -> Result<ProductionPreparationAdmission, AppError> {
    let stage = parse_stage(&request.stage)?;
    let (_, scene_shot_ids) = scene_scope(&state, &request.project_id, &request.scene_id).await?;
    validate_scene_shot_ids(&request.shot_ids, &scene_shot_ids)?;
    let result = state
        .production_preparation_service
        .admit(
            &request.project_id,
            &request.shot_ids,
            stage,
            request.allow_partial,
        )
        .await
        .map_err(map_preparation_error)?;
    Ok(result)
}

/// On-demand detail for the right-hand readiness/context inspector.
#[tauri::command(rename_all = "camelCase")]
pub async fn shot_production_plan_detail(
    state: State<'_, AppState>,
    request: ShotProductionPlanDetailRequest,
) -> Result<ShotProductionPlan, AppError> {
    let stage = parse_stage(&request.stage)?;
    let detail = state
        .production_preparation_service
        .plan_detail(&request.project_id, &request.shot_id, stage)
        .await
        .map_err(map_preparation_error)?;
    Ok(detail)
}

async fn scene_scope(
    state: &AppState,
    project_id: &str,
    scene_id: &str,
) -> Result<(String, Vec<String>), AppError> {
    let tree = state
        .production_structure_service
        .tree(project_id)
        .await
        .map_err(|error| AppError::invalid_input(error.to_string()))?;
    tree.series
        .into_iter()
        .flat_map(|series| series.episodes)
        .flat_map(|episode| episode.scenes)
        .find(|scene| scene.scene.id == scene_id)
        .map(|scene| (scene.scene.name, scene.shot_ids))
        .ok_or_else(|| AppError::invalid_input(format!("SCENE_NOT_FOUND: {scene_id}")))
}

fn validate_scene_shot_ids(shot_ids: &[String], scene_shot_ids: &[String]) -> Result<(), AppError> {
    if shot_ids.is_empty() {
        return Err(AppError::invalid_input("至少需要一个镜头".to_owned()));
    }
    if shot_ids.len() > 500 {
        return Err(AppError::invalid_input(
            "PREPARATION_BATCH_LIMIT: at most 500 shots".to_owned(),
        ));
    }
    let mut seen = std::collections::HashSet::with_capacity(shot_ids.len());
    for shot_id in shot_ids {
        if !seen.insert(shot_id) {
            return Err(AppError::invalid_input("镜头不能重复".to_owned()));
        }
        if !scene_shot_ids.iter().any(|candidate| candidate == shot_id) {
            return Err(AppError::invalid_input(format!(
                "SHOT_NOT_IN_SCENE: {shot_id}"
            )));
        }
    }
    Ok(())
}

fn parse_stage(value: &str) -> Result<ShotStage, AppError> {
    ShotStage::try_from_str(&value.trim().to_ascii_lowercase())
        .map_err(|error| AppError::invalid_input(format!("invalid stage: {error}")))
}

fn map_preparation_error(error: impl std::fmt::Display) -> AppError {
    AppError::invalid_input(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_stage;
    use crate::domain::ShotStage;

    #[test]
    fn parse_stage_accepts_wire_case() {
        assert_eq!(parse_stage(" IMAGE ").unwrap(), ShotStage::Image);
        assert_eq!(parse_stage("video").unwrap(), ShotStage::Video);
    }
}
