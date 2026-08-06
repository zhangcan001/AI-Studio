use crate::app_state::AppState;
use crate::application::preset_service::{PresetServiceError, PresetView};
use crate::commands::generation::InputValueDto;
use crate::error::AppError;
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetCreateRequest {
    pub project_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub name: String,
    pub values: BTreeMap<String, InputValueDto>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetUpdateRequest {
    pub project_id: String,
    pub preset_id: String,
    pub name: String,
    pub values: BTreeMap<String, InputValueDto>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn preset_list(
    state: State<'_, AppState>,
    project_id: String,
    workflow_version_id: String,
    recipe_id: String,
) -> Result<Vec<PresetView>, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .preset_service
        .list(&project_id, &workflow_version_id, &recipe_id)
        .await
        .map_err(map_preset_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn preset_create(
    state: State<'_, AppState>,
    request: PresetCreateRequest,
) -> Result<PresetView, AppError> {
    super::validate_project_id(&request.project_id)?;
    let values = into_application_values(request.values)?;
    state
        .preset_service
        .create(
            &request.project_id,
            &request.workflow_version_id,
            &request.recipe_id,
            &request.name,
            &values,
        )
        .await
        .map_err(map_preset_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn preset_update(
    state: State<'_, AppState>,
    request: PresetUpdateRequest,
) -> Result<PresetView, AppError> {
    super::validate_project_id(&request.project_id)?;
    let values = into_application_values(request.values)?;
    state
        .preset_service
        .update(
            &request.project_id,
            &request.preset_id,
            &request.name,
            &values,
        )
        .await
        .map_err(map_preset_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn preset_delete(
    state: State<'_, AppState>,
    project_id: String,
    preset_id: String,
) -> Result<(), AppError> {
    super::validate_project_id(&project_id)?;
    state
        .preset_service
        .delete(&project_id, &preset_id)
        .await
        .map_err(map_preset_error)
}

fn into_application_values(
    values: BTreeMap<String, InputValueDto>,
) -> Result<
    BTreeMap<String, crate::application::generation_input_preparer::GenerationInputValue>,
    AppError,
> {
    values
        .into_iter()
        .map(|(key, value)| Ok((key.clone(), value.into_application(&key)?)))
        .collect()
}

fn map_preset_error(error: PresetServiceError) -> AppError {
    match error {
        PresetServiceError::Repository(error) => match error {
            crate::application::ports::RepositoryError::PresetNameConflict { .. } => {
                AppError::invalid_input(error.to_string())
            }
            error => super::map_repository_error(&error),
        },
        PresetServiceError::InvalidRecipe(_)
        | PresetServiceError::ValuesInvalid(_)
        | PresetServiceError::InvalidProjectId
        | PresetServiceError::InvalidPresetId(_)
        | PresetServiceError::NameRequired
        | PresetServiceError::NameTooLong
        | PresetServiceError::NameConflict(_)
        | PresetServiceError::NotFound(_)
        | PresetServiceError::DefinitionNotFound { .. }
        | PresetServiceError::Domain(_) => AppError::invalid_input(error.to_string()),
    }
}
