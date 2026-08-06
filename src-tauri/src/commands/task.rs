use crate::{
    app_state::AppState,
    application::pagination::PageCursor,
    application::ports::TaskHistoryFilter,
    application::task_cancellation_service::TaskCancellationError,
    application::task_history_service::{
        ReusableGenerationDraftView, TaskDetailView, TaskHistoryError, TaskHistoryPageView,
    },
    application::task_query_service::{TaskQueryError, TaskView},
    application::task_recovery_service::{RecoveryReport, TaskRecoveryError},
    error::AppError,
};
use tauri::State;

#[tauri::command(rename_all = "camelCase")]
pub async fn task_get(
    state: State<'_, AppState>,
    project_id: String,
    task_id: String,
) -> Result<TaskView, AppError> {
    super::validate_project_id(&project_id)?;
    let task = state
        .task_query_service
        .get(&project_id, &task_id)
        .await
        .map_err(map_query_error)?
        .ok_or_else(|| AppError::task_not_found(format!("task {task_id} was not found")))?;
    Ok(task)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn task_list_recent(
    state: State<'_, AppState>,
    project_id: String,
    limit: Option<u32>,
) -> Result<Vec<TaskView>, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .task_query_service
        .list_recent(&project_id, limit.unwrap_or(10).min(50))
        .await
        .map_err(map_query_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn task_cancel(
    state: State<'_, AppState>,
    project_id: String,
    task_id: String,
) -> Result<TaskView, AppError> {
    super::validate_project_id(&project_id)?;
    let task = state
        .task_cancellation_service
        .request_cancel(&project_id, &task_id)
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

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskHistoryFilterDto {
    All,
    Active,
    Succeeded,
    Failed,
    Cancelled,
}

impl From<TaskHistoryFilterDto> for TaskHistoryFilter {
    fn from(value: TaskHistoryFilterDto) -> Self {
        match value {
            TaskHistoryFilterDto::All => Self::All,
            TaskHistoryFilterDto::Active => Self::Active,
            TaskHistoryFilterDto::Succeeded => Self::Succeeded,
            TaskHistoryFilterDto::Failed => Self::Failed,
            TaskHistoryFilterDto::Cancelled => Self::Cancelled,
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn task_history_page(
    state: State<'_, AppState>,
    project_id: String,
    filter: TaskHistoryFilterDto,
    cursor: Option<PageCursor>,
    limit: Option<u32>,
) -> Result<TaskHistoryPageView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .task_history_service
        .list_page(&project_id, filter.into(), cursor, limit.unwrap_or(30))
        .await
        .map_err(map_history_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn task_get_detail(
    state: State<'_, AppState>,
    project_id: String,
    task_id: String,
) -> Result<TaskDetailView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .task_history_service
        .get_detail(&project_id, &task_id)
        .await
        .map_err(map_history_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn task_get_reusable_draft(
    state: State<'_, AppState>,
    project_id: String,
    task_id: String,
) -> Result<ReusableGenerationDraftView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .task_history_service
        .get_reusable_draft(&project_id, &task_id)
        .await
        .map_err(map_history_error)
}

fn map_query_error(error: TaskQueryError) -> AppError {
    match error {
        TaskQueryError::InvalidProjectId(message) => AppError::invalid_input(message),
        TaskQueryError::InvalidTaskId(message) => AppError::invalid_input(message),
        TaskQueryError::Repository(error) => super::map_repository_error(&error),
    }
}

fn map_history_error(error: TaskHistoryError) -> AppError {
    match error {
        TaskHistoryError::InvalidProjectId => {
            AppError::invalid_input("INVALID_PROJECT_ID: project id must not be empty")
        }
        TaskHistoryError::InvalidTaskId(message) => AppError::invalid_input(message),
        TaskHistoryError::NotFound(task_id) => {
            AppError::task_not_found(format!("task {task_id} was not found"))
        }
        TaskHistoryError::DraftUnavailable(message) => {
            AppError::reusable_draft_unavailable(message)
        }
        TaskHistoryError::Repository(error) => super::map_repository_error(&error),
    }
}

fn map_cancellation_error(error: TaskCancellationError) -> AppError {
    match error {
        TaskCancellationError::InvalidProjectId(message) => AppError::invalid_input(message),
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
