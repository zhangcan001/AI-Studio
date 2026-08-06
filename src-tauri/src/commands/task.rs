use crate::{
    app_state::AppState,
    application::task_cancellation_service::TaskCancellationError,
    application::task_query_service::{TaskQueryError, TaskView},
    application::task_recovery_service::{RecoveryReport, TaskRecoveryError},
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

#[tauri::command(rename_all = "camelCase")]
pub async fn task_cancel(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<TaskView, AppError> {
    let task = state
        .task_cancellation_service
        .request_cancel(&task_id)
        .await
        .map_err(map_cancellation_error)?;
    state
        .task_query_service
        .view(task)
        .await
        .map_err(map_query_error)
}

#[tauri::command]
pub async fn task_reconcile_active(state: State<'_, AppState>) -> Result<RecoveryReport, AppError> {
    state
        .task_recovery_service
        .reconcile_active()
        .await
        .map_err(map_recovery_error)
}

fn map_query_error(error: TaskQueryError) -> AppError {
    match error {
        TaskQueryError::InvalidTaskId(message) => AppError::invalid_input(message),
        TaskQueryError::Repository(error) => super::map_repository_error(&error),
    }
}

fn map_cancellation_error(error: TaskCancellationError) -> AppError {
    match error {
        TaskCancellationError::InvalidTaskId(message) => AppError::invalid_input(message),
        TaskCancellationError::NotFound(task_id) => {
            AppError::task_not_found(format!("task {task_id} was not found"))
        }
        TaskCancellationError::NotCancellable { task_id, status } => {
            AppError::task_not_cancellable(format!(
                "task {task_id} cannot be cancelled from {}",
                status.as_str()
            ))
        }
        TaskCancellationError::Repository(error) => super::map_repository_error(&error),
        TaskCancellationError::Domain(error) => AppError::invalid_input(error.to_string()),
    }
}

fn map_recovery_error(error: TaskRecoveryError) -> AppError {
    match error {
        TaskRecoveryError::Repository(error) => super::map_repository_error(&error),
        TaskRecoveryError::Domain(error) => AppError::invalid_input(error.to_string()),
        TaskRecoveryError::Unresolved(message)
        | TaskRecoveryError::OutputCollection(message)
        | TaskRecoveryError::AssetImport(message) => AppError::internal(message),
    }
}
