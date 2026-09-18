use crate::{
    app_state::AppState,
    application::{
        comfy_memory_service::{ComfyMemoryReleaseError, ComfyMemoryReleaseResult},
        comfy_service::{CapabilitySummary, ComfyStatusView},
        settings_service::{EndpointTestView, SettingsView},
    },
    error::AppError,
};
use tauri::State;

#[tauri::command]
pub async fn comfy_get_status(state: State<'_, AppState>) -> Result<ComfyStatusView, AppError> {
    state.system.comfy.get_status().await
}

#[tauri::command]
pub async fn comfy_refresh_capabilities(
    state: State<'_, AppState>,
) -> Result<CapabilitySummary, AppError> {
    state.system.comfy.refresh_capabilities().await
}

#[tauri::command(rename_all = "camelCase")]
pub fn comfy_get_settings(state: tauri::State<'_, AppState>) -> Result<SettingsView, AppError> {
    Ok(state.system.settings.settings())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_test_connection(
    state: tauri::State<'_, AppState>,
    endpoint: String,
) -> Result<EndpointTestView, AppError> {
    state.system.settings.test_connection(&endpoint).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_save_endpoint(
    state: tauri::State<'_, AppState>,
    endpoint: String,
) -> Result<SettingsView, AppError> {
    state.system.settings.save_and_apply(&endpoint).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_free_memory(
    state: tauri::State<'_, AppState>,
) -> Result<ComfyMemoryReleaseResult, AppError> {
    state
        .system
        .comfy_memory
        .release()
        .await
        .map_err(map_memory_release_error)
}

fn map_memory_release_error(error: ComfyMemoryReleaseError) -> AppError {
    match error {
        ComfyMemoryReleaseError::Busy {
            active_tasks,
            active_production_items,
            comfy_running,
            comfy_pending,
        } => AppError::comfy_memory_busy(
            "当前仍有任务或 ComfyUI 队列活动，完成或取消后再释放模型内存。",
            serde_json::json!({
                "activeTasks": active_tasks,
                "activeProductionItems": active_production_items,
                "comfyRunning": comfy_running,
                "comfyPending": comfy_pending,
            }),
        ),
        ComfyMemoryReleaseError::TaskRepository(error)
        | ComfyMemoryReleaseError::ProductionRepository(error) => {
            super::map_repository_error(&error)
        }
        ComfyMemoryReleaseError::Comfy(crate::application::ports::ComfyAdapterError::Offline(
            message,
        )) => AppError::comfy_offline(message),
        ComfyMemoryReleaseError::Comfy(crate::application::ports::ComfyAdapterError::Timeout(
            message,
        )) => AppError::comfy_timeout(message),
        ComfyMemoryReleaseError::Comfy(error) => {
            AppError::comfy_memory_release_failed(error.to_string())
        }
    }
}
