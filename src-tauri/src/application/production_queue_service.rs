use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::generation_service::{CreateGenerationRequest, GenerationService, GenerationServiceError};
use crate::application::ports::{Clock, ProductionQueueRepository, RepositoryError, TaskRepository};
use crate::domain::{
    AssetId, ProductionBatch, ProductionBatchDetail, ProductionBatchId, ProductionBatchItem,
    ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus, SeedValue, TaskId,
    TaskStatus,
};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};
use tokio::time::{sleep, Duration};

const MAX_PRODUCTION_BATCH_ITEMS: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct CreateProductionBatchItem {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, GenerationInputValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateProductionBatchRequest {
    pub project_id: String,
    pub name: String,
    pub continue_on_failure: bool,
    pub items: Vec<CreateProductionBatchItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductionQueueOverview {
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

pub struct ProductionQueueService {
    repository: Arc<dyn ProductionQueueRepository>,
    task_repository: Arc<dyn TaskRepository>,
    generation_service: Arc<GenerationService>,
    clock: Arc<dyn Clock>,
    running_batches: Arc<Mutex<HashSet<String>>>,
}

impl ProductionQueueService {
    pub fn new(
        repository: Arc<dyn ProductionQueueRepository>,
        task_repository: Arc<dyn TaskRepository>,
        generation_service: Arc<GenerationService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            task_repository,
            generation_service,
            clock,
            running_batches: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn create(
        &self,
        request: CreateProductionBatchRequest,
    ) -> Result<ProductionBatchDetail, ProductionQueueError> {
        crate::domain::validate_project_id(&request.project_id)
            .map_err(|error| ProductionQueueError::InvalidInput(error.to_string()))?;
        let name = request.name.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(ProductionQueueError::InvalidInput(
                "production batch name must contain 1..120 characters".to_owned(),
            ));
        }
        if request.items.is_empty() || request.items.len() > MAX_PRODUCTION_BATCH_ITEMS {
            return Err(ProductionQueueError::InvalidInput(format!(
                "production batch must contain 1..{MAX_PRODUCTION_BATCH_ITEMS} items"
            )));
        }

        let now = self.clock.now();
        let batch_id = ProductionBatchId::new();
        let batch = ProductionBatch {
            id: batch_id.clone(),
            project_id: request.project_id,
            name: name.to_owned(),
            status: ProductionBatchStatus::Ready,
            continue_on_failure: request.continue_on_failure,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        let items = request
            .items
            .into_iter()
            .enumerate()
            .map(|(index, item)| ProductionBatchItem {
                id: ProductionBatchItemId::new(),
                batch_id: batch_id.clone(),
                ordinal: u32::try_from(index).expect("production batch item index must fit u32"),
                workflow_version_id: item.workflow_version_id,
                recipe_id: item.recipe_id,
                values_json: generation_values_to_json(&item.values),
                status: ProductionBatchItemStatus::Pending,
                task_id: None,
                retry_of_item_id: None,
                error_code: None,
                error_message: None,
                created_at: now,
                updated_at: now,
            })
            .collect::<Vec<_>>();
        self.repository.insert(&batch, &items).await?;
        Ok(ProductionBatchDetail { batch, items })
    }

    pub async fn list(&self, project_id: &str) -> Result<Vec<ProductionBatch>, ProductionQueueError> {
        crate::domain::validate_project_id(project_id)
            .map_err(|error| ProductionQueueError::InvalidInput(error.to_string()))?;
        Ok(self.repository.list(project_id).await?)
    }

    pub async fn overview(&self, project_id: &str) -> Result<ProductionQueueOverview, ProductionQueueError> {
        crate::domain::validate_project_id(project_id)
            .map_err(|error| ProductionQueueError::InvalidInput(error.to_string()))?;
        let batches = self.repository.list(project_id).await?;
        let mut overview = ProductionQueueOverview::default();
        for batch in batches {
            if batch.archived_at.is_some() {
                overview.archived_queues += 1;
                continue;
            }
            overview.total_queues += 1;
            match batch.status {
                ProductionBatchStatus::Running => overview.running_queues += 1,
                ProductionBatchStatus::Paused => overview.paused_queues += 1,
                ProductionBatchStatus::Completed => overview.completed_queues += 1,
                ProductionBatchStatus::Ready => {}
            }
            let Some(detail) = self.repository.find_detail(project_id, &batch.id).await? else {
                continue;
            };
            overview.total_items += detail.items.len();
            for item in detail.items {
                match item.status {
                    ProductionBatchItemStatus::Pending => overview.pending_items += 1,
                    ProductionBatchItemStatus::Dispatching | ProductionBatchItemStatus::Dispatched => {
                        overview.active_items += 1
                    }
                    ProductionBatchItemStatus::Succeeded => overview.succeeded_items += 1,
                    ProductionBatchItemStatus::Failed => overview.failed_items += 1,
                    ProductionBatchItemStatus::Cancelled => overview.cancelled_items += 1,
                    ProductionBatchItemStatus::Skipped => overview.skipped_items += 1,
                }
            }
        }
        Ok(overview)
    }

    pub async fn get(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<ProductionBatchDetail, ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        self.repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))
    }

    pub async fn start(self: &Arc<Self>, project_id: &str, batch_id: &str) -> Result<(), ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        if detail.batch.archived_at.is_some() {
            return Err(ProductionQueueError::InvalidState(
                "archived production batches must be restored before starting".to_owned(),
            ));
        }
        if detail.batch.status == ProductionBatchStatus::Completed {
            return Err(ProductionQueueError::InvalidState(
                "completed production batches cannot be restarted".to_owned(),
            ));
        }
        self.repository
            .set_batch_status(project_id, &batch_id, ProductionBatchStatus::Running, self.clock.now())
            .await?;
        self.spawn_if_needed(project_id.to_owned(), batch_id);
        Ok(())
    }

    pub async fn pause(&self, project_id: &str, batch_id: &str) -> Result<(), ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        if detail.batch.archived_at.is_some() {
            return Err(ProductionQueueError::InvalidState(
                "archived production batches cannot be paused".to_owned(),
            ));
        }
        if detail.batch.status == ProductionBatchStatus::Completed {
            return Err(ProductionQueueError::InvalidState(
                "completed production batches cannot be paused".to_owned(),
            ));
        }
        self.repository
            .set_batch_status(project_id, &batch_id, ProductionBatchStatus::Paused, self.clock.now())
            .await?;
        Ok(())
    }

    pub async fn archive(&self, project_id: &str, batch_id: &str) -> Result<(), ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        ensure_batch_not_active(&detail, "archive")?;
        if detail.batch.archived_at.is_none() {
            self.repository
                .set_archived_at(project_id, &batch_id, Some(self.clock.now()), self.clock.now())
                .await?;
        }
        Ok(())
    }

    pub async fn restore(&self, project_id: &str, batch_id: &str) -> Result<(), ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        if detail.batch.archived_at.is_some() {
            self.repository
                .set_archived_at(project_id, &batch_id, None, self.clock.now())
                .await?;
        }
        Ok(())
    }

    pub async fn delete(&self, project_id: &str, batch_id: &str) -> Result<(), ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        ensure_batch_not_active(&detail, "delete")?;
        if detail.batch.archived_at.is_none() {
            return Err(ProductionQueueError::InvalidState(
                "production batch must be archived before deletion".to_owned(),
            ));
        }
        if !self.repository.delete_batch(project_id, &batch_id).await? {
            return Err(ProductionQueueError::NotFound(batch_id.as_str().to_owned()));
        }
        Ok(())
    }

    pub async fn skip_item(
        &self,
        project_id: &str,
        batch_id: &str,
        item_id: &str,
    ) -> Result<ProductionBatchDetail, ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        if detail.batch.archived_at.is_some() {
            return Err(ProductionQueueError::InvalidState(
                "restore the archived production batch before skipping an item".to_owned(),
            ));
        }
        ensure_batch_not_active(&detail, "skip an item in")?;
        let item = detail
            .items
            .iter()
            .find(|item| item.id.as_str() == item_id)
            .ok_or_else(|| ProductionQueueError::NotFound(item_id.to_owned()))?;
        if !matches!(
            item.status,
            ProductionBatchItemStatus::Failed | ProductionBatchItemStatus::Cancelled
        ) {
            return Err(ProductionQueueError::InvalidState(
                "only failed or cancelled production items can be skipped".to_owned(),
            ));
        }
        if !self.repository.set_item_skipped(&item.id, self.clock.now()).await? {
            return Err(ProductionQueueError::InvalidState(
                "production item was not in a skippable state".to_owned(),
            ));
        }
        self.get(project_id, batch_id.as_str()).await
    }

    pub async fn requeue_item(
        &self,
        project_id: &str,
        batch_id: &str,
        item_id: &str,
    ) -> Result<ProductionBatchDetail, ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        if detail.batch.archived_at.is_some() {
            return Err(ProductionQueueError::InvalidState(
                "restore the archived production batch before requeueing an item".to_owned(),
            ));
        }
        ensure_batch_not_active(&detail, "requeue an item in")?;
        if detail.items.len() >= MAX_PRODUCTION_BATCH_ITEMS {
            return Err(ProductionQueueError::InvalidState(format!(
                "production batch already contains the maximum {MAX_PRODUCTION_BATCH_ITEMS} items"
            )));
        }
        let source = detail
            .items
            .iter()
            .find(|item| item.id.as_str() == item_id)
            .ok_or_else(|| ProductionQueueError::NotFound(item_id.to_owned()))?;
        if !is_safe_requeue_source(source) {
            return Err(ProductionQueueError::InvalidState(
                "this production item is not safe to requeue automatically; review the failure and inputs instead"
                    .to_owned(),
            ));
        }
        let ordinal = detail
            .items
            .iter()
            .map(|item| item.ordinal)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| ProductionQueueError::InvalidState("production queue ordinal overflow".to_owned()))?;
        let now = self.clock.now();
        let retry = ProductionBatchItem {
            id: ProductionBatchItemId::new(),
            batch_id: batch_id.clone(),
            ordinal,
            workflow_version_id: source.workflow_version_id.clone(),
            recipe_id: source.recipe_id.clone(),
            values_json: source.values_json.clone(),
            status: ProductionBatchItemStatus::Pending,
            task_id: None,
            retry_of_item_id: Some(source.id.as_str().to_owned()),
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        self.repository.append_requeue_item(&retry, now).await?;
        self.get(project_id, batch_id.as_str()).await
    }

    pub async fn recover_and_resume(self: &Arc<Self>) -> Result<(), ProductionQueueError> {
        let uncertain = self
            .repository
            .recover_uncertain_dispatches(self.clock.now())
            .await?;
        for batch_id in uncertain {
            tracing::warn!(batch_id = %batch_id.as_str(), "production batch paused after uncertain dispatch recovery");
        }
        let running = self.repository.list_running().await?;
        for batch in running {
            self.spawn_if_needed(batch.project_id, batch.id);
        }
        Ok(())
    }

    fn spawn_if_needed(self: &Arc<Self>, project_id: String, batch_id: ProductionBatchId) {
        let key = batch_id.as_str().to_owned();
        {
            let mut running = self
                .running_batches
                .lock()
                .expect("production queue runner registry mutex poisoned");
            if !running.insert(key.clone()) {
                return;
            }
        }
        let service = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(error) = service.run_loop(&project_id, &batch_id).await {
                tracing::error!(batch_id = %batch_id.as_str(), error = %error, "production queue runner failed");
                let _ = service
                    .repository
                    .set_batch_status(&project_id, &batch_id, ProductionBatchStatus::Paused, service.clock.now())
                    .await;
            }
            service
                .running_batches
                .lock()
                .expect("production queue runner registry mutex poisoned")
                .remove(&key);
        });
    }

    async fn run_loop(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
    ) -> Result<(), ProductionQueueError> {
        loop {
            let detail = self
                .repository
                .find_detail(project_id, batch_id)
                .await?
                .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
            if !matches!(
                detail.batch.status,
                ProductionBatchStatus::Running | ProductionBatchStatus::Paused
            ) {
                return Ok(());
            }

            if let Some(active) = detail
                .items
                .iter()
                .find(|item| item.status == ProductionBatchItemStatus::Dispatched)
            {
                let Some(task_id) = active.task_id.as_deref() else {
                    self.repository
                        .set_batch_status(project_id, batch_id, ProductionBatchStatus::Paused, self.clock.now())
                        .await?;
                    return Err(ProductionQueueError::InvalidState(
                        "dispatched production item has no task id".to_owned(),
                    ));
                };
                let task_id = TaskId::parse(task_id.to_owned())
                    .map_err(|error| ProductionQueueError::InvalidState(error.to_string()))?;
                let task = self.task_repository.find_by_id(&task_id).await?;
                let Some(task) = task else {
                    self.repository
                        .set_batch_status(project_id, batch_id, ProductionBatchStatus::Paused, self.clock.now())
                        .await?;
                    return Err(ProductionQueueError::InvalidState(
                        "production item references a missing task".to_owned(),
                    ));
                };
                let terminal = match task.status {
                    TaskStatus::Succeeded => Some((ProductionBatchItemStatus::Succeeded, None, None)),
                    TaskStatus::Failed => Some((
                        ProductionBatchItemStatus::Failed,
                        task.error.as_ref().map(|error| error.code.as_str()),
                        task.error.as_ref().map(|error| error.message.as_str()),
                    )),
                    TaskStatus::Cancelled => Some((ProductionBatchItemStatus::Cancelled, None, None)),
                    _ => None,
                };
                if let Some((status, code, message)) = terminal {
                    self.repository
                        .finish_item(&active.id, status, code, message, self.clock.now())
                        .await?;
                    if detail.batch.status == ProductionBatchStatus::Paused {
                        return Ok(());
                    }
                    if should_pause_after_terminal(
                        status,
                        code,
                        detail.batch.continue_on_failure,
                    ) {
                        self.repository
                            .set_batch_status(
                                project_id,
                                batch_id,
                                ProductionBatchStatus::Paused,
                                self.clock.now(),
                            )
                            .await?;
                        return Ok(());
                    }
                    continue;
                }
                sleep(Duration::from_millis(750)).await;
                continue;
            }

            if detail.batch.status == ProductionBatchStatus::Paused {
                return Ok(());
            }

            if detail
                .items
                .iter()
                .any(|item| item.status == ProductionBatchItemStatus::Dispatching)
            {
                self.repository
                    .set_batch_status(project_id, batch_id, ProductionBatchStatus::Paused, self.clock.now())
                    .await?;
                return Err(ProductionQueueError::InvalidState(
                    "production batch contains an unresolved dispatching item".to_owned(),
                ));
            }

            if let Some(next) = detail
                .items
                .iter()
                .find(|item| item.status == ProductionBatchItemStatus::Pending)
            {
                if !self
                    .repository
                    .set_item_dispatching(&next.id, self.clock.now())
                    .await?
                {
                    continue;
                }
                let values = match generation_values_from_json(&next.values_json) {
                    Ok(values) => values,
                    Err(error) => {
                        self.repository
                            .finish_item(
                                &next.id,
                                ProductionBatchItemStatus::Failed,
                                Some("QUEUE_VALUES_INVALID"),
                                Some(&error),
                                self.clock.now(),
                            )
                            .await?;
                        if !detail.batch.continue_on_failure {
                            self.repository
                                .set_batch_status(
                                    project_id,
                                    batch_id,
                                    ProductionBatchStatus::Paused,
                                    self.clock.now(),
                                )
                                .await?;
                            return Ok(());
                        }
                        continue;
                    }
                };
                let task = match self
                    .generation_service
                    .start_generation(CreateGenerationRequest {
                        project_id: project_id.to_owned(),
                        workflow_version_id: next.workflow_version_id.clone(),
                        recipe_id: next.recipe_id.clone(),
                        values,
                    })
                    .await
                {
                    Ok(task) => task,
                    Err(error) => {
                        let code = generation_start_error_code(&error);
                        let message = error.to_string();
                        self.repository
                            .finish_item(
                                &next.id,
                                ProductionBatchItemStatus::Failed,
                                Some(code),
                                Some(&message),
                                self.clock.now(),
                            )
                            .await?;
                        if !detail.batch.continue_on_failure {
                            self.repository
                                .set_batch_status(
                                    project_id,
                                    batch_id,
                                    ProductionBatchStatus::Paused,
                                    self.clock.now(),
                                )
                                .await?;
                            return Ok(());
                        }
                        continue;
                    }
                };
                if !self
                    .repository
                    .link_item_task(&next.id, task.id.as_str(), self.clock.now())
                    .await?
                {
                    self.repository
                        .set_batch_status(project_id, batch_id, ProductionBatchStatus::Paused, self.clock.now())
                        .await?;
                    return Err(ProductionQueueError::InvalidState(
                        "task was created but production item linkage was not persisted; batch paused to avoid duplicate dispatch"
                            .to_owned(),
                    ));
                }
                continue;
            }

            if detail.items.iter().all(|item| item.status.is_terminal()) {
                self.repository
                    .set_batch_status(
                        project_id,
                        batch_id,
                        ProductionBatchStatus::Completed,
                        self.clock.now(),
                    )
                    .await?;
                return Ok(());
            }

            sleep(Duration::from_millis(750)).await;
        }
    }
}

fn ensure_batch_not_active(
    detail: &ProductionBatchDetail,
    action: &str,
) -> Result<(), ProductionQueueError> {
    let has_active_item = detail.items.iter().any(|item| {
        matches!(
            item.status,
            ProductionBatchItemStatus::Dispatching | ProductionBatchItemStatus::Dispatched
        )
    });
    if detail.batch.status == ProductionBatchStatus::Running || has_active_item {
        return Err(ProductionQueueError::InvalidState(format!(
            "pause the production batch and wait for the active task to finish before attempting to {action} it"
        )));
    }
    Ok(())
}

fn is_safe_requeue_source(item: &ProductionBatchItem) -> bool {
    match item.status {
        ProductionBatchItemStatus::Cancelled => true,
        ProductionBatchItemStatus::Failed | ProductionBatchItemStatus::Skipped => item
            .error_code
            .as_deref()
            .map(is_transient_requeue_error)
            .unwrap_or(false),
        _ => false,
    }
}

fn is_transient_requeue_error(code: &str) -> bool {
    matches!(
        code,
        "COMFY_OFFLINE"
            | "COMFY_TIMEOUT"
            | "COMFY_STREAM_DISCONNECTED"
            | "COMFY_IMAGE_UPLOAD_FAILED"
            | "COMFY_INPUT_UPLOAD_FAILED"
            | "EXECUTION_INTERRUPTED"
    )
}

fn should_pause_after_terminal(
    status: ProductionBatchItemStatus,
    error_code: Option<&str>,
    continue_on_failure: bool,
) -> bool {
    status != ProductionBatchItemStatus::Succeeded
        && (!continue_on_failure || error_code == Some("EXECUTION_ERROR"))
}

fn generation_start_error_code(error: &GenerationServiceError) -> &'static str {
    match error {
        GenerationServiceError::DefinitionNotFound { .. } => "GENERATION_DEFINITION_NOT_FOUND",
        GenerationServiceError::Repository(_) => "QUEUE_REPOSITORY_ERROR",
        GenerationServiceError::Compile(_) => "QUEUE_COMPILE_ERROR",
        GenerationServiceError::InputPrepare(error) => error.code(),
        GenerationServiceError::Snapshot(_) => "SNAPSHOT_ERROR",
        GenerationServiceError::Domain(_) => "TASK_DOMAIN_ERROR",
        GenerationServiceError::Comfy(_) => "COMFY_ERROR",
        GenerationServiceError::StreamDisconnected(_) => "COMFY_STREAM_DISCONNECTED",
        GenerationServiceError::OutputCollection(_) => "OUTPUT_COLLECTION_ERROR",
        GenerationServiceError::AssetImport(_) => "ASSET_IMPORT_ERROR",
        GenerationServiceError::ExecutionFailed { .. } => "EXECUTION_ERROR",
    }
}

fn generation_values_to_json(values: &BTreeMap<String, GenerationInputValue>) -> Value {
    let mut object = Map::new();
    for (key, value) in values {
        let value = match value {
            GenerationInputValue::Text(value) => json!({"type": "string", "value": value}),
            GenerationInputValue::Integer(value) => json!({"type": "integer", "value": value}),
            GenerationInputValue::Seed(SeedValue::Random) => json!({"type": "seed_random"}),
            GenerationInputValue::Seed(SeedValue::Fixed(value)) => {
                json!({"type": "seed_fixed", "value": value.to_string()})
            }
            GenerationInputValue::ImageAsset(asset_id) => {
                json!({"type": "image_asset", "assetId": asset_id.as_str()})
            }
            GenerationInputValue::ImageAssets(asset_ids) => json!({
                "type": "image_assets",
                "assetIds": asset_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>()
            }),
            GenerationInputValue::VideoAsset(asset_id) => {
                json!({"type": "video_asset", "assetId": asset_id.as_str()})
            }
            GenerationInputValue::AudioAsset(asset_id) => {
                json!({"type": "audio_asset", "assetId": asset_id.as_str()})
            }
            GenerationInputValue::VideoAssets(asset_ids) => json!({
                "type": "video_assets",
                "assetIds": asset_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>()
            }),
            GenerationInputValue::AudioAssets(asset_ids) => json!({
                "type": "audio_assets",
                "assetIds": asset_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>()
            }),
        };
        object.insert(key.clone(), value);
    }
    Value::Object(object)
}

fn generation_values_from_json(value: &Value) -> Result<BTreeMap<String, GenerationInputValue>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "production queue values must be a JSON object".to_owned())?;
    object
        .iter()
        .map(|(key, value)| Ok((key.clone(), generation_value_from_json(key, value)?)))
        .collect()
}

fn generation_value_from_json(key: &str, value: &Value) -> Result<GenerationInputValue, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("production queue value for {key} must be an object"))?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("production queue value for {key} requires type"))?;
    match kind {
        "string" => Ok(GenerationInputValue::Text(required_string(object, "value", key)?)),
        "integer" => Ok(GenerationInputValue::Integer(
            object
                .get("value")
                .and_then(Value::as_i64)
                .ok_or_else(|| format!("production queue integer for {key} is invalid"))?,
        )),
        "seed_random" => Ok(GenerationInputValue::Seed(SeedValue::Random)),
        "seed_fixed" => Ok(GenerationInputValue::Seed(SeedValue::Fixed(
            required_string(object, "value", key)?
                .parse::<u64>()
                .map_err(|_| format!("production queue seed for {key} is invalid"))?,
        ))),
        "image_asset" => Ok(GenerationInputValue::ImageAsset(parse_asset(object, "assetId", key)?)),
        "video_asset" => Ok(GenerationInputValue::VideoAsset(parse_asset(object, "assetId", key)?)),
        "audio_asset" => Ok(GenerationInputValue::AudioAsset(parse_asset(object, "assetId", key)?)),
        "image_assets" => Ok(GenerationInputValue::ImageAssets(parse_assets(object, key)?)),
        "video_assets" => Ok(GenerationInputValue::VideoAssets(parse_assets(object, key)?)),
        "audio_assets" => Ok(GenerationInputValue::AudioAssets(parse_assets(object, key)?)),
        other => Err(format!("unsupported production queue value type {other} for {key}")),
    }
}

fn required_string(object: &Map<String, Value>, field: &str, key: &str) -> Result<String, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("production queue value for {key} requires {field}"))
}

fn parse_asset(object: &Map<String, Value>, field: &str, key: &str) -> Result<AssetId, String> {
    AssetId::parse(required_string(object, field, key)?)
        .map_err(|error| format!("production queue asset for {key} is invalid: {error}"))
}

fn parse_assets(object: &Map<String, Value>, key: &str) -> Result<Vec<AssetId>, String> {
    let values = object
        .get("assetIds")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("production queue asset list for {key} is invalid"))?;
    values
        .iter()
        .map(|value| {
            let value = value
                .as_str()
                .ok_or_else(|| format!("production queue asset list for {key} contains a non-string id"))?;
            AssetId::parse(value.to_owned())
                .map_err(|error| format!("production queue asset for {key} is invalid: {error}"))
        })
        .collect()
}

fn parse_batch_id(value: &str) -> Result<ProductionBatchId, ProductionQueueError> {
    ProductionBatchId::parse(value.to_owned())
        .map_err(|error| ProductionQueueError::InvalidInput(error.to_string()))
}

#[derive(Debug)]
pub enum ProductionQueueError {
    InvalidInput(String),
    InvalidState(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for ProductionQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::InvalidState(message) => write!(formatter, "PRODUCTION_QUEUE_INVALID_STATE: {message}"),
            Self::NotFound(id) => write!(formatter, "PRODUCTION_BATCH_NOT_FOUND: {id}"),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ProductionQueueError {}

impl From<RepositoryError> for ProductionQueueError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        generation_values_from_json, generation_values_to_json, is_transient_requeue_error,
        should_pause_after_terminal,
    };
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::domain::{AssetId, ProductionBatchItemStatus, SeedValue};
    use std::collections::BTreeMap;

    #[test]
    fn queue_values_round_trip_without_losing_seed_or_asset_identity() {
        let mut values = BTreeMap::new();
        values.insert("prompt".to_owned(), GenerationInputValue::Text("hello".to_owned()));
        values.insert("seed".to_owned(), GenerationInputValue::Seed(SeedValue::Fixed(42)));
        values.insert(
            "image".to_owned(),
            GenerationInputValue::ImageAsset(AssetId::parse("ast_test".to_owned()).unwrap()),
        );
        let json = generation_values_to_json(&values);
        assert_eq!(generation_values_from_json(&json).unwrap(), values);
    }

    #[test]
    fn execution_error_always_pauses_even_when_continue_on_failure_is_enabled() {
        assert!(!should_pause_after_terminal(
            ProductionBatchItemStatus::Succeeded,
            None,
            false,
        ));
        assert!(!should_pause_after_terminal(
            ProductionBatchItemStatus::Failed,
            Some("COMFY_TIMEOUT"),
            true,
        ));
        assert!(should_pause_after_terminal(
            ProductionBatchItemStatus::Failed,
            Some("EXECUTION_ERROR"),
            true,
        ));
        assert!(should_pause_after_terminal(
            ProductionBatchItemStatus::Cancelled,
            None,
            false,
        ));
    }

    #[test]
    fn requeue_policy_allows_transient_failures_but_blocks_execution_and_uncertain_dispatch() {
        assert!(is_transient_requeue_error("COMFY_TIMEOUT"));
        assert!(is_transient_requeue_error("COMFY_STREAM_DISCONNECTED"));
        assert!(!is_transient_requeue_error("EXECUTION_ERROR"));
        assert!(!is_transient_requeue_error("QUEUE_DISPATCH_UNCERTAIN"));
    }
}
