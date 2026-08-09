use crate::{
    app_state::AppState,
    application::{
        comfy_service::{CapabilitySummary, ComfyStatusView},
        settings_service::{EndpointTestView, SettingsView},
    },
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

#[tauri::command(rename_all = "camelCase")]
pub fn comfy_get_settings(state: tauri::State<'_, AppState>) -> Result<SettingsView, AppError> {
    Ok(state.settings_service.settings())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_test_connection(
    state: tauri::State<'_, AppState>,
    endpoint: String,
) -> Result<EndpointTestView, AppError> {
    state.settings_service.test_connection(&endpoint).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_save_endpoint(
    state: tauri::State<'_, AppState>,
    endpoint: String,
) -> Result<SettingsView, AppError> {
    state.settings_service.save_and_apply(&endpoint).await
}
