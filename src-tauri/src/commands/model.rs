use crate::app_state::AppState;
use crate::application::model_service::{
    CreateModelRequest, CreateModelVersionRequest, ModelServiceError, ModelVersionView, ModelView,
    UpdateModelRequest,
};
use crate::error::AppError;
use serde::Deserialize;
use serde_json::Value;
use tauri::State;

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCreateRequest {
    pub name: String,
    pub provider: String,
    #[serde(rename = "type")]
    pub model_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUpdateRequest {
    pub model_id: String,
    pub name: String,
    pub provider: String,
    #[serde(rename = "type")]
    pub model_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVersionCreateRequest {
    pub model_id: String,
    pub version: String,
    #[serde(default = "empty_object")]
    pub capabilities: Value,
    #[serde(default = "empty_object")]
    pub parameter_schema: Value,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_list(state: State<'_, AppState>) -> Result<Vec<ModelView>, AppError> {
    state.model_service.list().await.map_err(map_model_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_get(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<ModelView, AppError> {
    state
        .model_service
        .get(&model_id)
        .await
        .map_err(map_model_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_create(
    state: State<'_, AppState>,
    request: ModelCreateRequest,
) -> Result<ModelView, AppError> {
    state
        .model_service
        .create(CreateModelRequest {
            name: request.name,
            provider: request.provider,
            model_type: request.model_type,
            description: request.description,
            metadata: request.metadata,
        })
        .await
        .map_err(map_model_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_update(
    state: State<'_, AppState>,
    request: ModelUpdateRequest,
) -> Result<ModelView, AppError> {
    state
        .model_service
        .update(UpdateModelRequest {
            model_id: request.model_id,
            name: request.name,
            provider: request.provider,
            model_type: request.model_type,
            description: request.description,
            metadata: request.metadata,
        })
        .await
        .map_err(map_model_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_delete(state: State<'_, AppState>, model_id: String) -> Result<(), AppError> {
    state
        .model_service
        .delete(&model_id)
        .await
        .map_err(map_model_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_version_list(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<Vec<ModelVersionView>, AppError> {
    state
        .model_service
        .list_versions(&model_id)
        .await
        .map_err(map_model_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_version_current(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<Option<ModelVersionView>, AppError> {
    state
        .model_service
        .current_version(&model_id)
        .await
        .map_err(map_model_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_version_get(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<ModelVersionView, AppError> {
    state
        .model_service
        .get_version(&version_id)
        .await
        .map_err(map_model_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn model_version_create(
    state: State<'_, AppState>,
    request: ModelVersionCreateRequest,
) -> Result<ModelVersionView, AppError> {
    state
        .model_service
        .create_version(CreateModelVersionRequest {
            model_id: request.model_id,
            version: request.version,
            capabilities: request.capabilities,
            parameter_schema: request.parameter_schema,
        })
        .await
        .map_err(map_model_error)
}

fn map_model_error(error: ModelServiceError) -> AppError {
    match error {
        ModelServiceError::InvalidInput(_)
        | ModelServiceError::NotFound(_)
        | ModelServiceError::Domain(_) => AppError::invalid_input(error.to_string()),
        ModelServiceError::Repository(repository) => super::map_repository_error(&repository),
    }
}
