use crate::{
    app_state::AppState,
    application::{
        workflow_onboarding_service::CapabilityState,
        workflow_onboarding_service::{
            WorkflowAutoOnboardingPlanView, WorkflowImportCommitRequest,
            WorkflowOnboardingPublishView,
        },
        workflow_registry_service::{
            WorkflowRegistryMutationResult, WorkflowRegistryPurgeResult,
            WorkflowRegistryRestoreResult, WorkflowRegistryServiceError, WorkflowRegistryView,
        },
    },
    error::AppError,
};
use tauri::{AppHandle, State};

fn map_registry_error(error: WorkflowRegistryServiceError) -> AppError {
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
        | WorkflowRegistryServiceError::PurgeCleanupFailed(message)
        | WorkflowRegistryServiceError::CompensationFailed {
            operation: message, ..
        } => AppError::invalid_input(format!("{code}: {message}")),
        WorkflowRegistryServiceError::PurgeCompensationFailed {
            operation: message, ..
        } => AppError::invalid_input(format!("{code}: {message}")),
        WorkflowRegistryServiceError::Repository(error) => super::map_repository_error(&error),
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
        .workflow_registry_service
        .remove(&workflow_id)
        .await
        .map_err(map_registry_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_restore(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<WorkflowRegistryRestoreResult, AppError> {
    let mut restored = state
        .workflow_registry_service
        .restore(&workflow_id)
        .await
        .map_err(map_registry_error)?;

    if let Some(version_id) = restored.current_version_id.clone() {
        match state
            .workflow_lifecycle_service
            .restore_version(&version_id)
            .await
        {
            Ok(version_restore) => {
                restored.enabled = version_restore.enabled;
                restored.capability = version_restore.capability;
                restored.readiness = version_restore.readiness;
                return Ok(restored);
            }
            Err(error) if error.code() == "WORKFLOW_NOT_ARCHIVED" => {}
            Err(error) => return Err(AppError::invalid_input(error.to_string())),
        }

        match state
            .workflow_lifecycle_service
            .recheck_capability(&version_id)
            .await
        {
            Ok(capability) => {
                restored.capability = capability_state_name(capability.state).to_owned();
                if capability.state == CapabilityState::Ready {
                    state
                        .workflow_lifecycle_service
                        .set_enabled(&version_id, true)
                        .await
                        .map_err(|error| AppError::invalid_input(error.to_string()))?;
                    restored.enabled = true;
                    restored.readiness = "ACTIVE".to_owned();
                } else {
                    state
                        .workflow_lifecycle_service
                        .set_enabled(&version_id, false)
                        .await
                        .map_err(|error| AppError::invalid_input(error.to_string()))?;
                    restored.enabled = false;
                    restored.readiness = "RESTORED_NEEDS_ATTENTION".to_owned();
                }
            }
            Err(error) => {
                let capability = match error.code() {
                    "COMFY_OFFLINE" => "COMFY_OFFLINE",
                    "MISSING_NODES" | "MISSING_NODE" => "MISSING_NODES",
                    "INCOMPATIBLE_INPUT_VALUES" | "COMFY_PROTOCOL_ERROR" => {
                        "INCOMPATIBLE_INPUT_VALUES"
                    }
                    _ => "NOT_CHECKED",
                };
                state
                    .workflow_lifecycle_service
                    .set_enabled(&version_id, false)
                    .await
                    .map_err(|set_error| AppError::invalid_input(set_error.to_string()))?;
                restored.enabled = false;
                restored.capability = capability.to_owned();
                restored.readiness = "RESTORED_NEEDS_ATTENTION".to_owned();
            }
        }
    }

    Ok(restored)
}

fn capability_state_name(state: CapabilityState) -> &'static str {
    match state {
        CapabilityState::NotChecked => "NOT_CHECKED",
        CapabilityState::Ready => "READY",
        CapabilityState::MissingNodes => "MISSING_NODES",
        CapabilityState::IncompatibleInputValues => "INCOMPATIBLE_INPUT_VALUES",
        CapabilityState::ComfyOffline => "COMFY_OFFLINE",
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_purge(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<WorkflowRegistryPurgeResult, AppError> {
    state
        .workflow_registry_service
        .purge(&workflow_id)
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
