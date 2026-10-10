use super::error::ProductError;
use crate::application::{
    production_orchestrator_service::{ProductionOrchestratorError, ProductionOrchestratorService},
    production_queue_service::{ProductionQueueError, ProductionQueueService},
    task_query_service::TaskQueryService,
};
use crate::domain::{ProductionBatchItemStatus, ProductionBatchStatus};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// One list call owns these reads. Live detail/action requests never reuse them.
#[derive(Default)]
struct TaskParentReads {
    batch_ids: Option<Vec<String>>,
    items: HashMap<String, Vec<(String, String)>>,
}

mod workspace;
pub use workspace::{RunList, RunListFilter};
mod detail;
pub use detail::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRef {
    pub source: RunSource,
    pub id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunActionLocation {
    pub project_id: String,
    pub run_ref: RunRef,
    pub shot_id: Option<String>,
    pub stage: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunSource {
    ProductionRun,
    QueueBatch,
    Task,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProgress {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recoverability {
    pub retry_item_ids: Vec<String>,
    pub review_required: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductRun {
    #[serde(rename = "ref")]
    pub run_ref: RunRef,
    pub project_id: String,
    pub title: String,
    pub status: String,
    pub phase: String,
    pub created_at: String,
    pub updated_at: String,
    pub progress: RunProgress,
    pub recoverability: Recoverability,
    pub results_summary: Vec<String>,
    pub error_summary: Option<String>,
    pub preferred_parent: Option<RunRef>,
    pub available_actions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<RunDetail>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRetryRequest {
    #[serde(rename = "ref")]
    pub run_ref: RunRef,
    #[serde(default)]
    pub selected_item_ids: Vec<String>,
}

pub struct ProductRunFacade {
    queue: Arc<ProductionQueueService>,
    production: Arc<ProductionOrchestratorService>,
    tasks: Arc<TaskQueryService>,
}

fn queue_error(error: ProductionQueueError) -> ProductError {
    match error {
        ProductionQueueError::NotFound(_) => {
            ProductError::new("RUN_NOT_FOUND", "运行不存在或不属于当前项目。", None)
        }
        ProductionQueueError::InvalidInput(_)
        | ProductionQueueError::InvalidState(_)
        | ProductionQueueError::Busy(_) => ProductError::new(
            "RUN_NOT_RETRYABLE",
            "当前运行不能自动恢复，请检查运行状态和输入。",
            Some("REVIEW_RUN"),
        ),
        other => ProductError::internal(other),
    }
}

fn production_error(error: ProductionOrchestratorError) -> ProductError {
    match error {
        ProductionOrchestratorError::NotFound(_) => {
            ProductError::new("RUN_NOT_FOUND", "运行不存在或不属于当前项目。", None)
        }
        ProductionOrchestratorError::InvalidState(_)
        | ProductionOrchestratorError::InvalidInput(_) => ProductError::new(
            "RUN_NOT_RETRYABLE",
            "当前阶段不支持自动重试，请检查运行状态和输入。",
            Some("REVIEW_RUN"),
        ),
        other => ProductError::internal(other),
    }
}

fn terminal_status(progress: &RunProgress) -> &'static str {
    if progress.failed > 0 {
        if progress.succeeded > 0 {
            "PARTIAL"
        } else {
            "FAILED"
        }
    } else if progress.cancelled > 0 {
        if progress.succeeded > 0 {
            "PARTIAL"
        } else {
            "CANCELLED"
        }
    } else if progress.total > 0 && progress.succeeded == progress.total {
        "SUCCEEDED"
    } else {
        "PARTIAL"
    }
}

impl ProductRunFacade {
    pub fn new(
        queue: Arc<ProductionQueueService>,
        production: Arc<ProductionOrchestratorService>,
        tasks: Arc<TaskQueryService>,
    ) -> Self {
        Self {
            queue,
            production,
            tasks,
        }
    }

    async fn task_parent(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<Option<(String, String)>, ProductError> {
        for batch in self.queue.list(project_id).await.map_err(queue_error)? {
            let detail = self
                .queue
                .get(project_id, batch.id.as_str())
                .await
                .map_err(queue_error)?;
            if let Some(item) = detail
                .items
                .iter()
                .find(|item| item.task_id.as_deref() == Some(task_id))
            {
                return Ok(Some((
                    batch.id.as_str().to_owned(),
                    item.id.as_str().to_owned(),
                )));
            }
        }
        Ok(None)
    }

    pub async fn get(&self, project_id: &str, run_ref: RunRef) -> Result<ProductRun, ProductError> {
        self.get_with_parent_reads(project_id, run_ref, None).await
    }

    async fn task_parent_with_reads(
        &self,
        project_id: &str,
        task_id: &str,
        reads: &mut TaskParentReads,
    ) -> Result<Option<(String, String)>, ProductError> {
        if reads.batch_ids.is_none() {
            reads.batch_ids = Some(
                self.queue
                    .list(project_id)
                    .await
                    .map_err(queue_error)?
                    .into_iter()
                    .map(|batch| batch.id.as_str().to_owned())
                    .collect(),
            );
        }
        // Preserve queue order, archived parents and first matching item. Load lazily:
        // a later unread batch must not introduce an error after an earlier match.
        for batch_id in reads.batch_ids.as_ref().unwrap() {
            if !reads.items.contains_key(batch_id) {
                let detail = self
                    .queue
                    .get(project_id, batch_id)
                    .await
                    .map_err(queue_error)?;
                reads.items.insert(
                    batch_id.clone(),
                    detail
                        .items
                        .into_iter()
                        .filter_map(|item| {
                            item.task_id.map(|task| (task, item.id.as_str().to_owned()))
                        })
                        .collect(),
                );
            }
            if let Some((_, item_id)) = reads.items[batch_id]
                .iter()
                .find(|(task, _)| task == task_id)
            {
                return Ok(Some((batch_id.clone(), item_id.clone())));
            }
        }
        Ok(None)
    }

    async fn get_with_parent_reads(
        &self,
        project_id: &str,
        run_ref: RunRef,
        parent_reads: Option<&mut TaskParentReads>,
    ) -> Result<ProductRun, ProductError> {
        crate::domain::validate_project_id(project_id)
            .map_err(|_| ProductError::new("INVALID_INPUT", "项目标识无效。", None))?;
        let mut progress = RunProgress::default();
        let mut results = HashSet::new();
        let mut retry_item_ids = Vec::new();
        let mut review_required = 0;
        let mut deferred_error = None;
        let mut preferred_parent = None;
        let mut available_actions = Vec::new();
        let (title, status, phase, created_at, updated_at) = match run_ref.source {
            RunSource::QueueBatch => {
                let detail = self
                    .queue
                    .get(project_id, &run_ref.id)
                    .await
                    .map_err(queue_error)?;
                let deferred_start = self
                    .queue
                    .deferred_start_state(project_id, &run_ref.id)
                    .await
                    .map_err(queue_error)?;
                if let Some((state, reason)) = &deferred_start {
                    if state == "BLOCKED" && detail.batch.status == ProductionBatchStatus::Ready {
                        deferred_error = Some(
                            reason
                                .as_deref()
                                .map(|reason| format!("排队任务无法自动启动：{reason}"))
                                .unwrap_or_else(|| {
                                    "排队任务无法自动启动，请查看运行详情。".to_owned()
                                }),
                        );
                    }
                }
                if detail.batch.project_id != project_id {
                    return Err(ProductError::new(
                        "PROJECT_SCOPE_VIOLATION",
                        "无法访问其他项目的运行。",
                        None,
                    ));
                }
                if detail.batch.archived_at.is_none() {
                    let pending = detail
                        .items
                        .iter()
                        .any(|item| item.status == ProductionBatchItemStatus::Pending);
                    let active = detail.items.iter().any(|item| {
                        matches!(
                            item.status,
                            ProductionBatchItemStatus::Dispatching
                                | ProductionBatchItemStatus::Dispatched
                        )
                    });
                    if pending
                        && !(detail.batch.status == ProductionBatchStatus::Ready
                            && deferred_start
                                .as_ref()
                                .is_some_and(|(state, _)| state == "WAITING"))
                        && matches!(
                            detail.batch.status,
                            ProductionBatchStatus::Ready | ProductionBatchStatus::Paused
                        )
                    {
                        available_actions.push("START".to_owned());
                    }
                    if matches!(
                        detail.batch.status,
                        ProductionBatchStatus::Ready | ProductionBatchStatus::Running
                    ) {
                        available_actions.push("PAUSE".to_owned());
                    }
                    // Existing batch cancellation cancels pending items, not active tasks.
                    if pending && !active && detail.batch.status != ProductionBatchStatus::Completed
                    {
                        available_actions.push("CANCEL".to_owned());
                    }
                }
                let plan = self
                    .queue
                    .partial_resume_plan(project_id, &run_ref.id)
                    .await
                    .map_err(queue_error)?;
                progress.total = plan.logical_total;
                progress.succeeded = plan.resolved;
                review_required = plan.review_required;
                for entry in &plan.entries {
                    if let Some(item) = detail
                        .items
                        .iter()
                        .find(|item| item.id.as_str() == entry.leaf_item_id)
                    {
                        progress.failed +=
                            usize::from(item.status == ProductionBatchItemStatus::Failed);
                        progress.cancelled += usize::from(matches!(
                            item.status,
                            ProductionBatchItemStatus::Cancelled
                                | ProductionBatchItemStatus::Skipped
                        ));
                    }
                    if entry.eligibility == "AUTO_RESUMABLE" {
                        retry_item_ids.push(entry.leaf_item_id.clone());
                    }
                    if let Some(task_id) = &entry.task_id {
                        if let Some(task) = self
                            .tasks
                            .get(project_id, task_id)
                            .await
                            .map_err(ProductError::internal)?
                        {
                            results.extend(task.output_asset_ids);
                        }
                    }
                }
                preferred_parent = self
                    .production
                    .parent_run_for_batch(project_id, &run_ref.id)
                    .await
                    .map_err(production_error)?
                    .map(|id| RunRef {
                        source: RunSource::ProductionRun,
                        id,
                    });
                let status = match detail.batch.status {
                    ProductionBatchStatus::Ready if deferred_error.is_some() => "PAUSED",
                    ProductionBatchStatus::Ready => "QUEUED",
                    ProductionBatchStatus::Running => "RUNNING",
                    ProductionBatchStatus::Paused => "PAUSED",
                    ProductionBatchStatus::Completed => terminal_status(&progress),
                };
                (
                    detail.batch.name,
                    status.to_owned(),
                    "GENERATION".to_owned(),
                    detail.batch.created_at.to_rfc3339(),
                    detail.batch.updated_at.to_rfc3339(),
                )
            }
            RunSource::Task => {
                let task = self
                    .tasks
                    .get(project_id, &run_ref.id)
                    .await
                    .map_err(ProductError::internal)?
                    .ok_or_else(|| {
                        ProductError::new("RUN_NOT_FOUND", "运行不存在或不属于当前项目。", None)
                    })?;
                if task.project_id != project_id {
                    return Err(ProductError::new(
                        "PROJECT_SCOPE_VIOLATION",
                        "无法访问其他项目的运行。",
                        None,
                    ));
                }
                if !matches!(
                    task.status.as_str(),
                    "SUCCEEDED" | "FAILED" | "CANCELLED" | "CANCEL_REQUESTED"
                ) {
                    available_actions.push("CANCEL".to_owned());
                }
                progress.total = 1;
                progress.succeeded = usize::from(task.status == "SUCCEEDED");
                progress.failed = usize::from(task.status == "FAILED");
                progress.cancelled = usize::from(task.status == "CANCELLED");
                let parent = match parent_reads {
                    Some(reads) => {
                        self.task_parent_with_reads(project_id, &run_ref.id, reads)
                            .await?
                    }
                    None => self.task_parent(project_id, &run_ref.id).await?,
                };
                if let Some((batch_id, item_id)) = parent {
                    preferred_parent = Some(RunRef {
                        source: RunSource::QueueBatch,
                        id: batch_id.clone(),
                    });
                    let plan = self
                        .queue
                        .partial_resume_plan(project_id, &batch_id)
                        .await
                        .map_err(queue_error)?;
                    if plan.entries.iter().any(|entry| {
                        entry.leaf_item_id == item_id && entry.eligibility == "AUTO_RESUMABLE"
                    }) {
                        retry_item_ids.push(item_id);
                    }
                }
                review_required =
                    usize::from(task.error.as_ref().is_some_and(|e| is_input_error(&e.code)));
                results.extend(task.output_asset_ids);
                let status = match task.status.as_str() {
                    "SUCCEEDED" => "SUCCEEDED",
                    "FAILED" => "FAILED",
                    "CANCELLED" => "CANCELLED",
                    "QUEUED" | "CREATED" => "QUEUED",
                    _ => "RUNNING",
                };
                (
                    "历史生成任务".to_owned(),
                    status.to_owned(),
                    task.status,
                    task.created_at.to_rfc3339(),
                    task.finished_at
                        .or(task.started_at)
                        .or(task.queued_at)
                        .unwrap_or(task.created_at)
                        .to_rfc3339(),
                )
            }
            RunSource::ProductionRun => {
                let run = self
                    .production
                    .get_projection(project_id, &run_ref.id)
                    .await
                    .map_err(production_error)?;
                if run.project_id != project_id {
                    return Err(ProductError::new(
                        "PROJECT_SCOPE_VIOLATION",
                        "无法访问其他项目的运行。",
                        None,
                    ));
                }
                if run.status == "READY"
                    && run
                        .stages
                        .iter()
                        .any(|stage| stage.ordinal == 0 && stage.status == "READY")
                {
                    available_actions.push("START".to_owned());
                }
                if !matches!(run.status.as_str(), "SUCCEEDED" | "CANCELLED") {
                    available_actions.push("CANCEL".to_owned());
                }
                for stage in &run.stages {
                    let parents: HashSet<&str> = stage
                        .items
                        .iter()
                        .filter_map(|item| item.parent_stage_item_id.as_deref())
                        .collect();
                    for item in stage
                        .items
                        .iter()
                        .filter(|item| !parents.contains(item.id.as_str()))
                    {
                        progress.total += 1;
                        progress.succeeded += usize::from(item.status == "SUCCEEDED");
                        progress.failed += usize::from(item.status == "FAILED");
                        progress.cancelled +=
                            usize::from(item.status == "CANCELLED" || item.status == "SKIPPED");
                        if let Some(asset) = &item.asset_id {
                            results.insert(asset.clone());
                        }
                    }
                }
                let mut status = match run.status.as_str() {
                    "PARTIAL_FAILED" => "PARTIAL",
                    "SUCCEEDED" => terminal_status(&progress),
                    "FAILED" => "FAILED",
                    "CANCELLED" => "CANCELLED",
                    "RUNNING" => "RUNNING",
                    "WAITING_FOR_SELECTION" => "PAUSED",
                    _ => "QUEUED",
                };
                let mut can_retry_video = run
                    .stages
                    .iter()
                    .any(|stage| stage.ordinal == 2 && stage.status == "FAILED");
                // Parent lifecycle persistence can lag its child queue. Project current
                // facts without invoking the legacy synchronizing read or changing state.
                if run.status != "CANCELLED" {
                    if let Some(stage) = run
                        .stages
                        .iter()
                        .rev()
                        .find(|stage| stage.ordinal != 1 && stage.production_batch_id.is_some())
                    {
                        let batch_id = stage.production_batch_id.as_deref().unwrap();
                        let batch = self
                            .queue
                            .get(project_id, batch_id)
                            .await
                            .map_err(queue_error)?;
                        let plan = self
                            .queue
                            .partial_resume_plan(project_id, batch_id)
                            .await
                            .map_err(queue_error)?;
                        progress = RunProgress {
                            total: plan.logical_total,
                            succeeded: plan.resolved,
                            ..RunProgress::default()
                        };
                        for entry in &plan.entries {
                            if let Some(item) = batch
                                .items
                                .iter()
                                .find(|item| item.id.as_str() == entry.leaf_item_id)
                            {
                                progress.failed +=
                                    usize::from(item.status == ProductionBatchItemStatus::Failed);
                                progress.cancelled += usize::from(matches!(
                                    item.status,
                                    ProductionBatchItemStatus::Cancelled
                                        | ProductionBatchItemStatus::Skipped
                                ));
                            }
                        }
                        status = match batch.batch.status {
                            ProductionBatchStatus::Ready => "QUEUED",
                            ProductionBatchStatus::Running => "RUNNING",
                            ProductionBatchStatus::Paused => "PAUSED",
                            ProductionBatchStatus::Completed
                                if stage.ordinal == 0 && progress.succeeded > 0 =>
                            {
                                "PAUSED"
                            }
                            ProductionBatchStatus::Completed => terminal_status(&progress),
                        };
                        can_retry_video = stage.ordinal == 2
                            && progress.failed > 0
                            && !plan
                                .entries
                                .iter()
                                .any(|entry| entry.eligibility == "REVIEW_REQUIRED");
                    }
                }
                if can_retry_video {
                    retry_item_ids.push(run.id.clone());
                }
                (
                    run.name,
                    status.to_owned(),
                    format!("STAGE_{}", run.current_stage_ordinal),
                    run.created_at,
                    run.updated_at,
                )
            }
        };
        if !retry_item_ids.is_empty() {
            available_actions.push("RETRY".to_owned());
        }
        if status == "PAUSED" {
            available_actions.push("REVIEW_RECOVERY".to_owned());
        }
        if review_required > 0 {
            available_actions.push("EDIT_INPUT".to_owned());
        }
        let error_summary = deferred_error.or_else(|| {
            (progress.failed > 0 || review_required > 0)
                .then(|| "部分生成未完成，请检查失败项与输入。".to_owned())
        });
        let mut results_summary: Vec<String> = results.into_iter().collect();
        results_summary.sort();
        Ok(ProductRun {
            run_ref,
            project_id: project_id.to_owned(),
            title,
            status,
            phase,
            created_at,
            updated_at,
            progress,
            recoverability: Recoverability {
                retry_item_ids,
                review_required,
            },
            results_summary,
            error_summary,
            preferred_parent,
            available_actions,
            detail: None,
        })
    }

    pub async fn retry(
        &self,
        project_id: &str,
        request: RunRetryRequest,
    ) -> Result<ProductRun, ProductError> {
        // Scope validation precedes every mutation, even an idempotent replay.
        let current = self.get(project_id, request.run_ref.clone()).await?;
        if current.recoverability.review_required > 0
            && current.recoverability.retry_item_ids.is_empty()
        {
            let mut error = ProductError::new(
                "EDIT_INPUT_REQUIRED",
                "请先修正输入，再创建新的生成任务。",
                Some("EDIT_INPUT"),
            );
            let mut location = RunActionLocation {
                project_id: project_id.to_owned(),
                run_ref: request.run_ref.clone(),
                shot_id: None,
                stage: None,
            };
            if request.run_ref.source == RunSource::Task {
                if let Some((batch, item)) =
                    self.task_parent(project_id, &request.run_ref.id).await?
                {
                    let detail = self
                        .queue
                        .get(project_id, &batch)
                        .await
                        .map_err(queue_error)?;
                    if let Some(context) = detail
                        .items
                        .iter()
                        .find(|i| i.id.as_str() == item)
                        .and_then(|i| i.values_json.get("__ai_studio_direct_generation_context"))
                    {
                        location.shot_id = context
                            .get("shotId")
                            .and_then(|v| v.as_str())
                            .map(str::to_owned);
                        location.stage = context
                            .get("stage")
                            .and_then(|v| v.as_str())
                            .map(str::to_owned);
                    }
                }
            }
            error.details.action_location = Some(location);
            return Err(error);
        }
        match request.run_ref.source {
            RunSource::QueueBatch => {
                if request.selected_item_ids.is_empty() {
                    return Err(ProductError::new(
                        "RUN_NOT_RETRYABLE",
                        "请选择需要恢复的失败项。",
                        Some("REVIEW_RUN"),
                    ));
                }
                self.queue
                    .partial_resume(project_id, &request.run_ref.id, &request.selected_item_ids)
                    .await
                    .map_err(queue_error)?;
            }
            RunSource::ProductionRun => {
                self.production
                    .retry_video(project_id, &request.run_ref.id)
                    .await
                    .map_err(production_error)?;
            }
            RunSource::Task => {
                let Some((batch, item)) = self.task_parent(project_id, &request.run_ref.id).await?
                else {
                    return Err(ProductError::new(
                        "RUN_NOT_RETRYABLE",
                        "此历史任务没有可安全恢复的队列快照。",
                        Some("REVIEW_RUN"),
                    ));
                };
                self.queue
                    .partial_resume(project_id, &batch, &[item])
                    .await
                    .map_err(queue_error)?;
            }
        }
        self.get(project_id, request.run_ref).await
    }
}

fn is_input_error(code: &str) -> bool {
    matches!(
        code,
        "INPUT_REQUIRED"
            | "INPUT_OUT_OF_RANGE"
            | "INVALID_INPUT"
            | "VALIDATION_ERROR"
            | "MISSING_SOURCE_INPUT"
            | "MISSING_REQUIRED_INPUT"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_run_completed_is_not_always_success() {
        assert_eq!(
            terminal_status(&RunProgress {
                total: 2,
                succeeded: 1,
                failed: 1,
                cancelled: 0
            }),
            "PARTIAL"
        );
        assert_eq!(
            terminal_status(&RunProgress {
                total: 1,
                succeeded: 0,
                failed: 1,
                cancelled: 0
            }),
            "FAILED"
        );
        assert_eq!(
            terminal_status(&RunProgress {
                total: 1,
                succeeded: 1,
                failed: 0,
                cancelled: 0
            }),
            "SUCCEEDED"
        );
        assert_eq!(
            terminal_status(&RunProgress {
                total: 1,
                succeeded: 0,
                failed: 0,
                cancelled: 1
            }),
            "CANCELLED"
        );
    }
}
