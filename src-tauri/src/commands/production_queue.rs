use crate::{
    app_state::AppState,
    application::production_queue_service::{
        CreateProductionBatchItem, CreateProductionBatchRequest, ProductionQueueError,
        ProductionQueueOverview,
    },
    domain::{ProductionBatch, ProductionBatchDetail, ProductionBatchItem},
    error::AppError,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::State;

use super::generation::InputValueDto;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionQueueCreateRequest {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub continue_on_failure: bool,
    pub items: Vec<ProductionQueueCreateItemRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionQueueCreateItemRequest {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, InputValueDto>,
}

impl ProductionQueueCreateItemRequest {
    fn into_application(self) -> Result<CreateProductionBatchItem, AppError> {
        let values = self
            .values
            .into_iter()
            .map(|(key, value)| Ok((key.clone(), value.into_application(&key)?)))
            .collect::<Result<BTreeMap<_, _>, AppError>>()?;
        Ok(CreateProductionBatchItem {
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            values,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchSummaryView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub continue_on_failure: bool,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchItemView {
    pub id: String,
    pub ordinal: u32,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub status: String,
    pub task_id: Option<String>,
    pub retry_of_item_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchDetailView {
    #[serde(flatten)]
    pub batch: ProductionBatchSummaryView,
    pub total: usize,
    pub pending: usize,
    pub running: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub skipped: usize,
    pub items: Vec<ProductionBatchItemView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionQueueOverviewView {
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
}

#[tauri::command]
pub async fn production_queue_create(
    state: State<'_, AppState>,
    request: ProductionQueueCreateRequest,
) -> Result<ProductionBatchDetailView, AppError> {
    let items = request
        .items
        .into_iter()
        .map(ProductionQueueCreateItemRequest::into_application)
        .collect::<Result<Vec<_>, _>>()?;
    let detail = state
        .production_queue_service
        .create(CreateProductionBatchRequest {
            project_id: request.project_id,
            name: request.name,
            continue_on_failure: request.continue_on_failure,
            items,
        })
        .await
        .map_err(map_queue_error)?;
    Ok(detail.into())
}

#[tauri::command]
pub async fn production_queue_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ProductionBatchSummaryView>, AppError> {
    state
        .production_queue_service
        .list(&project_id)
        .await
        .map(|batches| batches.into_iter().map(Into::into).collect())
        .map_err(map_queue_error)
}

#[tauri::command]
pub async fn production_queue_overview(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<ProductionQueueOverviewView, AppError> {
    state
        .production_queue_service
        .overview(&project_id)
        .await
        .map(Into::into)
        .map_err(map_queue_error)
}

#[tauri::command]
pub async fn production_queue_get(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<ProductionBatchDetailView, AppError> {
    state
        .production_queue_service
        .get(&project_id, &batch_id)
        .await
        .map(Into::into)
        .map_err(map_queue_error)
}

#[tauri::command]
pub async fn production_queue_start(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<ProductionBatchDetailView, AppError> {
    state
        .production_queue_service
        .start(&project_id, &batch_id)
        .await
        .map_err(map_queue_error)?;
    production_queue_get(state, project_id, batch_id).await
}

#[tauri::command]
pub async fn production_queue_pause(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<ProductionBatchDetailView, AppError> {
    state
        .production_queue_service
        .pause(&project_id, &batch_id)
        .await
        .map_err(map_queue_error)?;
    production_queue_get(state, project_id, batch_id).await
}

#[tauri::command]
pub async fn production_queue_archive(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<ProductionBatchDetailView, AppError> {
    state
        .production_queue_service
        .archive(&project_id, &batch_id)
        .await
        .map_err(map_queue_error)?;
    production_queue_get(state, project_id, batch_id).await
}

#[tauri::command]
pub async fn production_queue_restore(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<ProductionBatchDetailView, AppError> {
    state
        .production_queue_service
        .restore(&project_id, &batch_id)
        .await
        .map_err(map_queue_error)?;
    production_queue_get(state, project_id, batch_id).await
}

#[tauri::command]
pub async fn production_queue_delete(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<(), AppError> {
    state
        .production_queue_service
        .delete(&project_id, &batch_id)
        .await
        .map_err(map_queue_error)
}

#[tauri::command]
pub async fn production_queue_skip_item(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
    item_id: String,
) -> Result<ProductionBatchDetailView, AppError> {
    state
        .production_queue_service
        .skip_item(&project_id, &batch_id, &item_id)
        .await
        .map(Into::into)
        .map_err(map_queue_error)
}

#[tauri::command]
pub async fn production_queue_requeue_item(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
    item_id: String,
) -> Result<ProductionBatchDetailView, AppError> {
    state
        .production_queue_service
        .requeue_item(&project_id, &batch_id, &item_id)
        .await
        .map(Into::into)
        .map_err(map_queue_error)
}

impl From<ProductionBatch> for ProductionBatchSummaryView {
    fn from(batch: ProductionBatch) -> Self {
        Self {
            id: batch.id.as_str().to_owned(),
            project_id: batch.project_id,
            name: batch.name,
            status: batch.status.as_str().to_owned(),
            continue_on_failure: batch.continue_on_failure,
            archived_at: batch.archived_at.map(|value| value.to_rfc3339()),
            created_at: batch.created_at.to_rfc3339(),
            updated_at: batch.updated_at.to_rfc3339(),
        }
    }
}

impl From<ProductionBatchItem> for ProductionBatchItemView {
    fn from(item: ProductionBatchItem) -> Self {
        Self {
            id: item.id.as_str().to_owned(),
            ordinal: item.ordinal,
            workflow_version_id: item.workflow_version_id,
            recipe_id: item.recipe_id,
            status: item.status.as_str().to_owned(),
            task_id: item.task_id,
            retry_of_item_id: item.retry_of_item_id,
            error_code: item.error_code,
            error_message: item.error_message,
        }
    }
}

impl From<ProductionBatchDetail> for ProductionBatchDetailView {
    fn from(detail: ProductionBatchDetail) -> Self {
        let mut pending = 0;
        let mut running = 0;
        let mut succeeded = 0;
        let mut failed = 0;
        let mut cancelled = 0;
        let mut skipped = 0;
        for item in &detail.items {
            match item.status.as_str() {
                "PENDING" => pending += 1,
                "DISPATCHING" | "DISPATCHED" => running += 1,
                "SUCCEEDED" => succeeded += 1,
                "FAILED" => failed += 1,
                "CANCELLED" => cancelled += 1,
                "SKIPPED" => skipped += 1,
                _ => {}
            }
        }
        Self {
            batch: detail.batch.into(),
            total: detail.items.len(),
            pending,
            running,
            succeeded,
            failed,
            cancelled,
            skipped,
            items: detail.items.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ProductionQueueOverview> for ProductionQueueOverviewView {
    fn from(overview: ProductionQueueOverview) -> Self {
        Self {
            total_queues: overview.total_queues,
            running_queues: overview.running_queues,
            paused_queues: overview.paused_queues,
            completed_queues: overview.completed_queues,
            archived_queues: overview.archived_queues,
            total_items: overview.total_items,
            pending_items: overview.pending_items,
            active_items: overview.active_items,
            succeeded_items: overview.succeeded_items,
            failed_items: overview.failed_items,
            cancelled_items: overview.cancelled_items,
            skipped_items: overview.skipped_items,
        }
    }
}

fn map_queue_error(error: ProductionQueueError) -> AppError {
    match error {
        ProductionQueueError::InvalidInput(message) | ProductionQueueError::InvalidState(message) => {
            AppError::invalid_input(message)
        }
        ProductionQueueError::NotFound(message) => AppError::invalid_input(message),
        ProductionQueueError::Repository(error) => super::map_repository_error(&error),
    }
}
