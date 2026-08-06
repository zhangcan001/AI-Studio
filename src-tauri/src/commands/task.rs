use crate::{
    app_state::AppState,
    application::task_query_service::{TaskQueryError, TaskView},
    error::AppError,
};
use tauri::State;

#[tauri::command(rename_all = "camelCase")]
pub async fn task_get(state: State<'_, AppState>, task_id: String) -> Result<TaskView, AppError> {
    let task = state
        .task_query_service
        .get(&task_id)
        .await
        .map_err(map_query_error)?
        .ok_or_else(|| AppError::task_not_found(format!("task {task_id} was not found")))?;
    Ok(task)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn task_list_recent(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<TaskView>, AppError> {
    state
        .task_query_service
        .list_recent(limit.unwrap_or(10).min(50))
        .await
        .map_err(map_query_error)
}

fn map_query_error(error: TaskQueryError) -> AppError {
    match error {
        TaskQueryError::InvalidTaskId(message) => AppError::invalid_input(message),
        TaskQueryError::Repository(error) => super::map_repository_error(&error),
    }
}
