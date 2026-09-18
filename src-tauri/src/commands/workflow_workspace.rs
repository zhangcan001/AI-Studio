use crate::{
    app_state::AppState,
    application::workflow_workspace_query_service::{
        WorkflowWorkspaceQueryMode, WorkflowWorkspaceQueryResponse,
    },
    error::AppError,
};
use tauri::State;

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_workspace_query(
    state: State<'_, AppState>,
    mode: WorkflowWorkspaceQueryMode,
) -> Result<WorkflowWorkspaceQueryResponse, AppError> {
    state
        .workflow
        .workspace_query
        .query(mode)
        .await
        .map_err(|error| AppError::workflow_onboarding(error.to_string()))
}
