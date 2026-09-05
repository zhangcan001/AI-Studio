use crate::application::{
    workflow_lifecycle_service::{
        WorkflowDeletionResult, WorkflowLifecycleError, WorkflowLifecycleService,
        WorkflowRestoreResult,
    },
    workflow_onboarding_service::CapabilityState,
    workflow_registry_service::{
        WorkflowRegistryMutationResult, WorkflowRegistryPurgeResult, WorkflowRegistryRestoreResult,
        WorkflowRegistryService, WorkflowRegistryServiceError,
    },
};
use std::{error::Error, fmt, sync::Arc};
use tokio::sync::Mutex;

pub struct WorkflowLifecycleCoordinator {
    registry: Arc<WorkflowRegistryService>,
    lifecycle: Arc<WorkflowLifecycleService>,
    gate: Arc<Mutex<()>>,
}

impl WorkflowLifecycleCoordinator {
    pub fn new(
        registry: Arc<WorkflowRegistryService>,
        lifecycle: Arc<WorkflowLifecycleService>,
    ) -> Self {
        let gate = registry.lifecycle_gate();
        Self {
            registry,
            lifecycle,
            gate,
        }
    }

    pub async fn remove_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryMutationResult, WorkflowLifecycleCoordinatorError> {
        let _guard = self.gate.lock().await;
        self.registry
            .remove_workflow_inner(workflow_id)
            .await
            .map_err(Into::into)
    }

    pub async fn restore_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryRestoreResult, WorkflowLifecycleCoordinatorError> {
        let _guard = self.gate.lock().await;
        self.restore_workflow_inner(workflow_id).await
    }

    pub async fn purge_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryPurgeResult, WorkflowLifecycleCoordinatorError> {
        let _guard = self.gate.lock().await;
        self.registry
            .purge_workflow_inner(workflow_id)
            .await
            .map_err(Into::into)
    }

    pub async fn delete_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<WorkflowDeletionResult, WorkflowLifecycleCoordinatorError> {
        let _guard = self.gate.lock().await;
        let Some(workflow_id) = self
            .registry
            .registry_workflow_id_for_version(workflow_version_id)
            .await?
        else {
            return self
                .lifecycle
                .delete_version(workflow_version_id)
                .await
                .map_err(Into::into);
        };
        let removed = self.registry.remove_workflow_inner(&workflow_id).await?;
        Ok(WorkflowDeletionResult {
            action: "REMOVE".to_owned(),
            delete_action: "REMOVE".to_owned(),
            project_binding_count: removed.cleared_binding_count,
            workflow_id,
            workflow_version_id: workflow_version_id.to_owned(),
            archived: true,
        })
    }

    pub async fn delete_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<WorkflowDeletionResult>, WorkflowLifecycleCoordinatorError> {
        let _guard = self.gate.lock().await;
        if !self.registry.registry_contains(workflow_id).await? {
            return self
                .lifecycle
                .delete_workflow(workflow_id)
                .await
                .map_err(Into::into);
        }
        let versions = self.registry.get(workflow_id).await?.versions;
        let removed = self.registry.remove_workflow_inner(workflow_id).await?;
        let mut cleared_binding_count = removed.cleared_binding_count;
        Ok(versions
            .into_iter()
            .map(|version| {
                let project_binding_count = cleared_binding_count;
                cleared_binding_count = 0;
                WorkflowDeletionResult {
                    action: "REMOVE".to_owned(),
                    delete_action: "REMOVE".to_owned(),
                    project_binding_count,
                    workflow_id: workflow_id.to_owned(),
                    workflow_version_id: version.workflow_version_id,
                    archived: true,
                }
            })
            .collect())
    }

    pub async fn restore_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<WorkflowRestoreResult, WorkflowLifecycleCoordinatorError> {
        let _guard = self.gate.lock().await;
        let Some(workflow_id) = self
            .registry
            .registry_workflow_id_for_version(workflow_version_id)
            .await?
        else {
            return self
                .lifecycle
                .restore_version(workflow_version_id)
                .await
                .map_err(Into::into);
        };
        let restored = self.restore_workflow_inner(&workflow_id).await?;
        Ok(WorkflowRestoreResult {
            workflow_version_id: restored
                .current_version_id
                .unwrap_or_else(|| workflow_version_id.to_owned()),
            archived: false,
            enabled: restored.enabled,
            capability: restored.capability,
            readiness: restored.readiness,
        })
    }

    async fn restore_workflow_inner(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryRestoreResult, WorkflowLifecycleCoordinatorError> {
        let mut restored = self.registry.restore_workflow_inner(workflow_id).await?;
        let Some(version_id) = restored.current_version_id.clone() else {
            return Ok(restored);
        };
        match self.lifecycle.restore_version(&version_id).await {
            Ok(version_restore) => {
                restored.enabled = version_restore.enabled;
                restored.capability = version_restore.capability;
                restored.readiness = version_restore.readiness;
                return Ok(restored);
            }
            Err(error) if error.code() == "WORKFLOW_NOT_ARCHIVED" => {}
            Err(error) => return Err(error.into()),
        }

        match self.lifecycle.recheck_capability(&version_id).await {
            Ok(capability) => {
                restored.capability = capability_state_name(capability.state).to_owned();
                restored.enabled = capability.state == CapabilityState::Ready;
            }
            Err(error) => {
                restored.capability = match error.code() {
                    "COMFY_OFFLINE" => "COMFY_OFFLINE",
                    "MISSING_NODES" | "MISSING_NODE" => "MISSING_NODES",
                    "INCOMPATIBLE_INPUT_VALUES" | "COMFY_PROTOCOL_ERROR" => {
                        "INCOMPATIBLE_INPUT_VALUES"
                    }
                    _ => "NOT_CHECKED",
                }
                .to_owned();
                restored.enabled = false;
            }
        }
        self.lifecycle
            .set_enabled(&version_id, restored.enabled)
            .await?;
        restored.readiness = if restored.enabled {
            "ACTIVE"
        } else {
            "RESTORED_NEEDS_ATTENTION"
        }
        .to_owned();
        Ok(restored)
    }
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

#[derive(Debug)]
pub enum WorkflowLifecycleCoordinatorError {
    Registry(WorkflowRegistryServiceError),
    Lifecycle(WorkflowLifecycleError),
}

impl WorkflowLifecycleCoordinatorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Registry(error) => error.code(),
            Self::Lifecycle(error) => error.code(),
        }
    }
}

impl fmt::Display for WorkflowLifecycleCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registry(error) => error.fmt(formatter),
            Self::Lifecycle(error) => error.fmt(formatter),
        }
    }
}

impl Error for WorkflowLifecycleCoordinatorError {}

impl From<WorkflowRegistryServiceError> for WorkflowLifecycleCoordinatorError {
    fn from(error: WorkflowRegistryServiceError) -> Self {
        Self::Registry(error)
    }
}

impl From<WorkflowLifecycleError> for WorkflowLifecycleCoordinatorError {
    fn from(error: WorkflowLifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}
