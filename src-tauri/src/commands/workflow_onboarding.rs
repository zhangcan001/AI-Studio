use crate::{
    app_state::AppState,
    application::workflow_onboarding_service::{
        WorkflowAutoOnboardingPlanView, WorkflowOnboardingDraftView, WorkflowOnboardingError,
        WorkflowOnboardingInputMappingRequest, WorkflowOnboardingMetadataRequest,
        WorkflowOnboardingOutputMappingRequest, WorkflowOnboardingPublishView,
        WorkflowOnboardingRemoveInputMappingRequest, WorkflowOnboardingValidationView,
        WorkflowWorkspaceView, MAX_WORKFLOW_IMPORT_BYTES,
    },
    error::AppError,
};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

fn map_onboarding_error(error: WorkflowOnboardingError) -> AppError {
    AppError::workflow_onboarding(format!("{}: {}", error.code(), error))
}

pub(crate) async fn pick_api_workflow_file(
    app_handle: &AppHandle,
) -> Result<Option<(Vec<u8>, String)>, AppError> {
    let Some(file) = app_handle
        .dialog()
        .file()
        .add_filter("ComfyUI Workflow JSON", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };

    let path = file
        .into_path()
        .map_err(|_| AppError::filesystem("selected workflow file is unavailable"))?;
    let is_json = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
    if !is_json {
        return Err(AppError::workflow_onboarding(
            "WORKFLOW_FILE_TYPE: select a .json ComfyUI workflow",
        ));
    }
    let original_filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("workflow.json")
        .to_owned();
    let file_size = tokio::fs::metadata(&path)
        .await
        .map_err(|_| AppError::filesystem("selected workflow file could not be inspected"))?
        .len();
    if file_size > MAX_WORKFLOW_IMPORT_BYTES {
        return Err(AppError::workflow_onboarding(format!(
            "WORKFLOW_FILE_TOO_LARGE: workflow import is {file_size} bytes; maximum is {MAX_WORKFLOW_IMPORT_BYTES} bytes"
        )));
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::filesystem("selected workflow file could not be read"))?;
    Ok(Some((bytes, original_filename)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_onboarding_pick_api_workflow(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    existing_workflow_id: Option<String>,
) -> Result<Option<WorkflowOnboardingDraftView>, AppError> {
    let Some((bytes, original_filename)) = pick_api_workflow_file(&app_handle).await? else {
        return Ok(None);
    };
    state
        .workflow_onboarding_service
        .import_bytes(bytes, original_filename, existing_workflow_id)
        .await
        .map(Some)
        .map_err(map_onboarding_error)
}

/// INTERNAL_ADVANCED_EDITING_ONLY. Formal import uses analyze followed by explicit commit.
#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_onboarding_auto_import_api_workflow(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    existing_workflow_id: Option<String>,
) -> Result<Option<WorkflowAutoOnboardingPlanView>, AppError> {
    let Some((bytes, original_filename)) = pick_api_workflow_file(&app_handle).await? else {
        return Ok(None);
    };
    state
        .workflow_onboarding_service
        .auto_onboard_bytes(bytes, original_filename, existing_workflow_id)
        .await
        .map(Some)
        .map_err(map_onboarding_error)
}

/// INTERNAL_ADVANCED_EDITING_ONLY. Formal import uses analyze followed by explicit commit.
#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_onboarding_auto_confirm(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<WorkflowAutoOnboardingPlanView, AppError> {
    state
        .workflow_onboarding_service
        .auto_confirm(&draft_id)
        .await
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_onboarding_regenerate_recipe(
    state: State<'_, AppState>,
    workflow_id: String,
    workflow_version: String,
    source_recipe_version: Option<String>,
) -> Result<WorkflowAutoOnboardingPlanView, AppError> {
    state
        .workflow_onboarding_service
        .regenerate_recipe_draft(
            &workflow_id,
            &workflow_version,
            source_recipe_version.as_deref(),
        )
        .await
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub fn workflow_onboarding_get(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<WorkflowOnboardingDraftView, AppError> {
    state
        .workflow_onboarding_service
        .get(&draft_id)
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_onboarding_check_capability(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<crate::application::workflow_onboarding_service::CapabilityCheckView, AppError> {
    state
        .workflow_onboarding_service
        .check_capability(&draft_id)
        .await
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub fn workflow_onboarding_set_metadata(
    state: State<'_, AppState>,
    draft_id: String,
    request: WorkflowOnboardingMetadataRequest,
) -> Result<WorkflowOnboardingDraftView, AppError> {
    state
        .workflow_onboarding_service
        .set_metadata(&draft_id, request)
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub fn workflow_onboarding_set_input_mapping(
    state: State<'_, AppState>,
    draft_id: String,
    request: WorkflowOnboardingInputMappingRequest,
) -> Result<WorkflowOnboardingDraftView, AppError> {
    state
        .workflow_onboarding_service
        .set_input_mapping(&draft_id, request)
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub fn workflow_onboarding_remove_input_mapping(
    state: State<'_, AppState>,
    draft_id: String,
    request: WorkflowOnboardingRemoveInputMappingRequest,
) -> Result<WorkflowOnboardingDraftView, AppError> {
    state
        .workflow_onboarding_service
        .remove_input_mapping(&draft_id, request)
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub fn workflow_onboarding_set_output_mapping(
    state: State<'_, AppState>,
    draft_id: String,
    request: WorkflowOnboardingOutputMappingRequest,
) -> Result<WorkflowOnboardingDraftView, AppError> {
    state
        .workflow_onboarding_service
        .set_output_mapping(&draft_id, request)
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub fn workflow_onboarding_validate(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<WorkflowOnboardingValidationView, AppError> {
    state
        .workflow_onboarding_service
        .validate(&draft_id)
        .map_err(map_onboarding_error)
}

/// INTERNAL_ADVANCED_EDITING_ONLY. Formal import uses analyze followed by explicit commit.
#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_onboarding_publish(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<WorkflowOnboardingPublishView, AppError> {
    state
        .workflow_onboarding_service
        .publish(&draft_id)
        .await
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub fn workflow_onboarding_discard(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<(), AppError> {
    state
        .workflow_onboarding_service
        .discard(&draft_id)
        .map_err(map_onboarding_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_workspace_list(
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowWorkspaceView>, AppError> {
    state
        .workflow_onboarding_service
        .list_workspace()
        .await
        .map_err(map_onboarding_error)
}
