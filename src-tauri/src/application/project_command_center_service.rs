//! Read-only project readiness aggregate.
//!
//! The command center is deliberately a view over existing tables and
//! services. It does not persist state, submit work to ComfyUI, or introduce
//! another queue, task history, audit stream, or workflow engine.

use crate::application::comfy_preflight_service::{
    ComfyPreflightReport, ComfyPreflightService, ComfyPreflightStatus,
};
use crate::application::comfy_service::{ComfyConnectionStatus, ComfyService, ComfyStatusView};
use crate::application::ports::{
    ProjectCommandCenterData, ProjectCommandCenterProjectRecord as ProjectRow,
    ProjectCommandCenterQueueItemRecord as QueueItemRow, ProjectCommandCenterRepository,
    RepositoryError,
};
use crate::application::production_audit_service::{
    ProductionAuditActivity, ProductionAuditHealth, ProductionAuditIssue, ProductionAuditService,
    ProductionAuditSummary,
};
use crate::application::production_queue_service::{
    build_retry_lineages_from_edges, RetryLineageEdge,
};
use crate::domain::validate_project_id;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};

const PROJECT_ACTION_PRIORITY_STRUCTURAL_BLOCKED: u8 = 1;
const PROJECT_ACTION_PRIORITY_COMFY_BLOCKED: u8 = 2;
const PROJECT_ACTION_PRIORITY_REVIEW_REQUIRED: u8 = 3;
const PROJECT_ACTION_PRIORITY_AUTO_RESUMABLE: u8 = 4;
const PROJECT_ACTION_PRIORITY_ACTIVE_PRODUCTION: u8 = 5;
const PROJECT_ACTION_PRIORITY_IMAGE_REVIEW: u8 = 6;
const PROJECT_ACTION_PRIORITY_VIDEO_REVIEW: u8 = 7;
const PROJECT_ACTION_PRIORITY_MISSING_CONFIG: u8 = 8;
const PROJECT_ACTION_PRIORITY_UNASSIGNED: u8 = 9;
const PROJECT_ACTION_PRIORITY_NO_SHOTS: u8 = 10;
const PROJECT_ACTION_PRIORITY_READY: u8 = 11;
const PROJECT_ACTION_PRIORITY_COMPLETE: u8 = 12;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterProjectView {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterStructureView {
    pub series_count: usize,
    pub episode_count: usize,
    pub scene_count: usize,
    pub assigned_shot_count: usize,
    pub unassigned_shot_count: usize,
    pub first_unassigned_shot_id: Option<String>,
    pub blocked: bool,
    pub scenes: Vec<ProjectCommandCenterSceneView>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterSceneView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub total: usize,
    pub completed: usize,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterShotView {
    pub total: usize,
    pub draft: usize,
    pub ready: usize,
    pub generating: usize,
    pub image_review: usize,
    pub image_selected: usize,
    pub video_review: usize,
    pub completed: usize,
    pub failed: usize,
    pub configured: usize,
    pub missing_config: usize,
    pub first_generating_shot_id: Option<String>,
    pub first_generating_task_id: Option<String>,
    pub first_image_review_shot_id: Option<String>,
    pub first_video_review_shot_id: Option<String>,
    pub first_missing_config_shot_id: Option<String>,
    pub first_ready_shot_id: Option<String>,
    pub first_completed_shot_id: Option<String>,
    pub first_completed_asset_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterQueueView {
    pub total_queues: usize,
    pub running_queues: usize,
    pub paused_queues: usize,
    pub completed_queues: usize,
    pub archived_queues: usize,
    pub total_items: usize,
    pub pending_items: usize,
    pub active_items: usize,
    pub succeeded_items: usize,
    pub failed_items: usize,
    pub cancelled_items: usize,
    pub skipped_items: usize,
    pub auto_resumable_items: usize,
    pub review_required_items: usize,
    pub first_active_batch_id: Option<String>,
    pub first_active_shot_id: Option<String>,
    pub first_active_task_id: Option<String>,
    pub first_auto_resumable_batch_id: Option<String>,
    pub first_auto_resumable_shot_id: Option<String>,
    pub first_auto_resumable_task_id: Option<String>,
    pub first_review_required_batch_id: Option<String>,
    pub first_review_required_shot_id: Option<String>,
    pub first_review_required_task_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterTaskAssetView {
    pub task_count: usize,
    pub active_task_count: usize,
    pub succeeded_task_count: usize,
    pub failed_task_count: usize,
    pub asset_count: usize,
    pub image_asset_count: usize,
    pub video_asset_count: usize,
    pub audio_asset_count: usize,
    pub other_asset_count: usize,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterConsistencyView {
    pub character_profiles: usize,
    pub scene_profiles: usize,
    pub prop_profiles: usize,
    pub style_profiles: usize,
    pub reference_sets: usize,
    pub shot_profile_bindings: usize,
    pub shot_reference_set_bindings: usize,
    pub scope_profile_bindings: usize,
    pub scope_reference_set_bindings: usize,
    pub consistency_in_use: bool,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterPreparationView {
    pub snapshot_count: usize,
    pub prepared_image_items: usize,
    pub prepared_video_items: usize,
    pub active_prepared_items: usize,
    pub latest_prepared_at: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterReferenceAnchorView {
    pub total: usize,
    pub usable: usize,
    pub character: usize,
    pub scene: usize,
    pub prop: usize,
    pub style: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterPromptTemplateView {
    pub id: String,
    pub name: String,
    pub version_count: usize,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterPromptTemplateSummary {
    pub total: usize,
    pub versions: usize,
    pub items: Vec<ProjectCommandCenterPromptTemplateView>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterReadinessView {
    pub status: Option<ComfyPreflightStatus>,
    pub connection: Option<String>,
    pub workflow_ready: usize,
    pub workflow_total: usize,
    pub runtime_busy: bool,
    pub active_task_count: usize,
    pub production_busy: bool,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterContentView {
    pub shots: usize,
    pub prompts: usize,
    pub assets: usize,
    pub scenes: usize,
    pub configured_shots: usize,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterProductionView {
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
    pub review_required: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterIssueView {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterQuickActionView {
    pub id: String,
    pub label: String,
    pub destination: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterComfyView {
    pub status: Option<ComfyStatusView>,
    pub preflight: Option<ComfyPreflightReport>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectCommandCenterActionKind {
    StructuralBlocked,
    ComfyBlocked,
    ReviewRequired,
    AutoResumable,
    ActiveProduction,
    ImageReview,
    VideoReview,
    MissingConfig,
    Unassigned,
    NoShots,
    Ready,
    Complete,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterNextAction {
    pub kind: ProjectCommandCenterActionKind,
    pub priority: u8,
    pub reason_code: String,
    pub reason: String,
    pub shot_id: Option<String>,
    pub batch_id: Option<String>,
    pub task_id: Option<String>,
    pub asset_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterView {
    pub project: ProjectCommandCenterProjectView,
    pub structure: ProjectCommandCenterStructureView,
    pub shots: ProjectCommandCenterShotView,
    pub queue: ProjectCommandCenterQueueView,
    pub tasks_assets: ProjectCommandCenterTaskAssetView,
    pub consistency: ProjectCommandCenterConsistencyView,
    pub preparation: ProjectCommandCenterPreparationView,
    pub reference_anchors: ProjectCommandCenterReferenceAnchorView,
    pub prompt_templates: ProjectCommandCenterPromptTemplateSummary,
    pub comfy: ProjectCommandCenterComfyView,
    pub readiness: ProjectCommandCenterReadinessView,
    pub content: ProjectCommandCenterContentView,
    pub production: ProjectCommandCenterProductionView,
    pub issues: Vec<ProjectCommandCenterIssueView>,
    pub audit: ProductionAuditSummary,
    pub recent_activity: Vec<ProductionAuditActivity>,
    pub recommended_action: ProjectCommandCenterNextAction,
    pub quick_actions: Vec<ProjectCommandCenterQuickActionView>,
    pub checked_at: String,
}

#[derive(Debug)]
pub enum ProjectCommandCenterError {
    InvalidInput(String),
    NotFound(String),
    Database(String),
    Audit(String),
}

impl fmt::Display for ProjectCommandCenterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) | Self::Audit(message) => {
                formatter.write_str(message)
            }
            Self::Database(message) => {
                write!(
                    formatter,
                    "project command center database error: {message}"
                )
            }
        }
    }
}

impl Error for ProjectCommandCenterError {}

pub struct ProjectCommandCenterService {
    repository: Arc<dyn ProjectCommandCenterRepository>,
    audit_service: Arc<ProductionAuditService>,
    comfy_service: Option<Arc<ComfyService>>,
    comfy_preflight_service: Option<Arc<ComfyPreflightService>>,
}

fn map_repository_error(error: RepositoryError) -> ProjectCommandCenterError {
    match error {
        RepositoryError::NotFound { entity, id } if entity == "project" => {
            ProjectCommandCenterError::NotFound(format!(
                "PROJECT_NOT_FOUND: project {id} was not found"
            ))
        }
        RepositoryError::Database { message } => ProjectCommandCenterError::Database(message),
        other => ProjectCommandCenterError::Database(other.to_string()),
    }
}

impl ProjectCommandCenterService {
    pub fn new(
        repository: Arc<dyn ProjectCommandCenterRepository>,
        audit_service: Arc<ProductionAuditService>,
    ) -> Self {
        Self {
            repository,
            audit_service,
            comfy_service: None,
            comfy_preflight_service: None,
        }
    }

    pub fn with_audit_service(mut self, service: Arc<ProductionAuditService>) -> Self {
        self.audit_service = service;
        self
    }

    pub fn with_comfy_cache_services(
        mut self,
        comfy_service: Arc<ComfyService>,
        comfy_preflight_service: Arc<ComfyPreflightService>,
    ) -> Self {
        self.comfy_service = Some(comfy_service);
        self.comfy_preflight_service = Some(comfy_preflight_service);
        self
    }

    pub async fn get(
        &self,
        project_id: &str,
    ) -> Result<ProjectCommandCenterView, ProjectCommandCenterError> {
        validate_project_id(project_id)
            .map_err(|error| ProjectCommandCenterError::InvalidInput(error.to_string()))?;

        let data = self
            .repository
            .load_project_command_center_data(project_id)
            .await
            .map_err(map_repository_error)?;
        let project = data.project.as_ref().ok_or_else(|| {
            ProjectCommandCenterError::NotFound(format!(
                "PROJECT_NOT_FOUND: project {project_id} was not found"
            ))
        })?;
        let project = project_view(project);
        let structure = load_structure(&data)?;
        let shots = load_shots(&data)?;
        let queue = load_queue(&data)?;
        let tasks_assets = load_tasks_assets(&data)?;
        let consistency = load_consistency(&data)?;
        let preparation = load_preparation(&data)?;
        let reference_anchors = load_reference_anchors(&data)?;
        let prompt_templates = load_prompt_templates(&data)?;
        let audit = self
            .audit_service
            .project_summary(project_id)
            .await
            .map_err(|error| ProjectCommandCenterError::Audit(error.to_string()))?;
        let recent_activity = self
            .audit_service
            .recent_activity(project_id, Some(20))
            .await
            .map_err(|error| ProjectCommandCenterError::Audit(error.to_string()))?;

        let comfy = ProjectCommandCenterComfyView {
            status: match &self.comfy_service {
                Some(service) => service.cached_status().await,
                None => None,
            },
            preflight: match &self.comfy_preflight_service {
                Some(service) => service.cached_current().await,
                None => None,
            },
        };
        let readiness = readiness_view(&comfy);
        let content = ProjectCommandCenterContentView {
            shots: shots.total,
            prompts: prompt_templates.total,
            assets: tasks_assets.asset_count,
            scenes: structure.scene_count,
            configured_shots: shots.configured,
        };
        let production = ProjectCommandCenterProductionView {
            active: count_u64(audit.active_runs)
                + count_u64(audit.active_batches)
                + queue.active_items,
            completed: shots.completed,
            failed: shots.failed
                + count_u64(audit.failed_runs)
                + count_u64(audit.failed_batches)
                + count_u64(audit.failed_items)
                + count_u64(audit.failed_tasks),
            review_required: queue.review_required_items + shots.image_review + shots.video_review,
        };
        let issues = issue_views(&audit.issues, comfy.preflight.as_ref(), structure.blocked);
        let checked_at = audit.checked_at.clone();

        let mut view = ProjectCommandCenterView {
            project,
            structure,
            shots,
            queue,
            tasks_assets,
            consistency,
            preparation,
            reference_anchors,
            prompt_templates,
            comfy,
            readiness,
            content,
            production,
            issues,
            audit,
            recent_activity,
            recommended_action: ProjectCommandCenterNextAction {
                kind: ProjectCommandCenterActionKind::NoShots,
                priority: PROJECT_ACTION_PRIORITY_NO_SHOTS,
                reason_code: "PROJECT_COMMAND_CENTER_NOT_EVALUATED".to_owned(),
                reason: "readiness is being evaluated".to_owned(),
                shot_id: None,
                batch_id: None,
                task_id: None,
                asset_id: None,
            },
            quick_actions: default_quick_actions(),
            checked_at,
        };
        view.recommended_action = recommend_next_project_action(&view);
        Ok(view)
    }
}

pub fn recommend_next_project_action(
    view: &ProjectCommandCenterView,
) -> ProjectCommandCenterNextAction {
    if view.structure.blocked || view.audit.health == ProductionAuditHealth::Blocked {
        return action(
            ProjectCommandCenterActionKind::StructuralBlocked,
            PROJECT_ACTION_PRIORITY_STRUCTURAL_BLOCKED,
            if view.audit.health == ProductionAuditHealth::Blocked {
                "AUDIT_BLOCKED"
            } else {
                "STRUCTURE_BLOCKED"
            },
            "project structure or persisted production lineage is blocked",
            None,
            None,
        );
    }

    if view.shots.total == 0 {
        return action(
            ProjectCommandCenterActionKind::NoShots,
            PROJECT_ACTION_PRIORITY_NO_SHOTS,
            "NO_SHOTS",
            "add at least one shot to begin production",
            None,
            None,
        );
    }

    let continuing_production = view.queue.active_items > 0
        || view.queue.running_queues > 0
        || view.queue.auto_resumable_items > 0
        || view.queue.review_required_items > 0
        || view.audit.active_runs > 0
        || view.shots.generating > 0
        || view.shots.image_review > 0
        || view.shots.video_review > 0
        || view.shots.completed < view.shots.total;
    let comfy_blocked = view
        .comfy
        .preflight
        .as_ref()
        .is_some_and(|preflight| preflight.status == ComfyPreflightStatus::Blocked)
        || view.comfy.status.as_ref().is_some_and(|status| {
            matches!(
                status.status,
                ComfyConnectionStatus::Offline | ComfyConnectionStatus::Incompatible
            )
        });
    if continuing_production && comfy_blocked {
        return action(
            ProjectCommandCenterActionKind::ComfyBlocked,
            PROJECT_ACTION_PRIORITY_COMFY_BLOCKED,
            "COMFY_BLOCKED",
            "the cached ComfyUI status or preflight blocks continuing production",
            None,
            None,
        );
    }

    if view.queue.review_required_items > 0 {
        return action_with_targets(
            ProjectCommandCenterActionKind::ReviewRequired,
            PROJECT_ACTION_PRIORITY_REVIEW_REQUIRED,
            "REVIEW_REQUIRED",
            "review failed or non-resumable production items",
            view.queue.first_review_required_shot_id.clone(),
            view.queue.first_review_required_batch_id.clone(),
            view.queue.first_review_required_task_id.clone(),
            None,
        );
    }
    if view.queue.auto_resumable_items > 0 {
        return action_with_targets(
            ProjectCommandCenterActionKind::AutoResumable,
            PROJECT_ACTION_PRIORITY_AUTO_RESUMABLE,
            "AUTO_RESUMABLE",
            "resume transiently failed production items",
            view.queue.first_auto_resumable_shot_id.clone(),
            view.queue.first_auto_resumable_batch_id.clone(),
            view.queue.first_auto_resumable_task_id.clone(),
            None,
        );
    }
    if view.queue.active_items > 0 || view.queue.running_queues > 0 || view.audit.active_runs > 0 {
        return action_with_targets(
            ProjectCommandCenterActionKind::ActiveProduction,
            PROJECT_ACTION_PRIORITY_ACTIVE_PRODUCTION,
            "ACTIVE_PRODUCTION",
            "production is currently active",
            view.queue
                .first_active_shot_id
                .clone()
                .or_else(|| view.shots.first_generating_shot_id.clone()),
            view.queue.first_active_batch_id.clone(),
            view.queue
                .first_active_task_id
                .clone()
                .or_else(|| view.shots.first_generating_task_id.clone()),
            None,
        );
    }
    if view.shots.image_review > 0 {
        return action(
            ProjectCommandCenterActionKind::ImageReview,
            PROJECT_ACTION_PRIORITY_IMAGE_REVIEW,
            "IMAGE_REVIEW",
            "review generated images before continuing",
            view.shots.first_image_review_shot_id.clone(),
            None,
        );
    }
    if view.shots.video_review > 0 {
        return action(
            ProjectCommandCenterActionKind::VideoReview,
            PROJECT_ACTION_PRIORITY_VIDEO_REVIEW,
            "VIDEO_REVIEW",
            "review generated videos before completing the project",
            view.shots.first_video_review_shot_id.clone(),
            None,
        );
    }
    if view.shots.missing_config > 0 {
        return action(
            ProjectCommandCenterActionKind::MissingConfig,
            PROJECT_ACTION_PRIORITY_MISSING_CONFIG,
            "MISSING_CONFIG",
            "configure the workflow and recipe for the next shot",
            view.shots.first_missing_config_shot_id.clone(),
            None,
        );
    }
    if view.structure.unassigned_shot_count > 0 {
        return action(
            ProjectCommandCenterActionKind::Unassigned,
            PROJECT_ACTION_PRIORITY_UNASSIGNED,
            "UNASSIGNED_SHOTS",
            "assign shots to the production structure",
            view.structure.first_unassigned_shot_id.clone(),
            None,
        );
    }
    if view.shots.completed == view.shots.total {
        return action_with_targets(
            ProjectCommandCenterActionKind::Complete,
            PROJECT_ACTION_PRIORITY_COMPLETE,
            "COMPLETE",
            "all shots have completed production",
            view.shots.first_completed_shot_id.clone(),
            None,
            None,
            view.shots.first_completed_asset_id.clone(),
        );
    }
    if view.shots.ready > 0 || view.shots.configured > view.shots.completed {
        return action(
            ProjectCommandCenterActionKind::Ready,
            PROJECT_ACTION_PRIORITY_READY,
            "READY",
            "the next configured shot is ready for production",
            view.shots.first_ready_shot_id.clone(),
            None,
        );
    }

    action(
        ProjectCommandCenterActionKind::Ready,
        PROJECT_ACTION_PRIORITY_READY,
        "READY",
        "the project is ready for the next production step",
        None,
        None,
    )
}

#[allow(non_snake_case)]
pub fn recommendNextProjectAction(
    view: &ProjectCommandCenterView,
) -> ProjectCommandCenterNextAction {
    recommend_next_project_action(view)
}

fn action(
    kind: ProjectCommandCenterActionKind,
    priority: u8,
    reason_code: &str,
    reason: &str,
    shot_id: Option<String>,
    batch_id: Option<String>,
) -> ProjectCommandCenterNextAction {
    ProjectCommandCenterNextAction {
        kind,
        priority,
        reason_code: reason_code.to_owned(),
        reason: reason.to_owned(),
        shot_id,
        batch_id,
        task_id: None,
        asset_id: None,
    }
}

fn action_with_targets(
    kind: ProjectCommandCenterActionKind,
    priority: u8,
    reason_code: &str,
    reason: &str,
    shot_id: Option<String>,
    batch_id: Option<String>,
    task_id: Option<String>,
    asset_id: Option<String>,
) -> ProjectCommandCenterNextAction {
    ProjectCommandCenterNextAction {
        kind,
        priority,
        reason_code: reason_code.to_owned(),
        reason: reason.to_owned(),
        shot_id,
        batch_id,
        task_id,
        asset_id,
    }
}

fn project_view(row: &ProjectRow) -> ProjectCommandCenterProjectView {
    ProjectCommandCenterProjectView {
        id: row.id.clone(),
        name: row.name.clone(),
        description: row.description.clone(),
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    }
}

fn load_structure(
    data: &ProjectCommandCenterData,
) -> Result<ProjectCommandCenterStructureView, ProjectCommandCenterError> {
    let row = &data.structure;
    Ok(ProjectCommandCenterStructureView {
        series_count: count(row.series_count),
        episode_count: count(row.episode_count),
        scene_count: count(row.scene_count),
        assigned_shot_count: count(row.assigned_shot_count),
        unassigned_shot_count: count(row.unassigned_shot_count),
        first_unassigned_shot_id: row.first_unassigned_shot_id.clone(),
        blocked: row.orphan_count > 0,
        scenes: data
            .scenes
            .iter()
            .map(|row| ProjectCommandCenterSceneView {
                id: row.id.clone(),
                name: row.name.clone(),
                path: format!("{} / {}", row.series_name, row.episode_name),
                total: count(row.total),
                completed: count(row.completed),
            })
            .collect(),
    })
}

fn load_shots(
    data: &ProjectCommandCenterData,
) -> Result<ProjectCommandCenterShotView, ProjectCommandCenterError> {
    if data.shots.is_empty() {
        return Ok(ProjectCommandCenterShotView::default());
    }
    let mut configured = HashSet::with_capacity(data.shot_configs.len());
    for row in &data.shot_configs {
        configured.insert((row.shot_id.clone(), row.stage.clone()));
    }
    let mut latest_status = HashMap::<(String, String), String>::new();
    let mut latest_task_id = HashMap::<(String, String), Option<String>>::new();
    for row in &data.shot_links {
        if let Some(status) = row.task_status.as_deref() {
            let key = (row.shot_id.clone(), row.stage.clone());
            if !latest_status.contains_key(&key) {
                latest_status.insert(key.clone(), status.to_owned());
                latest_task_id.insert(key, row.task_id.clone());
            }
        }
    }
    let mut view = ProjectCommandCenterShotView {
        total: data.shots.len(),
        ..Default::default()
    };
    for shot in &data.shots {
        let image_configured = configured.contains(&(shot.id.clone(), "image".to_owned()));
        let video_configured = configured.contains(&(shot.id.clone(), "video".to_owned()));
        let image_status = stage_status(
            "image",
            image_configured,
            shot.selected_image_asset_id.is_some(),
            latest_status
                .get(&(shot.id.clone(), "image".to_owned()))
                .map(String::as_str),
        );
        let video_status = stage_status(
            "video",
            video_configured,
            shot.selected_video_asset_id.is_some(),
            latest_status
                .get(&(shot.id.clone(), "video".to_owned()))
                .map(String::as_str),
        );
        let overall = overall_status(image_status, video_status, video_configured);
        match overall {
            "DRAFT" => view.draft += 1,
            "READY" => view.ready += 1,
            "GENERATING_IMAGE" | "GENERATING_VIDEO" => {
                view.generating += 1;
                set_first(&mut view.first_generating_shot_id, &shot.id);
                let generating_stage = if image_status == "GENERATING_IMAGE" {
                    "image"
                } else {
                    "video"
                };
                let task_id = latest_task_id
                    .get(&(shot.id.clone(), generating_stage.to_owned()))
                    .and_then(Clone::clone);
                if view.first_generating_task_id.is_none() {
                    view.first_generating_task_id = task_id;
                }
            }
            "IMAGE_REVIEW" => {
                view.image_review += 1;
                set_first(&mut view.first_image_review_shot_id, &shot.id);
            }
            "IMAGE_SELECTED" => view.image_selected += 1,
            "VIDEO_REVIEW" => {
                view.video_review += 1;
                set_first(&mut view.first_video_review_shot_id, &shot.id);
            }
            "COMPLETED" => {
                view.completed += 1;
                set_first(&mut view.first_completed_shot_id, &shot.id);
                if view.first_completed_asset_id.is_none() {
                    view.first_completed_asset_id = shot
                        .selected_video_asset_id
                        .clone()
                        .or_else(|| shot.selected_image_asset_id.clone());
                }
            }
            "FAILED" => view.failed += 1,
            _ => {}
        }
        if image_configured || video_configured {
            view.configured += 1;
        }
        if image_status == "DRAFT" || (video_configured && video_status == "DRAFT") {
            view.missing_config += 1;
            set_first(&mut view.first_missing_config_shot_id, &shot.id);
        }
        if overall == "READY" {
            set_first(&mut view.first_ready_shot_id, &shot.id);
        }
    }
    Ok(view)
}

fn stage_status(
    stage: &str,
    configured: bool,
    selected: bool,
    task_status: Option<&str>,
) -> &'static str {
    if let Some(status) = task_status {
        if matches!(
            status,
            "CREATED"
                | "VALIDATING"
                | "PREPARING"
                | "QUEUED"
                | "RUNNING"
                | "CANCEL_REQUESTED"
                | "COLLECTING"
        ) {
            return if stage == "image" {
                "GENERATING_IMAGE"
            } else {
                "GENERATING_VIDEO"
            };
        }
        if selected {
            return if stage == "image" {
                "IMAGE_SELECTED"
            } else {
                "COMPLETED"
            };
        }
        if status == "FAILED" {
            return "FAILED";
        }
        if status == "SUCCEEDED" {
            return if stage == "image" {
                "IMAGE_REVIEW"
            } else {
                "VIDEO_REVIEW"
            };
        }
    }
    if selected {
        if stage == "image" {
            "IMAGE_SELECTED"
        } else {
            "COMPLETED"
        }
    } else if configured {
        "READY"
    } else {
        "DRAFT"
    }
}

fn overall_status<'a>(image: &'a str, video: &'a str, has_video_stage: bool) -> &'a str {
    if has_video_stage {
        if video == "COMPLETED" {
            return "COMPLETED";
        }
        if video == "GENERATING_VIDEO" || video == "VIDEO_REVIEW" || video == "FAILED" {
            return video;
        }
    }
    if !has_video_stage && image == "IMAGE_SELECTED" {
        return "COMPLETED";
    }
    match image {
        "GENERATING_IMAGE" | "IMAGE_REVIEW" | "FAILED" => image,
        "IMAGE_SELECTED" => "IMAGE_SELECTED",
        "READY" if video == "READY" || !has_video_stage => "READY",
        _ if image == "READY" || video == "READY" => "READY",
        _ => "DRAFT",
    }
}

fn set_first(slot: &mut Option<String>, id: &str) {
    if slot.is_none() {
        *slot = Some(id.to_owned());
    }
}

fn load_queue(
    data: &ProjectCommandCenterData,
) -> Result<ProjectCommandCenterQueueView, ProjectCommandCenterError> {
    let batches = &data.queue_batches;
    let items = &data.queue_items;
    let active_batch_ids = batches
        .iter()
        .filter(|batch| batch.archived_at.is_none() && batch.status == "RUNNING")
        .map(|batch| batch.id.clone())
        .collect::<HashSet<_>>();
    let active_batches = batches
        .iter()
        .filter(|batch| batch.archived_at.is_none())
        .map(|batch| batch.id.clone())
        .collect::<HashSet<_>>();
    let mut view = ProjectCommandCenterQueueView {
        archived_queues: batches
            .iter()
            .filter(|batch| batch.archived_at.is_some())
            .count(),
        ..Default::default()
    };
    for batch in batches {
        if batch.archived_at.is_some() {
            continue;
        }
        view.total_queues += 1;
        match batch.status.as_str() {
            "RUNNING" => view.running_queues += 1,
            "PAUSED" => view.paused_queues += 1,
            "COMPLETED" => view.completed_queues += 1,
            _ => {}
        }
        if batch.status == "RUNNING" {
            set_first(&mut view.first_active_batch_id, &batch.id);
        }
    }
    let mut items_by_batch = HashMap::<String, Vec<&QueueItemRow>>::new();
    for item in items {
        if active_batches.contains(&item.batch_id) {
            items_by_batch
                .entry(item.batch_id.clone())
                .or_default()
                .push(item);
            view.total_items += 1;
            if active_batch_ids.contains(&item.batch_id) {
                set_first_queue_target(
                    &mut view.first_active_shot_id,
                    &mut view.first_active_task_id,
                    item,
                );
            }
            match item.status.as_str() {
                "PENDING" => view.pending_items += 1,
                "DISPATCHING" | "DISPATCHED" => {
                    view.active_items += 1;
                    set_first(&mut view.first_active_batch_id, &item.batch_id);
                    set_first_queue_target(
                        &mut view.first_active_shot_id,
                        &mut view.first_active_task_id,
                        item,
                    );
                }
                "SUCCEEDED" => view.succeeded_items += 1,
                "FAILED" => view.failed_items += 1,
                "CANCELLED" => view.cancelled_items += 1,
                "SKIPPED" => view.skipped_items += 1,
                _ => {}
            }
        }
    }
    let mut batch_ids = items_by_batch.keys().cloned().collect::<Vec<_>>();
    batch_ids.sort();
    for batch_id in batch_ids {
        let batch_items = items_by_batch
            .get(&batch_id)
            .expect("batch item group should exist");
        let edges = batch_items
            .iter()
            .map(|item| RetryLineageEdge {
                item_id: item.id.clone(),
                parent_item_id: item.retry_of_item_id.clone(),
                ordinal: item.ordinal,
            })
            .collect::<Vec<_>>();
        let lineages = build_retry_lineages_from_edges(&edges);
        let leaves = match lineages {
            Ok(lineages) => lineages,
            Err(_) => {
                if batch_items.iter().any(|item| item.status == "FAILED") {
                    view.review_required_items += 1;
                    set_first(&mut view.first_review_required_batch_id, &batch_id);
                }
                continue;
            }
        };
        let by_id = batch_items
            .iter()
            .map(|item| (item.id.as_str(), *item))
            .collect::<HashMap<_, _>>();
        for lineage in leaves {
            let Some(leaf) = by_id.get(lineage.leaf_item_id.as_str()) else {
                continue;
            };
            if is_auto_resumable(leaf.status.as_str(), leaf.error_code.as_deref()) {
                view.auto_resumable_items += 1;
                set_first(&mut view.first_auto_resumable_batch_id, &batch_id);
                set_first_queue_target(
                    &mut view.first_auto_resumable_shot_id,
                    &mut view.first_auto_resumable_task_id,
                    leaf,
                );
            } else if matches!(leaf.status.as_str(), "FAILED" | "CANCELLED" | "SKIPPED") {
                view.review_required_items += 1;
                set_first(&mut view.first_review_required_batch_id, &batch_id);
                set_first_queue_target(
                    &mut view.first_review_required_shot_id,
                    &mut view.first_review_required_task_id,
                    leaf,
                );
            }
        }
    }
    Ok(view)
}

fn is_auto_resumable(status: &str, error_code: Option<&str>) -> bool {
    status == "CANCELLED"
        || matches!(
            (status, error_code),
            (
                "FAILED" | "SKIPPED",
                Some(
                    "COMFY_OFFLINE"
                        | "COMFY_TIMEOUT"
                        | "COMFY_STREAM_DISCONNECTED"
                        | "COMFY_IMAGE_UPLOAD_FAILED"
                        | "COMFY_INPUT_UPLOAD_FAILED"
                        | "EXECUTION_INTERRUPTED"
                )
            )
        )
}

fn set_first_queue_target(
    shot_id: &mut Option<String>,
    task_id: &mut Option<String>,
    item: &QueueItemRow,
) {
    if shot_id.is_none() {
        *shot_id = item.shot_id.clone();
    }
    if task_id.is_none() {
        *task_id = item.task_id.clone();
    }
}

fn load_tasks_assets(
    data: &ProjectCommandCenterData,
) -> Result<ProjectCommandCenterTaskAssetView, ProjectCommandCenterError> {
    let mut view = ProjectCommandCenterTaskAssetView::default();
    for row in &data.task_counts {
        let amount = count(row.count);
        view.task_count += amount;
        match row.status.as_str() {
            "CREATED" | "VALIDATING" | "PREPARING" | "QUEUED" | "RUNNING" | "CANCEL_REQUESTED"
            | "COLLECTING" => view.active_task_count += amount,
            "SUCCEEDED" => view.succeeded_task_count += amount,
            "FAILED" => view.failed_task_count += amount,
            _ => {}
        }
    }
    for row in &data.asset_counts {
        let amount = count(row.count);
        view.asset_count += amount;
        match row.asset_type.as_str() {
            "IMAGE" => view.image_asset_count += amount,
            "VIDEO" => view.video_asset_count += amount,
            "AUDIO" => view.audio_asset_count += amount,
            _ => view.other_asset_count += amount,
        }
    }
    Ok(view)
}

fn load_consistency(
    data: &ProjectCommandCenterData,
) -> Result<ProjectCommandCenterConsistencyView, ProjectCommandCenterError> {
    let row = &data.consistency;
    let consistency_in_use = [
        row.character_profiles,
        row.scene_profiles,
        row.prop_profiles,
        row.style_profiles,
        row.reference_sets,
        row.shot_profile_bindings,
        row.shot_reference_set_bindings,
        row.scope_profile_bindings,
        row.scope_reference_set_bindings,
    ]
    .into_iter()
    .any(|value| value > 0);
    Ok(ProjectCommandCenterConsistencyView {
        character_profiles: count(row.character_profiles),
        scene_profiles: count(row.scene_profiles),
        prop_profiles: count(row.prop_profiles),
        style_profiles: count(row.style_profiles),
        reference_sets: count(row.reference_sets),
        shot_profile_bindings: count(row.shot_profile_bindings),
        shot_reference_set_bindings: count(row.shot_reference_set_bindings),
        scope_profile_bindings: count(row.scope_profile_bindings),
        scope_reference_set_bindings: count(row.scope_reference_set_bindings),
        consistency_in_use,
    })
}

fn load_preparation(
    data: &ProjectCommandCenterData,
) -> Result<ProjectCommandCenterPreparationView, ProjectCommandCenterError> {
    let row = &data.preparation;
    Ok(ProjectCommandCenterPreparationView {
        snapshot_count: count(row.snapshot_count),
        prepared_image_items: count(row.prepared_image_items),
        prepared_video_items: count(row.prepared_video_items),
        active_prepared_items: count(row.active_prepared_items),
        latest_prepared_at: row.latest_prepared_at.clone(),
    })
}

fn load_reference_anchors(
    data: &ProjectCommandCenterData,
) -> Result<ProjectCommandCenterReferenceAnchorView, ProjectCommandCenterError> {
    let mut view = ProjectCommandCenterReferenceAnchorView {
        total: data.reference_anchors.len(),
        ..Default::default()
    };
    for row in &data.reference_anchors {
        if row.asset_count > 0 {
            view.usable += 1;
        }
        match row.kind.as_str() {
            "CHARACTER" => view.character += 1,
            "SCENE" => view.scene += 1,
            "PROP" => view.prop += 1,
            "STYLE" => view.style += 1,
            _ => {}
        }
    }
    Ok(view)
}

fn load_prompt_templates(
    data: &ProjectCommandCenterData,
) -> Result<ProjectCommandCenterPromptTemplateSummary, ProjectCommandCenterError> {
    let versions = data
        .prompt_templates
        .iter()
        .map(|row| count(row.version_count))
        .sum();
    Ok(ProjectCommandCenterPromptTemplateSummary {
        total: data.prompt_templates.len(),
        versions,
        items: data
            .prompt_templates
            .iter()
            .map(|row| ProjectCommandCenterPromptTemplateView {
                id: row.id.clone(),
                name: row.name.clone(),
                version_count: count(row.version_count),
                updated_at: row.updated_at.clone(),
            })
            .collect(),
    })
}

fn count(value: i64) -> usize {
    usize::try_from(value).unwrap_or_default()
}

fn count_u64(value: u64) -> usize {
    usize::try_from(value).unwrap_or_default()
}

fn readiness_view(comfy: &ProjectCommandCenterComfyView) -> ProjectCommandCenterReadinessView {
    let Some(preflight) = comfy.preflight.as_ref() else {
        return ProjectCommandCenterReadinessView {
            status: None,
            connection: comfy
                .status
                .as_ref()
                .map(|status| connection_name(status.status)),
            workflow_ready: 0,
            workflow_total: 0,
            runtime_busy: false,
            active_task_count: 0,
            production_busy: false,
        };
    };
    ProjectCommandCenterReadinessView {
        status: Some(preflight.status),
        connection: Some(connection_name(preflight.connection)),
        workflow_ready: preflight.workflow_summary.workflow_ready,
        workflow_total: preflight.workflow_summary.workflow_total,
        runtime_busy: preflight.runtime_busy,
        active_task_count: preflight.active_task_count,
        production_busy: preflight.production_busy,
    }
}

fn connection_name(status: ComfyConnectionStatus) -> String {
    match status {
        ComfyConnectionStatus::Connected => "CONNECTED".to_owned(),
        ComfyConnectionStatus::Offline => "OFFLINE".to_owned(),
        ComfyConnectionStatus::Incompatible => "INCOMPATIBLE".to_owned(),
    }
}

fn issue_views(
    audit_issues: &[ProductionAuditIssue],
    preflight: Option<&ComfyPreflightReport>,
    structure_blocked: bool,
) -> Vec<ProjectCommandCenterIssueView> {
    let mut issues = Vec::new();
    if structure_blocked {
        issues.push(ProjectCommandCenterIssueView {
            id: "structure:blocked".to_owned(),
            severity: "ERROR".to_owned(),
            title: "项目结构已阻断".to_owned(),
            detail: "项目结构存在无法继续生产的断链。".to_owned(),
            source: "production".to_owned(),
        });
    }
    issues.extend(
        audit_issues
            .iter()
            .map(|issue| ProjectCommandCenterIssueView {
                id: format!(
                    "production:{}:{}:{}",
                    issue.code, issue.entity_type, issue.entity_id
                ),
                severity: format!("{:?}", issue.severity).to_uppercase(),
                title: issue.code.clone(),
                detail: format!(
                    "{} · {} {}",
                    issue.message, issue.entity_type, issue.entity_id
                ),
                source: "production".to_owned(),
            }),
    );
    if let Some(preflight) = preflight {
        issues.extend(
            preflight
                .issues
                .iter()
                .map(|issue| ProjectCommandCenterIssueView {
                    id: format!(
                        "runtime:{}:{}",
                        issue.code,
                        issue.workflow_id.as_deref().unwrap_or_default()
                    ),
                    severity: format!("{:?}", issue.severity).to_uppercase(),
                    title: issue.title.clone(),
                    detail: match issue.suggested_action.as_deref() {
                        Some(action) => format!("{} 建议：{}", issue.detail, action),
                        None => issue.detail.clone(),
                    },
                    source: "runtime".to_owned(),
                }),
        );
    }
    issues
}

fn default_quick_actions() -> Vec<ProjectCommandCenterQuickActionView> {
    [
        ("create", "创作工作台", "studio"),
        ("shots", "镜头生产", "shots"),
        ("assets", "一致性资产", "assets"),
        ("preparation", "生产准备", "shots"),
        ("tasks", "任务历史", "tasks"),
        ("workflows", "工作流", "workflows"),
        ("settings", "运行时设置", "settings"),
    ]
    .into_iter()
    .map(
        |(id, label, destination)| ProjectCommandCenterQuickActionView {
            id: id.to_owned(),
            label: label.to_owned(),
            destination: destination.to_owned(),
        },
    )
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::production_audit_service::ProductionAuditIssue;
    use crate::infrastructure::database::{
        initialize, SqliteProductionAuditRepository, SqliteProjectCommandCenterRepository,
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::tempdir;

    const PROJECT: &str = "prj_00000000-0000-0000-0000-000000000039";
    const NOW: &str = "2026-08-18T00:00:00Z";

    fn service(pool: &SqlitePool) -> ProjectCommandCenterService {
        ProjectCommandCenterService::new(
            Arc::new(SqliteProjectCommandCenterRepository::new(pool.clone())),
            Arc::new(ProductionAuditService::new(Arc::new(
                SqliteProductionAuditRepository::new(pool.clone()),
            ))),
        )
    }

    fn base_view() -> ProjectCommandCenterView {
        ProjectCommandCenterView {
            project: ProjectCommandCenterProjectView {
                id: PROJECT.to_owned(),
                name: "Command center".to_owned(),
                description: None,
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
            },
            structure: ProjectCommandCenterStructureView::default(),
            shots: ProjectCommandCenterShotView::default(),
            queue: ProjectCommandCenterQueueView::default(),
            tasks_assets: ProjectCommandCenterTaskAssetView::default(),
            consistency: ProjectCommandCenterConsistencyView::default(),
            preparation: ProjectCommandCenterPreparationView::default(),
            reference_anchors: ProjectCommandCenterReferenceAnchorView::default(),
            prompt_templates: ProjectCommandCenterPromptTemplateSummary::default(),
            comfy: ProjectCommandCenterComfyView::default(),
            readiness: ProjectCommandCenterReadinessView {
                status: None,
                connection: None,
                workflow_ready: 0,
                workflow_total: 0,
                runtime_busy: false,
                active_task_count: 0,
                production_busy: false,
            },
            content: ProjectCommandCenterContentView::default(),
            production: ProjectCommandCenterProductionView::default(),
            issues: Vec::new(),
            audit: ProductionAuditSummary {
                project_id: PROJECT.to_owned(),
                active_runs: 0,
                completed_runs: 0,
                failed_runs: 0,
                active_batches: 0,
                paused_batches: 0,
                failed_batches: 0,
                logical_items: 0,
                attempts: 0,
                succeeded_items: 0,
                failed_items: 0,
                review_required_items: 0,
                tasks: 0,
                succeeded_tasks: 0,
                failed_tasks: 0,
                assets: 0,
                unassigned_shots: 0,
                checked_at: NOW.to_owned(),
                health: ProductionAuditHealth::Healthy,
                issues: Vec::<ProductionAuditIssue>::new(),
            },
            recent_activity: Vec::new(),
            recommended_action: action(
                ProjectCommandCenterActionKind::NoShots,
                PROJECT_ACTION_PRIORITY_NO_SHOTS,
                "TEST",
                "test",
                None,
                None,
            ),
            quick_actions: default_quick_actions(),
            checked_at: NOW.to_owned(),
        }
    }

    fn kind(view: &ProjectCommandCenterView) -> ProjectCommandCenterActionKind {
        recommend_next_project_action(view).kind
    }

    #[test]
    fn recommendation_priority_is_deterministic() {
        let mut view = base_view();
        view.shots.total = 1;
        view.shots.completed = 1;
        view.structure.unassigned_shot_count = 1;
        view.structure.blocked = true;
        view.audit.health = ProductionAuditHealth::Blocked;
        view.queue.review_required_items = 1;
        view.queue.auto_resumable_items = 1;
        view.shots.image_review = 1;
        view.shots.video_review = 1;
        assert_eq!(recommend_next_project_action(&view).priority, 1);
        assert_eq!(
            kind(&view),
            ProjectCommandCenterActionKind::StructuralBlocked
        );
        view.structure.blocked = false;
        view.audit.health = ProductionAuditHealth::Healthy;
        assert_eq!(
            recommend_next_project_action(&view).kind,
            ProjectCommandCenterActionKind::ReviewRequired
        );
        view.queue.review_required_items = 0;
        assert_eq!(
            recommend_next_project_action(&view).kind,
            ProjectCommandCenterActionKind::AutoResumable
        );
        view.queue.auto_resumable_items = 0;
        assert_eq!(
            recommend_next_project_action(&view).kind,
            ProjectCommandCenterActionKind::ImageReview
        );
    }

    #[test]
    fn recommendation_covers_empty_active_review_resume_image_video_unassigned_and_complete() {
        let mut view = base_view();
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::NoShots);

        view.shots.total = 1;
        view.shots.generating = 1;
        view.queue.active_items = 1;
        view.queue.first_active_batch_id = Some("batch-active".to_owned());
        view.queue.first_active_shot_id = Some("shot-active".to_owned());
        view.queue.first_active_task_id = Some("task-active".to_owned());
        let active = recommend_next_project_action(&view);
        assert_eq!(
            active.kind,
            ProjectCommandCenterActionKind::ActiveProduction
        );
        assert_eq!(active.shot_id.as_deref(), Some("shot-active"));
        assert_eq!(active.batch_id.as_deref(), Some("batch-active"));
        assert_eq!(active.task_id.as_deref(), Some("task-active"));

        view.queue.active_items = 0;
        view.shots.generating = 0;
        view.queue.review_required_items = 1;
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::ReviewRequired);

        view.queue.review_required_items = 0;
        view.queue.auto_resumable_items = 1;
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::AutoResumable);

        view.queue.auto_resumable_items = 0;
        view.shots.image_review = 1;
        view.shots.first_image_review_shot_id = Some("shot-image".to_owned());
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::ImageReview);

        view.shots.image_review = 0;
        view.shots.video_review = 1;
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::VideoReview);

        view.shots.video_review = 0;
        view.structure.unassigned_shot_count = 1;
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::Unassigned);

        view.structure.unassigned_shot_count = 0;
        view.shots.completed = 1;
        view.shots.first_completed_shot_id = Some("shot-complete".to_owned());
        view.shots.first_completed_asset_id = Some("asset-complete".to_owned());
        let complete = recommend_next_project_action(&view);
        assert_eq!(complete.kind, ProjectCommandCenterActionKind::Complete);
        assert_eq!(complete.shot_id.as_deref(), Some("shot-complete"));
        assert_eq!(complete.asset_id.as_deref(), Some("asset-complete"));
    }

    #[test]
    fn comfy_block_is_only_reported_while_continuing_production() {
        let mut view = base_view();
        view.shots.total = 1;
        view.shots.completed = 1;
        view.comfy.preflight = Some(ComfyPreflightReport {
            endpoint: "http://cached".to_owned(),
            status: ComfyPreflightStatus::Blocked,
            checked_at: NOW.to_owned(),
            connection: ComfyConnectionStatus::Offline,
            comfyui_version: None,
            python_version: None,
            gpu: None,
            vram_total: None,
            vram_free: None,
            node_count: None,
            runtime_busy: false,
            active_task_count: 0,
            production_busy: false,
            workflow_summary:
                crate::application::comfy_preflight_service::ComfyPreflightWorkflowSummary {
                    workflow_total: 0,
                    workflow_ready: 0,
                    workflow_blocked: 0,
                    items: Vec::new(),
                },
            issues: Vec::new(),
        });
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::Complete);
        view.shots.completed = 0;
        view.shots.ready = 1;
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::ComfyBlocked);
    }

    #[tokio::test]
    async fn empty_project_loads_as_no_shots() {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("command-center.db"))
            .await
            .expect("database should migrate");
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES (?, 'Command center', ?, ?, ?)",
        )
        .bind(PROJECT)
        .bind(
            directory
                .path()
                .join("project")
                .to_string_lossy()
                .to_string(),
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("project should insert");

        let view = service(&pool)
            .get(PROJECT)
            .await
            .expect("empty project should load");
        assert_eq!(view.shots.total, 0);
        assert_eq!(
            view.recommended_action.kind,
            ProjectCommandCenterActionKind::NoShots
        );
    }

    #[tokio::test]
    async fn five_hundred_shot_project_is_loaded_with_set_based_summary() {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("command-center-500.db"))
            .await
            .expect("database should migrate");
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES (?, 'Scale project', ?, ?, ?)",
        )
        .bind(PROJECT)
        .bind(
            directory
                .path()
                .join("project")
                .to_string_lossy()
                .to_string(),
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("project should insert");
        let mut transaction = pool.begin().await.expect("transaction should begin");
        for ordinal in 0..500_i64 {
            sqlx::query(
                "INSERT INTO shots
                 (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 'prompt', ?, ?)",
            )
            .bind(format!("shot-{ordinal:03}"))
            .bind(PROJECT)
            .bind(ordinal)
            .bind(format!("Shot {ordinal}"))
            .bind(NOW)
            .bind(NOW)
            .execute(&mut *transaction)
            .await
            .expect("shot should insert");
        }
        transaction
            .commit()
            .await
            .expect("transaction should commit");

        let view = service(&pool)
            .get(PROJECT)
            .await
            .expect("500-shot project should load");
        assert_eq!(view.shots.total, 500);
        assert_eq!(view.structure.unassigned_shot_count, 500);
        assert_eq!(view.tasks_assets.task_count, 0);
    }
}
