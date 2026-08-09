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
pub async fn runtime_activity_status(
    state: State<'_, AppState>,
) -> Result<RuntimeActivityStatusView, AppError> {
    state.diagnostics_service.runtime_activity_status().await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn diagnostics_summary(
    state: State<'_, AppState>,
) -> Result<DiagnosticsSummaryView, AppError> {
    Ok(state.diagnostics_service.summary().await)
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
    let summary = state.diagnostics_service.summary().await;
    state
        .diagnostics_service
        .export_bundle(destination, summary)
        .await
        .map(Some)
}
