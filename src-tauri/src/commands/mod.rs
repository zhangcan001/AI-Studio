pub mod asset;
pub mod batch_workflow_preset;
pub mod catalog;
pub mod comfy;
pub mod consistency_assets;
pub mod diagnostics;
pub mod episode_production;
pub mod generation;
pub mod h3_local_import;
pub mod organization;
pub mod preflight;
pub mod preset;
pub mod production_audit;
pub mod production_batch_runbook;
pub mod production_item_review;
pub mod production_orchestrator;
pub mod production_queue;
pub mod production_structure;
pub mod project;
pub mod project_command_center;
pub mod prompt_library;
pub mod prompt_template;
pub mod reference_anchor;
pub mod scene_production;
pub mod series_production;
pub mod settings;
pub mod shot;
pub mod shot_batch;
pub mod shot_bulk;
pub mod shot_readiness;
pub mod task;
pub mod workflow_benchmark;
pub mod workflow_library;
pub mod workflow_lifecycle;
pub mod workflow_onboarding;

use crate::{app_state::AppState, error::AppError};
use serde::Serialize;
use tauri::State;

pub(crate) fn validate_project_id(value: &str) -> Result<(), AppError> {
    crate::domain::validate_project_id(value)
        .map_err(|error| AppError::invalid_input(error.to_string()))
}

pub(crate) fn map_repository_error(error: &crate::application::ports::RepositoryError) -> AppError {
    match error {
        crate::application::ports::RepositoryError::WorkflowVersionConflict { .. } => {
            AppError::workflow_version_conflict(error.to_string())
        }
        crate::application::ports::RepositoryError::RecipeVersionConflict { .. } => {
            AppError::recipe_version_conflict(error.to_string())
        }
        crate::application::ports::RepositoryError::NotFound { .. } => {
            AppError::database(error.to_string())
        }
        _ => AppError::database(error.to_string()),
    }
}

#[derive(Debug, Serialize)]
pub struct AppStatus {
    pub backend: &'static str,
    pub database: &'static str,
    pub version: &'static str,
}

#[tauri::command]
pub fn ping() -> Result<&'static str, AppError> {
    Ok("pong")
}

#[tauri::command]
pub fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, AppError> {
    if !state.data_dirs.root.is_dir() || !state.data_dirs.database.is_file() {
        return Err(AppError::internal(
            "application data directory or database is not ready",
        ));
    }

    Ok(AppStatus {
        backend: "ready",
        database: "ready",
        version: env!("CARGO_PKG_VERSION"),
    })
}
