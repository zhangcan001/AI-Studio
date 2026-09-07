use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::ports::{
    Clock, GenerationDefinitionRepository, PresetRepository, RepositoryError,
    WorkflowBenchmarkCandidateRecord, WorkflowBenchmarkDraft, WorkflowBenchmarkExperimentRecord,
    WorkflowBenchmarkQueueLink, WorkflowBenchmarkRepository, WorkflowBenchmarkRunRecord,
    WorkflowBenchmarkSnapshot,
};
use crate::application::production_queue_service::{
    generation_values_from_json, generation_values_to_json, CreateProductionBatchItem,
    CreateProductionBatchRequest, ProductionQueueError, ProductionQueueService,
};
use crate::application::production_start_admission_service::ProductionStartAdmissionService;
use crate::application::scheduler::scheduler_decision;
use crate::compiler::{RecipeParser, RecipeValidator};
use crate::domain::{InputDefinition, OutputType, PresetId, Recipe, SeedValue};
use chrono::DateTime;
use serde::Serialize;
use serde_json::Value;
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
    pub repeat_count: u32,
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
    pub workflow_id: Option<String>,
    pub workflow_version: Option<String>,
    pub workflow_sha256: Option<String>,
    pub recipe_version: Option<String>,
    pub recipe_sha256: Option<String>,
    pub runtime_package: Option<String>,
    pub runtime_profile: Option<String>,
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
    pub workflow_id: Option<String>,
    pub workflow_version: Option<String>,
    pub workflow_sha256: Option<String>,
    pub recipe_version: Option<String>,
    pub recipe_sha256: Option<String>,
    pub runtime_package: Option<String>,
    pub runtime_profile: Option<String>,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub task_status: Option<String>,
    pub task_created_at: Option<String>,
    pub task_started_at: Option<String>,
    pub task_finished_at: Option<String>,
    pub execution_duration_ms: Option<i64>,
    pub telemetry: Option<WorkflowBenchmarkTelemetryView>,
    pub runs: Vec<WorkflowBenchmarkRunView>,
    pub aggregate: WorkflowBenchmarkAggregateView,
    pub quality: Option<WorkflowBenchmarkQualityView>,
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
pub struct WorkflowBenchmarkRunView {
    pub id: String,
    pub candidate_id: String,
    pub run_number: u32,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub output_asset_id: Option<String>,
    pub generation_execution_id: Option<String>,
    pub compiled_workflow_sha256: Option<String>,
    pub runtime_profile: Option<String>,
    pub concurrency_class: Option<String>,
    pub queue_wait_ms: Option<i64>,
    pub prepare_ms: Option<i64>,
    pub submit_ms: Option<i64>,
    pub comfy_execution_ms: Option<i64>,
    pub collect_ms: Option<i64>,
    pub total_ms: Option<i64>,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub output_file_size: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkMetricSummaryView {
    pub min: Option<i64>,
    pub median: Option<i64>,
    pub mean: Option<i64>,
    pub p95: Option<i64>,
    pub max: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkAggregateView {
    pub runs_total: u32,
    pub runs_success: u32,
    pub runs_failed: u32,
    pub success_rate: f64,
    pub total_ms: WorkflowBenchmarkMetricSummaryView,
    pub comfy_execution_ms: WorkflowBenchmarkMetricSummaryView,
    pub prepare_ms_mean: Option<i64>,
    pub collect_ms_mean: Option<i64>,
    pub output_size_mean: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkQualityView {
    pub prompt_adherence: Option<i64>,
    pub visual_quality: Option<i64>,
    pub motion_quality: Option<i64>,
    pub reference_consistency: Option<i64>,
    pub overall: Option<i64>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowBenchmarkQualityRequest {
    pub prompt_adherence: Option<i64>,
    pub visual_quality: Option<i64>,
    pub motion_quality: Option<i64>,
    pub reference_consistency: Option<i64>,
    pub overall: Option<i64>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkRecommendationView {
    pub kind: String,
    pub candidate_id: Option<String>,
    pub label: Option<String>,
    pub rationale: String,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkComparisonView {
    pub directly_comparable: bool,
    pub reason: Option<String>,
    pub recommendations: Vec<WorkflowBenchmarkRecommendationView>,
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
    pub repeat_count: u32,
    pub seed_strategy: String,
    pub recommendation_type: Option<String>,
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
    pub comparison: WorkflowBenchmarkComparisonView,
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
    workflow_id: String,
    workflow_version: String,
    workflow_sha256: String,
    recipe_version: String,
    recipe_sha256: String,
    runtime_package: Option<String>,
    runtime_profile: String,
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
    repository: Arc<dyn WorkflowBenchmarkRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    preset_repository: Arc<dyn PresetRepository>,
    production_queue_service: Arc<ProductionQueueService>,
    production_start_admission_service: Option<Arc<ProductionStartAdmissionService>>,
    clock: Arc<dyn Clock>,
}

impl WorkflowBenchmarkService {
    pub fn new(
        repository: Arc<dyn WorkflowBenchmarkRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        preset_repository: Arc<dyn PresetRepository>,
        production_queue_service: Arc<ProductionQueueService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            definition_repository,
            preset_repository,
            production_queue_service,
            production_start_admission_service: None,
            clock,
        }
    }

    pub fn with_start_admission_service(
        mut self,
        service: Arc<ProductionStartAdmissionService>,
    ) -> Self {
        self.production_start_admission_service = Some(service);
        self
    }

    async fn start_production(&self, project_id: &str, batch_id: &str) -> Result<(), String> {
        #[cfg(test)]
        if self.production_start_admission_service.is_none() {
            return self
                .production_queue_service
                .start_for_test(project_id, batch_id)
                .await
                .map_err(|error| error.to_string());
        }
        let service = self
            .production_start_admission_service
            .as_ref()
            .ok_or_else(|| "PRODUCTION_START_ADMISSION_UNAVAILABLE".to_owned())?;
        service
            .start(project_id, batch_id)
            .await
            .map_err(|error| error.to_string())
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
            normalized_seed_strategy(&request.seed_mode),
            &base_values_json,
            &asset_ids_json,
            &now,
            &drafts,
        )
        .await?;

        let queue_items = drafts
            .iter()
            .flat_map(|candidate| {
                std::iter::repeat_with(|| CreateProductionBatchItem {
                    workflow_version_id: candidate.workflow_version_id.clone(),
                    recipe_id: candidate.recipe_id.clone(),
                    values: candidate.values.clone(),
                })
                .take(request.repeat_count as usize)
            })
            .collect::<Vec<_>>();
        let queue_request = CreateProductionBatchRequest {
            project_id: request.project_id.clone(),
            name: format!("Benchmark · {}", request.name.trim()),
            continue_on_failure: true,
            items: queue_items,
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
            .link_queue(
                &experiment_id,
                queue.batch.id.as_str(),
                &queue.items,
                request.repeat_count,
            )
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
                .start_production(&request.project_id, queue.batch.id.as_str())
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
        let rows = self.repository.list_experiments(project_id, limit).await?;

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
        let snapshot = self
            .repository
            .load_experiment_snapshot(project_id, experiment_id, &self.clock.now().to_rfc3339())
            .await?
            .ok_or_else(|| WorkflowBenchmarkError::NotFound(experiment_id.to_owned()))?;
        let candidates = build_candidate_views(&snapshot)?;
        let row = &snapshot.experiment;
        let status =
            derive_experiment_status(&row.status, row.production_batch_id.as_deref(), &candidates);
        if status != row.status {
            self.set_experiment_status(&row.id, &status).await?;
        }
        let summary = summary_from_candidates(&row, status.clone(), &candidates);
        let comparison = build_comparison(&row, &candidates);
        Ok(WorkflowBenchmarkView {
            id: row.id.clone(),
            project_id: row.project_id.clone(),
            name: row.name.clone(),
            media_type: row.media_type.clone(),
            status,
            base_values: parse_json_value(&row.base_values_json)?,
            asset_ids: parse_string_array(&row.asset_ids_json)?,
            winner_candidate_id: row.winner_candidate_id.clone(),
            production_batch_id: row.production_batch_id.clone(),
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
            candidates,
            summary,
            comparison,
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
            if !self
                .repository
                .candidate_belongs_to_experiment(project_id, experiment_id, candidate_id)
                .await?
            {
                return Err(WorkflowBenchmarkError::InvalidInput(
                    "最佳候选不属于当前实验。".to_owned(),
                ));
            }
        }
        if !self
            .repository
            .set_winner(
                project_id,
                experiment_id,
                candidate_id,
                &self.clock.now().to_rfc3339(),
            )
            .await?
        {
            return Err(WorkflowBenchmarkError::NotFound(experiment_id.to_owned()));
        }
        self.get(project_id, experiment_id).await
    }

    pub async fn set_recommendation(
        &self,
        project_id: &str,
        experiment_id: &str,
        recommendation_type: Option<&str>,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        if let Some(kind) = recommendation_type {
            if !matches!(
                kind,
                "FASTEST" | "MOST_STABLE" | "BEST_QUALITY" | "BEST_BALANCE"
            ) {
                return Err(WorkflowBenchmarkError::InvalidInput(
                    "Benchmark 推荐类型无效。".to_owned(),
                ));
            }
        }
        if !self
            .repository
            .set_recommendation(
                project_id,
                experiment_id,
                recommendation_type,
                &self.clock.now().to_rfc3339(),
            )
            .await?
        {
            return Err(WorkflowBenchmarkError::NotFound(experiment_id.to_owned()));
        }
        self.get(project_id, experiment_id).await
    }

    pub async fn save_quality(
        &self,
        project_id: &str,
        experiment_id: &str,
        candidate_id: &str,
        request: WorkflowBenchmarkQualityRequest,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        for (label, value) in [
            ("Prompt Adherence", request.prompt_adherence),
            ("Visual Quality", request.visual_quality),
            ("Motion Quality", request.motion_quality),
            ("Reference Consistency", request.reference_consistency),
            ("Overall", request.overall),
        ] {
            if let Some(value) = value {
                if !(1..=5).contains(&value) {
                    return Err(WorkflowBenchmarkError::InvalidInput(format!(
                        "{label} 评分必须在 1–5 之间。"
                    )));
                }
            }
        }
        if request
            .note
            .as_deref()
            .is_some_and(|note| note.chars().count() > 2000)
        {
            return Err(WorkflowBenchmarkError::InvalidInput(
                "质量评分备注不能超过 2000 个字符。".to_owned(),
            ));
        }
        if !self
            .repository
            .candidate_belongs_to_experiment(project_id, experiment_id, candidate_id)
            .await?
        {
            return Err(WorkflowBenchmarkError::InvalidInput(
                "质量评分候选不属于当前实验。".to_owned(),
            ));
        }
        let now = self.clock.now().to_rfc3339();
        self.repository
            .save_quality(
                candidate_id,
                request.prompt_adherence,
                request.visual_quality,
                request.motion_quality,
                request.reference_consistency,
                request.overall,
                request.note.as_deref(),
                &now,
            )
            .await?;
        self.get(project_id, experiment_id).await
    }

    pub async fn clone_experiment(
        &self,
        project_id: &str,
        experiment_id: &str,
        name: Option<String>,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
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
        self.repository
            .clone_experiment_atomic(project_id, experiment_id, &new_id, next_name, &now)
            .await
            .map_err(|error| match error {
                RepositoryError::NotFound { .. } => {
                    WorkflowBenchmarkError::NotFound(experiment_id.to_owned())
                }
                error => error.into(),
            })?;
        self.get(project_id, &new_id).await
    }

    pub async fn queue_existing(
        &self,
        project_id: &str,
        experiment_id: &str,
        auto_start: bool,
    ) -> Result<WorkflowBenchmarkView, WorkflowBenchmarkError> {
        validate_project_id(project_id)?;
        let snapshot = self
            .repository
            .load_experiment_snapshot(project_id, experiment_id, &self.clock.now().to_rfc3339())
            .await?
            .ok_or_else(|| WorkflowBenchmarkError::NotFound(experiment_id.to_owned()))?;
        let row = snapshot.experiment;
        if row.status != "DRAFT" || row.production_batch_id.is_some() {
            return Err(WorkflowBenchmarkError::InvalidInput(
                "只有尚未创建生产批次的 DRAFT Benchmark 才能运行。".to_owned(),
            ));
        }

        let available = self.definition_repository.list_available().await?;
        let candidate_rows = snapshot.candidates;
        if candidate_rows.len() < 2 || candidate_rows.len() > MAX_BENCHMARK_CANDIDATES {
            return Err(WorkflowBenchmarkError::InvalidInput(
                "Benchmark 候选数量不在允许范围内。".to_owned(),
            ));
        }

        let repeat_count = u32::try_from(row.repeat_count).unwrap_or(3).clamp(1, 10);
        let mut items = Vec::with_capacity(candidate_rows.len() * repeat_count as usize);
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
            for _ in 0..repeat_count {
                items.push(CreateProductionBatchItem {
                    workflow_version_id: candidate.workflow_version_id.clone(),
                    recipe_id: candidate.recipe_id.clone(),
                    values: values.clone(),
                });
            }
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
            .link_queue(
                experiment_id,
                queue.batch.id.as_str(),
                &queue.items,
                u32::try_from(row.repeat_count).unwrap_or(1),
            )
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
                .start_production(project_id, queue.batch.id.as_str())
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
        if !self
            .repository
            .delete_atomic(project_id, experiment_id)
            .await?
        {
            return Err(WorkflowBenchmarkError::NotFound(experiment_id.to_owned()));
        }
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
            let frozen_definition = self
                .definition_repository
                .find(&candidate.workflow_version_id, &candidate.recipe_id)
                .await?
                .ok_or_else(|| {
                    WorkflowBenchmarkError::InvalidInput(format!(
                        "Workflow / Recipe disappeared while freezing candidate: {} / {}",
                        candidate.workflow_version_id, candidate.recipe_id
                    ))
                })?;
            let recipe = RecipeParser::parse(&frozen_definition.recipe_yaml)
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
            let runtime_profile = scheduler_decision(&frozen_definition, &recipe)
                .profile
                .as_str()
                .to_owned();
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
                workflow_id: frozen_definition.workflow_id,
                workflow_version: frozen_definition.workflow_version,
                workflow_sha256: frozen_definition.workflow_sha256,
                recipe_version: frozen_definition.recipe_version,
                recipe_sha256: frozen_definition.recipe_sha256,
                runtime_package: frozen_definition.package_name.clone(),
                runtime_profile,
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
        seed_strategy: &str,
        base_values_json: &Value,
        asset_ids_json: &str,
        now: &str,
        drafts: &[CandidateDraft],
    ) -> Result<(), WorkflowBenchmarkError> {
        let candidates = drafts
            .iter()
            .map(|draft| {
                Ok::<_, WorkflowBenchmarkError>(WorkflowBenchmarkCandidateRecord {
                    id: draft.id.clone(),
                    position: i64::from(draft.position),
                    workflow_version_id: draft.workflow_version_id.clone(),
                    recipe_id: draft.recipe_id.clone(),
                    preset_id: draft.preset_id.clone(),
                    preset_name: draft.preset_name.clone(),
                    label: draft.label.clone(),
                    values_json: draft.values_json.to_string(),
                    asset_ids_json: serde_json::to_string(&draft.asset_ids).map_err(|error| {
                        WorkflowBenchmarkError::Serialization(error.to_string())
                    })?,
                    production_batch_item_id: None,
                    task_id: None,
                    workflow_id: Some(draft.workflow_id.clone()),
                    workflow_version: Some(draft.workflow_version.clone()),
                    workflow_sha256: Some(draft.workflow_sha256.clone()),
                    recipe_version: Some(draft.recipe_version.clone()),
                    recipe_sha256: Some(draft.recipe_sha256.clone()),
                    runtime_package: draft.runtime_package.clone(),
                    runtime_profile: Some(draft.runtime_profile.clone()),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let draft = WorkflowBenchmarkDraft {
            experiment: WorkflowBenchmarkExperimentRecord {
                id: experiment_id.to_owned(),
                project_id: request.project_id.clone(),
                name: request.name.trim().to_owned(),
                media_type: request.media_type.clone(),
                status: "DRAFT".to_owned(),
                base_values_json: base_values_json.to_string(),
                asset_ids_json: asset_ids_json.to_owned(),
                winner_candidate_id: None,
                production_batch_id: None,
                seed_strategy: seed_strategy.to_owned(),
                fixed_seed: request.fixed_seed.map(|seed| seed.to_string()),
                repeat_count: i64::from(request.repeat_count),
                recommendation_type: None,
                created_at: now.to_owned(),
                updated_at: now.to_owned(),
            },
            candidates,
            repeat_count: request.repeat_count,
        };
        self.repository.create_draft_atomic(&draft).await?;
        Ok(())
    }

    async fn link_queue(
        &self,
        experiment_id: &str,
        batch_id: &str,
        items: &[crate::domain::ProductionBatchItem],
        repeat_count: u32,
    ) -> Result<(), WorkflowBenchmarkError> {
        let repeat_count = usize::try_from(repeat_count)
            .map_err(|_| WorkflowBenchmarkError::InvalidInput("运行次数溢出。".to_owned()))?;
        let mut links = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let candidate_position = u32::try_from(index / repeat_count)
                .map_err(|_| WorkflowBenchmarkError::InvalidInput("候选序号溢出。".to_owned()))?;
            let run_number = u32::try_from(index % repeat_count + 1)
                .map_err(|_| WorkflowBenchmarkError::InvalidInput("运行序号溢出。".to_owned()))?;
            links.push(WorkflowBenchmarkQueueLink {
                production_batch_item_id: item.id.as_str().to_owned(),
                candidate_position,
                run_number,
                values_json: item.values_json.to_string(),
            });
        }
        self.repository
            .link_queue_atomic(
                experiment_id,
                batch_id,
                &links,
                &self.clock.now().to_rfc3339(),
            )
            .await?;
        Ok(())
    }

    async fn mark_queue_link_failed(
        &self,
        experiment_id: &str,
        batch_id: &str,
    ) -> Result<(), WorkflowBenchmarkError> {
        self.repository
            .mark_queue_link_failed(experiment_id, batch_id, &self.clock.now().to_rfc3339())
            .await?;
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
        self.repository
            .verify_frozen_assets(project_id, &asset_ids)
            .await
            .map_err(|error| match error {
                RepositoryError::Integrity { message } => {
                    WorkflowBenchmarkError::InvalidInput(message)
                }
                error => error.into(),
            })
    }

    async fn set_experiment_status(
        &self,
        experiment_id: &str,
        status: &str,
    ) -> Result<(), WorkflowBenchmarkError> {
        self.repository
            .set_status(experiment_id, status, &self.clock.now().to_rfc3339())
            .await?;
        Ok(())
    }

    async fn summary_for_row(
        &self,
        row: WorkflowBenchmarkExperimentRecord,
    ) -> Result<WorkflowBenchmarkSummaryView, WorkflowBenchmarkError> {
        let snapshot = self
            .repository
            .load_experiment_snapshot(&row.project_id, &row.id, &self.clock.now().to_rfc3339())
            .await?
            .ok_or_else(|| WorkflowBenchmarkError::NotFound(row.id.clone()))?;
        let candidates = build_candidate_views(&snapshot)?;
        let status =
            derive_experiment_status(&row.status, row.production_batch_id.as_deref(), &candidates);
        if status != row.status {
            self.set_experiment_status(&row.id, &status).await?;
        }
        Ok(summary_from_candidates(&row, status, &candidates))
    }
}

fn build_candidate_views(
    snapshot: &WorkflowBenchmarkSnapshot,
) -> Result<Vec<WorkflowBenchmarkCandidateView>, WorkflowBenchmarkError> {
    let mut runs_by_candidate: BTreeMap<String, Vec<WorkflowBenchmarkRunView>> = BTreeMap::new();
    for run in &snapshot.runs {
        let status = effective_candidate_status(run.status.as_deref(), None);
        runs_by_candidate
            .entry(run.candidate_id.clone())
            .or_default()
            .push(run_to_view(run, status));
    }

    let mut quality_by_candidate = snapshot
        .quality
        .iter()
        .map(|quality| {
            (
                quality.candidate_id.clone(),
                WorkflowBenchmarkQualityView {
                    prompt_adherence: quality.prompt_adherence,
                    visual_quality: quality.visual_quality,
                    motion_quality: quality.motion_quality,
                    reference_consistency: quality.reference_consistency,
                    overall: quality.overall,
                    note: quality.note.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let reviews_by_item = snapshot
        .reviews
        .iter()
        .map(|review| (review.production_batch_item_id.as_str(), review))
        .collect::<BTreeMap<_, _>>();

    let mut views = Vec::with_capacity(snapshot.candidates.len());
    for row in &snapshot.candidates {
        let values = parse_json_value(&row.values_json)?;
        let asset_ids = parse_string_array(&row.asset_ids_json)?;
        let runs = runs_by_candidate.remove(&row.id).unwrap_or_default();
        let first_run = runs.first();
        let task_id = first_run
            .and_then(|run| run.task_id.clone())
            .or(row.task_id.clone());
        let output_asset_ids = runs
            .iter()
            .filter_map(|run| run.output_asset_id.clone())
            .collect::<Vec<_>>();
        let review = row
            .production_batch_item_id
            .as_deref()
            .and_then(|item_id| reviews_by_item.get(item_id).copied());
        let task_status = candidate_status_from_runs(&runs, row.task_id.as_deref());
        let aggregate = aggregate_runs(&runs);
        let telemetry = runs.first().and_then(telemetry_from_run);
        views.push(WorkflowBenchmarkCandidateView {
            id: row.id.clone(),
            position: u32::try_from(row.position).unwrap_or_default(),
            workflow_version_id: row.workflow_version_id.clone(),
            recipe_id: row.recipe_id.clone(),
            preset_id: row.preset_id.clone(),
            preset_name: row.preset_name.clone(),
            label: row.label.clone(),
            compatibility: "COMPATIBLE".to_owned(),
            compatibility_reasons: Vec::new(),
            frozen_values: values,
            asset_ids,
            workflow_id: row.workflow_id.clone(),
            workflow_version: row.workflow_version.clone(),
            workflow_sha256: row.workflow_sha256.clone(),
            recipe_version: row.recipe_version.clone(),
            recipe_sha256: row.recipe_sha256.clone(),
            runtime_package: row.runtime_package.clone(),
            runtime_profile: row.runtime_profile.clone(),
            production_batch_item_id: row.production_batch_item_id.clone(),
            task_id,
            task_status,
            task_created_at: None,
            task_started_at: None,
            task_finished_at: None,
            execution_duration_ms: aggregate.total_ms.median,
            telemetry,
            runs,
            aggregate,
            quality: quality_by_candidate.remove(&row.id),
            output_asset_ids,
            review_status: review.map(|review| review.review_status.clone()),
            review_note: review.map(|review| review.review_note.clone()),
        });
    }
    Ok(views)
}

fn run_to_view(
    run: &WorkflowBenchmarkRunRecord,
    status: Option<String>,
) -> WorkflowBenchmarkRunView {
    WorkflowBenchmarkRunView {
        id: run.id.clone(),
        candidate_id: run.candidate_id.clone(),
        run_number: u32::try_from(run.run_number).unwrap_or(1),
        production_batch_item_id: run.production_batch_item_id.clone(),
        task_id: run.task_id.clone(),
        snapshot_id: run.snapshot_id.clone(),
        output_asset_id: run.output_asset_id.clone(),
        generation_execution_id: run.generation_execution_id.clone(),
        compiled_workflow_sha256: run.compiled_workflow_sha256.clone(),
        runtime_profile: run.runtime_profile.clone(),
        concurrency_class: run.concurrency_class.clone(),
        queue_wait_ms: run.queue_wait_ms,
        prepare_ms: run.prepare_ms,
        submit_ms: run.submit_ms,
        comfy_execution_ms: run.comfy_execution_ms,
        collect_ms: run.collect_ms,
        total_ms: run.total_ms,
        status,
        error_code: run.error_code.clone(),
        output_file_size: run.output_file_size,
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
            workflow_id: Some(self.workflow_id),
            workflow_version: Some(self.workflow_version),
            workflow_sha256: Some(self.workflow_sha256),
            recipe_version: Some(self.recipe_version),
            recipe_sha256: Some(self.recipe_sha256),
            runtime_package: self.runtime_package,
            runtime_profile: Some(self.runtime_profile),
        }
    }
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
    if !matches!(
        request.seed_mode.as_str(),
        "FIXED" | "EXPLORATION" | "FIXED_SEED" | "RANDOM_SEED"
    ) {
        return Err(WorkflowBenchmarkError::InvalidInput(
            "Seed 模式必须是 FIXED_SEED 或 RANDOM_SEED。".to_owned(),
        ));
    }
    if !matches!(request.repeat_count, 1 | 3 | 5 | 10) {
        return Err(WorkflowBenchmarkError::InvalidInput(
            "Benchmark 重复次数只能是 1、3、5 或 10。".to_owned(),
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

fn normalized_seed_strategy(seed_mode: &str) -> &'static str {
    match seed_mode {
        "EXPLORATION" | "RANDOM_SEED" => "RANDOM_SEED",
        _ => "FIXED_SEED",
    }
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
            && matches!(seed_mode, "FIXED" | "FIXED_SEED")
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
        candidate_status_from_runs(candidate.runs.as_slice(), candidate.task_status.as_deref())
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
            "SUCCEEDED" | "FAILED" | "PARTIAL" | "CANCELLED" | "SKIPPED"
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

fn candidate_status_from_runs(
    runs: &[WorkflowBenchmarkRunView],
    fallback_task_status: Option<&str>,
) -> Option<String> {
    if runs.is_empty() {
        return fallback_task_status
            .and_then(|status| effective_candidate_status(Some(status), None))
            .or_else(|| Some("QUEUED".to_owned()));
    }
    let statuses = runs
        .iter()
        .filter_map(|run| run.status.as_deref())
        .collect::<Vec<_>>();
    if statuses.iter().any(|status| *status == "RUNNING") {
        return Some("RUNNING".to_owned());
    }
    if statuses.iter().any(|status| *status == "QUEUED") || statuses.len() < runs.len() {
        return Some("QUEUED".to_owned());
    }
    if statuses.iter().all(|status| *status == "SUCCEEDED") {
        return Some("SUCCEEDED".to_owned());
    }
    if statuses
        .iter()
        .all(|status| matches!(*status, "FAILED" | "CANCELLED" | "SKIPPED" | "SUCCEEDED"))
    {
        return Some("PARTIAL".to_owned());
    }
    Some("QUEUED".to_owned())
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
    row: &WorkflowBenchmarkExperimentRecord,
    status: String,
    candidates: &[WorkflowBenchmarkCandidateView],
) -> WorkflowBenchmarkSummaryView {
    let succeeded_count = candidates
        .iter()
        .filter(|candidate| {
            candidate_status_from_runs(&candidate.runs, candidate.task_status.as_deref()).as_deref()
                == Some("SUCCEEDED")
        })
        .count();
    let failed_count = candidates
        .iter()
        .filter(|candidate| {
            matches!(
                candidate_status_from_runs(&candidate.runs, candidate.task_status.as_deref())
                    .as_deref(),
                Some("FAILED") | Some("PARTIAL") | Some("CANCELLED") | Some("SKIPPED")
            )
        })
        .count();
    let fastest = candidates
        .iter()
        .filter(|candidate| {
            candidate_status_from_runs(&candidate.runs, candidate.task_status.as_deref()).as_deref()
                == Some("SUCCEEDED")
        })
        .filter_map(|candidate| {
            candidate
                .aggregate
                .total_ms
                .median
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
        repeat_count: u32::try_from(row.repeat_count).unwrap_or(1),
        seed_strategy: row.seed_strategy.clone(),
        recommendation_type: row.recommendation_type.clone(),
        candidate_count: candidates.len(),
        succeeded_count,
        failed_count,
        fastest_candidate_id: fastest.map(|(candidate, _)| candidate.id.clone()),
        fastest_duration_ms: fastest.map(|(_, duration)| duration),
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    }
}

fn metric_summary(values: impl Iterator<Item = i64>) -> WorkflowBenchmarkMetricSummaryView {
    let mut values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return WorkflowBenchmarkMetricSummaryView::default();
    }
    values.sort_unstable();
    let sum = values.iter().copied().sum::<i64>();
    let median = if values.len() % 2 == 0 {
        (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2
    } else {
        values[values.len() / 2]
    };
    let p95_index = ((values.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);
    WorkflowBenchmarkMetricSummaryView {
        min: values.first().copied(),
        median: Some(median),
        mean: Some(sum / i64::try_from(values.len()).unwrap_or(1)),
        p95: values.get(p95_index).copied(),
        max: values.last().copied(),
    }
}

fn mean_metric(values: impl Iterator<Item = i64>) -> Option<i64> {
    let values = values.collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.iter().copied().sum::<i64>() / values.len() as i64)
}

fn aggregate_runs(runs: &[WorkflowBenchmarkRunView]) -> WorkflowBenchmarkAggregateView {
    let runs_success = runs
        .iter()
        .filter(|run| run.status.as_deref() == Some("SUCCEEDED"))
        .count() as u32;
    let runs_failed = runs
        .iter()
        .filter(|run| {
            matches!(
                run.status.as_deref(),
                Some("FAILED") | Some("CANCELLED") | Some("SKIPPED")
            )
        })
        .count() as u32;
    WorkflowBenchmarkAggregateView {
        runs_total: runs.len() as u32,
        runs_success,
        runs_failed,
        success_rate: if runs.is_empty() {
            0.0
        } else {
            runs_success as f64 / runs.len() as f64
        },
        total_ms: metric_summary(runs.iter().filter_map(|run| run.total_ms)),
        comfy_execution_ms: metric_summary(runs.iter().filter_map(|run| run.comfy_execution_ms)),
        prepare_ms_mean: mean_metric(runs.iter().filter_map(|run| run.prepare_ms)),
        collect_ms_mean: mean_metric(runs.iter().filter_map(|run| run.collect_ms)),
        output_size_mean: mean_metric(runs.iter().filter_map(|run| run.output_file_size)),
    }
}

fn telemetry_from_run(run: &WorkflowBenchmarkRunView) -> Option<WorkflowBenchmarkTelemetryView> {
    let has_metadata = run.compiled_workflow_sha256.is_some()
        || run.runtime_profile.is_some()
        || run.queue_wait_ms.is_some()
        || run.prepare_ms.is_some()
        || run.comfy_execution_ms.is_some()
        || run.collect_ms.is_some()
        || run.total_ms.is_some();
    has_metadata.then(|| WorkflowBenchmarkTelemetryView {
        compiled_workflow_sha256: run.compiled_workflow_sha256.clone(),
        runtime_profile: run.runtime_profile.clone(),
        queue_wait_ms: run.queue_wait_ms,
        prepare_ms: run.prepare_ms,
        comfy_execution_ms: run.comfy_execution_ms,
        collection_ms: run.collect_ms,
        total_ms: run.total_ms,
    })
}

fn build_comparison(
    experiment: &WorkflowBenchmarkExperimentRecord,
    candidates: &[WorkflowBenchmarkCandidateView],
) -> WorkflowBenchmarkComparisonView {
    if candidates.len() < 2 {
        return WorkflowBenchmarkComparisonView {
            directly_comparable: false,
            reason: Some("至少需要两个候选才能比较。".to_owned()),
            recommendations: Vec::new(),
        };
    }
    let first_signature = comparison_signature(&candidates[0], experiment.seed_strategy.as_str());
    let mismatch = candidates.iter().skip(1).find_map(|candidate| {
        (comparison_signature(candidate, experiment.seed_strategy.as_str()) != first_signature)
            .then(|| candidate.label.clone())
    });
    if let Some(label) = mismatch {
        return WorkflowBenchmarkComparisonView {
            directly_comparable: false,
            reason: Some(format!(
                "NOT_DIRECTLY_COMPARABLE：候选 {label} 与基准的 Prompt、分辨率、时长或 Seed 不一致。"
            )),
            recommendations: Vec::new(),
        };
    }
    let mut recommendations = Vec::new();
    let successful = candidates
        .iter()
        .filter(|candidate| candidate.aggregate.runs_success > 0);
    let fastest = successful
        .clone()
        .filter_map(|candidate| {
            candidate
                .aggregate
                .total_ms
                .median
                .map(|value| (candidate, value))
        })
        .min_by_key(|(_, value)| *value);
    recommendations.push(recommendation(
        "FASTEST",
        fastest.map(|(candidate, _)| candidate),
        "在已完成运行中位总耗时最低。",
    ));
    let stable = candidates.iter().max_by(|left, right| {
        left.aggregate
            .success_rate
            .partial_cmp(&right.aggregate.success_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                compare_optional_i64(
                    left.aggregate.total_ms.median,
                    right.aggregate.total_ms.median,
                )
                .reverse()
            })
    });
    recommendations.push(recommendation(
        "MOST_STABLE",
        stable,
        "成功率最高；相同成功率时取中位总耗时更低者。",
    ));
    let quality = candidates
        .iter()
        .filter_map(|candidate| {
            candidate
                .quality
                .as_ref()
                .and_then(|quality| quality.overall)
                .map(|score| (candidate, score))
        })
        .max_by_key(|(_, score)| *score);
    let has_quality = quality.is_some();
    recommendations.push(recommendation(
        "BEST_QUALITY",
        quality.map(|(candidate, _)| candidate),
        if has_quality {
            "人工质量评分最高。"
        } else {
            "尚未录入人工质量评分。"
        },
    ));
    let balanced = candidates.iter().max_by(|left, right| {
        left.aggregate
            .success_rate
            .partial_cmp(&right.aggregate.success_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| quality_score(left).cmp(&quality_score(right)))
            .then_with(|| {
                compare_optional_i64(
                    right.aggregate.total_ms.median,
                    left.aggregate.total_ms.median,
                )
            })
    });
    recommendations.push(recommendation(
        "BEST_BALANCE",
        balanced,
        "综合成功率、人工质量评分和中位总耗时。",
    ));
    WorkflowBenchmarkComparisonView {
        directly_comparable: true,
        reason: None,
        recommendations,
    }
}

fn comparison_signature(candidate: &WorkflowBenchmarkCandidateView, seed_strategy: &str) -> String {
    let mut values = BTreeMap::new();
    if let Value::Object(object) = &candidate.frozen_values {
        for (key, value) in object {
            if is_benchmark_controlled_key(key)
                && (seed_strategy != "RANDOM_SEED" || !key.eq_ignore_ascii_case("seed"))
            {
                values.insert(key.to_ascii_lowercase().replace('-', "_"), value.clone());
            }
        }
    }
    serde_json::to_string(&values).unwrap_or_default()
}

fn quality_score(candidate: &WorkflowBenchmarkCandidateView) -> i64 {
    candidate
        .quality
        .as_ref()
        .and_then(|quality| quality.overall)
        .unwrap_or(0)
}

fn compare_optional_i64(left: Option<i64>, right: Option<i64>) -> std::cmp::Ordering {
    left.unwrap_or(i64::MAX).cmp(&right.unwrap_or(i64::MAX))
}

fn recommendation(
    kind: &str,
    candidate: Option<&WorkflowBenchmarkCandidateView>,
    rationale: &str,
) -> WorkflowBenchmarkRecommendationView {
    WorkflowBenchmarkRecommendationView {
        kind: kind.to_owned(),
        candidate_id: candidate.map(|candidate| candidate.id.clone()),
        label: candidate.map(|candidate| candidate.label.clone()),
        rationale: rationale.to_owned(),
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

#[cfg(test)]
struct BenchmarkTelemetryInput {
    compiled_workflow_sha256: Option<String>,
    runtime_profile: Option<String>,
    task_created_at: Option<String>,
    task_queued_at: Option<String>,
    prepare_started_at: Option<String>,
    prepared_at: Option<String>,
    submitted_at: Option<String>,
    execution_started_at: Option<String>,
    execution_finished_at: Option<String>,
    collection_finished_at: Option<String>,
}

#[cfg(test)]
fn benchmark_telemetry(
    runtime: &BenchmarkTelemetryInput,
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

#[cfg(test)]
fn duration_between(start: Option<&str>, finish: Option<&str>) -> Option<i64> {
    let start = DateTime::parse_from_rfc3339(start?).ok()?;
    let finish = DateTime::parse_from_rfc3339(finish?).ok()?;
    let duration = finish.signed_duration_since(start).num_milliseconds();
    (duration >= 0).then_some(duration)
}

impl From<ProductionQueueError> for WorkflowBenchmarkError {
    fn from(error: ProductionQueueError) -> Self {
        Self::Queue(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_runs, benchmark_telemetry, collect_asset_ids, derive_experiment_status,
        effective_candidate_status, is_benchmark_controlled_key, merge_candidate_values,
        output_matches_media, BenchmarkTelemetryInput, WorkflowBenchmarkRunView,
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
        let runtime = BenchmarkTelemetryInput {
            task_created_at: Some("2026-01-01T00:00:00Z".to_owned()),
            task_queued_at: Some("2026-01-01T00:00:05Z".to_owned()),
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

    fn benchmark_run(
        status: &str,
        total_ms: i64,
        comfy_execution_ms: i64,
    ) -> WorkflowBenchmarkRunView {
        WorkflowBenchmarkRunView {
            id: format!("run-{total_ms}"),
            candidate_id: "candidate".to_owned(),
            run_number: 1,
            production_batch_item_id: None,
            task_id: None,
            snapshot_id: None,
            output_asset_id: None,
            generation_execution_id: None,
            compiled_workflow_sha256: None,
            runtime_profile: None,
            concurrency_class: None,
            queue_wait_ms: None,
            prepare_ms: Some(3),
            submit_ms: None,
            comfy_execution_ms: Some(comfy_execution_ms),
            collect_ms: Some(4),
            total_ms: Some(total_ms),
            status: Some(status.to_owned()),
            error_code: None,
            output_file_size: Some(100),
        }
    }

    #[test]
    fn benchmark_aggregate_uses_deterministic_median_and_p95() {
        let runs = vec![
            benchmark_run("SUCCEEDED", 100, 70),
            benchmark_run("SUCCEEDED", 200, 80),
            benchmark_run("FAILED", 300, 90),
            benchmark_run("SUCCEEDED", 400, 100),
            benchmark_run("SUCCEEDED", 500, 110),
        ];
        let aggregate = aggregate_runs(&runs);
        assert_eq!(aggregate.runs_total, 5);
        assert_eq!(aggregate.runs_success, 4);
        assert_eq!(aggregate.runs_failed, 1);
        assert_eq!(aggregate.success_rate, 0.8);
        assert_eq!(aggregate.total_ms.median, Some(300));
        assert_eq!(aggregate.total_ms.mean, Some(300));
        assert_eq!(aggregate.total_ms.p95, Some(500));
        assert_eq!(aggregate.comfy_execution_ms.median, Some(90));
        assert_eq!(aggregate.prepare_ms_mean, Some(3));
        assert_eq!(aggregate.collect_ms_mean, Some(4));
        assert_eq!(aggregate.output_size_mean, Some(100));
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
            workflow_id: None,
            workflow_version: None,
            workflow_sha256: None,
            recipe_version: None,
            recipe_sha256: None,
            runtime_package: None,
            runtime_profile: None,
            production_batch_item_id: Some("i1".to_owned()),
            task_id: Some("t1".to_owned()),
            task_status: Some("SUCCEEDED".to_owned()),
            task_created_at: None,
            task_started_at: None,
            task_finished_at: None,
            execution_duration_ms: Some(10),
            telemetry: None,
            runs: Vec::new(),
            aggregate: super::WorkflowBenchmarkAggregateView::default(),
            quality: None,
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
            workflow_id: None,
            workflow_version: None,
            workflow_sha256: None,
            recipe_version: None,
            recipe_sha256: None,
            runtime_package: None,
            runtime_profile: None,
            production_batch_item_id: Some("item".to_owned()),
            task_id: None,
            task_status: status.map(ToOwned::to_owned),
            task_created_at: None,
            task_started_at: None,
            task_finished_at: None,
            execution_duration_ms: None,
            telemetry: None,
            runs: Vec::new(),
            aggregate: super::WorkflowBenchmarkAggregateView::default(),
            quality: None,
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
