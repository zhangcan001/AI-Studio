use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::generation_service::{
    CreateGenerationRequest, GenerationService, GenerationServiceError,
};
use crate::application::ports::{
    ActiveProductionItem, Clock, GenerationDefinitionRepository, ProductionQueueRepository,
    RepositoryError, ShotBatchRepository, TaskRepository,
};
use crate::application::task_recovery_service::TaskRecoveryService;
use crate::compiler::{RecipeParser, RecipeValidator, SeedResolver};
use crate::domain::{
    AssetId, InputDefinition, ProductionBatch, ProductionBatchDetail, ProductionBatchId,
    ProductionBatchItem, ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus,
    Recipe, SeedValue, TaskId, TaskStatus,
};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductionAdmissionView {
    pub busy: bool,
    pub batch_id: Option<String>,
    pub project_id: Option<String>,
    pub batch_name: Option<String>,
    pub active_task_id: Option<String>,
}

pub struct ProductionQueueService {
    repository: Arc<dyn ProductionQueueRepository>,
    task_repository: Arc<dyn TaskRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    generation_service: Arc<GenerationService>,
    shot_batch_repository: Arc<dyn ShotBatchRepository>,
    task_recovery_service: Arc<TaskRecoveryService>,
    clock: Arc<dyn Clock>,
    running_batches: Arc<Mutex<HashSet<String>>>,
    admission_gate: Arc<AsyncMutex<()>>,
    recovery_tasks: Arc<Mutex<HashSet<String>>>,
}

impl ProductionQueueService {
    pub fn new(
        repository: Arc<dyn ProductionQueueRepository>,
        task_repository: Arc<dyn TaskRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        generation_service: Arc<GenerationService>,
        shot_batch_repository: Arc<dyn ShotBatchRepository>,
        task_recovery_service: Arc<TaskRecoveryService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            task_repository,
            definition_repository,
            generation_service,
            shot_batch_repository,
            task_recovery_service,
            clock,
            running_batches: Arc::new(Mutex::new(HashSet::new())),
            admission_gate: Arc::new(AsyncMutex::new(())),
            recovery_tasks: Arc::new(Mutex::new(HashSet::new())),
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
        let mut recipes = HashMap::<(String, String), Recipe>::new();
        let mut items = Vec::with_capacity(request.items.len());
        for (index, item) in request.items.into_iter().enumerate() {
            let definition_key = (item.workflow_version_id.clone(), item.recipe_id.clone());
            let recipe = if let Some(recipe) = recipes.get(&definition_key) {
                recipe.clone()
            } else {
                let recipe = self
                    .load_recipe(&item.workflow_version_id, &item.recipe_id)
                    .await?;
                recipes.insert(definition_key, recipe.clone());
                recipe
            };
            let values = freeze_random_seed_values(item.values, &recipe)
                .map_err(ProductionQueueError::InvalidInput)?;
            items.push(ProductionBatchItem {
                id: ProductionBatchItemId::new(),
                batch_id: batch_id.clone(),
                ordinal: u32::try_from(index).expect("production batch item index must fit u32"),
                workflow_version_id: item.workflow_version_id,
                recipe_id: item.recipe_id,
                values_json: generation_values_to_json(&values),
                status: ProductionBatchItemStatus::Pending,
                task_id: None,
                retry_of_item_id: None,
                error_code: None,
                error_message: None,
                created_at: now,
                updated_at: now,
            });
        }
        self.repository.insert(&batch, &items).await?;
        Ok(ProductionBatchDetail { batch, items })
    }

    async fn load_recipe(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Recipe, ProductionQueueError> {
        let definition = self
            .definition_repository
            .find(workflow_version_id, recipe_id)
            .await?
            .ok_or_else(|| {
                ProductionQueueError::InvalidInput(format!(
                    "generation Recipe is unavailable for workflow version {workflow_version_id} and Recipe {recipe_id}"
                ))
            })?;
        let recipe = RecipeParser::parse(&definition.recipe_yaml).map_err(|error| {
            ProductionQueueError::InvalidInput(format!("generation Recipe is invalid: {error}"))
        })?;
        RecipeValidator::validate(&recipe).map_err(|error| {
            ProductionQueueError::InvalidInput(format!("generation Recipe is invalid: {error}"))
        })?;
        Ok(recipe)
    }

    async fn prepare_queue_values(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        values_json: &Value,
    ) -> Result<BTreeMap<String, GenerationInputValue>, ProductionQueueError> {
        let values =
            generation_values_from_json(values_json).map_err(ProductionQueueError::InvalidInput)?;
        let recipe = self.load_recipe(workflow_version_id, recipe_id).await?;
        freeze_random_seed_values(values, &recipe).map_err(ProductionQueueError::InvalidInput)
    }

    pub async fn list(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProductionBatch>, ProductionQueueError> {
        crate::domain::validate_project_id(project_id)
            .map_err(|error| ProductionQueueError::InvalidInput(error.to_string()))?;
        Ok(self.repository.list(project_id).await?)
    }

    pub async fn overview(
        &self,
        project_id: &str,
    ) -> Result<ProductionQueueOverview, ProductionQueueError> {
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
                    ProductionBatchItemStatus::Dispatching
                    | ProductionBatchItemStatus::Dispatched => overview.active_items += 1,
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

    pub async fn admission_status(&self) -> Result<ProductionAdmissionView, ProductionQueueError> {
        self.admission_status_excluding(None).await
    }

    pub async fn acquire_interactive_admission(
        &self,
    ) -> Result<OwnedMutexGuard<()>, ProductionQueueError> {
        let guard = self.acquire_runtime_configuration_admission().await;
        let status = self.admission_status_excluding(None).await?;
        if status.busy {
            return Err(ProductionQueueError::Busy(status));
        }
        Ok(guard)
    }

    /// Serializes endpoint/configuration changes with interactive generation
    /// and production dispatch. Callers must perform their final activity
    /// check while holding the returned guard.
    pub async fn acquire_runtime_configuration_admission(&self) -> OwnedMutexGuard<()> {
        Arc::clone(&self.admission_gate).lock_owned().await
    }

    async fn admission_status_excluding(
        &self,
        excluded_batch_id: Option<&ProductionBatchId>,
    ) -> Result<ProductionAdmissionView, ProductionQueueError> {
        let active = self.repository.list_active_items().await?;
        let running = self.repository.list_running().await?;
        Ok(find_admission_blocker(
            &running,
            &active,
            excluded_batch_id.map(ProductionBatchId::as_str),
        )
        .unwrap_or_default())
    }

    pub async fn start(
        self: &Arc<Self>,
        project_id: &str,
        batch_id: &str,
    ) -> Result<(), ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let _admission = Arc::clone(&self.admission_gate).lock_owned().await;
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
        let blocker = self.admission_status_excluding(Some(&batch_id)).await?;
        if blocker.busy {
            return Err(ProductionQueueError::Busy(blocker));
        }
        self.repository
            .set_batch_status(
                project_id,
                &batch_id,
                ProductionBatchStatus::Running,
                self.clock.now(),
            )
            .await?;
        self.spawn_if_needed(project_id.to_owned(), batch_id);
        Ok(())
    }

    pub async fn pause(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<(), ProductionQueueError> {
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
            .set_batch_status(
                project_id,
                &batch_id,
                ProductionBatchStatus::Paused,
                self.clock.now(),
            )
            .await?;
        Ok(())
    }

    pub async fn cancel_pending(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<ProductionBatchDetail, ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let _admission = Arc::clone(&self.admission_gate).lock_owned().await;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        if detail.batch.archived_at.is_some() {
            return Err(ProductionQueueError::InvalidState(
                "archived production batches must be restored before cancellation".to_owned(),
            ));
        }
        if detail.batch.status == ProductionBatchStatus::Running
            || detail.items.iter().any(|item| {
                matches!(
                    item.status,
                    ProductionBatchItemStatus::Dispatching | ProductionBatchItemStatus::Dispatched
                )
            })
        {
            return Err(ProductionQueueError::InvalidState(
                "production batches with active work cannot be cancelled; pause and wait for the active task to finish".to_owned(),
            ));
        }
        if detail.batch.status == ProductionBatchStatus::Completed {
            return Err(ProductionQueueError::InvalidState(
                "completed production batches cannot be cancelled".to_owned(),
            ));
        }
        let cancelled = self
            .repository
            .cancel_pending_items(project_id, &batch_id, self.clock.now())
            .await?;
        if cancelled == 0 {
            return Err(ProductionQueueError::InvalidState(
                "当前没有可取消的待开始队列项目。".to_owned(),
            ));
        }
        self.repository
            .set_batch_status(
                project_id,
                &batch_id,
                ProductionBatchStatus::Completed,
                self.clock.now(),
            )
            .await?;
        self.get(project_id, batch_id.as_str()).await
    }

    pub async fn archive(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<(), ProductionQueueError> {
        let batch_id = parse_batch_id(batch_id)?;
        let detail = self
            .repository
            .find_detail(project_id, &batch_id)
            .await?
            .ok_or_else(|| ProductionQueueError::NotFound(batch_id.as_str().to_owned()))?;
        ensure_batch_not_active(&detail, "archive")?;
        if detail.batch.archived_at.is_none() {
            self.repository
                .set_archived_at(
                    project_id,
                    &batch_id,
                    Some(self.clock.now()),
                    self.clock.now(),
                )
                .await?;
        }
        Ok(())
    }

    pub async fn restore(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<(), ProductionQueueError> {
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

    pub async fn delete(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<(), ProductionQueueError> {
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
        if !self
            .repository
            .set_item_skipped(&item.id, self.clock.now())
            .await?
        {
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
            .ok_or_else(|| {
                ProductionQueueError::InvalidState("production queue ordinal overflow".to_owned())
            })?;
        let now = self.clock.now();
        let source_values = self
            .prepare_queue_values(
                &source.workflow_version_id,
                &source.recipe_id,
                &source.values_json,
            )
            .await?;
        let retry = ProductionBatchItem {
            id: ProductionBatchItemId::new(),
            batch_id: batch_id.clone(),
            ordinal,
            workflow_version_id: source.workflow_version_id.clone(),
            recipe_id: source.recipe_id.clone(),
            values_json: generation_values_to_json(&source_values),
            status: ProductionBatchItemStatus::Pending,
            task_id: None,
            retry_of_item_id: Some(source.id.as_str().to_owned()),
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        if !self
            .shot_batch_repository
            .append_requeue_item_with_binding(&retry, source.id.as_str(), now)
            .await?
        {
            self.repository.append_requeue_item(&retry, now).await?;
        }
        self.get(project_id, batch_id.as_str()).await
    }

    pub async fn requeue_item_by_item(
        &self,
        project_id: &str,
        item_id: &str,
    ) -> Result<ProductionBatchDetail, ProductionQueueError> {
        let batches = self.list(project_id).await?;
        for batch in batches {
            let Some(detail) = self.repository.find_detail(project_id, &batch.id).await? else {
                continue;
            };
            if detail.items.iter().any(|item| item.id.as_str() == item_id) {
                return self
                    .requeue_item(project_id, batch.id.as_str(), item_id)
                    .await;
            }
        }
        Err(ProductionQueueError::NotFound(item_id.to_owned()))
    }

    pub async fn recover_and_resume(self: &Arc<Self>) -> Result<(), ProductionQueueError> {
        let _admission = Arc::clone(&self.admission_gate).lock_owned().await;
        let uncertain = self
            .repository
            .recover_uncertain_dispatches(self.clock.now())
            .await?;
        for batch_id in uncertain {
            tracing::warn!(batch_id = %batch_id.as_str(), "production batch paused after uncertain dispatch recovery");
        }
        let mut active = self.repository.list_active_items().await?;
        for record in &active {
            if record.item.status != ProductionBatchItemStatus::Dispatched {
                continue;
            }
            let Some(task_id) = record.item.task_id.as_deref() else {
                continue;
            };
            let task_id = TaskId::parse(task_id.to_owned())
                .map_err(|error| ProductionQueueError::InvalidState(error.to_string()))?;
            let Some(task) = self.task_repository.find_by_id(&task_id).await? else {
                continue;
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
                    .finish_item(&record.item.id, status, code, message, self.clock.now())
                    .await?;
            }
        }
        active = self.repository.list_active_items().await?;
        {
            let mut recovery_tasks = self
                .recovery_tasks
                .lock()
                .expect("production recovery task registry mutex poisoned");
            for record in &active {
                if let Some(task_id) = &record.item.task_id {
                    recovery_tasks.insert(task_id.clone());
                }
            }
        }
        let running = self.repository.list_running().await?;
        let selection = select_recovery(&running, &active);

        for batch in &running {
            if selection.primary_batch_id.as_deref() != Some(batch.id.as_str()) {
                self.repository
                    .set_batch_status(
                        &batch.project_id,
                        &batch.id,
                        ProductionBatchStatus::Paused,
                        self.clock.now(),
                    )
                    .await?;
            }
        }

        if selection.conflict {
            tracing::warn!(
                code = "PRODUCTION_ADMISSION_RECOVERY_CONFLICT",
                active_items = active.len(),
                "multiple active production tasks found; all queue dispatch is paused"
            );
            let mut observed = HashSet::new();
            for record in active {
                if observed.insert(record.batch.id.as_str().to_owned()) {
                    self.spawn_if_needed(record.batch.project_id, record.batch.id);
                }
            }
        } else if let Some(primary_id) = selection.primary_batch_id {
            let primary = active
                .iter()
                .map(|record| &record.batch)
                .chain(running.iter())
                .find(|batch| batch.id.as_str() == primary_id);
            if let Some(primary) = primary {
                self.spawn_if_needed(primary.project_id.clone(), primary.id.clone());
            }
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
                tracing::error!(batch_id = %batch_id.as_str(), error_type = std::any::type_name_of_val(&error), "production queue runner failed");
                let _ = service
                    .repository
                    .set_batch_status(
                        &project_id,
                        &batch_id,
                        ProductionBatchStatus::Paused,
                        service.clock.now(),
                    )
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
                        .set_batch_status(
                            project_id,
                            batch_id,
                            ProductionBatchStatus::Paused,
                            self.clock.now(),
                        )
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
                        .set_batch_status(
                            project_id,
                            batch_id,
                            ProductionBatchStatus::Paused,
                            self.clock.now(),
                        )
                        .await?;
                    return Err(ProductionQueueError::InvalidState(
                        "production item references a missing task".to_owned(),
                    ));
                };
                let terminal = match task.status {
                    TaskStatus::Succeeded => {
                        Some((ProductionBatchItemStatus::Succeeded, None, None))
                    }
                    TaskStatus::Failed => Some((
                        ProductionBatchItemStatus::Failed,
                        task.error.as_ref().map(|error| error.code.as_str()),
                        task.error.as_ref().map(|error| error.message.as_str()),
                    )),
                    TaskStatus::Cancelled => {
                        Some((ProductionBatchItemStatus::Cancelled, None, None))
                    }
                    _ => None,
                };
                if let Some((status, code, message)) = terminal {
                    self.repository
                        .finish_item(&active.id, status, code, message, self.clock.now())
                        .await?;
                    self.recovery_tasks
                        .lock()
                        .expect("production recovery task registry mutex poisoned")
                        .remove(task_id.as_str());
                    if detail.batch.status == ProductionBatchStatus::Paused {
                        return Ok(());
                    }
                    if should_pause_after_terminal(status, code, detail.batch.continue_on_failure) {
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
                let needs_recovery_observation = self
                    .recovery_tasks
                    .lock()
                    .expect("production recovery task registry mutex poisoned")
                    .contains(task_id.as_str());
                if needs_recovery_observation {
                    sleep(Duration::from_secs(2)).await;
                    if let Err(error) = self.task_recovery_service.reconcile_active().await {
                        tracing::warn!(
                            task_id = %task_id.as_str(),
                            error_type = std::any::type_name_of_val(&error),
                            "production restart task observation was deferred"
                        );
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
                    .set_batch_status(
                        project_id,
                        batch_id,
                        ProductionBatchStatus::Paused,
                        self.clock.now(),
                    )
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
                let values = match self
                    .prepare_queue_values(
                        &next.workflow_version_id,
                        &next.recipe_id,
                        &next.values_json,
                    )
                    .await
                {
                    Ok(values) => values,
                    Err(error) => {
                        let message = error.to_string();
                        self.repository
                            .finish_item(
                                &next.id,
                                ProductionBatchItemStatus::Failed,
                                Some("QUEUE_VALUES_INVALID"),
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
                let item_id = next.id.as_str().to_owned();
                let shot_batch_repository = Arc::clone(&self.shot_batch_repository);
                let queue_repository = Arc::clone(&self.repository);
                let clock = Arc::clone(&self.clock);
                let task = match self
                    .generation_service
                    .start_generation_with_task_hook(
                        CreateGenerationRequest {
                            project_id: project_id.to_owned(),
                            workflow_version_id: next.workflow_version_id.clone(),
                            recipe_id: next.recipe_id.clone(),
                            values,
                        },
                        move |task| {
                            let item_id = item_id.clone();
                            let shot_batch_repository = Arc::clone(&shot_batch_repository);
                            let queue_repository = Arc::clone(&queue_repository);
                            let clock = Arc::clone(&clock);
                            let task_id = task.id.as_str().to_owned();
                            async move {
                                if shot_batch_repository
                                    .bind_shot_item_task(&item_id, &task_id, clock.now())
                                    .await?
                                {
                                    return Ok(());
                                }
                                if queue_repository
                                    .link_item_task(
                                        &ProductionBatchItemId::parse(item_id.clone()).map_err(
                                            |error| RepositoryError::integrity(error.to_string()),
                                        )?,
                                        &task_id,
                                        clock.now(),
                                    )
                                    .await?
                                {
                                    Ok(())
                                } else {
                                    Err(RepositoryError::integrity(
                                        "production item task linkage was not persisted",
                                    ))
                                }
                            }
                        },
                    )
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
                debug_assert!(!task.id.as_str().is_empty());
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

fn find_admission_blocker(
    running: &[ProductionBatch],
    active: &[ActiveProductionItem],
    excluded_batch_id: Option<&str>,
) -> Option<ProductionAdmissionView> {
    if let Some(record) = active
        .iter()
        .find(|record| Some(record.batch.id.as_str()) != excluded_batch_id)
    {
        return Some(ProductionAdmissionView {
            busy: true,
            batch_id: Some(record.batch.id.as_str().to_owned()),
            project_id: Some(record.batch.project_id.clone()),
            batch_name: Some(record.batch.name.clone()),
            active_task_id: record.item.task_id.clone(),
        });
    }
    running
        .iter()
        .find(|batch| Some(batch.id.as_str()) != excluded_batch_id)
        .map(|batch| ProductionAdmissionView {
            busy: true,
            batch_id: Some(batch.id.as_str().to_owned()),
            project_id: Some(batch.project_id.clone()),
            batch_name: Some(batch.name.clone()),
            active_task_id: None,
        })
}

#[derive(Debug, Default, PartialEq, Eq)]
struct RecoverySelection {
    primary_batch_id: Option<String>,
    conflict: bool,
}

fn select_recovery(
    running: &[ProductionBatch],
    active: &[ActiveProductionItem],
) -> RecoverySelection {
    if active.len() > 1 {
        return RecoverySelection {
            primary_batch_id: None,
            conflict: true,
        };
    }
    if let Some(record) = active.first() {
        return RecoverySelection {
            primary_batch_id: Some(record.batch.id.as_str().to_owned()),
            conflict: false,
        };
    }
    RecoverySelection {
        primary_batch_id: running.first().map(|batch| batch.id.as_str().to_owned()),
        conflict: false,
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
        GenerationServiceError::TaskCreatedHook { .. } => "TASK_HOOK_ERROR",
        GenerationServiceError::ExecutionFailed { .. } => "EXECUTION_ERROR",
    }
}

pub(crate) fn generation_values_to_json(values: &BTreeMap<String, GenerationInputValue>) -> Value {
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

pub(crate) fn freeze_random_seed_values(
    values: BTreeMap<String, GenerationInputValue>,
    recipe: &Recipe,
) -> Result<BTreeMap<String, GenerationInputValue>, String> {
    let mut resolver = SeedResolver::default();
    values
        .into_iter()
        .map(|(key, value)| {
            let value = match value {
                GenerationInputValue::Seed(seed @ SeedValue::Random) => {
                    let Some(InputDefinition::Seed { min, max, .. }) = recipe.inputs.get(&key)
                    else {
                        return Err(format!(
                            "seed input \"{key}\" is not declared as a Recipe seed input"
                        ));
                    };
                    let resolved = resolver.resolve(&key, &seed, *min, *max);
                    GenerationInputValue::Seed(SeedValue::Fixed(resolved))
                }
                GenerationInputValue::Seed(seed @ SeedValue::Fixed(value)) => {
                    let Some(InputDefinition::Seed { min, max, .. }) = recipe.inputs.get(&key)
                    else {
                        return Err(format!(
                            "seed input \"{key}\" is not declared as a Recipe seed input"
                        ));
                    };
                    if min.is_some_and(|min| value < min) || max.is_some_and(|max| value > max) {
                        return Err(format!(
                            "seed input \"{key}\" value {value} is outside the Recipe range"
                        ));
                    }
                    GenerationInputValue::Seed(seed)
                }
                other => other,
            };
            Ok((key, value))
        })
        .collect()
}

fn generation_values_from_json(
    value: &Value,
) -> Result<BTreeMap<String, GenerationInputValue>, String> {
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
        "string" => Ok(GenerationInputValue::Text(required_string(
            object, "value", key,
        )?)),
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
        "image_asset" => Ok(GenerationInputValue::ImageAsset(parse_asset(
            object, "assetId", key,
        )?)),
        "video_asset" => Ok(GenerationInputValue::VideoAsset(parse_asset(
            object, "assetId", key,
        )?)),
        "audio_asset" => Ok(GenerationInputValue::AudioAsset(parse_asset(
            object, "assetId", key,
        )?)),
        "image_assets" => Ok(GenerationInputValue::ImageAssets(parse_assets(
            object, key,
        )?)),
        "video_assets" => Ok(GenerationInputValue::VideoAssets(parse_assets(
            object, key,
        )?)),
        "audio_assets" => Ok(GenerationInputValue::AudioAssets(parse_assets(
            object, key,
        )?)),
        other => Err(format!(
            "unsupported production queue value type {other} for {key}"
        )),
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
            let value = value.as_str().ok_or_else(|| {
                format!("production queue asset list for {key} contains a non-string id")
            })?;
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
    Busy(ProductionAdmissionView),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for ProductionQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::InvalidState(message) => {
                write!(formatter, "PRODUCTION_QUEUE_INVALID_STATE: {message}")
            }
            Self::Busy(_) => write!(
                formatter,
                "PRODUCTION_QUEUE_BUSY: A production queue is already running. Pause or finish it before starting another queue."
            ),
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
        find_admission_blocker, freeze_random_seed_values, generation_values_from_json,
        generation_values_to_json, is_transient_requeue_error, select_recovery,
        should_pause_after_terminal,
    };
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::application::ports::ActiveProductionItem;
    use crate::domain::{
        AssetId, InputDefinition, ProductionBatch, ProductionBatchId, ProductionBatchItem,
        ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus, Recipe,
        SeedDefault, SeedValue, WorkflowRef,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn queue_values_round_trip_without_losing_seed_or_asset_identity() {
        let mut values = BTreeMap::new();
        values.insert(
            "prompt".to_owned(),
            GenerationInputValue::Text("hello".to_owned()),
        );
        values.insert(
            "seed".to_owned(),
            GenerationInputValue::Seed(SeedValue::Fixed(42)),
        );
        values.insert(
            "image".to_owned(),
            GenerationInputValue::ImageAsset(AssetId::parse("ast_test".to_owned()).unwrap()),
        );
        let json = generation_values_to_json(&values);
        assert_eq!(generation_values_from_json(&json).unwrap(), values);
    }

    #[test]
    fn production_queue_freezes_random_seed_before_persistence() {
        let mut values = BTreeMap::new();
        values.insert(
            "seed_random".to_owned(),
            GenerationInputValue::Seed(SeedValue::Random),
        );
        values.insert(
            "seed_fixed".to_owned(),
            GenerationInputValue::Seed(SeedValue::Fixed(42)),
        );

        let recipe = Recipe {
            schema_version: 1,
            id: "test_recipe".to_owned(),
            name: "Test Recipe".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: [
                (
                    "seed_random".to_owned(),
                    InputDefinition::Seed {
                        label: "Random Seed".to_owned(),
                        default: SeedDefault::Random,
                        min: Some(10),
                        max: Some(20),
                    },
                ),
                (
                    "seed_fixed".to_owned(),
                    InputDefinition::Seed {
                        label: "Fixed Seed".to_owned(),
                        default: SeedDefault::Fixed(42),
                        min: Some(0),
                        max: Some(100),
                    },
                ),
            ]
            .into_iter()
            .collect(),
            bindings: Vec::new(),
            outputs: Vec::new(),
        };

        let frozen = freeze_random_seed_values(values, &recipe).unwrap();
        let random_seed = match frozen.get("seed_random") {
            Some(GenerationInputValue::Seed(SeedValue::Fixed(seed))) => *seed,
            other => panic!("expected a bounded fixed seed, got {other:?}"),
        };
        assert!((10..=20).contains(&random_seed));
        assert!(matches!(
            frozen.get("seed_random"),
            Some(GenerationInputValue::Seed(SeedValue::Fixed(_)))
        ));
        assert_eq!(
            frozen.get("seed_fixed"),
            Some(&GenerationInputValue::Seed(SeedValue::Fixed(42)))
        );
    }

    #[test]
    fn production_queue_rejects_fixed_seed_outside_recipe_range() {
        let recipe = Recipe {
            schema_version: 1,
            id: "test_recipe".to_owned(),
            name: "Test Recipe".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: [(
                "seed".to_owned(),
                InputDefinition::Seed {
                    label: "Seed".to_owned(),
                    default: SeedDefault::Random,
                    min: Some(0),
                    max: Some(100),
                },
            )]
            .into_iter()
            .collect(),
            bindings: Vec::new(),
            outputs: Vec::new(),
        };
        let values = [(
            "seed".to_owned(),
            GenerationInputValue::Seed(SeedValue::Fixed(101)),
        )]
        .into_iter()
        .collect();

        let error = freeze_random_seed_values(values, &recipe).unwrap_err();
        assert!(error.contains("outside the Recipe range"));
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

    #[test]
    fn global_admission_blocks_other_projects_and_allows_the_same_batch() {
        let running = batch("project-a", "Running A", ProductionBatchStatus::Running);
        let paused_active = batch("project-b", "Paused B", ProductionBatchStatus::Paused);
        let active = active_item(paused_active.clone(), Some("tsk_active"));

        let blocker = find_admission_blocker(&[running], &[active.clone()], None).unwrap();
        assert_eq!(blocker.project_id.as_deref(), Some("project-b"));
        assert_eq!(blocker.active_task_id.as_deref(), Some("tsk_active"));

        assert!(find_admission_blocker(&[], &[active], Some(paused_active.id.as_str())).is_none());
    }

    #[test]
    fn terminal_item_release_leaves_admission_available() {
        assert!(find_admission_blocker(&[], &[], None).is_none());
    }

    #[test]
    fn recovery_selects_one_deterministic_running_batch() {
        let first = batch("project-a", "First", ProductionBatchStatus::Running);
        let second = batch("project-b", "Second", ProductionBatchStatus::Running);
        let selection = select_recovery(&[first.clone(), second], &[]);
        assert_eq!(
            selection.primary_batch_id.as_deref(),
            Some(first.id.as_str())
        );
        assert!(!selection.conflict);
    }

    #[test]
    fn recovery_prioritizes_the_batch_with_an_active_task() {
        let first = batch("project-a", "First", ProductionBatchStatus::Running);
        let second = batch("project-b", "Second", ProductionBatchStatus::Running);
        let selection = select_recovery(
            &[first, second.clone()],
            &[active_item(second.clone(), Some("tsk_active"))],
        );
        assert_eq!(
            selection.primary_batch_id.as_deref(),
            Some(second.id.as_str())
        );
        assert!(!selection.conflict);
    }

    #[test]
    fn recovery_conflict_never_selects_a_dispatching_primary() {
        let first = batch("project-a", "First", ProductionBatchStatus::Running);
        let second = batch("project-b", "Second", ProductionBatchStatus::Running);
        let selection = select_recovery(
            &[first.clone(), second.clone()],
            &[
                active_item(first, Some("tsk_first")),
                active_item(second, Some("tsk_second")),
            ],
        );
        assert_eq!(selection.primary_batch_id, None);
        assert!(selection.conflict);
    }

    fn batch(project_id: &str, name: &str, status: ProductionBatchStatus) -> ProductionBatch {
        let now = Utc::now();
        ProductionBatch {
            id: ProductionBatchId::new(),
            project_id: project_id.to_owned(),
            name: name.to_owned(),
            status,
            continue_on_failure: false,
            archived_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn active_item(batch: ProductionBatch, task_id: Option<&str>) -> ActiveProductionItem {
        let now = Utc::now();
        ActiveProductionItem {
            item: ProductionBatchItem {
                id: ProductionBatchItemId::new(),
                batch_id: batch.id.clone(),
                ordinal: 0,
                workflow_version_id: "wfv_test".to_owned(),
                recipe_id: "rcp_test".to_owned(),
                values_json: json!({}),
                status: ProductionBatchItemStatus::Dispatched,
                task_id: task_id.map(ToOwned::to_owned),
                retry_of_item_id: None,
                error_code: None,
                error_message: None,
                created_at: now,
                updated_at: now,
            },
            batch,
        }
    }
}
