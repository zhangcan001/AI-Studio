use crate::{
    app_state::AppState,
    application::reference_anchor_service::{
        CreateReferenceAnchorRequest, ReferenceAnchorError, ReferenceAnchorView,
        UpdateReferenceAnchorRequest,
    },
    domain::ReferenceAnchorKind,
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceAnchorRequest {
    pub project_id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub asset_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceAnchorUpdateRequest {
    pub project_id: String,
    pub anchor_id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub asset_ids: Vec<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_anchors_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ReferenceAnchorView>, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .reference_anchor_service
        .list(&project_id)
        .await
        .map_err(map_reference_anchor_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_anchor_get(
    state: State<'_, AppState>,
    project_id: String,
    anchor_id: String,
) -> Result<ReferenceAnchorView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .reference_anchor_service
        .get(&project_id, &anchor_id)
        .await
        .map_err(map_reference_anchor_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_anchor_create(
    state: State<'_, AppState>,
    request: ReferenceAnchorRequest,
) -> Result<ReferenceAnchorView, AppError> {
    super::validate_project_id(&request.project_id)?;
    let kind = parse_kind(&request.kind)?;
    state
        .reference_anchor_service
        .create(CreateReferenceAnchorRequest {
            project_id: request.project_id,
            kind,
            name: request.name,
            description: request.description,
            asset_ids: request.asset_ids,
        })
        .await
        .map_err(map_reference_anchor_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_anchor_update(
    state: State<'_, AppState>,
    request: ReferenceAnchorUpdateRequest,
) -> Result<ReferenceAnchorView, AppError> {
    super::validate_project_id(&request.project_id)?;
    let kind = parse_kind(&request.kind)?;
    state
        .reference_anchor_service
        .update(UpdateReferenceAnchorRequest {
            project_id: request.project_id,
            anchor_id: request.anchor_id,
            kind,
            name: request.name,
            description: request.description,
            asset_ids: request.asset_ids,
        })
        .await
        .map_err(map_reference_anchor_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_anchor_delete(
    state: State<'_, AppState>,
    project_id: String,
    anchor_id: String,
) -> Result<(), AppError> {
    super::validate_project_id(&project_id)?;
    state
        .reference_anchor_service
        .delete(&project_id, &anchor_id)
        .await
        .map_err(map_reference_anchor_error)
}

fn parse_kind(value: &str) -> Result<ReferenceAnchorKind, AppError> {
    ReferenceAnchorKind::try_from_db(value.trim())
        .map_err(|error| AppError::invalid_input(format!("REFERENCE_ANCHOR_KIND_INVALID: {error}")))
}

fn map_reference_anchor_error(error: ReferenceAnchorError) -> AppError {
    match error {
        ReferenceAnchorError::InvalidInput(message) => AppError::invalid_input(message),
        ReferenceAnchorError::AssetProjectMismatch { .. }
        | ReferenceAnchorError::ImageRequired(_)
        | ReferenceAnchorError::AssetNotFound(_) => AppError::invalid_input(error.to_string()),
        ReferenceAnchorError::NotFound(message) => AppError::database(message),
        ReferenceAnchorError::Repository(repository) => super::map_repository_error(&repository),
    }
}
