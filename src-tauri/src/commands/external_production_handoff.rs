use crate::{
    app_state::AppState,
    application::external_production_handoff_service::{
        ExternalProductionHandoffConfirmResult, ExternalProductionHandoffError,
        ExternalProductionHandoffHistoryItem, ExternalProductionHandoffPreview,
    },
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffPreviewRequest {
    pub project_id: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffConfirmRequest {
    pub project_id: String,
    pub content: String,
    pub expected_document_sha256: String,
}

fn map_error(error: ExternalProductionHandoffError) -> AppError {
    AppError::external_production_handoff(error.code(), error.to_string())
}

#[tauri::command]
pub async fn external_production_handoff_preview(
    state: State<'_, AppState>,
    request: ExternalProductionHandoffPreviewRequest,
) -> Result<ExternalProductionHandoffPreview, AppError> {
    state
        .external_production_handoff_service
        .preview(&request.project_id, &request.content)
        .await
        .map_err(map_error)
}

#[tauri::command]
pub async fn external_production_handoff_confirm(
    state: State<'_, AppState>,
    request: ExternalProductionHandoffConfirmRequest,
) -> Result<ExternalProductionHandoffConfirmResult, AppError> {
    state
        .external_production_handoff_service
        .confirm(
            &request.project_id,
            &request.content,
            &request.expected_document_sha256,
        )
        .await
        .map_err(map_error)
}

#[tauri::command]
pub async fn external_production_handoff_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ExternalProductionHandoffHistoryItem>, AppError> {
    state
        .external_production_handoff_service
        .list(&project_id)
        .await
        .map_err(map_error)
}

#[tauri::command]
pub async fn external_production_handoff_mappings(
    state: State<'_, AppState>,
    project_id: String,
    handoff_id: String,
) -> Result<
    Vec<crate::application::external_production_handoff_service::ExternalProductionHandoffEntityMappingView>,
    AppError,
>{
    state
        .external_production_handoff_service
        .mappings(&project_id, &handoff_id)
        .await
        .map_err(map_error)
}
