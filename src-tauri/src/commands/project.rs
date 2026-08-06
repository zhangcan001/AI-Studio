use crate::{
    app_state::AppState,
    application::project_service::{ProjectServiceError, ProjectView},
    error::AppError,
};
use tauri::State;

#[tauri::command(rename_all = "camelCase")]
pub async fn project_list(state: State<'_, AppState>) -> Result<Vec<ProjectView>, AppError> {
    state
        .project_service
        .list()
        .await
        .map_err(map_project_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_create(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<ProjectView, AppError> {
    state
        .project_service
        .create(&name, description.as_deref())
        .await
        .map_err(map_project_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_update(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
    description: Option<String>,
) -> Result<ProjectView, AppError> {
    state
        .project_service
        .update(&project_id, &name, description.as_deref())
        .await
        .map_err(map_project_error)
}

fn map_project_error(error: ProjectServiceError) -> AppError {
    match error {
        ProjectServiceError::InvalidName(message)
        | ProjectServiceError::InvalidDescription(message)
        | ProjectServiceError::InvalidProjectId(message) => AppError::invalid_input(message),
        ProjectServiceError::NotFound(project_id) => {
            AppError::project_not_found(format!("project {project_id} was not found"))
        }
        ProjectServiceError::Repository(error) => super::map_repository_error(&error),
        ProjectServiceError::Directory(error) => AppError::filesystem(error.to_string()),
        ProjectServiceError::Compensation {
            repository,
            cleanup,
        } => AppError::filesystem(format!(
            "project creation failed: {repository}; cleanup failed: {cleanup}"
        )),
    }
}
