use crate::{
    app_state::AppState,
    application::production_audit_service::{
        ProductionAuditActivity, ProductionAuditError, ProductionAuditIntegrity,
        ProductionAuditLineage, ProductionAuditSummary,
    },
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditProjectRequest {
    pub project_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditRecentActivityRequest {
    pub project_id: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditLineageRequest {
    pub project_id: String,
    pub root_type: String,
    pub root_id: String,
}

fn map_audit_error(error: ProductionAuditError) -> AppError {
    match error {
        ProductionAuditError::InvalidInput(message) => AppError::invalid_input(message),
        ProductionAuditError::NotFound(message) if message.starts_with("project not found") => {
            AppError::project_not_found(message)
        }
        ProductionAuditError::NotFound(message) => AppError::database(message),
        ProductionAuditError::Database(error) => AppError::database(error.to_string()),
    }
}

#[tauri::command]
pub async fn production_audit_summary(
    state: State<'_, AppState>,
    request: ProductionAuditProjectRequest,
) -> Result<ProductionAuditSummary, AppError> {
    state
        .production_audit_service
        .summary(&request.project_id)
        .await
        .map_err(map_audit_error)
}

#[tauri::command]
pub async fn production_audit_recent_activity(
    state: State<'_, AppState>,
    request: ProductionAuditRecentActivityRequest,
) -> Result<Vec<ProductionAuditActivity>, AppError> {
    state
        .production_audit_service
        .recent_activity(&request.project_id, request.limit)
        .await
        .map_err(map_audit_error)
}

#[tauri::command]
pub async fn production_audit_lineage(
    state: State<'_, AppState>,
    request: ProductionAuditLineageRequest,
) -> Result<ProductionAuditLineage, AppError> {
    state
        .production_audit_service
        .lineage(&request.project_id, &request.root_type, &request.root_id)
        .await
        .map_err(map_audit_error)
}

#[tauri::command]
pub async fn production_audit_integrity(
    state: State<'_, AppState>,
    request: ProductionAuditProjectRequest,
) -> Result<ProductionAuditIntegrity, AppError> {
    state
        .production_audit_service
        .integrity(&request.project_id)
        .await
        .map_err(map_audit_error)
}
