use std::sync::Arc;

use serde::Serialize;

use crate::application::project_command_center_service::{
    ProjectCommandCenterError, ProjectCommandCenterNextAction, ProjectCommandCenterProjectView,
    ProjectCommandCenterReadinessView, ProjectCommandCenterService, ProjectCommandCenterView,
};

use super::error::{ProductError, ProductErrorDetails};
use super::selection_ref::ExactGeneratorSelection;
use crate::application::project_workflow_binding_service::{
    ProjectWorkflowBindingService, ProjectWorkflowBindingUpsertRequest, ProjectWorkflowConfigView,
};
use serde::Deserialize;

/// Read-only adapter: the command center remains the sole statistics authority.
pub struct ProductProjectFacade {
    command_center: Arc<ProjectCommandCenterService>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratorBindingSetRequest {
    pub stage: String,
    pub mode: String,
    pub selection_ref: String,
    pub expected_revision: Option<i64>,
    pub expected_binding_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorBindingSummary {
    pub stage: String,
    pub mode: String,
    pub selection_ref: String,
    pub revision: i64,
    pub binding_instance_id: String,
}

pub fn binding_summaries(
    config: ProjectWorkflowConfigView,
) -> Result<Vec<GeneratorBindingSummary>, ProductError> {
    config
        .image_default
        .into_iter()
        .chain(config.video_default)
        .chain(config.video_mode_overrides)
        .map(|binding| {
            Ok(GeneratorBindingSummary {
                selection_ref: ExactGeneratorSelection {
                    workflow_version_id: binding.workflow_version_id,
                    recipe_id: binding.recipe_id,
                }
                .encode()?,
                stage: binding.stage,
                mode: binding.mode,
                revision: binding.revision,
                binding_instance_id: binding.binding_instance_id,
            })
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOverview {
    pub generator_bindings: Vec<GeneratorBindingSummary>,
    pub blocking_state: crate::application::production_audit_service::ProductionAuditHealth,
    pub project: ProjectCommandCenterProjectView,
    pub progress: ProjectProgress,
    pub next_action: ProjectCommandCenterNextAction,
    pub blocking_issues: Vec<ProjectBlockingIssue>,
    pub active_runs: ActiveRunsSummary,
    pub recent_results: ResultsSummary,
    pub runtime_readiness: ProjectCommandCenterReadinessView,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBlockingIssue {
    pub severity: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRunsSummary {
    pub running_batches: usize,
    pub paused_batches: usize,
    pub active_tasks: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultsSummary {
    pub total: usize,
    pub images: usize,
    pub videos: usize,
}

impl From<ProjectCommandCenterView> for ProjectOverview {
    fn from(view: ProjectCommandCenterView) -> Self {
        Self {
            generator_bindings: Vec::new(),
            blocking_state: view.audit.health,
            project: view.project,
            progress: ProjectProgress {
                total: view.shots.total,
                completed: view.shots.completed,
                failed: view.shots.failed,
            },
            next_action: view.recommended_action,
            blocking_issues: view
                .issues
                .into_iter()
                .map(|issue| ProjectBlockingIssue {
                    severity: issue.severity,
                    title: issue.title,
                })
                .collect(),
            active_runs: ActiveRunsSummary {
                running_batches: view.queue.running_queues,
                paused_batches: view.queue.paused_queues,
                active_tasks: view.tasks_assets.active_task_count,
            },
            recent_results: ResultsSummary {
                total: view.tasks_assets.asset_count,
                images: view.tasks_assets.image_asset_count,
                videos: view.tasks_assets.video_asset_count,
            },
            runtime_readiness: view.readiness,
        }
    }
}

impl ProductProjectFacade {
    /// The existing service validates project, lifecycle, exact pair and OCC;
    /// decode alone never grants authorization. Conflict is never retried.
    pub async fn set_generator_binding(
        authority: &ProjectWorkflowBindingService,
        project_id: &str,
        request: GeneratorBindingSetRequest,
    ) -> Result<Vec<GeneratorBindingSummary>, ProductError> {
        let pair = ExactGeneratorSelection::decode(&request.selection_ref)?;
        let config = authority
            .upsert(
                project_id,
                ProjectWorkflowBindingUpsertRequest {
                    stage: request.stage,
                    mode: request.mode,
                    workflow_version_id: pair.workflow_version_id,
                    recipe_id: pair.recipe_id,
                    expected_revision: request.expected_revision,
                    expected_binding_instance_id: request.expected_binding_instance_id,
                },
            )
            .await
            .map_err(ProductError::binding)?;
        binding_summaries(config)
    }
    pub fn new(command_center: Arc<ProjectCommandCenterService>) -> Self {
        Self { command_center }
    }

    pub async fn get_overview(&self, project_id: &str) -> Result<ProjectOverview, ProductError> {
        self.command_center
            .get(project_id)
            .await
            .map(ProjectOverview::from)
            .map_err(|error| {
                let (code, message) = match &error {
                    ProjectCommandCenterError::NotFound(_) => {
                        ("PROJECT_NOT_FOUND", "项目不存在，请返回项目列表。")
                    }
                    ProjectCommandCenterError::InvalidInput(_) => {
                        ("INVALID_INPUT", "项目标识无效，请重新选择项目。")
                    }
                    ProjectCommandCenterError::Database(_)
                    | ProjectCommandCenterError::Audit(_) => {
                        ("PROJECT_READ_FAILED", "暂时无法读取项目，请稍后重试。")
                    }
                };
                ProductError {
                    code,
                    message,
                    details: ProductErrorDetails {
                        retryable: code == "PROJECT_READ_FAILED",
                        technical_details: Some(error.to_string()),
                        ..Default::default()
                    },
                }
            })
    }
}
