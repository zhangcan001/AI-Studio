use crate::{
    app_state::AppState,
    application::{
        workflow_lifecycle_coordinator::WorkflowLifecycleCoordinatorError,
        workflow_onboarding_service::{
            WorkflowAutoOnboardingPlanView, WorkflowImportCommitRequest,
            WorkflowOnboardingPublishView,
        },
        workflow_registry_service::{
            WorkflowPurgeInspection, WorkflowRegistryMutationResult, WorkflowRegistryPurgeResult,
            WorkflowRegistryRestoreResult, WorkflowRegistryServiceError, WorkflowRegistryView,
        },
    },
    error::AppError,
};
use tauri::{AppHandle, State};

pub(super) fn map_registry_error(error: WorkflowRegistryServiceError) -> AppError {
    let code = error.code();
    match error {
        WorkflowRegistryServiceError::WorkflowNotFound(message)
        | WorkflowRegistryServiceError::VersionNotFound {
            workflow_version_id: message,
        } => AppError::invalid_input(format!("{code}: {message}")),
        WorkflowRegistryServiceError::Blocked(message)
        | WorkflowRegistryServiceError::NotRemoved(message)
        | WorkflowRegistryServiceError::PurgeBlocked(message)
        | WorkflowRegistryServiceError::PurgePackage(message)
        | WorkflowRegistryServiceError::CompensationFailed {
            operation: message, ..
        } => AppError::invalid_input(format!("{code}: {message}")),
        WorkflowRegistryServiceError::PurgeRecoveryBlocked {
            operation: message, ..
        } => AppError::invalid_input(format!("{code}: {message}")),
        WorkflowRegistryServiceError::PurgeCompensationFailed {
            operation: message, ..
        } => AppError::invalid_input(format!("{code}: {message}")),
        WorkflowRegistryServiceError::Repository(error) => super::map_repository_error(&error),
    }
}

fn map_coordinator_error(error: WorkflowLifecycleCoordinatorError) -> AppError {
    match error {
        WorkflowLifecycleCoordinatorError::Registry(error) => map_registry_error(error),
        WorkflowLifecycleCoordinatorError::Lifecycle(error) => {
            AppError::invalid_input(error.to_string())
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_analyze_import(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    existing_workflow_id: Option<String>,
) -> Result<Option<WorkflowAutoOnboardingPlanView>, AppError> {
    let Some((bytes, original_filename)) =
        super::workflow_onboarding::pick_api_workflow_file(&app_handle).await?
    else {
        return Ok(None);
    };
    state
        .workflow_onboarding_service
        .analyze_import_bytes(bytes, original_filename, existing_workflow_id)
        .await
        .map(Some)
        .map_err(|error| AppError::workflow_onboarding(format!("{}: {error}", error.code())))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_commit_import(
    state: State<'_, AppState>,
    request: WorkflowImportCommitRequest,
) -> Result<WorkflowOnboardingPublishView, AppError> {
    let set_current = request.set_current;
    let published = state
        .workflow_onboarding_service
        .commit_import(request)
        .await
        .map_err(|error| AppError::workflow_onboarding(format!("{}: {error}", error.code())))?;
    if set_current {
        if let Some(version_id) = &published.workflow_version_id {
            state
                .workflow_registry_service
                .set_current_version(&published.workflow_id, version_id)
                .await
                .map_err(map_registry_error)?;
        }
    }
    Ok(published)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_list_registry(
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowRegistryView>, AppError> {
    state
        .workflow_registry_service
        .list()
        .await
        .map_err(map_registry_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_get_registry(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<WorkflowRegistryView, AppError> {
    state
        .workflow_registry_service
        .get(&workflow_id)
        .await
        .map_err(map_registry_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_rename(
    state: State<'_, AppState>,
    workflow_id: String,
    name: String,
) -> Result<WorkflowRegistryView, AppError> {
    state
        .workflow_registry_service
        .rename(&workflow_id, &name)
        .await
        .map_err(map_registry_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_set_current_version(
    state: State<'_, AppState>,
    workflow_id: String,
    workflow_version_id: String,
) -> Result<WorkflowRegistryView, AppError> {
    state
        .workflow_registry_service
        .set_current_version(&workflow_id, &workflow_version_id)
        .await
        .map_err(map_registry_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_remove(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<WorkflowRegistryMutationResult, AppError> {
    state
        .workflow_lifecycle_coordinator
        .remove_workflow(&workflow_id)
        .await
        .map_err(map_coordinator_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_restore(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<WorkflowRegistryRestoreResult, AppError> {
    state
        .workflow_lifecycle_coordinator
        .restore_workflow(&workflow_id)
        .await
        .map_err(map_coordinator_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_purge(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<WorkflowRegistryPurgeResult, AppError> {
    state
        .workflow_lifecycle_coordinator
        .purge_workflow(&workflow_id)
        .await
        .map_err(map_coordinator_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_inspect_purge(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<WorkflowPurgeInspection, AppError> {
    state
        .workflow_registry_service
        .inspect_purge(&workflow_id)
        .await
        .map_err(map_registry_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_rerecognize(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<WorkflowAutoOnboardingPlanView, AppError> {
    state
        .workflow_onboarding_service
        .rerecognize_workflow(&workflow_id)
        .await
        .map_err(|error| AppError::workflow_onboarding(format!("{}: {error}", error.code())))
}
