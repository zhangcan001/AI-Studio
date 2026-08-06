use crate::{
    app_state::AppState, application::workflow_library_service::WorkflowSyncReport, error::AppError,
};
use tauri::State;

#[tauri::command]
pub async fn workflow_library_refresh(
    state: State<'_, AppState>,
) -> Result<WorkflowSyncReport, AppError> {
    state
        .workflow_library_service
        .sync()
        .await
        .map_err(|error| AppError::workflow_package_invalid(error.to_string()))
}
