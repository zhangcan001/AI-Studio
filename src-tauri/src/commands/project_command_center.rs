use crate::{
    app_state::AppState,
    application::project_command_center_service::{
        ProjectCommandCenterError, ProjectCommandCenterView,
    },
    error::AppError,
};

fn map_error(error: ProjectCommandCenterError) -> AppError {
    match error {
        ProjectCommandCenterError::InvalidInput(message) => AppError::invalid_input(message),
        ProjectCommandCenterError::NotFound(message) => AppError::project_not_found(message),
        ProjectCommandCenterError::Audit(message) => AppError::database(message),
        ProjectCommandCenterError::Database(error) => AppError::database(error.to_string()),
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_command_center_get(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<ProjectCommandCenterView, AppError> {
    state
        .project_command_center_service
        .get(&project_id)
        .await
        .map_err(map_error)
}
