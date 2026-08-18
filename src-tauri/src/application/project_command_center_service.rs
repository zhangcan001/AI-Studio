//! Read-only project readiness aggregate.
//!
//! The command center is deliberately a view over existing tables and
//! services. It does not persist state, submit work to ComfyUI, or introduce
//! another queue, task history, audit stream, or workflow engine.

use crate::application::comfy_preflight_service::{
    ComfyPreflightReport, ComfyPreflightService, ComfyPreflightStatus,
};
use crate::application::comfy_service::{ComfyConnectionStatus, ComfyService, ComfyStatusView};
use crate::application::production_audit_service::{
    ProductionAuditActivity, ProductionAuditHealth, ProductionAuditIssue, ProductionAuditService,
    ProductionAuditSummary,
};
use crate::application::production_queue_service::{
    build_retry_lineages_from_edges, RetryLineageEdge,
};
use crate::domain::validate_project_id;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
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
    pub first_image_review_shot_id: Option<String>,
    pub first_video_review_shot_id: Option<String>,
    pub first_missing_config_shot_id: Option<String>,
    pub first_ready_shot_id: Option<String>,
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
    pub first_auto_resumable_batch_id: Option<String>,
    pub first_review_required_batch_id: Option<String>,
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
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandCenterView {
    pub project: ProjectCommandCenterProjectView,
    pub structure: ProjectCommandCenterStructureView,
    pub shots: ProjectCommandCenterShotView,
    pub queue: ProjectCommandCenterQueueView,
    pub tasks_assets: ProjectCommandCenterTaskAssetView,
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
    Database(sqlx::Error),
    Audit(String),
}

impl fmt::Display for ProjectCommandCenterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) | Self::Audit(message) => {
                formatter.write_str(message)
            }
            Self::Database(error) => {
                write!(formatter, "project command center database error: {error}")
            }
        }
    }
}

impl Error for ProjectCommandCenterError {}

impl From<sqlx::Error> for ProjectCommandCenterError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub struct ProjectCommandCenterService {
    pool: SqlitePool,
    audit_service: Arc<ProductionAuditService>,
    comfy_service: Option<Arc<ComfyService>>,
    comfy_preflight_service: Option<Arc<ComfyPreflightService>>,
}

impl ProjectCommandCenterService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            audit_service: Arc::new(ProductionAuditService::new(pool.clone())),
            pool,
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

        let project = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, description, created_at, updated_at
             FROM projects WHERE id = ?",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            ProjectCommandCenterError::NotFound(format!(
                "PROJECT_NOT_FOUND: project {project_id} was not found"
            ))
        })?;

        let structure = load_structure(&self.pool, project_id).await?;
        let shots = load_shots(&self.pool, project_id).await?;
        let queue = load_queue(&self.pool, project_id).await?;
        let tasks_assets = load_tasks_assets(&self.pool, project_id).await?;
        let reference_anchors = load_reference_anchors(&self.pool, project_id).await?;
        let prompt_templates = load_prompt_templates(&self.pool, project_id).await?;
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
            project: project.into_view(),
            structure,
            shots,
            queue,
            tasks_assets,
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
        return action(
            ProjectCommandCenterActionKind::ReviewRequired,
            PROJECT_ACTION_PRIORITY_REVIEW_REQUIRED,
            "REVIEW_REQUIRED",
            "review failed or non-resumable production items",
            None,
            view.queue.first_review_required_batch_id.clone(),
        );
    }
    if view.queue.auto_resumable_items > 0 {
        return action(
            ProjectCommandCenterActionKind::AutoResumable,
            PROJECT_ACTION_PRIORITY_AUTO_RESUMABLE,
            "AUTO_RESUMABLE",
            "resume transiently failed production items",
            None,
            view.queue.first_auto_resumable_batch_id.clone(),
        );
    }
    if view.queue.active_items > 0 || view.queue.running_queues > 0 || view.audit.active_runs > 0 {
        return action(
            ProjectCommandCenterActionKind::ActiveProduction,
            PROJECT_ACTION_PRIORITY_ACTIVE_PRODUCTION,
            "ACTIVE_PRODUCTION",
            "production is currently active",
            None,
            view.queue.first_active_batch_id.clone(),
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
        return action(
            ProjectCommandCenterActionKind::Complete,
            PROJECT_ACTION_PRIORITY_COMPLETE,
            "COMPLETE",
            "all shots have completed production",
            None,
            None,
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
    }
}

#[derive(Debug, FromRow)]
struct ProjectRow {
    id: String,
    name: String,
    description: Option<String>,
    created_at: String,
    updated_at: String,
}

impl ProjectRow {
    fn into_view(self) -> ProjectCommandCenterProjectView {
        ProjectCommandCenterProjectView {
            id: self.id,
            name: self.name,
            description: self.description,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct StructureRow {
    series_count: i64,
    episode_count: i64,
    scene_count: i64,
    assigned_shot_count: i64,
    unassigned_shot_count: i64,
    first_unassigned_shot_id: Option<String>,
    orphan_count: i64,
}

async fn load_structure(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<ProjectCommandCenterStructureView, ProjectCommandCenterError> {
    let row = sqlx::query_as::<_, StructureRow>(
        "SELECT
           (SELECT COUNT(*) FROM production_series WHERE project_id = ?) AS series_count,
           (SELECT COUNT(*) FROM production_episodes e
              JOIN production_series s ON s.id = e.series_id
              WHERE s.project_id = ?) AS episode_count,
           (SELECT COUNT(*) FROM production_scenes c
              JOIN production_episodes e ON e.id = c.episode_id
              JOIN production_series s ON s.id = e.series_id
              WHERE s.project_id = ?) AS scene_count,
           (SELECT COUNT(*) FROM shot_scene_assignments a
              JOIN shots sh ON sh.id = a.shot_id
              WHERE sh.project_id = ?) AS assigned_shot_count,
           (SELECT COUNT(*) FROM shots sh
              LEFT JOIN shot_scene_assignments a ON a.shot_id = sh.id
              WHERE sh.project_id = ? AND a.shot_id IS NULL) AS unassigned_shot_count,
           (SELECT MIN(sh.id) FROM shots sh
              LEFT JOIN shot_scene_assignments a ON a.shot_id = sh.id
              WHERE sh.project_id = ? AND a.shot_id IS NULL) AS first_unassigned_shot_id,
           (SELECT COUNT(*) FROM production_episodes e
              LEFT JOIN production_series s ON s.id = e.series_id WHERE s.id IS NULL)
           + (SELECT COUNT(*) FROM production_scenes c
              LEFT JOIN production_episodes e ON e.id = c.episode_id WHERE e.id IS NULL)
           + (SELECT COUNT(*) FROM shot_scene_assignments a
              LEFT JOIN shots sh ON sh.id = a.shot_id WHERE sh.id IS NULL) AS orphan_count",
    )
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    let scenes = sqlx::query_as::<_, SceneSummaryRow>(
        "SELECT c.id, c.name, s.name AS series_name, e.name AS episode_name,
                COUNT(a.shot_id) AS total,
                COALESCE(SUM(CASE WHEN sh.selected_video_asset_id IS NOT NULL
                    OR (sh.selected_image_asset_id IS NOT NULL AND NOT EXISTS (
                        SELECT 1 FROM shot_stage_configs vc
                        WHERE vc.shot_id = sh.id AND vc.stage = 'video'
                    )) THEN 1 ELSE 0 END), 0) AS completed
         FROM production_scenes c
         JOIN production_episodes e ON e.id = c.episode_id
         JOIN production_series s ON s.id = e.series_id
         LEFT JOIN shot_scene_assignments a ON a.scene_id = c.id
         LEFT JOIN shots sh ON sh.id = a.shot_id AND sh.project_id = ?
         WHERE s.project_id = ?
         GROUP BY c.id, c.name, s.name, e.name
         ORDER BY s.ordinal, e.ordinal, c.ordinal, c.id",
    )
    .bind(project_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| ProjectCommandCenterSceneView {
        id: row.id,
        name: row.name,
        path: format!("{} / {}", row.series_name, row.episode_name),
        total: count(row.total),
        completed: count(row.completed),
    })
    .collect();

    Ok(ProjectCommandCenterStructureView {
        series_count: count(row.series_count),
        episode_count: count(row.episode_count),
        scene_count: count(row.scene_count),
        assigned_shot_count: count(row.assigned_shot_count),
        unassigned_shot_count: count(row.unassigned_shot_count),
        first_unassigned_shot_id: row.first_unassigned_shot_id,
        blocked: row.orphan_count > 0,
        scenes,
    })
}

#[derive(Debug, FromRow)]
struct SceneSummaryRow {
    id: String,
    name: String,
    series_name: String,
    episode_name: String,
    total: i64,
    completed: i64,
}

#[derive(Debug, FromRow)]
struct ShotRow {
    id: String,
    selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct ShotConfigRow {
    shot_id: String,
    stage: String,
}

#[derive(Debug, FromRow)]
struct ShotLinkStatusRow {
    shot_id: String,
    stage: String,
    task_status: Option<String>,
}

async fn load_shots(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<ProjectCommandCenterShotView, ProjectCommandCenterError> {
    let shots = sqlx::query_as::<_, ShotRow>(
        "SELECT id, selected_image_asset_id, selected_video_asset_id
         FROM shots WHERE project_id = ? ORDER BY ordinal ASC, id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    if shots.is_empty() {
        return Ok(ProjectCommandCenterShotView::default());
    }

    let config_rows = sqlx::query_as::<_, ShotConfigRow>(
        "SELECT c.shot_id, c.stage
         FROM shot_stage_configs c JOIN shots sh ON sh.id = c.shot_id
         WHERE sh.project_id = ? ORDER BY c.shot_id, c.stage",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let mut configured = HashSet::with_capacity(config_rows.len());
    for row in config_rows {
        configured.insert((row.shot_id, row.stage));
    }

    let links = sqlx::query_as::<_, ShotLinkStatusRow>(
        "SELECT l.shot_id, l.stage, t.status AS task_status
         FROM shot_generation_links l
         JOIN shots sh ON sh.id = l.shot_id
         LEFT JOIN tasks t ON t.id = l.task_id
         WHERE sh.project_id = ?
         ORDER BY l.shot_id ASC, l.stage ASC, l.created_at DESC, l.id DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let mut latest_status = HashMap::<(String, String), String>::new();
    for row in links {
        if let Some(status) = row.task_status {
            latest_status
                .entry((row.shot_id, row.stage))
                .or_insert(status);
        }
    }

    let mut view = ProjectCommandCenterShotView {
        total: shots.len(),
        ..Default::default()
    };
    for shot in shots {
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
            "COMPLETED" => view.completed += 1,
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

#[derive(Debug, FromRow)]
struct QueueBatchRow {
    id: String,
    status: String,
    archived_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct QueueItemRow {
    id: String,
    batch_id: String,
    ordinal: i64,
    status: String,
    retry_of_item_id: Option<String>,
    error_code: Option<String>,
}

async fn load_queue(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<ProjectCommandCenterQueueView, ProjectCommandCenterError> {
    let batches = sqlx::query_as::<_, QueueBatchRow>(
        "SELECT id, status, archived_at FROM production_batches
         WHERE project_id = ? ORDER BY id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let items = sqlx::query_as::<_, QueueItemRow>(
        "SELECT i.id, i.batch_id, i.ordinal, i.status, i.retry_of_item_id, i.error_code
         FROM production_batch_items i JOIN production_batches b ON b.id = i.batch_id
         WHERE b.project_id = ? ORDER BY i.batch_id ASC, i.ordinal ASC, i.id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

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
    for batch in &batches {
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
    for item in &items {
        if active_batches.contains(&item.batch_id) {
            items_by_batch
                .entry(item.batch_id.clone())
                .or_default()
                .push(item);
            view.total_items += 1;
            match item.status.as_str() {
                "PENDING" => view.pending_items += 1,
                "DISPATCHING" | "DISPATCHED" => {
                    view.active_items += 1;
                    set_first(&mut view.first_active_batch_id, &item.batch_id);
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
            } else if matches!(leaf.status.as_str(), "FAILED" | "CANCELLED" | "SKIPPED") {
                view.review_required_items += 1;
                set_first(&mut view.first_review_required_batch_id, &batch_id);
            }
        }
    }
    let _ = active_batch_ids;
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

#[derive(Debug, FromRow)]
struct AssetTypeRow {
    asset_type: String,
    count: i64,
}

#[derive(Debug, FromRow)]
struct TaskCountRow {
    status: String,
    count: i64,
}

async fn load_tasks_assets(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<ProjectCommandCenterTaskAssetView, ProjectCommandCenterError> {
    let tasks = sqlx::query_as::<_, TaskCountRow>(
        "SELECT status, COUNT(*) AS count FROM tasks
         WHERE project_id = ? GROUP BY status ORDER BY status",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let assets = sqlx::query_as::<_, AssetTypeRow>(
        "SELECT UPPER(type) AS asset_type, COUNT(*) AS count FROM assets
         WHERE project_id = ? GROUP BY UPPER(type) ORDER BY UPPER(type)",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let mut view = ProjectCommandCenterTaskAssetView::default();
    for row in tasks {
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
    for row in assets {
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

#[derive(Debug, FromRow)]
struct AnchorRow {
    kind: String,
    asset_count: i64,
}

async fn load_reference_anchors(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<ProjectCommandCenterReferenceAnchorView, ProjectCommandCenterError> {
    let rows = sqlx::query_as::<_, AnchorRow>(
        "SELECT a.kind, COUNT(aa.asset_id) AS asset_count
         FROM reference_anchors a
         LEFT JOIN reference_anchor_assets aa ON aa.anchor_id = a.id
         WHERE a.project_id = ? GROUP BY a.id, a.kind ORDER BY a.kind, a.id",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let mut view = ProjectCommandCenterReferenceAnchorView {
        total: rows.len(),
        ..Default::default()
    };
    for row in rows {
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

#[derive(Debug, FromRow)]
struct PromptTemplateRow {
    id: String,
    name: String,
    version_count: i64,
    updated_at: String,
}

async fn load_prompt_templates(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<ProjectCommandCenterPromptTemplateSummary, ProjectCommandCenterError> {
    let rows = sqlx::query_as::<_, PromptTemplateRow>(
        "SELECT e.id, e.name, COUNT(v.id) AS version_count, e.updated_at
         FROM prompt_entries e
         JOIN prompt_versions v ON v.prompt_id = e.id
         WHERE e.project_id = ? AND e.kind = 'prompt'
           AND instr(v.text, '{{') > 0
         GROUP BY e.id, e.name, e.updated_at
         ORDER BY e.updated_at DESC, e.id DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let versions = rows.iter().map(|row| count(row.version_count)).sum();
    Ok(ProjectCommandCenterPromptTemplateSummary {
        total: rows.len(),
        versions,
        items: rows
            .into_iter()
            .map(|row| ProjectCommandCenterPromptTemplateView {
                id: row.id,
                name: row.name,
                version_count: count(row.version_count),
                updated_at: row.updated_at,
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
        ("assets", "资产库", "assets"),
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
    use crate::infrastructure::database::initialize;
    use tempfile::tempdir;

    const PROJECT: &str = "prj_00000000-0000-0000-0000-000000000039";
    const NOW: &str = "2026-08-18T00:00:00Z";

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
        assert_eq!(
            kind(&view),
            ProjectCommandCenterActionKind::ActiveProduction
        );

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
        assert_eq!(kind(&view), ProjectCommandCenterActionKind::Complete);
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

        let view = ProjectCommandCenterService::new(pool)
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

        let view = ProjectCommandCenterService::new(pool)
            .get(PROJECT)
            .await
            .expect("500-shot project should load");
        assert_eq!(view.shots.total, 500);
        assert_eq!(view.structure.unassigned_shot_count, 500);
        assert_eq!(view.tasks_assets.task_count, 0);
    }
}
