use crate::{
    app_state::AppState, application::production_batch_runbook_service::ProductionBatchRunbook,
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchRunbookRequest {
    pub project_id: String,
    #[serde(default)]
    pub series_id: Option<String>,
}

#[tauri::command]
pub async fn production_batch_runbook(
    state: State<'_, AppState>,
    request: ProductionBatchRunbookRequest,
) -> Result<ProductionBatchRunbook, AppError> {
    state
        .production_batch_runbook_service
        .list(&request.project_id, request.series_id.as_deref())
        .await
        .map_err(|error| AppError::invalid_input(error.to_string()))
}
