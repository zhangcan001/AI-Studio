use crate::{
    app_state::AppState,
    application::batch_workflow_preset_service::{
        BatchWorkflowPresetInput, BatchWorkflowPresetView,
    },
    error::AppError,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn batch_workflow_presets_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BatchWorkflowPresetView>, AppError> {
    state.batch_workflow_preset_service.list().await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn batch_workflow_preset_create(
    state: tauri::State<'_, AppState>,
    input: BatchWorkflowPresetInput,
) -> Result<BatchWorkflowPresetView, AppError> {
    state.batch_workflow_preset_service.create(input).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn batch_workflow_preset_update(
    state: tauri::State<'_, AppState>,
    request: BatchWorkflowPresetUpdateRequest,
) -> Result<BatchWorkflowPresetView, AppError> {
    state
        .batch_workflow_preset_service
        .update(&request.preset_id, request.input)
        .await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchWorkflowPresetUpdateRequest {
    pub preset_id: String,
    pub input: BatchWorkflowPresetInput,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn batch_workflow_preset_delete(
    state: tauri::State<'_, AppState>,
    preset_id: String,
) -> Result<(), AppError> {
    state.batch_workflow_preset_service.delete(&preset_id).await
}
