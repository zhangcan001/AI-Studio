use crate::application::comfy_preflight_service::ComfyPreflightService;
use crate::application::ports::{ProductionStructureRepository, RepositoryError};
use crate::application::shot_context_resolver::{
    ShotContextResolver, ShotContextResolverError, CONTEXT_BATCH_LIMIT,
};
use crate::application::shot_readiness_evaluator::{
    evaluate, ReadinessEnvironmentSnapshot, ReadinessEvaluationInput, ReadinessStageInput,
};
use crate::application::workflow_lifecycle_service::{
    WorkflowLifecycleService, WorkflowProductionWorkspaceResponse,
};
use crate::domain::shot::ShotStage;
use crate::domain::shot_context::ResolvedShotContext;
use crate::domain::shot_readiness::{ReadinessCheckState, ShotReadiness, ShotReadinessStatus};
use chrono::{DateTime, Utc};
use std::fmt;
use std::sync::Arc;

pub const READINESS_BATCH_LIMIT: usize = CONTEXT_BATCH_LIMIT;

#[derive(Debug)]
pub enum ShotReadinessServiceError {
    BatchLimit { limit: usize },
    Context(String),
    Repository(String),
    Comfy(String),
    Workflow(String),
    SceneNotFound(String),
}

impl fmt::Display for ShotReadinessServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BatchLimit { limit } => {
                write!(formatter, "READINESS_BATCH_LIMIT: at most {limit} shots")
            }
            Self::Context(message) => write!(formatter, "{message}"),
            Self::Repository(message) => write!(formatter, "REPOSITORY_ERROR: {message}"),
            Self::Comfy(message) => write!(formatter, "COMFY_PREFLIGHT_ERROR: {message}"),
            Self::Workflow(message) => write!(formatter, "WORKFLOW_ERROR: {message}"),
            Self::SceneNotFound(id) => write!(formatter, "SCENE_NOT_FOUND: {id}"),
        }
    }
}

impl std::error::Error for ShotReadinessServiceError {}

impl From<RepositoryError> for ShotReadinessServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error.to_string())
    }
}

impl From<ShotContextResolverError> for ShotReadinessServiceError {
    fn from(error: ShotContextResolverError) -> Self {
        match error {
            ShotContextResolverError::ContextBatchLimit { limit } => Self::BatchLimit { limit },
            other => Self::Context(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotReadinessSummaryItem {
    pub shot_id: String,
    pub ordinal: u32,
    pub name: String,
    pub status: ShotReadinessStatus,
    pub score: i32,
    pub warning_count: usize,
    pub incomplete_count: usize,
    pub blocker_count: usize,
    pub context_hash: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneReadinessSummary {
    pub project_id: String,
    pub scene_id: String,
    pub stage: String,
    pub total: usize,
    pub ready: usize,
    pub incomplete: usize,
    pub blocked: usize,
    pub warning_count: usize,
    pub items: Vec<ShotReadinessSummaryItem>,
}

pub struct ShotReadinessService {
    resolver: Arc<ShotContextResolver>,
    comfy_preflight_service: Arc<ComfyPreflightService>,
    workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
    structure_repository: Arc<dyn ProductionStructureRepository>,
}

impl ShotReadinessService {
    pub fn new(
        resolver: Arc<ShotContextResolver>,
        comfy_preflight_service: Arc<ComfyPreflightService>,
        workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
        structure_repository: Arc<dyn ProductionStructureRepository>,
    ) -> Self {
        Self {
            resolver,
            comfy_preflight_service,
            workflow_lifecycle_service,
            structure_repository,
        }
    }

    pub async fn readiness_cached(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
    ) -> Result<ShotReadiness, ShotReadinessServiceError> {
        let contexts = self
            .resolve_many(project_id, &[shot_id.to_owned()], stage)
            .await?;
        let report = self.comfy_preflight_service.cached_current().await;
        let workspace = self.workspace().await?;
        let snapshot = ReadinessEnvironmentSnapshot::new(report, workspace);
        evaluate_context(
            contexts.into_iter().next().ok_or_else(|| {
                ShotReadinessServiceError::Context(format!("CONTEXT_SHOT_NOT_FOUND: {shot_id}"))
            })?,
            &snapshot,
            true,
            Utc::now(),
        )
    }

    pub async fn preflight(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
    ) -> Result<ShotReadiness, ShotReadinessServiceError> {
        let contexts = self
            .resolve_many(project_id, &[shot_id.to_owned()], stage)
            .await?;
        let report = self
            .comfy_preflight_service
            .current()
            .await
            .map_err(|error| ShotReadinessServiceError::Comfy(error.to_string()))?;
        let workspace = self.workspace().await?;
        let snapshot = ReadinessEnvironmentSnapshot::new(Some(report), workspace);
        evaluate_context(
            contexts.into_iter().next().ok_or_else(|| {
                ShotReadinessServiceError::Context(format!("CONTEXT_SHOT_NOT_FOUND: {shot_id}"))
            })?,
            &snapshot,
            false,
            Utc::now(),
        )
    }

    pub async fn readiness_many_cached(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
    ) -> Result<Vec<ShotReadiness>, ShotReadinessServiceError> {
        self.evaluate_many(project_id, shot_ids, stage, true).await
    }

    pub async fn preflight_many(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
    ) -> Result<Vec<ShotReadiness>, ShotReadinessServiceError> {
        self.evaluate_many(project_id, shot_ids, stage, false).await
    }

    pub async fn scene_readiness_cached(
        &self,
        project_id: &str,
        scene_id: &str,
        stage: ShotStage,
    ) -> Result<SceneReadinessSummary, ShotReadinessServiceError> {
        self.scene_summary(project_id, scene_id, stage, true).await
    }

    pub async fn scene_preflight(
        &self,
        project_id: &str,
        scene_id: &str,
        stage: ShotStage,
    ) -> Result<SceneReadinessSummary, ShotReadinessServiceError> {
        self.scene_summary(project_id, scene_id, stage, false).await
    }

    async fn evaluate_many(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
        cached: bool,
    ) -> Result<Vec<ShotReadiness>, ShotReadinessServiceError> {
        if shot_ids.len() > READINESS_BATCH_LIMIT {
            return Err(ShotReadinessServiceError::BatchLimit {
                limit: READINESS_BATCH_LIMIT,
            });
        }
        if shot_ids.is_empty() {
            return Ok(Vec::new());
        }
        let contexts = self.resolve_many(project_id, shot_ids, stage).await?;
        let report = if cached {
            self.comfy_preflight_service.cached_current().await
        } else {
            Some(
                self.comfy_preflight_service
                    .current()
                    .await
                    .map_err(|error| ShotReadinessServiceError::Comfy(error.to_string()))?,
            )
        };
        let workspace = self.workspace().await?;
        let snapshot = ReadinessEnvironmentSnapshot::new(report, workspace);
        let evaluated_at = Utc::now();
        contexts
            .into_iter()
            .map(|context| evaluate_context(context, &snapshot, cached, evaluated_at))
            .collect()
    }

    async fn scene_summary(
        &self,
        project_id: &str,
        scene_id: &str,
        stage: ShotStage,
        cached: bool,
    ) -> Result<SceneReadinessSummary, ShotReadinessServiceError> {
        let tree = self.structure_repository.load_tree_data(project_id).await?;
        if !tree
            .scenes
            .iter()
            .any(|scene| scene.id.as_str() == scene_id)
        {
            return Err(ShotReadinessServiceError::SceneNotFound(
                scene_id.to_owned(),
            ));
        }
        let mut assigned = tree
            .assignments
            .iter()
            .filter(|assignment| assignment.scene_id.as_str() == scene_id)
            .map(|assignment| (assignment.ordinal, assignment.shot_id.clone()))
            .collect::<Vec<_>>();
        assigned.sort_by_key(|(ordinal, shot_id)| (*ordinal, shot_id.clone()));
        if assigned.len() > READINESS_BATCH_LIMIT {
            return Err(ShotReadinessServiceError::BatchLimit {
                limit: READINESS_BATCH_LIMIT,
            });
        }
        let shot_ids = assigned
            .iter()
            .map(|(_, id)| id.clone())
            .collect::<Vec<_>>();
        let contexts = self.resolve_many(project_id, &shot_ids, stage).await?;
        let report = if cached {
            self.comfy_preflight_service.cached_current().await
        } else {
            Some(
                self.comfy_preflight_service
                    .current()
                    .await
                    .map_err(|error| ShotReadinessServiceError::Comfy(error.to_string()))?,
            )
        };
        let workspace = self.workspace().await?;
        let snapshot = ReadinessEnvironmentSnapshot::new(report, workspace);
        let evaluated_at = Utc::now();
        let evaluated = contexts
            .into_iter()
            .map(|context| {
                let readiness = evaluate_context(context.clone(), &snapshot, cached, evaluated_at)?;
                let item = summary_item(
                    &readiness,
                    context.structure.shot.ordinal,
                    context.structure.shot.name.clone(),
                );
                Ok::<_, ShotReadinessServiceError>((context.structure.shot.id, item))
            })
            .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
        let items = assigned
            .iter()
            .filter_map(|(_, shot_id)| evaluated.get(shot_id).cloned())
            .collect::<Vec<_>>();
        Ok(scene_summary_from_items(
            project_id.to_owned(),
            scene_id.to_owned(),
            stage,
            items,
        ))
    }

    async fn resolve_many(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
    ) -> Result<Vec<ResolvedShotContext>, ShotReadinessServiceError> {
        self.resolver
            .resolve_many_draft(project_id, shot_ids, stage)
            .await
            .map_err(Into::into)
    }

    async fn workspace(
        &self,
    ) -> Result<WorkflowProductionWorkspaceResponse, ShotReadinessServiceError> {
        self.workflow_lifecycle_service
            .list_workspace()
            .await
            .map_err(|error| ShotReadinessServiceError::Workflow(error.to_string()))
    }
}

pub fn evaluate_context(
    context: ResolvedShotContext,
    environment: &ReadinessEnvironmentSnapshot,
    cached: bool,
    evaluated_at: DateTime<Utc>,
) -> Result<ShotReadiness, ShotReadinessServiceError> {
    let stage_input = ReadinessStageInput {
        selected_image_asset_id: context.stage_input.selected_image_asset_id.clone(),
        selected_image_sha256: context.stage_input.selected_image_sha256.clone(),
    };
    Ok(evaluate(&ReadinessEvaluationInput {
        context: &context,
        environment,
        stage_input: Some(&stage_input),
        evaluated_at,
        cached,
    }))
}

fn summary_item(readiness: &ShotReadiness, ordinal: u32, name: String) -> ShotReadinessSummaryItem {
    let warning_count = readiness
        .gates
        .iter()
        .filter(|gate| gate.state == ReadinessCheckState::Warning)
        .count();
    let incomplete_count = readiness
        .gates
        .iter()
        .filter(|gate| gate.state == ReadinessCheckState::Incomplete)
        .count();
    let blocker_count = readiness
        .gates
        .iter()
        .filter(|gate| gate.state == ReadinessCheckState::Blocker)
        .count();
    ShotReadinessSummaryItem {
        shot_id: readiness.shot_id.clone(),
        ordinal,
        name,
        status: readiness.status,
        score: readiness.score,
        warning_count,
        incomplete_count,
        blocker_count,
        context_hash: readiness.context_hash.clone(),
    }
}

fn scene_summary_from_items(
    project_id: String,
    scene_id: String,
    stage: ShotStage,
    items: Vec<ShotReadinessSummaryItem>,
) -> SceneReadinessSummary {
    let ready = items
        .iter()
        .filter(|item| item.status == ShotReadinessStatus::Ready)
        .count();
    let incomplete = items
        .iter()
        .filter(|item| item.status == ShotReadinessStatus::Incomplete)
        .count();
    let blocked = items
        .iter()
        .filter(|item| item.status == ShotReadinessStatus::Blocked)
        .count();
    let warning_count = items.iter().map(|item| item.warning_count).sum();
    SceneReadinessSummary {
        project_id,
        scene_id,
        stage: stage.as_str().to_owned(),
        total: items.len(),
        ready,
        incomplete,
        blocked,
        warning_count,
        items,
    }
}
