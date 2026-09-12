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
        ProductionPreparationAdmission, ProjectPreparationView, ScenePreparationView,
        ShotProductionPlan, ShotProductionPlanSummary, ShotReadinessStatus, ShotStage,
    },
    error::AppError,
};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashSet;
use tauri::State;

const MAX_PROJECT_PLAN_SHOTS: usize = 500;
const MAX_PREPARATION_BATCH_ITEMS: usize = 100;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectProductionPreflightRequest {
    pub project_id: String,
    pub stage: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectProductionAdmitRequest {
    pub project_id: String,
    pub stage: String,
    pub shot_ids: Vec<String>,
    #[serde(default)]
    pub allow_partial: bool,
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

/// Read-only project preparation plan. The project scope is resolved once and
/// evaluated by the existing ProductionPreparationService; no batch or task
/// is created here.
#[tauri::command(rename_all = "camelCase")]
pub async fn project_production_preflight(
    state: State<'_, AppState>,
    request: ProjectProductionPreflightRequest,
) -> Result<ProjectPreparationView, AppError> {
    let stage = parse_stage(&request.stage)?;
    let shot_ids = project_scope(&state, &request.project_id).await?;
    if shot_ids.is_empty() {
        return Ok(project_view(request.project_id, stage, Vec::new()));
    }
    let plans = state
        .production_preparation_service
        .plan_many(&request.project_id, &shot_ids, stage)
        .await
        .map_err(map_preparation_error)?;
    Ok(project_view(request.project_id, stage, plans))
}

/// Explicit project preparation admission. The service revalidates live and
/// creates only READY batch/snapshot records; queue start remains separate.
#[tauri::command(rename_all = "camelCase")]
pub async fn project_production_admit(
    state: State<'_, AppState>,
    request: ProjectProductionAdmitRequest,
) -> Result<ProductionPreparationAdmission, AppError> {
    let stage = parse_stage(&request.stage)?;
    let project_shot_ids = project_scope(&state, &request.project_id).await?;
    validate_project_shot_ids(
        &request.shot_ids,
        &project_shot_ids,
        MAX_PREPARATION_BATCH_ITEMS,
    )?;
    state
        .production_preparation_service
        .admit(
            &request.project_id,
            &request.shot_ids,
            stage,
            request.allow_partial,
        )
        .await
        .map_err(map_preparation_error)
}

fn project_view(
    project_id: String,
    stage: ShotStage,
    plans: Vec<ShotProductionPlan>,
) -> ProjectPreparationView {
    let items = plans
        .iter()
        .map(ShotProductionPlanSummary::from)
        .collect::<Vec<_>>();
    ProjectPreparationView {
        project_id,
        stage: stage.as_str().to_owned(),
        total: items.len(),
        ready_count: items
            .iter()
            .filter(|item| item.status == ShotReadinessStatus::Ready)
            .count(),
        incomplete_count: items
            .iter()
            .filter(|item| item.status == ShotReadinessStatus::Incomplete)
            .count(),
        blocked_count: items
            .iter()
            .filter(|item| item.status == ShotReadinessStatus::Blocked)
            .count(),
        prepared_count: items.iter().filter(|item| item.already_prepared).count(),
        warning_count: items.iter().map(|item| item.warning_count).sum(),
        items,
        evaluated_at: Utc::now(),
    }
}

async fn project_scope(state: &AppState, project_id: &str) -> Result<Vec<String>, AppError> {
    let tree = state
        .production_structure_service
        .tree(project_id)
        .await
        .map_err(|error| AppError::invalid_input(error.to_string()))?;
    let mut seen = HashSet::new();
    let mut shot_ids = Vec::new();
    for series in tree.series {
        for episode in series.episodes {
            for scene in episode.scenes {
                for shot_id in scene.shot_ids {
                    if seen.insert(shot_id.clone()) {
                        shot_ids.push(shot_id);
                    }
                }
            }
        }
    }
    for shot_id in tree.unassigned_shot_ids {
        if seen.insert(shot_id.clone()) {
            shot_ids.push(shot_id);
        }
    }
    if shot_ids.len() > MAX_PROJECT_PLAN_SHOTS {
        return Err(AppError::invalid_input(format!(
            "PROJECT_PREPARATION_SCOPE_TOO_LARGE: at most {MAX_PROJECT_PLAN_SHOTS} shots"
        )));
    }
    Ok(shot_ids)
}

fn validate_project_shot_ids(
    shot_ids: &[String],
    project_shot_ids: &[String],
    max_items: usize,
) -> Result<(), AppError> {
    if shot_ids.is_empty() {
        return Err(AppError::invalid_input("至少需要一个镜头".to_owned()));
    }
    if shot_ids.len() > max_items {
        return Err(AppError::invalid_input(format!(
            "PREPARATION_BATCH_LIMIT: at most {max_items} shots"
        )));
    }
    let project_ids = project_shot_ids.iter().collect::<HashSet<_>>();
    let mut seen = HashSet::with_capacity(shot_ids.len());
    for shot_id in shot_ids {
        if !seen.insert(shot_id) {
            return Err(AppError::invalid_input("镜头不能重复".to_owned()));
        }
        if !project_ids.contains(shot_id) {
            return Err(AppError::invalid_input(format!(
                "SHOT_NOT_IN_PROJECT: {shot_id}"
            )));
        }
    }
    Ok(())
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
    use super::{parse_stage, validate_project_shot_ids};
    use crate::domain::ShotStage;

    #[test]
    fn parse_stage_accepts_wire_case() {
        assert_eq!(parse_stage(" IMAGE ").unwrap(), ShotStage::Image);
        assert_eq!(parse_stage("video").unwrap(), ShotStage::Video);
    }

    #[test]
    fn project_admit_rejects_duplicate_cross_project_and_over_limit_selection() {
        let project_ids = vec!["shot-1".to_owned(), "shot-2".to_owned()];
        assert!(validate_project_shot_ids(&["shot-1".to_owned()], &project_ids, 100).is_ok());
        assert!(validate_project_shot_ids(
            &["shot-1".to_owned(), "shot-1".to_owned()],
            &project_ids,
            100
        )
        .is_err());
        assert!(
            validate_project_shot_ids(&["other-project-shot".to_owned()], &project_ids, 100)
                .is_err()
        );
        let over_limit = (0..101)
            .map(|index| format!("shot-{index}"))
            .collect::<Vec<_>>();
        assert!(validate_project_shot_ids(&over_limit, &over_limit, 100).is_err());
    }
}
