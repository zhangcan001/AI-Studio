use crate::{
    app_state::AppState, application::comfy_preflight_service::ComfyPreflightReport,
    error::AppError,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_preflight_current(
    state: tauri::State<'_, AppState>,
) -> Result<ComfyPreflightReport, AppError> {
    state.comfy_preflight_service.current().await
}
