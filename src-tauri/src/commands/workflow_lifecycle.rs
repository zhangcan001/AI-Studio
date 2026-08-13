use crate::{
    app_state::AppState,
    application::workflow_lifecycle_service::{
        WorkflowExportView, WorkflowLifecycleError, WorkflowProductionWorkspaceResponse,
        WorkflowRestoreView, WorkflowVersionDiffView, MAX_WORKFLOW_ARCHIVE_BYTES,
    },
    error::AppError,
};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

fn map_error(error: WorkflowLifecycleError) -> AppError {
    AppError::workflow_onboarding(format!("{}: {}", error.code(), error))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_runtime_workspace_list(
    state: State<'_, AppState>,
) -> Result<WorkflowProductionWorkspaceResponse, AppError> {
    state
        .workflow_lifecycle_service
        .list_workspace()
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_runtime_workspace_refresh(
    state: State<'_, AppState>,
) -> Result<WorkflowProductionWorkspaceResponse, AppError> {
    state
        .workflow_lifecycle_service
        .refresh_workspace()
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_runtime_diagnostics(
    state: State<'_, AppState>,
) -> Result<WorkflowProductionWorkspaceResponse, AppError> {
    state
        .workflow_lifecycle_service
        .list_workspace_diagnostics()
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_recheck_all_capabilities(
    state: State<'_, AppState>,
) -> Result<
    Vec<crate::application::workflow_lifecycle_service::WorkflowCapabilityBatchView>,
    AppError,
> {
    state
        .workflow_lifecycle_service
        .recheck_all_capabilities()
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_set_enabled(
    state: State<'_, AppState>,
    workflow_version_id: String,
    enabled: bool,
) -> Result<(), AppError> {
    state
        .workflow_lifecycle_service
        .set_enabled(&workflow_version_id, enabled)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_recheck_capability(
    state: State<'_, AppState>,
    workflow_version_id: String,
) -> Result<crate::application::workflow_onboarding_service::CapabilityCheckView, AppError> {
    state
        .workflow_lifecycle_service
        .recheck_capability(&workflow_version_id)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_duplicate_recipe(
    state: State<'_, AppState>,
    workflow_version_id: String,
    recipe_id: Option<String>,
    recipe_version: Option<String>,
) -> Result<crate::application::workflow_onboarding_service::WorkflowOnboardingDraftView, AppError>
{
    state
        .workflow_lifecycle_service
        .duplicate_recipe(&workflow_version_id, recipe_id, recipe_version)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_compare_versions(
    state: State<'_, AppState>,
    version_a_id: String,
    version_b_id: String,
) -> Result<WorkflowVersionDiffView, AppError> {
    state
        .workflow_lifecycle_service
        .compare_versions(&version_a_id, &version_b_id)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_export_package(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    workflow_version_id: String,
) -> Result<Option<WorkflowExportView>, AppError> {
    let export = state
        .workflow_lifecycle_service
        .export_package(&workflow_version_id)
        .await
        .map_err(map_error)?;
    let file_name = export.file_name.clone();
    let Some(file) = app_handle
        .dialog()
        .file()
        .add_filter("AI Studio Workflow Package", &["zip"])
        .set_file_name(&file_name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|_| AppError::filesystem("export destination is unavailable"))?;
    tokio::fs::write(path, &export.bytes)
        .await
        .map_err(|error| {
            AppError::filesystem(format!("workflow package export failed: {error}"))
        })?;
    Ok(Some(export))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_import_package_backup(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<WorkflowRestoreView>, AppError> {
    let Some(file) = app_handle
        .dialog()
        .file()
        .add_filter("AI Studio Workflow Package", &["zip"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|_| AppError::filesystem("workflow package source is unavailable"))?;
    let is_zip = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
    if !is_zip {
        return Err(AppError::workflow_onboarding(
            "PACKAGE_ARCHIVE_INVALID: select a .zip AI Studio workflow package",
        ));
    }
    let size = tokio::fs::metadata(&path)
        .await
        .map_err(|_| AppError::filesystem("workflow package source could not be inspected"))?
        .len();
    if size > MAX_WORKFLOW_ARCHIVE_BYTES as u64 {
        return Err(AppError::workflow_onboarding(
            "PACKAGE_ARCHIVE_TOO_LARGE: archive exceeds the 64 MiB compressed limit",
        ));
    }
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        AppError::filesystem(format!(
            "workflow package source could not be read: {error}"
        ))
    })?;
    state
        .workflow_lifecycle_service
        .restore_package(bytes)
        .await
        .map(Some)
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_clean_staging(
    state: State<'_, AppState>,
    staging_id: String,
) -> Result<(), AppError> {
    state
        .workflow_lifecycle_service
        .cleanup_staging(&staging_id)
        .await
        .map_err(map_error)
}
