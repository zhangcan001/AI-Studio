//! Context-aware, generation-free production preparation.
//!
//! This service is the only place where a live context/readiness bundle is
//! turned into frozen queue values.  It never starts a queue, creates a Task,
//! or submits anything to ComfyUI.

use crate::application::generation_input_preparer::GenerationInputPreparer;
use crate::application::ports::{
    Clock, GenerationDefinition, GenerationDefinitionRepository, ProjectRepository,
    RepositoryError, ShotBatchBinding, ShotBatchRepository,
};
use crate::application::production_queue_service::generation_values_to_json;
use crate::application::shot_batch_service::{
    ShotBatchService, ShotBatchServiceError, MAX_SHOT_BATCH_ITEMS,
};
use crate::application::shot_readiness_service::{
    ReadinessBundle, ShotReadinessService, ShotReadinessServiceError,
};
use crate::compiler::{RecipeParser, WorkflowCompiler};
use crate::domain::production_preparation::{
    ComfyCapabilityEvidence, PreparationSnapshotIdentity, PreparationSnapshotRecord,
    PreparationSnapshotV1, ProductionPreparationAdmission, ResolvedShotContextView,
    ScenePreparationView, ShotProductionPlan, ShotProductionPlanSummary,
};
use crate::domain::{
    CompileRequest, ProductionBatch, ProductionBatchId, ProductionBatchItem, ProductionBatchItemId,
    ProductionBatchItemStatus, ProductionBatchStatus, ReadinessCheck, ReadinessCheckState,
    ReadinessGateKey, ReadinessGateResult, ResolvedShotContext, ShotReadiness, ShotReadinessStatus,
    ShotStage, WorkflowDocument,
};
use chrono::Utc;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

pub const PREPARATION_BATCH_LIMIT: usize = MAX_SHOT_BATCH_ITEMS;

#[derive(Clone, Debug)]
struct EvaluatedPreparation {
    context: ResolvedShotContext,
    readiness: ShotReadiness,
    values: Option<
        BTreeMap<String, crate::application::generation_input_preparer::GenerationInputValue>,
    >,
    current_stage_status: Option<String>,
    existing_batch_ids: Vec<String>,
    matching_prepared_batch_ids: Vec<String>,
    stale_prepared_batch_ids: Vec<String>,
    snapshot_identity: Option<PreparationSnapshotIdentity>,
    comfy_capability_evidence: ComfyCapabilityEvidence,
}

pub struct ProductionPreparationService {
    shot_batch_service: Arc<ShotBatchService>,
    shot_batch_repository: Arc<dyn ShotBatchRepository>,
    readiness_service: Arc<ShotReadinessService>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    clock: Arc<dyn Clock>,
    admission_gate: Mutex<()>,
}

impl ProductionPreparationService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shot_batch_service: Arc<ShotBatchService>,
        shot_batch_repository: Arc<dyn ShotBatchRepository>,
        readiness_service: Arc<ShotReadinessService>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            shot_batch_service,
            shot_batch_repository,
            readiness_service,
            definition_repository,
            project_repository,
            clock,
            admission_gate: Mutex::new(()),
        }
    }

    /// Runs one live resolver + preflight bundle for the requested Shots and
    /// returns complete plan details.  No queue data is written.
    pub async fn plan_many(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
    ) -> Result<Vec<ShotProductionPlan>, ProductionPreparationError> {
        validate_project(project_id)?;
        validate_shot_scope(shot_ids, 500)?;
        let evaluated = self.evaluate_many(project_id, shot_ids, stage).await?;
        Ok(evaluated.into_iter().map(to_plan).collect())
    }

    pub async fn plan_summaries(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
    ) -> Result<Vec<ShotProductionPlanSummary>, ProductionPreparationError> {
        Ok(self
            .plan_many(project_id, shot_ids, stage)
            .await?
            .iter()
            .map(ShotProductionPlanSummary::from)
            .collect())
    }

    pub async fn plan_detail(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
    ) -> Result<ShotProductionPlan, ProductionPreparationError> {
        let mut plans = self
            .plan_many(project_id, &[shot_id.to_owned()], stage)
            .await?;
        plans
            .pop()
            .ok_or_else(|| ProductionPreparationError::NotFound(shot_id.to_owned()))
    }

    /// Converts a set of summary cards into the scene-level response used by
    /// the preparation page.  The caller supplies the already-loaded scene
    /// identity; this function performs no database or runtime calls.
    pub fn scene_view(
        project_id: impl Into<String>,
        scene_id: impl Into<String>,
        scene_name: impl Into<String>,
        stage: ShotStage,
        items: Vec<ShotProductionPlanSummary>,
        evaluated_at: chrono::DateTime<Utc>,
    ) -> ScenePreparationView {
        let ready_count = items
            .iter()
            .filter(|item| item.status == ShotReadinessStatus::Ready)
            .count();
        let incomplete_count = items
            .iter()
            .filter(|item| item.status == ShotReadinessStatus::Incomplete)
            .count();
        let blocked_count = items
            .iter()
            .filter(|item| item.status == ShotReadinessStatus::Blocked)
            .count();
        let prepared_count = items.iter().filter(|item| item.already_prepared).count();
        let warning_count = items.iter().map(|item| item.warning_count).sum();
        ScenePreparationView {
            project_id: project_id.into(),
            scene_id: scene_id.into(),
            scene_name: scene_name.into(),
            stage: stage.as_str().to_owned(),
            total: items.len(),
            ready_count,
            incomplete_count,
            blocked_count,
            prepared_count,
            warning_count,
            items,
            evaluated_at,
        }
    }

    /// Re-resolves and live-preflights the submitted Shot IDs before writing.
    /// The request contains no client-provided values, hashes, or readiness.
    pub async fn admit(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
        allow_partial: bool,
    ) -> Result<ProductionPreparationAdmission, ProductionPreparationError> {
        validate_project(project_id)?;
        validate_shot_scope(shot_ids, PREPARATION_BATCH_LIMIT)?;
        let _guard = self.admission_gate.lock().await;
        let evaluated = self.evaluate_many(project_id, shot_ids, stage).await?;

        let skipped_incomplete = evaluated
            .iter()
            .filter(|item| item.readiness.status == ShotReadinessStatus::Incomplete)
            .count();
        let skipped_blocked = evaluated
            .iter()
            .filter(|item| item.readiness.status == ShotReadinessStatus::Blocked)
            .count();
        if !allow_partial && (skipped_incomplete > 0 || skipped_blocked > 0) {
            return Err(ProductionPreparationError::NotReady {
                incomplete: skipped_incomplete,
                blocked: skipped_blocked,
            });
        }

        let mut ready = Vec::new();
        let mut already_prepared_count = 0;
        let mut matching_prepared_batch_ids = BTreeSet::new();
        for item in evaluated {
            if item.readiness.status != ShotReadinessStatus::Ready {
                continue;
            }
            for batch_id in &item.matching_prepared_batch_ids {
                matching_prepared_batch_ids.insert(batch_id.clone());
            }
            if item.readiness.status == ShotReadinessStatus::Ready
                && (!item.matching_prepared_batch_ids.is_empty()
                    || (item.existing_batch_ids.len() > 0
                        && item.stale_prepared_batch_ids.is_empty()))
            {
                // A matching snapshot is the normal idempotent path.  A
                // legacy active Shot binding without a snapshot is also not
                // duplicated; changed-context preparations remain allowed
                // because they have an explicit stale snapshot record.
                already_prepared_count += 1;
                continue;
            }
            ready.push(item);
        }

        if ready.is_empty() {
            return Ok(ProductionPreparationAdmission {
                project_id: project_id.to_owned(),
                stage: stage.as_str().to_owned(),
                requested_count: shot_ids.len(),
                created_count: 0,
                skipped_incomplete,
                skipped_blocked,
                already_prepared_count,
                created_batch_ids: Vec::new(),
                matching_prepared_batch_ids: matching_prepared_batch_ids.into_iter().collect(),
            });
        }

        let project = self
            .project_repository
            .find_by_id(project_id)
            .await?
            .ok_or_else(|| ProductionPreparationError::NotFound(project_id.to_owned()))?;
        let now = self.clock.now();
        let batch_id = ProductionBatchId::new();
        let label = match stage {
            ShotStage::Image => "镜头关键帧准备",
            ShotStage::Video => "镜头视频准备",
        };
        let batch = ProductionBatch {
            id: batch_id.clone(),
            project_id: project_id.to_owned(),
            name: format!(
                "{label} · {} · {}",
                project.name,
                now.format("%Y-%m-%d %H:%M:%S")
            ),
            status: ProductionBatchStatus::Ready,
            continue_on_failure: stage == ShotStage::Video,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        let mut items = Vec::with_capacity(ready.len());
        let mut bindings = Vec::with_capacity(ready.len());
        let mut snapshots = Vec::with_capacity(ready.len());
        for (ordinal, evaluated) in ready.iter().enumerate() {
            let values = evaluated.values.clone().ok_or_else(|| {
                ProductionPreparationError::InvalidInput(format!(
                    "镜头 {} 缺少可冻结的 generation values",
                    evaluated.context.structure.shot.id
                ))
            })?;
            let workflow_version_id = evaluated
                .context
                .workflow
                .workflow_version_id
                .clone()
                .ok_or_else(|| {
                    ProductionPreparationError::InvalidInput("工作流版本缺失".to_owned())
                })?;
            let recipe_id = evaluated
                .context
                .workflow
                .recipe_id
                .clone()
                .ok_or_else(|| {
                    ProductionPreparationError::InvalidInput("Recipe 缺失".to_owned())
                })?;
            let item_id = ProductionBatchItemId::new();
            let values_json = generation_values_to_json(&values);
            let item = ProductionBatchItem {
                id: item_id.clone(),
                batch_id: batch_id.clone(),
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    ProductionPreparationError::InvalidInput("批次序号超出范围".to_owned())
                })?,
                workflow_version_id,
                recipe_id,
                values_json: values_json.clone(),
                status: ProductionBatchItemStatus::Pending,
                task_id: None,
                retry_of_item_id: None,
                error_code: None,
                error_message: None,
                created_at: now,
                updated_at: now,
            };
            let item_id_string = item_id.as_str().to_owned();
            let snapshot_id = format!("pps_{}", Uuid::new_v4().simple());
            let snapshot = PreparationSnapshotV1::from_context(
                &evaluated.context,
                &evaluated.readiness,
                values_json,
                evaluated.comfy_capability_evidence.clone(),
                now,
            );
            snapshots.push(PreparationSnapshotRecord {
                id: snapshot_id,
                project_id: project_id.to_owned(),
                shot_id: evaluated.context.structure.shot.id.clone(),
                stage,
                context_hash: evaluated.context.resolver_identity.context_hash.clone(),
                production_batch_id: batch_id.as_str().to_owned(),
                production_batch_item_id: item_id_string.clone(),
                snapshot,
                created_at: now,
            });
            bindings.push(ShotBatchBinding {
                shot_id: evaluated.context.structure.shot.id.clone(),
                stage,
                production_batch_item_id: item_id_string,
            });
            items.push(item);
        }
        self.shot_batch_service
            .insert_prepared_batch_with_bindings(&batch, &items, &bindings, &snapshots)
            .await?;

        Ok(ProductionPreparationAdmission {
            project_id: project_id.to_owned(),
            stage: stage.as_str().to_owned(),
            requested_count: shot_ids.len(),
            created_count: items.len(),
            skipped_incomplete,
            skipped_blocked,
            already_prepared_count,
            created_batch_ids: vec![batch.id.as_str().to_owned()],
            matching_prepared_batch_ids: matching_prepared_batch_ids.into_iter().collect(),
        })
    }

    /// Read one frozen snapshot for audit/inspector use without exposing a
    /// second runtime or generation path.
    pub async fn preparation_snapshot(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
    ) -> Result<Option<PreparationSnapshotRecord>, ProductionPreparationError> {
        validate_project(project_id)?;
        Ok(self
            .shot_batch_repository
            .find_preparation_snapshot(project_id, production_batch_item_id)
            .await?)
    }

    async fn evaluate_many(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
    ) -> Result<Vec<EvaluatedPreparation>, ProductionPreparationError> {
        let ReadinessBundle {
            contexts,
            readiness,
            evaluated_at: _,
            comfy_capability_evidence,
        } = self
            .readiness_service
            .preflight_bundle_many(project_id, shot_ids, stage)
            .await?;
        if contexts.len() != readiness.len() {
            return Err(ProductionPreparationError::InvalidInput(
                "上下文与 readiness 数量不一致".to_owned(),
            ));
        }
        let active_bindings = self
            .shot_batch_service
            .list_active_shot_bindings(project_id, stage, shot_ids)
            .await?;
        let prepared_records = self
            .shot_batch_service
            .list_prepared_shot_records(project_id, stage, shot_ids)
            .await?;
        let statuses = self
            .shot_batch_service
            .current_stage_statuses(project_id, stage, shot_ids)
            .await?;
        let mut definitions = HashMap::<(String, String), Option<GenerationDefinition>>::new();
        let mut result = Vec::with_capacity(contexts.len());
        for (context, readiness) in contexts.into_iter().zip(readiness) {
            let workflow_key = context
                .workflow
                .workflow_version_id
                .clone()
                .zip(context.workflow.recipe_id.clone());
            let definition = if let Some((workflow_version_id, recipe_id)) = workflow_key.as_ref() {
                let key = (workflow_version_id.clone(), recipe_id.clone());
                if !definitions.contains_key(&key) {
                    definitions.insert(
                        key.clone(),
                        self.definition_repository
                            .find(workflow_version_id, recipe_id)
                            .await?,
                    );
                }
                definitions.get(&key).cloned().flatten()
            } else {
                None
            };
            let (readiness, values) = if workflow_key.is_some() && definition.is_none() {
                (
                    readiness_with_blocker(
                        readiness,
                        &context,
                        "GENERATION_DEFINITION_NOT_FOUND",
                        "所选工作流 Recipe 定义不存在。".to_owned(),
                    ),
                    None,
                )
            } else {
                prepare_generation_values(&context, readiness, definition.as_ref())
            };
            let shot_id = context.structure.shot.id.clone();
            let existing_batch_ids = active_bindings
                .iter()
                .filter(|binding| binding.shot_id == shot_id)
                .map(|binding| binding.production_batch_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let matching = prepared_records
                .iter()
                .filter(|record| {
                    record.shot_id == shot_id
                        && record.context_hash == context.resolver_identity.context_hash
                })
                .collect::<Vec<_>>();
            let matching_prepared_batch_ids = matching
                .iter()
                .map(|record| record.production_batch_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let stale_prepared_batch_ids = prepared_records
                .iter()
                .filter(|record| {
                    record.shot_id == shot_id
                        && record.context_hash != context.resolver_identity.context_hash
                })
                .map(|record| record.production_batch_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let snapshot_identity = matching.first().map(|record| PreparationSnapshotIdentity {
                snapshot_id: record.snapshot_id.clone(),
                production_batch_id: record.production_batch_id.clone(),
                production_batch_item_id: record.production_batch_item_id.clone(),
                context_hash: record.context_hash.clone(),
            });
            result.push(EvaluatedPreparation {
                current_stage_status: statuses.get(&shot_id).cloned(),
                context,
                readiness,
                values,
                existing_batch_ids,
                matching_prepared_batch_ids,
                stale_prepared_batch_ids,
                snapshot_identity,
                comfy_capability_evidence: comfy_capability_evidence.clone(),
            });
        }
        Ok(result)
    }
}

fn prepare_generation_values(
    context: &ResolvedShotContext,
    readiness: ShotReadiness,
    definition: Option<&GenerationDefinition>,
) -> (
    ShotReadiness,
    Option<BTreeMap<String, crate::application::generation_input_preparer::GenerationInputValue>>,
) {
    let Some(definition) = definition else {
        return (readiness, None);
    };
    let recipe = match RecipeParser::parse(&definition.recipe_yaml) {
        Ok(recipe) => recipe,
        Err(error) => {
            return (
                readiness_with_blocker(
                    readiness,
                    context,
                    "GENERATION_RECIPE_INVALID",
                    format!("Recipe 无法解析：{error}"),
                ),
                None,
            )
        }
    };
    let values =
        match ShotBatchService::prepare_values_from_context(context.stage, context, &recipe) {
            Ok(values) => values,
            Err(error) => {
                return (
                    readiness_with_blocker(readiness, context, "GENERATION_VALUES_INVALID", error),
                    None,
                )
            }
        };
    let workflow = match WorkflowDocument::parse(definition.workflow_json.clone()) {
        Ok(workflow) => workflow,
        Err(error) => {
            return (
                readiness_with_blocker(
                    readiness,
                    context,
                    "GENERATION_WORKFLOW_INVALID",
                    format!("工作流无法解析：{error}"),
                ),
                None,
            )
        }
    };
    let request = CompileRequest::new(GenerationInputPreparer::preflight_values(&values));
    if let Err(error) = WorkflowCompiler.compile(&workflow, &recipe, &request) {
        return (
            readiness_with_blocker(
                readiness,
                context,
                "GENERATION_VALUES_INVALID",
                format!("输入检查失败：{error}"),
            ),
            None,
        );
    }
    (readiness, Some(values))
}

fn readiness_with_blocker(
    readiness: ShotReadiness,
    context: &ResolvedShotContext,
    code: &str,
    message: String,
) -> ShotReadiness {
    let mut gates = readiness.gates.clone();
    let check = ReadinessCheck::new(
        ReadinessGateKey::Workflow,
        ReadinessCheckState::Blocker,
        code,
        message,
        "ProductionPreparationService",
    );
    if let Some(gate) = gates
        .iter_mut()
        .find(|gate| gate.key == ReadinessGateKey::Workflow)
    {
        gate.checks.push(check);
        gate.state = gate
            .checks
            .iter()
            .map(|check| check.state)
            .max_by_key(|state| state.severity())
            .unwrap_or(ReadinessCheckState::Pass);
    } else {
        gates.push(ReadinessGateResult::new(
            ReadinessGateKey::Workflow,
            vec![check],
        ));
    }
    ShotReadiness::from_gates(
        readiness.project_id,
        readiness.shot_id,
        readiness.stage,
        readiness.context_hash,
        gates,
        readiness.evaluated_at,
        readiness.comfy_checked_at,
        readiness.cached,
        context.partial,
    )
}

fn validate_project(project_id: &str) -> Result<(), ProductionPreparationError> {
    crate::domain::validate_project_id(project_id)
        .map_err(|error| ProductionPreparationError::InvalidInput(error.to_string()))
}

fn validate_shot_scope(
    shot_ids: &[String],
    limit: usize,
) -> Result<(), ProductionPreparationError> {
    if shot_ids.is_empty() {
        return Err(ProductionPreparationError::InvalidInput(
            "至少需要一个镜头".to_owned(),
        ));
    }
    if shot_ids.len() > limit {
        return Err(ProductionPreparationError::BatchLimit { limit });
    }
    let mut seen = HashSet::with_capacity(shot_ids.len());
    if shot_ids.iter().any(|shot_id| !seen.insert(shot_id)) {
        return Err(ProductionPreparationError::InvalidInput(
            "镜头不能重复".to_owned(),
        ));
    }
    Ok(())
}

fn to_plan(evaluated: EvaluatedPreparation) -> ShotProductionPlan {
    let context_hash = evaluated.context.resolver_identity.context_hash.clone();
    let blockers = readiness_messages(&evaluated.readiness, ReadinessCheckState::Blocker);
    let mut warnings = readiness_messages(&evaluated.readiness, ReadinessCheckState::Warning);
    if !evaluated.stale_prepared_batch_ids.is_empty() {
        warnings.push("已有旧上下文准备版本".to_owned());
    }
    ShotProductionPlan {
        project_id: evaluated.context.project_id.clone(),
        shot_id: evaluated.context.structure.shot.id.clone(),
        ordinal: evaluated.context.structure.shot.ordinal,
        name: evaluated.context.structure.shot.name.clone(),
        scene_id: evaluated
            .context
            .structure
            .scene
            .as_ref()
            .map(|value| value.id.clone()),
        stage: evaluated.context.stage.as_str().to_owned(),
        context_hash,
        resolved_context: ResolvedShotContextView::from(&evaluated.context),
        readiness: evaluated.readiness,
        workflow_version_id: evaluated.context.workflow.workflow_version_id.clone(),
        recipe_id: evaluated.context.workflow.recipe_id.clone(),
        current_stage_status: evaluated.current_stage_status,
        existing_batch_ids: evaluated.existing_batch_ids,
        matching_prepared_batch_ids: evaluated.matching_prepared_batch_ids,
        stale_prepared_batch_ids: evaluated.stale_prepared_batch_ids,
        already_prepared: evaluated.snapshot_identity.is_some(),
        blockers,
        warnings,
        snapshot_identity: evaluated.snapshot_identity,
    }
}

impl From<&ShotProductionPlan> for ShotProductionPlanSummary {
    fn from(plan: &ShotProductionPlan) -> Self {
        let warning_count = plan
            .readiness
            .gates
            .iter()
            .filter(|gate| gate.state == ReadinessCheckState::Warning)
            .count();
        let incomplete_count = plan
            .readiness
            .gates
            .iter()
            .filter(|gate| gate.state == ReadinessCheckState::Incomplete)
            .count();
        let blocker_count = plan
            .readiness
            .gates
            .iter()
            .filter(|gate| gate.state == ReadinessCheckState::Blocker)
            .count();
        let character_names = plan
            .resolved_context
            .profiles
            .characters
            .iter()
            .map(|profile| profile.profile_id.clone())
            .collect::<Vec<_>>();
        ShotProductionPlanSummary {
            shot_id: plan.shot_id.clone(),
            ordinal: plan.ordinal,
            name: plan.name.clone(),
            status: plan.readiness.status,
            score: plan.readiness.score,
            warning_count,
            incomplete_count,
            blocker_count,
            context_hash: plan.context_hash.clone(),
            character_count: character_names.len(),
            character_names,
            scene_profile_name: plan
                .resolved_context
                .profiles
                .scene
                .as_ref()
                .map(|profile| profile.profile_id.clone()),
            reference_count: plan.resolved_context.reference_assets.len(),
            workflow_version_id: plan.workflow_version_id.clone(),
            recipe_id: plan.recipe_id.clone(),
            current_stage_status: plan.current_stage_status.clone(),
            already_prepared: plan.already_prepared,
            existing_batch_ids: plan.existing_batch_ids.clone(),
            matching_prepared_batch_ids: plan.matching_prepared_batch_ids.clone(),
            stale_prepared_batch_ids: plan.stale_prepared_batch_ids.clone(),
            blockers: plan.blockers.clone(),
            warnings: plan.warnings.clone(),
            legacy: plan.resolved_context.legacy.uses_legacy_shot_references,
        }
    }
}

fn readiness_messages(readiness: &ShotReadiness, state: ReadinessCheckState) -> Vec<String> {
    readiness
        .gates
        .iter()
        .flat_map(|gate| gate.checks.iter())
        .filter(|check| check.state == state)
        .map(|check| check.message.clone())
        .collect()
}

#[derive(Debug)]
pub enum ProductionPreparationError {
    InvalidInput(String),
    BatchLimit { limit: usize },
    NotFound(String),
    NotReady { incomplete: usize, blocked: usize },
    Repository(RepositoryError),
    ShotBatch(ShotBatchServiceError),
    Readiness(ShotReadinessServiceError),
}

impl fmt::Display for ProductionPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::BatchLimit { limit } => {
                write!(formatter, "PREPARATION_BATCH_LIMIT: at most {limit} shots")
            }
            Self::NotFound(id) => write!(formatter, "PREPARATION_NOT_FOUND: {id}"),
            Self::NotReady {
                incomplete,
                blocked,
            } => write!(
                formatter,
                "PREPARATION_NOT_READY: {incomplete} incomplete and {blocked} blocked shots"
            ),
            Self::Repository(error) => error.fmt(formatter),
            Self::ShotBatch(error) => error.fmt(formatter),
            Self::Readiness(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionPreparationError {}

impl From<RepositoryError> for ProductionPreparationError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<ShotBatchServiceError> for ProductionPreparationError {
    fn from(error: ShotBatchServiceError) -> Self {
        Self::ShotBatch(error)
    }
}

impl From<ShotReadinessServiceError> for ProductionPreparationError {
    fn from(error: ShotReadinessServiceError) -> Self {
        Self::Readiness(error)
    }
}
