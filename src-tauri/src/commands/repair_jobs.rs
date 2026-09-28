use crate::{app_state::AppState, application::repair_jobs::RepairJobStatusView, error::AppError};
use tauri::State;

/// Read-only status of application-level data repair jobs (migration 040).
#[tauri::command]
pub async fn repair_jobs_status(
    state: State<'_, AppState>,
) -> Result<Vec<RepairJobStatusView>, AppError> {
    state
        .system
        .repair_jobs
        .status()
        .await
        .map_err(|error| AppError::database(error.to_string()))
}
