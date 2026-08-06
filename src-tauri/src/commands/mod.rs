pub mod asset;
pub mod catalog;
pub mod comfy;
pub mod generation;
pub mod project;
pub mod task;
pub mod workflow_library;

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
    pub data_root: String,
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
        data_root: state.data_dirs.root.display().to_string(),
        version: env!("CARGO_PKG_VERSION"),
    })
}
