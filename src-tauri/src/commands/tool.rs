use crate::app_state::AppState;
use crate::application::tool_service::{
    CapabilityView, CreateCapabilityRequest, CreateToolInstanceRequest, CreateToolRequest,
    CreateToolVersionRequest, ToolInstanceView, ToolServiceError, ToolVersionView, ToolView,
    UpdateToolRequest,
};
use crate::domain::ToolHealthStatus;
use crate::error::AppError;
use serde::Deserialize;
use serde_json::Value;
use tauri::State;

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCreateRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUpdateRequest {
    pub tool_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInstanceCreateRequest {
    pub tool_id: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolHealthRequest {
    pub instance_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersionCreateRequest {
    pub tool_id: String,
    pub version: String,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilityCreateRequest {
    pub tool_id: String,
    pub capability_name: String,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_list(state: State<'_, AppState>) -> Result<Vec<ToolView>, AppError> {
    state.tool_service.list().await.map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_get(state: State<'_, AppState>, tool_id: String) -> Result<ToolView, AppError> {
    state
        .tool_service
        .get(&tool_id)
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_create(
    state: State<'_, AppState>,
    request: ToolCreateRequest,
) -> Result<ToolView, AppError> {
    state
        .tool_service
        .create(CreateToolRequest {
            name: request.name,
            tool_type: request.tool_type,
            description: request.description,
            metadata: request.metadata,
        })
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_update(
    state: State<'_, AppState>,
    request: ToolUpdateRequest,
) -> Result<ToolView, AppError> {
    state
        .tool_service
        .update(UpdateToolRequest {
            tool_id: request.tool_id,
            name: request.name,
            tool_type: request.tool_type,
            description: request.description,
            metadata: request.metadata,
        })
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_delete(state: State<'_, AppState>, tool_id: String) -> Result<(), AppError> {
    state
        .tool_service
        .delete(&tool_id)
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_instance_list(
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<Vec<ToolInstanceView>, AppError> {
    state
        .tool_service
        .list_instances(&tool_id)
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_instance_get(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<ToolInstanceView, AppError> {
    state
        .tool_service
        .get_instance(&instance_id)
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_instance_create(
    state: State<'_, AppState>,
    request: ToolInstanceCreateRequest,
) -> Result<ToolInstanceView, AppError> {
    state
        .tool_service
        .create_instance(CreateToolInstanceRequest {
            tool_id: request.tool_id,
            path: request.path,
            endpoint: request.endpoint,
        })
        .await
        .map_err(map_tool_error)
}

/// Records a caller-supplied state only; it does not probe, start, or stop a process.
#[tauri::command(rename_all = "camelCase")]
pub async fn tool_instance_record_health(
    state: State<'_, AppState>,
    request: ToolHealthRequest,
) -> Result<ToolInstanceView, AppError> {
    let status = ToolHealthStatus::try_from_db(request.status.trim())
        .map_err(|error| AppError::invalid_input(error.to_string()))?;
    state
        .tool_service
        .record_health(&request.instance_id, status)
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_version_list(
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<Vec<ToolVersionView>, AppError> {
    state
        .tool_service
        .list_versions(&tool_id)
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_version_get(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<ToolVersionView, AppError> {
    state
        .tool_service
        .get_version(&version_id)
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_version_create(
    state: State<'_, AppState>,
    request: ToolVersionCreateRequest,
) -> Result<ToolVersionView, AppError> {
    state
        .tool_service
        .create_version(CreateToolVersionRequest {
            tool_id: request.tool_id,
            version: request.version,
            metadata: request.metadata,
        })
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_capability_list(
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<Vec<CapabilityView>, AppError> {
    state
        .tool_service
        .list_capabilities(&tool_id)
        .await
        .map_err(map_tool_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_capability_create(
    state: State<'_, AppState>,
    request: ToolCapabilityCreateRequest,
) -> Result<CapabilityView, AppError> {
    state
        .tool_service
        .create_capability(CreateCapabilityRequest {
            tool_id: request.tool_id,
            capability_name: request.capability_name,
            metadata: request.metadata,
        })
        .await
        .map_err(map_tool_error)
}

fn map_tool_error(error: ToolServiceError) -> AppError {
    match error {
        ToolServiceError::InvalidInput(_)
        | ToolServiceError::NotFound(_)
        | ToolServiceError::Domain(_) => AppError::invalid_input(error.to_string()),
        ToolServiceError::Repository(repository) => super::map_repository_error(&repository),
    }
}
