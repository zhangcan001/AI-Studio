use crate::{
    app_state::AppState,
    application::diagnostics_service::{
        DiagnosticsExportView, DiagnosticsSummaryView, RuntimeActivityStatusView,
    },
    error::AppError,
};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command(rename_all = "camelCase")]
pub async fn diagnostics_execution_health(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<crate::application::diagnostics_service::execution::ExecutionHealth, AppError> {
    state.system.diagnostics.execution_health(&project_id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn diagnostics_recent_failures(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<crate::application::diagnostics_service::execution::RecentFailure>, AppError> {
    state.system.diagnostics.recent_failures(&project_id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn diagnostics_task_timeline(
    state: State<'_, AppState>,
    project_id: String,
    task_id: String,
) -> Result<crate::application::diagnostics_service::execution::TaskTimeline, AppError> {
    state
        .system
        .diagnostics
        .task_timeline(&project_id, &task_id)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn runtime_activity_status(
    state: State<'_, AppState>,
) -> Result<RuntimeActivityStatusView, AppError> {
    state.system.diagnostics.runtime_activity_status().await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn diagnostics_summary(
    state: State<'_, AppState>,
) -> Result<DiagnosticsSummaryView, AppError> {
    Ok(state.system.diagnostics.summary().await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn diagnostics_export(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<DiagnosticsExportView>, AppError> {
    let suggested_name = format!(
        "AI-Studio-Diagnostics-{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let Some(file) = app_handle
        .dialog()
        .file()
        .add_filter("AI Studio 诊断包", &["zip"])
        .set_file_name(&suggested_name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let destination = file
        .into_path()
        .map_err(|_| AppError::filesystem("诊断包保存位置不可用"))?;
    let summary = state.system.diagnostics.summary().await;
    state
        .system
        .diagnostics
        .export_bundle(destination, summary)
        .await
        .map(Some)
}
