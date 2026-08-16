use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::ports::{
    Clock, GenerationDefinitionRepository, PresetRepository, RepositoryError,
};
use crate::application::production_queue_service::{
    generation_values_from_json, generation_values_to_json, CreateProductionBatchItem,
    CreateProductionBatchRequest, ProductionQueueError, ProductionQueueService,
};
use crate::compiler::{RecipeParser, RecipeValidator};
use crate::domain::{InputDefinition, OutputType, PresetId, Recipe, SeedValue};
use chrono::DateTime;
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};
use uuid::Uuid;

pub const MAX_BENCHMARK_CANDIDATES: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowBenchmarkCandidateRequest {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub preset_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowBenchmarkCreateRequest {
    pub project_id: String,
    pub name: String,
    pub media_type: String,
    pub base_values: BTreeMap<String, GenerationInputValue>,
    pub candidates: Vec<WorkflowBenchmarkCandidateRequest>,
    pub seed_mode: String,
    pub fixed_seed: Option<u64>,
    pub auto_start: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkCandidatePreviewView {
    pub id: String,
    pub position: u32,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub preset_id: Option<String>,
    pub preset_name: Option<String>,
    pub label: String,
    pub compatibility: String,
    pub compatibility_reasons: Vec<String>,
    pub frozen_values: Value,
    pub asset_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkCandidateView {
    pub id: String,
    pub position: u32,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub preset_id: Option<String>,
    pub preset_name: Option<String>,
    pub label: String,
    pub compatibility: String,
    pub compatibility_reasons: Vec<String>,
    pub frozen_values: Value,
    pub asset_ids: Vec<String>,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub task_status: Option<String>,
    pub task_created_at: Option<String>,
    pub task_started_at: Option<String>,
    pub task_finished_at: Option<String>,
    pub execution_duration_ms: Option<i64>,
    pub telemetry: Option<WorkflowBenchmarkTelemetryView>,
    pub output_asset_ids: Vec<String>,
    pub review_status: Option<String>,
    pub review_note: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkTelemetryView {
    pub compiled_workflow_sha256: Option<String>,
    pub runtime_profile: Option<String>,
    pub queue_wait_ms: Option<i64>,
    pub prepare_ms: Option<i64>,
    pub comfy_execution_ms: Option<i64>,
    pub collection_ms: Option<i64>,
    pub total_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkSummaryView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub media_type: String,
    pub status: String,
    pub winner_candidate_id: Option<String>,
    pub production_batch_id: Option<String>,
    pub candidate_count: usize,
    pub succeeded_count: usize,
    pub failed_count: usize,
    pub fastest_candidate_id: Option<String>,
    pub fastest_duration_ms: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub media_type: String,
    pub status: String,
    pub base_values: Value,
    pub asset_ids: Vec<String>,
    pub winner_candidate_id: Option<String>,
    pub production_batch_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub candidates: Vec<WorkflowBenchmarkCandidateView>,
    pub summary: WorkflowBenchmarkSummaryView,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkDeleteView {
    pub deleted: bool,
    pub experiment_id: String,
}

#[derive(Clone, Debug)]
struct CandidateDraft {
    id: String,
    position: u32,
    workflow_version_id: String,
    recipe_id: String,
    preset_id: Option<String>,
    preset_name: Option<String>,
    label: String,
    compatibility: String,
    compatibility_reasons: Vec<String>,
    values: BTreeMap<String, GenerationInputValue>,
    values_json: Value,
    asset_ids: Vec<String>,
}

#[derive(Debug)]
pub enum WorkflowBenchmarkError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
    Queue(String),
    InvalidRecipe(String),
    Serialization(String),
}

impl fmt::Display for WorkflowBenchmarkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::NotFound(message) => write!(formatter, "BENCHMARK_NOT_FOUND: {message}"),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Queue(message) => write!(formatter, "BENCHMARK_QUEUE_ERROR: {message}"),
            Self::InvalidRecipe(message) => write!(formatter, "RECIPE_INVALID: {message}"),
            Self::Serialization(message) => {
                write!(formatter, "BENCHMARK_SERIALIZATION_ERROR: {message}")
            }
        }
    }
}

impl Error for WorkflowBenchmarkError {}

impl From<RepositoryError> for WorkflowBenchmarkError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[derive(Clone)]
pub struct WorkflowBenchmarkService {
    pool: SqlitePool,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    preset_repository: Arc<dyn PresetRepository>,
    production_queue_service: Arc<ProductionQueueService>,
    clock: Arc<dyn Clock>,
}

impl WorkflowBenchmarkService {
    pub fn new(
        pool: SqlitePool,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        preset_repository: Arc<dyn PresetRepository>,
        production_queue_service: Arc<ProductionQueueService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            pool,
            definition_repository,
            preset_repository,
            production_queue_service,
            clock,
        }
    }

    pub async fn preview(
        &self,
        request: &WorkflowBenchmarkCreateRequest,
    ) -> Result<Vec<WorkflowBenchmarkCandidatePreviewView>, WorkflowBenchmarkError> {
        validate_request_shape(request)?;
        let drafts = self.build_candidate_drafts(request).await?;
        Ok(drafts.into_iter().map(CandidateDraft::preview).collect())
    }

    pub async fn create(
        &self,
        request: WorkflowBenchmarkCreateRequest,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_request_shape(&request)?;
        let drafts = self.build_candidate_drafts(&request).await?;
        let incompatible = drafts
            .iter()
            .filter(|candidate| candidate.compatibility == "INCOMPATIBLE")
            .map(|candidate| {
                format!(
                    "{}：{}",
                    candidate.label,
                    candidate.compatibility_reasons.join("；")
                )
            })
            .collect::<Vec<_>>();
        if !incompatible.is_empty() {
            return Err(WorkflowBenchmarkError::InvalidInput(format!(
                "存在不可比较的候选：{}",
                incompatible.join(" | ")
            )));
        }

        let experiment_id = format!("bmk_{}", Uuid::new_v4().simple());
        let now = self.clock.now().to_rfc3339();
        let base_values_json = generation_values_to_json(&request.base_values);
        let asset_ids = collect_asset_ids(&request.base_values);
        let asset_ids_json = serde_json::to_string(&asset_ids)
            .map_err(|error| WorkflowBenchmarkError::Serialization(error.to_string()))?;
        self.insert_draft(
            &experiment_id,
            &request,
            &base_values_json,
            &asset_ids_json,
            &now,
            &drafts,
        )
        .await?;

        let queue_request = CreateProductionBatchRequest {
            project_id: request.project_id.clone(),
            name: format!("Benchmark · {}", request.name.trim()),
            continue_on_failure: true,
            items: drafts
                .iter()
                .map(|candidate| CreateProductionBatchItem {
                    workflow_version_id: candidate.workflow_version_id.clone(),
                    recipe_id: candidate.recipe_id.clone(),
                    values: candidate.values.clone(),
                })
                .collect(),
        };
        let queue = match self.production_queue_service.create(queue_request).await {
            Ok(queue) => queue,
            Err(error) => {
                self.set_experiment_status(&experiment_id, "FAILED_TO_QUEUE")
                    .await?;
                return Err(WorkflowBenchmarkError::Queue(error.to_string()));
            }
        };

        if let Err(error) = self
            .link_queue(&experiment_id, queue.batch.id.as_str(), &queue.items)
            .await
        {
            if let Err(compensation_error) = self
                .mark_queue_link_failed(&experiment_id, queue.batch.id.as_str())
                .await
            {
                tracing::error!(
                    error = %compensation_error,
                    experiment_id = %experiment_id,
                    batch_id = queue.batch.id.as_str(),
                    "failed to record benchmark queue link compensation"
                );
            }
            return Err(WorkflowBenchmarkError::Queue(error.to_string()));
        }

        if request.auto_start {
            if let Err(error) = self
                .production_queue_service
                .start(&request.project_id, queue.batch.id.as_str())
                .await
            {
                // Admission failures leave a durable QUEUED experiment that can
                // be started from the normal Production Queue UI.
                tracing::info!(error = %error, experiment_id = %experiment_id, "benchmark queue was created but not started");
            }
        }

        self.get(&request.project_id, &experiment_id).await
    }

    pub async fn list(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<WorkflowBenchmarkSummaryView>, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        let limit = i64::from(limit.clamp(1, 50));
        let rows = sqlx::query_as::<_, BenchmarkExperimentRow>(
            "SELECT id, project_id, name, media_type, status, base_values_json,
                    asset_ids_json, winner_candidate_id, production_batch_id,
                    created_at, updated_at
             FROM benchmark_experiments
             WHERE project_id = ?
             ORDER BY created_at DESC, id ASC
             LIMIT ?",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            WorkflowBenchmarkError::Repository(RepositoryError::database(error.to_string()))
        })?;

        let mut summaries = Vec::with_capacity(rows.len());
        for row in rows {
            summaries.push(self.summary_for_row(row).await?);
        }
        Ok(summaries)
    }

    pub async fn get(
        &self,
        project_id: &str,
        experiment_id: &str,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        let row = self
            .load_experiment(project_id, experiment_id)
            .await?
            .ok_or_else(|| WorkflowBenchmarkError::NotFound(experiment_id.to_owned()))?;
        let candidates = self.load_candidates(&row).await?;
        let status =
            derive_experiment_status(&row.status, row.production_batch_id.as_deref(), &candidates);
        if status != row.status {
            self.set_experiment_status(&row.id, &status).await?;
        }
        let summary = summary_from_candidates(&row, status.clone(), &candidates);
        Ok(WorkflowBenchmarkView {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            media_type: row.media_type,
            status,
            base_values: parse_json_value(&row.base_values_json)?,
            asset_ids: parse_string_array(&row.asset_ids_json)?,
            winner_candidate_id: row.winner_candidate_id,
            production_batch_id: row.production_batch_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            candidates,
            summary,
        })
    }

    pub async fn set_winner(
        &self,
        project_id: &str,
        experiment_id: &str,
        candidate_id: Option<&str>,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        if let Some(candidate_id) = candidate_id {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM benchmark_candidates c
                 INNER JOIN benchmark_experiments e ON e.id = c.experiment_id
                 WHERE c.id = ? AND e.id = ? AND e.project_id = ?",
            )
            .bind(candidate_id)
            .bind(experiment_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| {
                WorkflowBenchmarkError::Repository(RepositoryError::database(error.to_string()))
            })?;
            if exists == 0 {
                return Err(WorkflowBenchmarkError::InvalidInput(
                    "最佳候选不属于当前实验。".to_owned(),
                ));
            }
        }
        let result = sqlx::query(
            "UPDATE benchmark_experiments SET winner_candidate_id = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(candidate_id)
        .bind(self.clock.now().to_rfc3339())
        .bind(experiment_id)
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            WorkflowBenchmarkError::Repository(RepositoryError::database(error.to_string()))
        })?;
        if result.rows_affected() == 0 {
            return Err(WorkflowBenchmarkError::NotFound(experiment_id.to_owned()));
        }
        self.get(project_id, experiment_id).await
    }

    pub async fn clone_experiment(
        &self,
        project_id: &str,
        experiment_id: &str,
        name: Option<String>,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        let row = self
            .load_experiment(project_id, experiment_id)
            .await?
            .ok_or_else(|| WorkflowBenchmarkError::NotFound(experiment_id.to_owned()))?;
        let candidates = self.load_candidate_rows(&row.id).await?;
        let new_id = format!("bmk_{}", Uuid::new_v4().simple());
        let next_name = name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Benchmark 副本");
        if next_name.chars().count() > 120 {
            return Err(WorkflowBenchmarkError::InvalidInput(
                "Benchmark 名称不能超过 120 个字符。".to_owned(),
            ));
        }
        let now = self.clock.now().to_rfc3339();
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        sqlx::query(
            "INSERT INTO benchmark_experiments
             (id, project_id, name, media_type, status, base_values_json, asset_ids_json,
              winner_candidate_id, production_batch_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'DRAFT', ?, ?, NULL, NULL, ?, ?)",
        )
        .bind(&new_id)
        .bind(project_id)
        .bind(next_name)
        .bind(&row.media_type)
        .bind(&row.base_values_json)
        .bind(&row.asset_ids_json)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        for candidate in candidates {
            let id = format!("bmc_{}", Uuid::new_v4().simple());
            sqlx::query(
                "INSERT INTO benchmark_candidates
                 (id, experiment_id, position, workflow_version_id, recipe_id, preset_id,
                  preset_name, label, values_json, asset_ids_json, production_batch_item_id,
                  task_id, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?)",
            )
            .bind(id)
            .bind(&new_id)
            .bind(candidate.position)
            .bind(&candidate.workflow_version_id)
            .bind(&candidate.recipe_id)
            .bind(&candidate.preset_id)
            .bind(&candidate.preset_name)
            .bind(&candidate.label)
            .bind(&candidate.values_json)
            .bind(&candidate.asset_ids_json)
            .bind(&now)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        }
        transaction.commit().await.map_err(db_error)?;
        self.get(project_id, &new_id).await
    }

    pub async fn queue_existing(
        &self,
        project_id: &str,
        experiment_id: &str,
        auto_start: bool,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        let row = self
            .load_experiment(project_id, experiment_id)
            .await?
            .ok_or_else(|| WorkflowBenchmarkError::NotFound(experiment_id.to_owned()))?;
        if row.status != "DRAFT" || row.production_batch_id.is_some() {
            return Err(WorkflowBenchmarkError::InvalidInput(
                "只有尚未创建生产批次的 DRAFT Benchmark 才能运行。".to_owned(),
            ));
        }

        let available = self.definition_repository.list_available().await?;
        let candidate_rows = self.load_candidate_rows(experiment_id).await?;
        if candidate_rows.len() < 2 || candidate_rows.len() > MAX_BENCHMARK_CANDIDATES {
            return Err(WorkflowBenchmarkError::InvalidInput(
                "Benchmark 候选数量不在允许范围内。".to_owned(),
            ));
        }

        let mut items = Vec::with_capacity(candidate_rows.len());
        for candidate in candidate_rows {
            let available_definition = available.iter().find(|definition| {
                definition.workflow_version_id == candidate.workflow_version_id
                    && definition.recipe_id == candidate.recipe_id
            });
            if available_definition.is_none() {
                return Err(WorkflowBenchmarkError::InvalidInput(format!(
                    "候选 {} 的 Workflow / Recipe 不可用、已停用或已归档。",
                    candidate.label
                )));
            }
            let frozen_values = parse_json_value(&candidate.values_json)?;
            let values = generation_values_from_json(&frozen_values)
                .map_err(WorkflowBenchmarkError::InvalidInput)?;
            self.verify_frozen_assets(project_id, &candidate.asset_ids_json, &values)
                .await?;
            items.push(CreateProductionBatchItem {
                workflow_version_id: candidate.workflow_version_id,
                recipe_id: candidate.recipe_id,
                values,
            });
        }

        let queue = match self
            .production_queue_service
            .create(CreateProductionBatchRequest {
                project_id: project_id.to_owned(),
                name: format!("Benchmark · {}", row.name.trim()),
                continue_on_failure: true,
                items,
            })
            .await
        {
            Ok(queue) => queue,
            Err(error) => {
                self.set_experiment_status(experiment_id, "FAILED_TO_QUEUE")
                    .await?;
                return Err(WorkflowBenchmarkError::Queue(error.to_string()));
            }
        };

        if let Err(error) = self
            .link_queue(experiment_id, queue.batch.id.as_str(), &queue.items)
            .await
        {
            if let Err(compensation_error) = self
                .mark_queue_link_failed(experiment_id, queue.batch.id.as_str())
                .await
            {
                tracing::error!(
                    error = %compensation_error,
                    experiment_id = %experiment_id,
                    batch_id = queue.batch.id.as_str(),
                    "failed to record benchmark queue link compensation"
                );
            }
            return Err(WorkflowBenchmarkError::Queue(error.to_string()));
        }

        if auto_start {
            if let Err(error) = self
                .production_queue_service
                .start(project_id, queue.batch.id.as_str())
                .await
            {
                tracing::info!(
                    error = %error,
                    experiment_id = %experiment_id,
                    "cloned benchmark queue was created but not started"
                );
            }
        }

        self.get(project_id, experiment_id).await
    }

    pub async fn delete(
        &self,
        project_id: &str,
        experiment_id: &str,
    ) -> Result<WorkflowBenchmarkDeleteView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        let result =
            sqlx::query("DELETE FROM benchmark_experiments WHERE id = ? AND project_id = ?")
                .bind(experiment_id)
                .bind(project_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(db_error)?;
            return Err(WorkflowBenchmarkError::NotFound(experiment_id.to_owned()));
        }
        transaction.commit().await.map_err(db_error)?;
        Ok(WorkflowBenchmarkDeleteView {
            deleted: true,
            experiment_id: experiment_id.to_owned(),
        })
    }

    async fn build_candidate_drafts(
        &self,
        request: &WorkflowBenchmarkCreateRequest,
    ) -> Result<Vec<CandidateDraft>, WorkflowBenchmarkError> {
        let available = self.definition_repository.list_available().await?;
        let mut drafts = Vec::with_capacity(request.candidates.len());
        for (index, candidate) in request.candidates.iter().enumerate() {
            let definition = available
                .iter()
                .find(|definition| {
                    definition.workflow_version_id == candidate.workflow_version_id
                        && definition.recipe_id == candidate.recipe_id
                })
                .ok_or_else(|| {
                    WorkflowBenchmarkError::InvalidInput(format!(
                        "Workflow / Recipe 不可用、已停用或已归档：{} / {}",
                        candidate.workflow_version_id, candidate.recipe_id
                    ))
                })?;
            let recipe = RecipeParser::parse(&definition.recipe_yaml)
                .map_err(|error| WorkflowBenchmarkError::InvalidRecipe(error.to_string()))?;
            RecipeValidator::validate(&recipe)
                .map_err(|error| WorkflowBenchmarkError::InvalidRecipe(error.to_string()))?;
            let preset = self.load_preset(request, candidate).await?;
            let preset_values = preset
                .as_ref()
                .map(|preset| generation_values_from_json(&preset.values_json))
                .transpose()
                .map_err(WorkflowBenchmarkError::InvalidInput)?
                .unwrap_or_default();
            let (values, compatibility, reasons) = merge_candidate_values(
                &recipe,
                &request.base_values,
                &preset_values,
                request.seed_mode.as_str(),
                request.fixed_seed,
                output_matches_media(&recipe, &request.media_type),
            )?;
            let label = candidate
                .label
                .as_deref()
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "{} · Recipe {}{}",
                        definition.name,
                        definition.recipe_version,
                        preset
                            .as_ref()
                            .map(|preset| format!(" · {}", preset.name))
                            .unwrap_or_default()
                    )
                });
            if label.chars().count() > 120 {
                return Err(WorkflowBenchmarkError::InvalidInput(
                    "候选显示名不能超过 120 个字符。".to_owned(),
                ));
            }
            drafts.push(CandidateDraft {
                id: format!("bmc_{}", Uuid::new_v4().simple()),
                position: u32::try_from(index).expect("benchmark candidate index fits u32"),
                workflow_version_id: candidate.workflow_version_id.clone(),
                recipe_id: candidate.recipe_id.clone(),
                preset_id: candidate.preset_id.clone(),
                preset_name: preset.as_ref().map(|preset| preset.name.clone()),
                label,
                compatibility,
                compatibility_reasons: reasons,
                values_json: generation_values_to_json(&values),
                asset_ids: collect_asset_ids(&values),
                values,
            });
        }
        Ok(drafts)
    }

    async fn load_preset(
        &self,
        request: &WorkflowBenchmarkCreateRequest,
        candidate: &WorkflowBenchmarkCandidateRequest,
    ) -> Result<Option<crate::domain::Preset>, WorkflowBenchmarkError> {
        let Some(preset_id) = candidate.preset_id.as_deref() else {
            return Ok(None);
        };
        let preset_id = PresetId::parse(preset_id.to_owned())
            .map_err(|error| WorkflowBenchmarkError::InvalidInput(error.to_string()))?;
        let preset = self
            .preset_repository
            .find_by_id(&request.project_id, &preset_id)
            .await?
            .ok_or_else(|| {
                WorkflowBenchmarkError::InvalidInput("选择的 Preset 已不存在。".to_owned())
            })?;
        if preset.workflow_version_id != candidate.workflow_version_id
            || preset.recipe_id != candidate.recipe_id
        {
            return Err(WorkflowBenchmarkError::InvalidInput(
                "Preset 不属于候选 Workflow / Recipe。".to_owned(),
            ));
        }
        Ok(Some(preset))
    }

    async fn insert_draft(
        &self,
        experiment_id: &str,
        request: &WorkflowBenchmarkCreateRequest,
        base_values_json: &Value,
        asset_ids_json: &str,
        now: &str,
        drafts: &[CandidateDraft],
    ) -> Result<(), WorkflowBenchmarkError> {
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        sqlx::query(
            "INSERT INTO benchmark_experiments
             (id, project_id, name, media_type, status, base_values_json, asset_ids_json,
              winner_candidate_id, production_batch_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'DRAFT', ?, ?, NULL, NULL, ?, ?)",
        )
        .bind(experiment_id)
        .bind(&request.project_id)
        .bind(request.name.trim())
        .bind(&request.media_type)
        .bind(base_values_json.to_string())
        .bind(asset_ids_json)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        for draft in drafts {
            sqlx::query(
                "INSERT INTO benchmark_candidates
                 (id, experiment_id, position, workflow_version_id, recipe_id, preset_id,
                  preset_name, label, values_json, asset_ids_json, production_batch_item_id,
                  task_id, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?)",
            )
            .bind(&draft.id)
            .bind(experiment_id)
            .bind(draft.position)
            .bind(&draft.workflow_version_id)
            .bind(&draft.recipe_id)
            .bind(&draft.preset_id)
            .bind(&draft.preset_name)
            .bind(&draft.label)
            .bind(draft.values_json.to_string())
            .bind(
                serde_json::to_string(&draft.asset_ids)
                    .map_err(|error| WorkflowBenchmarkError::Serialization(error.to_string()))?,
            )
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        }
        transaction.commit().await.map_err(db_error)
    }

    async fn link_queue(
        &self,
        experiment_id: &str,
        batch_id: &str,
        items: &[crate::domain::ProductionBatchItem],
    ) -> Result<(), WorkflowBenchmarkError> {
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        sqlx::query(
            "UPDATE benchmark_experiments SET production_batch_id = ?, status = 'QUEUED', updated_at = ? WHERE id = ?",
        )
        .bind(batch_id)
        .bind(self.clock.now().to_rfc3339())
        .bind(experiment_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        for item in items {
            sqlx::query(
                "UPDATE benchmark_candidates
                 SET production_batch_item_id = ?, values_json = ?
                 WHERE experiment_id = ? AND position = ?",
            )
            .bind(item.id.as_str())
            .bind(item.values_json.to_string())
            .bind(experiment_id)
            .bind(item.ordinal)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        }
        transaction.commit().await.map_err(db_error)
    }

    async fn mark_queue_link_failed(
        &self,
        experiment_id: &str,
        batch_id: &str,
    ) -> Result<(), WorkflowBenchmarkError> {
        sqlx::query(
            "UPDATE benchmark_experiments
             SET production_batch_id = ?, status = 'FAILED_TO_QUEUE', updated_at = ?
             WHERE id = ?",
        )
        .bind(batch_id)
        .bind(self.clock.now().to_rfc3339())
        .bind(experiment_id)
        .execute(&self.pool)
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn verify_frozen_assets(
        &self,
        project_id: &str,
        stored_asset_ids_json: &str,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), WorkflowBenchmarkError> {
        let mut asset_ids = parse_string_array(stored_asset_ids_json)?;
        asset_ids.extend(collect_asset_ids(values));
        asset_ids.sort();
        asset_ids.dedup();
        for asset_id in asset_ids {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM assets WHERE id = ? AND project_id = ?",
            )
            .bind(&asset_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map_err(db_error)?;
            if exists == 0 {
                return Err(WorkflowBenchmarkError::InvalidInput(format!(
                    "Benchmark 冻结素材不存在或不属于当前项目：{asset_id}"
                )));
            }
        }
        Ok(())
    }

    async fn set_experiment_status(
        &self,
        experiment_id: &str,
        status: &str,
    ) -> Result<(), WorkflowBenchmarkError> {
        sqlx::query("UPDATE benchmark_experiments SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(self.clock.now().to_rfc3339())
            .bind(experiment_id)
            .execute(&self.pool)
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn load_experiment(
        &self,
        project_id: &str,
        experiment_id: &str,
    ) -> Result<Option<BenchmarkExperimentRow>, WorkflowBenchmarkError> {
        sqlx::query_as::<_, BenchmarkExperimentRow>(
            "SELECT id, project_id, name, media_type, status, base_values_json,
                    asset_ids_json, winner_candidate_id, production_batch_id,
                    created_at, updated_at
             FROM benchmark_experiments WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(experiment_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_error)
    }

    async fn load_candidate_rows(
        &self,
        experiment_id: &str,
    ) -> Result<Vec<BenchmarkCandidateRow>, WorkflowBenchmarkError> {
        sqlx::query_as::<_, BenchmarkCandidateRow>(
            "SELECT id, position, workflow_version_id, recipe_id,
                    preset_id, preset_name, label, values_json, asset_ids_json,
                    production_batch_item_id, task_id
             FROM benchmark_candidates
             WHERE experiment_id = ? ORDER BY position ASC",
        )
        .bind(experiment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)
    }

    async fn load_candidates(
        &self,
        experiment: &BenchmarkExperimentRow,
    ) -> Result<Vec<WorkflowBenchmarkCandidateView>, WorkflowBenchmarkError> {
        let rows = self.load_candidate_rows(&experiment.id).await?;
        let mut views = Vec::with_capacity(rows.len());
        for row in rows {
            let values = parse_json_value(&row.values_json)?;
            let asset_ids = parse_string_array(&row.asset_ids_json)?;
            let runtime = if let Some(item_id) = row.production_batch_item_id.as_deref() {
                sqlx::query_as::<_, BenchmarkItemRuntimeRow>(
                    "SELECT i.task_id, i.status AS item_status,
                            t.status AS task_status, t.created_at AS task_created_at,
                            t.queued_at AS task_queued_at, t.started_at AS task_started_at,
                            t.finished_at AS task_finished_at,
                            t.compiled_workflow_sha256, t.runtime_profile,
                            t.prepare_started_at, t.prepared_at, t.submitted_at,
                            t.execution_started_at, t.execution_finished_at,
                            t.collection_finished_at
                     FROM production_batch_items i
                     LEFT JOIN tasks t ON t.id = i.task_id
                     WHERE i.id = ?",
                )
                .bind(item_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_error)?
            } else {
                None
            };
            let task_id = runtime
                .as_ref()
                .and_then(|runtime| runtime.task_id.clone())
                .or(row.task_id.clone());
            let output_asset_ids = if let Some(task_id) = task_id.as_deref() {
                sqlx::query_scalar::<_, String>(
                    "SELECT asset_id FROM task_output_assets WHERE task_id = ? ORDER BY output_id ASC, ordinal ASC",
                )
                .bind(task_id)
                .fetch_all(&self.pool)
                .await
                .map_err(db_error)?
            } else {
                Vec::new()
            };
            let review = if let Some(item_id) = row.production_batch_item_id.as_deref() {
                sqlx::query_as::<_, BenchmarkReviewRow>(
                    "SELECT review_status, review_note FROM production_item_reviews
                     WHERE production_batch_item_id = ? ORDER BY version DESC LIMIT 1",
                )
                .bind(item_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_error)?
            } else {
                None
            };
            let (task_status, task_created_at, task_started_at, task_finished_at) = runtime
                .as_ref()
                .map(|runtime| {
                    let task_status = effective_candidate_status(
                        runtime.task_status.as_deref(),
                        Some(runtime.item_status.as_str()),
                    );
                    (
                        task_status,
                        runtime.task_created_at.clone(),
                        runtime.task_started_at.clone(),
                        runtime.task_finished_at.clone(),
                    )
                })
                .unwrap_or((None, None, None, None));
            views.push(WorkflowBenchmarkCandidateView {
                id: row.id,
                position: u32::try_from(row.position).unwrap_or_default(),
                workflow_version_id: row.workflow_version_id,
                recipe_id: row.recipe_id,
                preset_id: row.preset_id,
                preset_name: row.preset_name,
                label: row.label,
                compatibility: "COMPATIBLE".to_owned(),
                compatibility_reasons: Vec::new(),
                frozen_values: values,
                asset_ids,
                production_batch_item_id: row.production_batch_item_id,
                task_id,
                task_status,
                task_created_at,
                task_started_at,
                task_finished_at,
                execution_duration_ms: runtime.as_ref().and_then(|runtime| {
                    execution_duration_ms(
                        runtime.task_created_at.as_deref(),
                        runtime.task_started_at.as_deref(),
                        runtime.task_finished_at.as_deref(),
                    )
                }),
                telemetry: runtime.as_ref().and_then(benchmark_telemetry),
                output_asset_ids,
                review_status: review.as_ref().map(|review| review.review_status.clone()),
                review_note: review.map(|review| review.review_note),
            });
        }
        Ok(views)
    }

    async fn summary_for_row(
        &self,
        row: BenchmarkExperimentRow,
    ) -> Result<WorkflowBenchmarkSummaryView, WorkflowBenchmarkError> {
        let candidates = self.load_candidates(&row).await?;
        let status =
            derive_experiment_status(&row.status, row.production_batch_id.as_deref(), &candidates);
        if status != row.status {
            self.set_experiment_status(&row.id, &status).await?;
        }
        Ok(summary_from_candidates(&row, status, &candidates))
    }
}

impl CandidateDraft {
    fn preview(self) -> WorkflowBenchmarkCandidatePreviewView {
        WorkflowBenchmarkCandidatePreviewView {
            id: self.id,
            position: self.position,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            preset_id: self.preset_id,
            preset_name: self.preset_name,
            label: self.label,
            compatibility: self.compatibility,
            compatibility_reasons: self.compatibility_reasons,
            frozen_values: self.values_json,
            asset_ids: self.asset_ids,
        }
    }
}

#[derive(Debug, FromRow)]
struct BenchmarkExperimentRow {
    id: String,
    project_id: String,
    name: String,
    media_type: String,
    status: String,
    base_values_json: String,
    asset_ids_json: String,
    winner_candidate_id: Option<String>,
    production_batch_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct BenchmarkCandidateRow {
    id: String,
    position: i64,
    workflow_version_id: String,
    recipe_id: String,
    preset_id: Option<String>,
    preset_name: Option<String>,
    label: String,
    values_json: String,
    asset_ids_json: String,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct BenchmarkItemRuntimeRow {
    task_id: Option<String>,
    item_status: String,
    task_status: Option<String>,
    task_created_at: Option<String>,
    task_queued_at: Option<String>,
    task_started_at: Option<String>,
    task_finished_at: Option<String>,
    compiled_workflow_sha256: Option<String>,
    runtime_profile: Option<String>,
    prepare_started_at: Option<String>,
    prepared_at: Option<String>,
    submitted_at: Option<String>,
    execution_started_at: Option<String>,
    execution_finished_at: Option<String>,
    collection_finished_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct BenchmarkReviewRow {
    review_status: String,
    review_note: String,
}

fn validate_request_shape(
    request: &WorkflowBenchmarkCreateRequest,
) -> Result<(), WorkflowBenchmarkError> {
    validate_project_id(&request.project_id)?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(WorkflowBenchmarkError::InvalidInput(
            "Benchmark 名称必须为 1–120 个字符。".to_owned(),
        ));
    }
    if !matches!(request.media_type.as_str(), "IMAGE" | "VIDEO") {
        return Err(WorkflowBenchmarkError::InvalidInput(
            "Benchmark 只能选择 IMAGE 或 VIDEO。".to_owned(),
        ));
    }
    if request.candidates.len() < 2 || request.candidates.len() > MAX_BENCHMARK_CANDIDATES {
        return Err(WorkflowBenchmarkError::InvalidInput(format!(
            "Benchmark 必须包含 2–{} 个候选。",
            MAX_BENCHMARK_CANDIDATES
        )));
    }
    if !matches!(request.seed_mode.as_str(), "FIXED" | "EXPLORATION") {
        return Err(WorkflowBenchmarkError::InvalidInput(
            "Seed 模式必须是 FIXED 或 EXPLORATION。".to_owned(),
        ));
    }
    if request.seed_mode == "FIXED" {
        if let Some(seed) = request.fixed_seed {
            if seed > 1_125_899_906_842_624 {
                return Err(WorkflowBenchmarkError::InvalidInput(
                    "固定 Seed 超出当前产品允许范围。".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_project_id(project_id: &str) -> Result<(), WorkflowBenchmarkError> {
    crate::domain::validate_project_id(project_id)
        .map_err(|error| WorkflowBenchmarkError::InvalidInput(error.to_string()))
}

fn output_matches_media(recipe: &Recipe, media_type: &str) -> bool {
    let expected = match media_type {
        "IMAGE" => OutputType::Image,
        "VIDEO" => OutputType::Video,
        _ => return false,
    };
    recipe
        .outputs
        .iter()
        .any(|output| output.output_type == expected)
}

fn merge_candidate_values(
    recipe: &Recipe,
    base_values: &BTreeMap<String, GenerationInputValue>,
    preset_values: &BTreeMap<String, GenerationInputValue>,
    seed_mode: &str,
    fixed_seed: Option<u64>,
    media_matches: bool,
) -> Result<(BTreeMap<String, GenerationInputValue>, String, Vec<String>), WorkflowBenchmarkError> {
    let mut values = BTreeMap::new();
    let mut reasons = Vec::new();
    let mut incompatible = !media_matches;
    if !media_matches {
        reasons.push("输出媒体类型与实验不匹配。".to_owned());
    }
    let mut used_base_keys = HashSet::new();
    for (key, definition) in &recipe.inputs {
        let override_value = preset_values.get(key);
        let base_value = base_values.get(key);
        let selected = if is_benchmark_controlled_key(key) {
            if let (Some(base_value), Some(override_value)) = (base_value, override_value) {
                if base_value != override_value {
                    reasons.push(format!(
                        "参数 {key} 的 Preset 值被 Benchmark 基准输入覆盖。"
                    ));
                }
            }
            base_value.or(override_value)
        } else {
            override_value.or(base_value)
        };
        if let Some(value) = selected {
            if !value_matches_definition(definition, value) {
                incompatible = true;
                reasons.push(format!("参数 {key} 的类型与候选 Recipe 不兼容。"));
                continue;
            }
            values.insert(key.clone(), value.clone());
            if base_value.is_some() {
                used_base_keys.insert(key.clone());
            }
        } else if let Some(default) = default_value(definition, fixed_seed) {
            values.insert(key.clone(), default);
        } else if is_required(definition) {
            incompatible = true;
            reasons.push(format!("缺少必填参数 {key}。"));
        }
        if matches!(definition, InputDefinition::Seed { .. })
            && seed_mode == "FIXED"
            && fixed_seed.is_none()
            && !matches!(
                values.get(key),
                Some(GenerationInputValue::Seed(SeedValue::Fixed(_)))
            )
        {
            reasons.push(format!("候选 Seed 字段 {key} 未使用固定值，可比性降低。"));
        }
        if let Some(fixed_seed) = fixed_seed {
            if matches!(definition, InputDefinition::Seed { .. }) {
                values.insert(
                    key.clone(),
                    GenerationInputValue::Seed(SeedValue::Fixed(fixed_seed)),
                );
            }
        }
    }
    for key in base_values.keys() {
        if !used_base_keys.contains(key) {
            reasons.push(format!("基准参数 {key} 不存在于该候选，未迁移。"));
        }
    }
    let compatibility = if incompatible {
        "INCOMPATIBLE"
    } else if !reasons.is_empty() {
        "PARTIAL"
    } else {
        "COMPATIBLE"
    };
    Ok((values, compatibility.to_owned(), reasons))
}

fn is_benchmark_controlled_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    matches!(
        normalized.as_str(),
        "prompt"
            | "seed"
            | "image"
            | "images"
            | "video"
            | "videos"
            | "audio"
            | "audios"
            | "first_frame"
            | "last_frame"
            | "reference_image"
            | "reference_images"
            | "reference_video"
            | "reference_videos"
            | "reference_audio"
            | "reference_audios"
            | "width"
            | "height"
            | "duration"
            | "duration_seconds"
            | "fps"
            | "frame_count"
            | "frames"
    )
}

fn value_matches_definition(definition: &InputDefinition, value: &GenerationInputValue) -> bool {
    matches!(
        (definition, value),
        (
            InputDefinition::TextArea { .. },
            GenerationInputValue::Text(_)
        ) | (
            InputDefinition::Integer { .. },
            GenerationInputValue::Integer(_)
        ) | (
            InputDefinition::Number { .. },
            GenerationInputValue::Number(_)
        ) | (InputDefinition::Seed { .. }, GenerationInputValue::Seed(_))
            | (
                InputDefinition::Image { .. },
                GenerationInputValue::ImageAsset(_)
            )
            | (
                InputDefinition::Images { .. },
                GenerationInputValue::ImageAssets(_)
            )
            | (
                InputDefinition::Video { .. },
                GenerationInputValue::VideoAsset(_)
            )
            | (
                InputDefinition::Videos { .. },
                GenerationInputValue::VideoAssets(_)
            )
            | (
                InputDefinition::Audio { .. },
                GenerationInputValue::AudioAsset(_)
            )
            | (
                InputDefinition::Audios { .. },
                GenerationInputValue::AudioAssets(_)
            )
    )
}

fn is_required(definition: &InputDefinition) -> bool {
    match definition {
        InputDefinition::TextArea { required, .. }
        | InputDefinition::Integer { required, .. }
        | InputDefinition::Number { required, .. }
        | InputDefinition::Image { required, .. }
        | InputDefinition::Images { required, .. }
        | InputDefinition::Video { required, .. }
        | InputDefinition::Audio { required, .. }
        | InputDefinition::Videos { required, .. }
        | InputDefinition::Audios { required, .. } => *required,
        InputDefinition::Seed { .. } => false,
    }
}

fn default_value(
    definition: &InputDefinition,
    fixed_seed: Option<u64>,
) -> Option<GenerationInputValue> {
    match definition {
        InputDefinition::TextArea { default, .. } => {
            default.clone().map(GenerationInputValue::Text)
        }
        InputDefinition::Integer { default, .. } => default.map(GenerationInputValue::Integer),
        InputDefinition::Number { default, .. } => default.map(GenerationInputValue::Number),
        InputDefinition::Seed { default, .. } => match (default, fixed_seed) {
            (_, Some(seed)) => Some(GenerationInputValue::Seed(SeedValue::Fixed(seed))),
            (crate::domain::SeedDefault::Fixed(seed), None) => {
                Some(GenerationInputValue::Seed(SeedValue::Fixed(*seed)))
            }
            (crate::domain::SeedDefault::Random, None) => {
                Some(GenerationInputValue::Seed(SeedValue::Random))
            }
        },
        InputDefinition::Image { .. }
        | InputDefinition::Images { .. }
        | InputDefinition::Video { .. }
        | InputDefinition::Audio { .. }
        | InputDefinition::Videos { .. }
        | InputDefinition::Audios { .. } => None,
    }
}

fn collect_asset_ids(values: &BTreeMap<String, GenerationInputValue>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values.values() {
        match value {
            GenerationInputValue::ImageAsset(id)
            | GenerationInputValue::VideoAsset(id)
            | GenerationInputValue::AudioAsset(id) => result.push(id.as_str().to_owned()),
            GenerationInputValue::ImageAssets(ids)
            | GenerationInputValue::VideoAssets(ids)
            | GenerationInputValue::AudioAssets(ids) => {
                result.extend(ids.iter().map(|id| id.as_str().to_owned()))
            }
            _ => {}
        }
    }
    result.sort();
    result.dedup();
    result
}

fn parse_json_value(value: &str) -> Result<Value, WorkflowBenchmarkError> {
    serde_json::from_str(value)
        .map_err(|error| WorkflowBenchmarkError::Serialization(error.to_string()))
}

fn parse_string_array(value: &str) -> Result<Vec<String>, WorkflowBenchmarkError> {
    serde_json::from_str(value)
        .map_err(|error| WorkflowBenchmarkError::Serialization(error.to_string()))
}

fn derive_experiment_status(
    stored_status: &str,
    production_batch_id: Option<&str>,
    candidates: &[WorkflowBenchmarkCandidateView],
) -> String {
    if production_batch_id.is_none() {
        return stored_status.to_owned();
    }
    if candidates.is_empty() {
        return "QUEUED".to_owned();
    }
    let status_for = |candidate: &WorkflowBenchmarkCandidateView| {
        candidate
            .task_status
            .as_deref()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "QUEUED".to_owned())
    };
    if candidates
        .iter()
        .all(|candidate| status_for(candidate) == "QUEUED")
    {
        return "QUEUED".to_owned();
    }
    if candidates
        .iter()
        .any(|candidate| status_for(candidate) == "RUNNING")
    {
        return "RUNNING".to_owned();
    }
    let succeeded = candidates
        .iter()
        .filter(|candidate| status_for(candidate) == "SUCCEEDED")
        .count();
    let terminal = candidates.iter().all(|candidate| {
        matches!(
            status_for(candidate).as_str(),
            "SUCCEEDED" | "FAILED" | "CANCELLED" | "SKIPPED"
        )
    });
    if !terminal {
        return "RUNNING".to_owned();
    }
    if succeeded == candidates.len() {
        "COMPLETED".to_owned()
    } else if succeeded == 0
        && candidates
            .iter()
            .all(|candidate| matches!(status_for(candidate).as_str(), "CANCELLED" | "SKIPPED"))
    {
        "CANCELLED".to_owned()
    } else {
        "PARTIAL".to_owned()
    }
}

fn effective_candidate_status(
    task_status: Option<&str>,
    item_status: Option<&str>,
) -> Option<String> {
    let status = task_status.or(item_status)?;
    let effective = match status {
        "CREATED" | "VALIDATING" | "PREPARING" | "QUEUED" | "CANCEL_REQUESTED" | "PENDING"
        | "DISPATCHING" => "QUEUED",
        "DISPATCHED" | "RUNNING" | "COLLECTING" => "RUNNING",
        "SUCCEEDED" => "SUCCEEDED",
        "FAILED" => "FAILED",
        "CANCELLED" => "CANCELLED",
        "SKIPPED" => "SKIPPED",
        other => other,
    };
    Some(effective.to_owned())
}

fn summary_from_candidates(
    row: &BenchmarkExperimentRow,
    status: String,
    candidates: &[WorkflowBenchmarkCandidateView],
) -> WorkflowBenchmarkSummaryView {
    let succeeded_count = candidates
        .iter()
        .filter(|candidate| candidate.task_status.as_deref() == Some("SUCCEEDED"))
        .count();
    let failed_count = candidates
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.task_status.as_deref(),
                Some("FAILED") | Some("CANCELLED") | Some("SKIPPED")
            )
        })
        .count();
    let fastest = candidates
        .iter()
        .filter(|candidate| candidate.task_status.as_deref() == Some("SUCCEEDED"))
        .filter_map(|candidate| {
            candidate
                .execution_duration_ms
                .map(|duration| (candidate, duration))
        })
        .min_by_key(|(_, duration)| *duration);
    WorkflowBenchmarkSummaryView {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        name: row.name.clone(),
        media_type: row.media_type.clone(),
        status,
        winner_candidate_id: row.winner_candidate_id.clone(),
        production_batch_id: row.production_batch_id.clone(),
        candidate_count: candidates.len(),
        succeeded_count,
        failed_count,
        fastest_candidate_id: fastest.map(|(candidate, _)| candidate.id.clone()),
        fastest_duration_ms: fastest.map(|(_, duration)| duration),
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    }
}

fn execution_duration_ms(
    created_at: Option<&str>,
    started_at: Option<&str>,
    finished_at: Option<&str>,
) -> Option<i64> {
    let start = started_at.or(created_at)?;
    let start = DateTime::parse_from_rfc3339(start).ok()?;
    let finish = DateTime::parse_from_rfc3339(finished_at?).ok()?;
    let duration = finish.signed_duration_since(start).num_milliseconds();
    (duration >= 0).then_some(duration)
}

fn benchmark_telemetry(
    runtime: &BenchmarkItemRuntimeRow,
) -> Option<WorkflowBenchmarkTelemetryView> {
    let has_metadata = runtime.compiled_workflow_sha256.is_some()
        || runtime.runtime_profile.is_some()
        || runtime.prepare_started_at.is_some()
        || runtime.prepared_at.is_some()
        || runtime.submitted_at.is_some()
        || runtime.execution_started_at.is_some()
        || runtime.execution_finished_at.is_some()
        || runtime.collection_finished_at.is_some();
    has_metadata.then(|| WorkflowBenchmarkTelemetryView {
        compiled_workflow_sha256: runtime.compiled_workflow_sha256.clone(),
        runtime_profile: runtime.runtime_profile.clone(),
        queue_wait_ms: duration_between(
            runtime.task_queued_at.as_deref(),
            runtime.execution_started_at.as_deref(),
        ),
        prepare_ms: duration_between(
            runtime.prepare_started_at.as_deref(),
            runtime.prepared_at.as_deref(),
        ),
        comfy_execution_ms: duration_between(
            runtime.execution_started_at.as_deref(),
            runtime.execution_finished_at.as_deref(),
        ),
        collection_ms: duration_between(
            runtime.execution_finished_at.as_deref(),
            runtime.collection_finished_at.as_deref(),
        ),
        total_ms: duration_between(
            runtime.task_created_at.as_deref(),
            runtime.collection_finished_at.as_deref(),
        ),
    })
}

fn duration_between(start: Option<&str>, finish: Option<&str>) -> Option<i64> {
    let start = DateTime::parse_from_rfc3339(start?).ok()?;
    let finish = DateTime::parse_from_rfc3339(finish?).ok()?;
    let duration = finish.signed_duration_since(start).num_milliseconds();
    (duration >= 0).then_some(duration)
}

fn db_error(error: sqlx::Error) -> WorkflowBenchmarkError {
    WorkflowBenchmarkError::Repository(RepositoryError::database(error.to_string()))
}

impl From<ProductionQueueError> for WorkflowBenchmarkError {
    fn from(error: ProductionQueueError) -> Self {
        Self::Queue(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        benchmark_telemetry, collect_asset_ids, derive_experiment_status,
        effective_candidate_status, is_benchmark_controlled_key, merge_candidate_values,
        output_matches_media, BenchmarkItemRuntimeRow,
    };
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::domain::{InputDefinition, Recipe, SeedDefault, SeedValue, WorkflowRef};
    use std::collections::BTreeMap;

    fn recipe() -> Recipe {
        Recipe {
            schema_version: 1,
            id: "recipe".to_owned(),
            name: "Recipe".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: BTreeMap::from([
                (
                    "prompt".to_owned(),
                    InputDefinition::TextArea {
                        label: "Prompt".to_owned(),
                        required: true,
                        default: None,
                    },
                ),
                (
                    "cfg".to_owned(),
                    InputDefinition::Number {
                        label: "CFG".to_owned(),
                        required: false,
                        default: Some(5.5),
                        min: None,
                        max: None,
                        step: None,
                    },
                ),
                (
                    "seed".to_owned(),
                    InputDefinition::Seed {
                        label: "Seed".to_owned(),
                        default: SeedDefault::Random,
                        min: Some(0),
                        max: Some(1000),
                    },
                ),
                (
                    "width".to_owned(),
                    InputDefinition::Integer {
                        label: "Width".to_owned(),
                        required: false,
                        default: Some(864),
                        min: Some(1),
                        max: Some(4096),
                        step: Some(1),
                    },
                ),
                (
                    "duration_seconds".to_owned(),
                    InputDefinition::Integer {
                        label: "Duration".to_owned(),
                        required: false,
                        default: Some(5),
                        min: Some(1),
                        max: Some(15),
                        step: Some(1),
                    },
                ),
                (
                    "image".to_owned(),
                    InputDefinition::Image {
                        label: "Image".to_owned(),
                        required: false,
                    },
                ),
            ]),
            bindings: Vec::new(),
            outputs: Vec::new(),
        }
    }

    #[test]
    fn benchmark_merges_only_semantically_matching_keys_and_preserves_numbers() {
        let base = BTreeMap::from([
            (
                "prompt".to_owned(),
                GenerationInputValue::Text("hello".to_owned()),
            ),
            ("cfg".to_owned(), GenerationInputValue::Number(5.5)),
            ("unknown".to_owned(), GenerationInputValue::Integer(2)),
        ]);
        let (values, status, reasons) =
            merge_candidate_values(&recipe(), &base, &BTreeMap::new(), "FIXED", Some(123), true)
                .unwrap();
        assert_eq!(status, "PARTIAL");
        assert_eq!(values["cfg"], GenerationInputValue::Number(5.5));
        assert_eq!(
            values["seed"],
            GenerationInputValue::Seed(SeedValue::Fixed(123))
        );
        assert!(reasons.iter().any(|reason| reason.contains("unknown")));
    }

    #[test]
    fn mismatched_output_is_incompatible() {
        let (_values, status, reasons) = merge_candidate_values(
            &recipe(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            "EXPLORATION",
            None,
            false,
        )
        .unwrap();
        assert_eq!(status, "INCOMPATIBLE");
        assert!(reasons.iter().any(|reason| reason.contains("媒体")));
    }

    #[test]
    fn benchmark_base_controls_identity_and_dimensions_while_preset_controls_tuning() {
        let base = BTreeMap::from([
            (
                "prompt".to_owned(),
                GenerationInputValue::Text("base prompt".to_owned()),
            ),
            (
                "seed".to_owned(),
                GenerationInputValue::Seed(SeedValue::Fixed(101)),
            ),
            ("width".to_owned(), GenerationInputValue::Integer(864)),
            (
                "duration_seconds".to_owned(),
                GenerationInputValue::Integer(5),
            ),
            (
                "image".to_owned(),
                GenerationInputValue::ImageAsset(
                    crate::domain::AssetId::parse("ast_base").unwrap(),
                ),
            ),
        ]);
        let preset = BTreeMap::from([
            (
                "prompt".to_owned(),
                GenerationInputValue::Text("preset prompt".to_owned()),
            ),
            (
                "seed".to_owned(),
                GenerationInputValue::Seed(SeedValue::Fixed(202)),
            ),
            ("width".to_owned(), GenerationInputValue::Integer(1920)),
            (
                "duration_seconds".to_owned(),
                GenerationInputValue::Integer(10),
            ),
            (
                "image".to_owned(),
                GenerationInputValue::ImageAsset(
                    crate::domain::AssetId::parse("ast_preset").unwrap(),
                ),
            ),
            ("cfg".to_owned(), GenerationInputValue::Number(7.0)),
        ]);

        let (values, status, reasons) =
            merge_candidate_values(&recipe(), &base, &preset, "FIXED", None, true).unwrap();

        assert_eq!(status, "PARTIAL");
        assert_eq!(
            values["prompt"],
            GenerationInputValue::Text("base prompt".to_owned())
        );
        assert_eq!(
            values["seed"],
            GenerationInputValue::Seed(SeedValue::Fixed(101))
        );
        assert_eq!(values["width"], GenerationInputValue::Integer(864));
        assert_eq!(values["duration_seconds"], GenerationInputValue::Integer(5));
        assert_eq!(
            values["image"],
            GenerationInputValue::ImageAsset(crate::domain::AssetId::parse("ast_base").unwrap())
        );
        assert_eq!(values["cfg"], GenerationInputValue::Number(7.0));
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("prompt") && reason.contains("覆盖")));
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("width") && reason.contains("覆盖")));
    }

    #[test]
    fn controlled_key_helper_covers_identity_and_runtime_shape_inputs() {
        for key in [
            "prompt",
            "image",
            "reference_video",
            "duration_seconds",
            "frame-count",
        ] {
            assert!(
                is_benchmark_controlled_key(key),
                "{key} should be controlled"
            );
        }
        assert!(!is_benchmark_controlled_key("cfg"));
        assert!(!is_benchmark_controlled_key("steps"));
    }

    #[test]
    fn asset_ids_are_deduplicated_and_sorted() {
        let values = BTreeMap::from([
            (
                "a".to_owned(),
                GenerationInputValue::ImageAssets(vec![
                    crate::domain::AssetId::parse("ast_b").unwrap(),
                    crate::domain::AssetId::parse("ast_a").unwrap(),
                ]),
            ),
            (
                "b".to_owned(),
                GenerationInputValue::ImageAsset(crate::domain::AssetId::parse("ast_a").unwrap()),
            ),
        ]);
        assert_eq!(collect_asset_ids(&values), vec!["ast_a", "ast_b"]);
    }

    #[test]
    fn benchmark_telemetry_hook_reads_compiled_sha_profile_and_timings() {
        let runtime = BenchmarkItemRuntimeRow {
            task_id: Some("tsk_benchmark".to_owned()),
            item_status: "SUCCEEDED".to_owned(),
            task_status: Some("SUCCEEDED".to_owned()),
            task_created_at: Some("2026-01-01T00:00:00Z".to_owned()),
            task_queued_at: Some("2026-01-01T00:00:05Z".to_owned()),
            task_started_at: Some("2026-01-01T00:00:07Z".to_owned()),
            task_finished_at: Some("2026-01-01T00:00:19Z".to_owned()),
            compiled_workflow_sha256: Some("compiled-sha".to_owned()),
            runtime_profile: Some("H3_FAST".to_owned()),
            prepare_started_at: Some("2026-01-01T00:00:01Z".to_owned()),
            prepared_at: Some("2026-01-01T00:00:03Z".to_owned()),
            submitted_at: Some("2026-01-01T00:00:04Z".to_owned()),
            execution_started_at: Some("2026-01-01T00:00:07Z".to_owned()),
            execution_finished_at: Some("2026-01-01T00:00:17Z".to_owned()),
            collection_finished_at: Some("2026-01-01T00:00:19Z".to_owned()),
        };
        let telemetry = benchmark_telemetry(&runtime).expect("telemetry should be exposed");
        assert_eq!(
            telemetry.compiled_workflow_sha256.as_deref(),
            Some("compiled-sha")
        );
        assert_eq!(telemetry.runtime_profile.as_deref(), Some("H3_FAST"));
        assert_eq!(telemetry.queue_wait_ms, Some(2_000));
        assert_eq!(telemetry.prepare_ms, Some(2_000));
        assert_eq!(telemetry.comfy_execution_ms, Some(10_000));
        assert_eq!(telemetry.collection_ms, Some(2_000));
        assert_eq!(telemetry.total_ms, Some(19_000));
    }

    #[test]
    fn status_is_partial_when_one_candidate_fails() {
        let mut first = super::WorkflowBenchmarkCandidateView {
            id: "a".to_owned(),
            position: 0,
            workflow_version_id: "w".to_owned(),
            recipe_id: "r".to_owned(),
            preset_id: None,
            preset_name: None,
            label: "a".to_owned(),
            compatibility: "COMPATIBLE".to_owned(),
            compatibility_reasons: Vec::new(),
            frozen_values: serde_json::json!({}),
            asset_ids: Vec::new(),
            production_batch_item_id: Some("i1".to_owned()),
            task_id: Some("t1".to_owned()),
            task_status: Some("SUCCEEDED".to_owned()),
            task_created_at: None,
            task_started_at: None,
            task_finished_at: None,
            execution_duration_ms: Some(10),
            telemetry: None,
            output_asset_ids: Vec::new(),
            review_status: None,
            review_note: None,
        };
        let second = super::WorkflowBenchmarkCandidateView {
            task_status: Some("FAILED".to_owned()),
            id: "b".to_owned(),
            position: 1,
            ..first.clone()
        };
        first.task_status = Some("SUCCEEDED".to_owned());
        assert_eq!(
            derive_experiment_status("RUNNING", Some("pbt_1"), &[first, second]),
            "PARTIAL"
        );
    }

    fn candidate_with_status(status: Option<&str>) -> super::WorkflowBenchmarkCandidateView {
        super::WorkflowBenchmarkCandidateView {
            id: status.unwrap_or("missing").to_owned(),
            position: 0,
            workflow_version_id: "w".to_owned(),
            recipe_id: "r".to_owned(),
            preset_id: None,
            preset_name: None,
            label: "candidate".to_owned(),
            compatibility: "COMPATIBLE".to_owned(),
            compatibility_reasons: Vec::new(),
            frozen_values: serde_json::json!({}),
            asset_ids: Vec::new(),
            production_batch_item_id: Some("item".to_owned()),
            task_id: None,
            task_status: status.map(ToOwned::to_owned),
            task_created_at: None,
            task_started_at: None,
            task_finished_at: None,
            execution_duration_ms: None,
            telemetry: None,
            output_asset_ids: Vec::new(),
            review_status: None,
            review_note: None,
        }
    }

    #[test]
    fn status_derivation_covers_waiting_running_success_partial_and_cancelled() {
        assert_eq!(
            derive_experiment_status(
                "DRAFT",
                None,
                &[candidate_with_status(None), candidate_with_status(None)]
            ),
            "DRAFT"
        );
        assert_eq!(
            derive_experiment_status(
                "QUEUED",
                Some("batch"),
                &[
                    candidate_with_status(Some("QUEUED")),
                    candidate_with_status(None)
                ]
            ),
            "QUEUED"
        );
        assert_eq!(
            derive_experiment_status(
                "QUEUED",
                Some("batch"),
                &[
                    candidate_with_status(Some("RUNNING")),
                    candidate_with_status(Some("QUEUED"))
                ]
            ),
            "RUNNING"
        );
        assert_eq!(
            derive_experiment_status(
                "RUNNING",
                Some("batch"),
                &[
                    candidate_with_status(Some("SUCCEEDED")),
                    candidate_with_status(Some("SUCCEEDED"))
                ]
            ),
            "COMPLETED"
        );
        assert_eq!(
            derive_experiment_status(
                "RUNNING",
                Some("batch"),
                &[
                    candidate_with_status(Some("SUCCEEDED")),
                    candidate_with_status(Some("FAILED"))
                ]
            ),
            "PARTIAL"
        );
        assert_eq!(
            derive_experiment_status(
                "RUNNING",
                Some("batch"),
                &[
                    candidate_with_status(Some("CANCELLED")),
                    candidate_with_status(Some("SKIPPED"))
                ]
            ),
            "CANCELLED"
        );
        assert_eq!(
            derive_experiment_status(
                "RUNNING",
                Some("batch"),
                &[
                    candidate_with_status(Some("FAILED")),
                    candidate_with_status(Some("SKIPPED"))
                ]
            ),
            "PARTIAL"
        );
    }

    #[test]
    fn effective_status_prefers_task_and_falls_back_to_batch_item() {
        assert_eq!(
            effective_candidate_status(None, Some("CANCELLED")),
            Some("CANCELLED".to_owned())
        );
        assert_eq!(
            effective_candidate_status(None, Some("SKIPPED")),
            Some("SKIPPED".to_owned())
        );
        assert_eq!(
            effective_candidate_status(Some("RUNNING"), Some("PENDING")),
            Some("RUNNING".to_owned())
        );
    }

    #[test]
    fn media_output_helper_is_strict() {
        let recipe = recipe();
        assert!(!output_matches_media(&recipe, "IMAGE"));
    }
}
