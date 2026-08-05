use crate::{
    app_state::AppState,
    application::comfy_service::{CapabilitySummary, ComfyStatusView},
    error::AppError,
};
use tauri::State;

#[tauri::command]
pub async fn comfy_get_status(state: State<'_, AppState>) -> Result<ComfyStatusView, AppError> {
    state.comfy_service.get_status().await
}

#[tauri::command]
pub async fn comfy_refresh_capabilities(
    state: State<'_, AppState>,
) -> Result<CapabilitySummary, AppError> {
    state.comfy_service.refresh_capabilities().await
}
