use crate::app_state::AppState;
use crate::application::prompt_library_service::{
    PromptEntryView, PromptLibraryError, PromptVersionView,
};
use crate::error::AppError;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptLibraryCreateRequest {
    pub project_id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptLibraryVersionRequest {
    pub project_id: String,
    pub prompt_id: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptLibraryMetadataRequest {
    pub project_id: String,
    pub prompt_id: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_library_list(
    state: State<'_, AppState>,
    project_id: String,
    kind: Option<String>,
    keyword: Option<String>,
    tag: Option<String>,
) -> Result<Vec<PromptEntryView>, AppError> {
    state
        .prompt_library_service
        .list(
            &project_id,
            kind.as_deref(),
            keyword.as_deref(),
            tag.as_deref(),
        )
        .await
        .map_err(map_prompt_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_library_get(
    state: State<'_, AppState>,
    project_id: String,
    prompt_id: String,
) -> Result<PromptEntryView, AppError> {
    state
        .prompt_library_service
        .get(&project_id, &prompt_id)
        .await
        .map_err(map_prompt_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_library_create(
    state: State<'_, AppState>,
    request: PromptLibraryCreateRequest,
) -> Result<PromptEntryView, AppError> {
    state
        .prompt_library_service
        .create(
            &request.project_id,
            &request.kind,
            &request.name,
            &request.tags,
            &request.text,
        )
        .await
        .map_err(map_prompt_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_library_add_version(
    state: State<'_, AppState>,
    request: PromptLibraryVersionRequest,
) -> Result<PromptVersionView, AppError> {
    state
        .prompt_library_service
        .add_version(&request.project_id, &request.prompt_id, &request.text)
        .await
        .map_err(map_prompt_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_library_update_metadata(
    state: State<'_, AppState>,
    request: PromptLibraryMetadataRequest,
) -> Result<PromptEntryView, AppError> {
    state
        .prompt_library_service
        .update_metadata(
            &request.project_id,
            &request.prompt_id,
            &request.name,
            &request.tags,
        )
        .await
        .map_err(map_prompt_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_library_delete(
    state: State<'_, AppState>,
    project_id: String,
    prompt_id: String,
) -> Result<(), AppError> {
    state
        .prompt_library_service
        .delete(&project_id, &prompt_id)
        .await
        .map_err(map_prompt_error)
}

fn map_prompt_error(error: PromptLibraryError) -> AppError {
    match error {
        PromptLibraryError::InvalidInput(message) | PromptLibraryError::NotFound(message) => {
            AppError::invalid_input(message)
        }
        PromptLibraryError::Repository(error) => super::map_repository_error(&error),
    }
}
