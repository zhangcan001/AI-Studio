//! Exercise queue lifecycle through real application services and SQLite;
//! only the external Comfy boundary and scheduling barriers are controlled.
use super::*;
use ai_studio_lib::domain::TaskStatus;

async fn fixture(
    comfy: Arc<ControlledComfy>,
) -> (tempfile::TempDir, SqlitePool, Services, String, String) {
    fixture_with_gate(comfy, None).await
}

async fn fixture_with_gate(
    comfy: Arc<ControlledComfy>,
    gate: Option<Arc<QueueGate>>,
) -> (tempfile::TempDir, SqlitePool, Services, String, String) {
    let directory = tempdir().unwrap();
    let pool = initialize(&directory.path().join("owned.sqlite"))
        .await
        .unwrap();
    let project = directory.path().join("project");
    fs::create_dir_all(&project).unwrap();
    seed_database(&pool, &project).await;
    let package = directory.path().join("package");
    fs::create_dir_all(&package).unwrap();
    write_package(&package, &png_bytes([70, 80, 90, 255])).await;
    let repository = gate.map(|gate| {
        Arc::new(GatedQueueRepository {
            inner: Arc::new(SqliteProductionQueueRepository::new(pool.clone())),
            pool: pool.clone(),
            gate,
        }) as Arc<dyn ProductionQueueRepository>
    });
    let services = build_services_with_repository(&pool, comfy, &package, repository);
    let (batch, item, _) = create_package_batch(&services, &package).await;
    (directory, pool, services, batch, item)
}

#[tokio::test]
async fn manual_recovery_does_not_fail_live_preparing_task() {
    let mut adapter = ControlledComfy::new(ComfyBehavior::Success);
    adapter.hold_upload = true;
    let comfy = Arc::new(adapter);
    let (_directory, pool, services, batch, item) = fixture(comfy.clone()).await;
    services
        .queue
        .start_for_test(PROJECT_ID, &batch)
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        comfy.upload_entered.notified(),
    )
    .await
    .unwrap();
    let tasks = SqliteTaskRepository::new(pool.clone());
    let before = tasks.list_active().await.unwrap().pop().unwrap();
    assert_eq!(before.status, TaskStatus::Preparing);
    assert!(before.prompt_id.is_none());
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
    let report = services.recovery.reconcile_active().await.unwrap();
    let after = tasks.find_by_id(&before.id).await.unwrap().unwrap();
    comfy.upload_release.notify_one();
    assert_eq!(
        after.status,
        TaskStatus::Preparing,
        "manual sync must not mark a live upload as a crashed execution"
    );
    assert!(after.error.is_none());
    assert_eq!(report.deferred, 1);
    wait_for_item_status(&pool, &item, ProductionBatchItemStatus::Succeeded).await;
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stream_disconnect_is_recovered_without_manual_sync_or_second_submit() {
    let comfy = Arc::new(ControlledComfy::new(
        ComfyBehavior::DisconnectThenHistorySuccess,
    ));
    let (_directory, pool, services, batch, item) = fixture(comfy.clone()).await;
    services
        .queue
        .start_for_test(PROJECT_ID, &batch)
        .await
        .unwrap();
    wait_for_event(&pool, "TASK_STREAM_DISCONNECTED").await;
    // Do not invoke recover_and_resume: this is a live, same-process loss of
    // the generation worker, not an application restart or a manual Sync.
    wait_for_item_status(&pool, &item, ProductionBatchItemStatus::Succeeded).await;
    wait_for_batch_status(&pool, &batch, ProductionBatchStatus::Completed).await;
    let detail = services.queue.get(PROJECT_ID, &batch).await.unwrap();
    assert_eq!(detail.batch.status, ProductionBatchStatus::Completed);
    assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Succeeded);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&pool, "tasks").await, 1);
    let task_id =
        ai_studio_lib::domain::TaskId::parse(detail.items[0].task_id.clone().unwrap()).unwrap();
    let mappings = SqliteAssetRepository::new(pool.clone())
        .list_output_mappings(&task_id)
        .await
        .unwrap();
    assert_eq!(mappings.len(), 1);
}

#[tokio::test]
async fn recovery_defers_created_task_while_queue_linkage_hook_is_live() {
    let comfy = Arc::new(ControlledComfy::new(ComfyBehavior::Success));
    let (_directory, pool, services, _batch, _item) = fixture(comfy.clone()).await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let generation = services.generation.clone();
    let hook_entered = entered.clone();
    let hook_release = release.clone();
    let start = tokio::spawn(async move {
        generation.start_generation_with_task_hook(
            ai_studio_lib::application::generation_service::CreateGenerationRequest {
                project_id: PROJECT_ID.to_owned(), workflow_version_id: WORKFLOW_VERSION_ID.to_owned(), recipe_id: RECIPE_ID.to_owned(),
                model_version_id: None, prompt_version_id: None, tool_instance_id: None, tool_version_id: None,
                values: std::collections::BTreeMap::from([("prompt".to_owned(), ai_studio_lib::application::generation_input_preparer::GenerationInputValue::Text("linkage regression".to_owned()))]),
                reference_manifest: None, submission_idempotency_key: None, submission_attempt: None, parent_task_id: None,
            }, move |_| async move {
                hook_entered.notify_one(); hook_release.notified().await;
                // Fail the hook after the ownership check, so this test also
                // verifies cleanup on a pre-execution error (no remote POST).
                Err(RepositoryError::integrity("controlled linkage failure"))
            }
        ).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(20), entered.notified())
        .await
        .unwrap();
    let tasks = SqliteTaskRepository::new(pool);
    let task = tasks.list_active().await.unwrap().pop().unwrap();
    assert_eq!(task.status, TaskStatus::Created);
    let report = services.recovery.reconcile_active().await.unwrap();
    let unchanged = tasks.find_by_id(&task.id).await.unwrap().unwrap();
    release.notify_one();
    assert!(start.await.unwrap().is_err());
    assert_eq!(unchanged.status, TaskStatus::Created);
    assert_eq!(report.deferred, 1);
    assert!(!services.generation.has_live_execution(&task.id));
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
    let failed = tasks.find_by_id(&task.id).await.unwrap().unwrap();
    assert_eq!(failed.status, TaskStatus::Failed);
    assert_eq!(failed.error.unwrap().code, "TASK_HOOK_FAILED");
}

#[tokio::test]
async fn disconnected_prompt_remains_pending_offline_then_recovers_same_identity() {
    let comfy = Arc::new(ControlledComfy::new(
        ComfyBehavior::DisconnectThenHistorySuccess,
    ));
    let (_directory, pool, services, batch, item) = fixture(comfy.clone()).await;
    services
        .queue
        .start_for_test(PROJECT_ID, &batch)
        .await
        .unwrap();
    wait_for_event(&pool, "TASK_STREAM_DISCONNECTED").await;
    comfy.set_behavior(ComfyBehavior::Offline);
    wait_for_event(&pool, "TASK_RECOVERY_DEFERRED").await;
    let before = services.queue.get(PROJECT_ID, &batch).await.unwrap();
    assert_eq!(
        before.items[0].status,
        ProductionBatchItemStatus::Dispatched
    );
    let original = SqliteTaskRepository::new(pool.clone())
        .list_active()
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert!(original.prompt_id.is_some());
    comfy.set_behavior(ComfyBehavior::Success);
    wait_for_item_status(&pool, &item, ProductionBatchItemStatus::Succeeded).await;
    let after = SqliteTaskRepository::new(pool.clone())
        .find_by_id(&original.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, TaskStatus::Succeeded);
    assert_eq!(after.prompt_id, original.prompt_id);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&pool, "tasks").await, 1);
}

use ai_studio_lib::application::ports::{
    ActiveProductionItem, RepositoryError, TerminalItemTransition,
};
use ai_studio_lib::domain::{
    ProductionBatch, ProductionBatchDetail, ProductionBatchId, ProductionBatchItem,
    ProductionBatchItemId, ProductionPackageBatchBinding, ProductionPackageProvenance,
};
use chrono::DateTime;
use std::sync::atomic::AtomicBool;
#[derive(Default)]
struct QueueGate {
    pause_read_armed: AtomicBool,
    pause_read_entered: tokio::sync::Notify,
    pause_read_release: tokio::sync::Notify,
    claim_armed: AtomicBool,
    claim_entered: tokio::sync::Notify,
    claim_release: tokio::sync::Notify,
    claim_finished: tokio::sync::Notify,
    dispatch_armed: AtomicBool,
    dispatch_entered: tokio::sync::Notify,
    dispatch_release: tokio::sync::Notify,
    cancel_read_entered: tokio::sync::Notify,
    finish_armed: AtomicBool,
    finish_entered: tokio::sync::Notify,
    finish_release: tokio::sync::Notify,
}
struct GatedQueueRepository {
    inner: Arc<SqliteProductionQueueRepository>,
    pool: SqlitePool,
    gate: Arc<QueueGate>,
}
// Scheduling barriers wrap the real port implementation: no fake state or
// rewritten production decisions. Every call retains the SQLite transaction.
#[async_trait]
impl ProductionQueueRepository for GatedQueueRepository {
    async fn insert(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
    ) -> Result<(), RepositoryError> {
        self.inner.insert(batch, items).await
    }
    async fn insert_with_provenance(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
        provenance: &ProductionPackageProvenance,
    ) -> Result<(), RepositoryError> {
        self.inner
            .insert_with_provenance(batch, items, provenance)
            .await
    }
    async fn list_package_bindings(
        &self,
        _project_id: &str,
    ) -> Result<Vec<ProductionPackageBatchBinding>, RepositoryError> {
        self.inner.list_package_bindings(_project_id).await
    }
    async fn list(&self, project_id: &str) -> Result<Vec<ProductionBatch>, RepositoryError> {
        self.inner.list(project_id).await
    }
    async fn list_running(&self) -> Result<Vec<ProductionBatch>, RepositoryError> {
        self.inner.list_running().await
    }
    async fn list_active_items(&self) -> Result<Vec<ActiveProductionItem>, RepositoryError> {
        self.inner.list_active_items().await
    }
    async fn list_non_terminal_items(&self) -> Result<Vec<ActiveProductionItem>, RepositoryError> {
        self.inner.list_non_terminal_items().await
    }
    async fn find_detail(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
    ) -> Result<Option<ProductionBatchDetail>, RepositoryError> {
        let result = self.inner.find_detail(project_id, batch_id).await?;
        if result.as_ref().is_some_and(|detail| {
            detail.batch.status == ProductionBatchStatus::Paused
                && detail.items.iter().any(|item| {
                    item.status == ProductionBatchItemStatus::Dispatching && item.task_id.is_none()
                })
        }) {
            self.gate.cancel_read_entered.notify_one();
        }
        if self.gate.pause_read_armed.load(Ordering::SeqCst)
            && result.as_ref().is_some_and(|d| {
                d.batch.status == ProductionBatchStatus::Paused
                    && d.items
                        .iter()
                        .any(|i| i.status == ProductionBatchItemStatus::Dispatched)
            })
        {
            let succeeded = SqliteTaskRepository::new(self.pool.clone())
                .list_recent(PROJECT_ID, 10)
                .await
                .unwrap()
                .iter()
                .any(|t| t.status == TaskStatus::Succeeded);
            if succeeded && self.gate.pause_read_armed.swap(false, Ordering::SeqCst) {
                self.gate.pause_read_entered.notify_one();
                self.gate.pause_read_release.notified().await;
            }
        }
        Ok(result)
    }
    async fn set_batch_status(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        status: ProductionBatchStatus,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        self.inner
            .set_batch_status(project_id, batch_id, status, updated_at)
            .await
    }
    async fn set_item_dispatching(
        &self,
        item_id: &ProductionBatchItemId,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let held = self.gate.claim_armed.swap(false, Ordering::SeqCst);
        if held {
            self.gate.claim_entered.notify_one();
            self.gate.claim_release.notified().await;
        }
        let result = self.inner.set_item_dispatching(item_id, updated_at).await;
        if self.gate.dispatch_armed.swap(false, Ordering::SeqCst) {
            self.gate.dispatch_entered.notify_one();
            self.gate.dispatch_release.notified().await;
        }
        if held {
            self.gate.claim_finished.notify_one();
        }
        result
    }
    async fn cancel_pending_items(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        updated_at: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        self.inner
            .cancel_pending_items(project_id, batch_id, updated_at)
            .await
    }
    async fn cancel_pending_items_and_complete(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        updated_at: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        self.inner
            .cancel_pending_items_and_complete(project_id, batch_id, updated_at)
            .await
    }
    async fn link_item_task(
        &self,
        item_id: &ProductionBatchItemId,
        task_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        self.inner
            .link_item_task(item_id, task_id, updated_at)
            .await
    }
    async fn finish_item(
        &self,
        item_id: &ProductionBatchItemId,
        status: ProductionBatchItemStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        if self.gate.finish_armed.swap(false, Ordering::SeqCst) {
            self.gate.finish_entered.notify_one();
            self.gate.finish_release.notified().await;
        }
        self.inner
            .finish_item(item_id, status, error_code, error_message, updated_at)
            .await
    }
    async fn finish_item_and_settle_batch(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        transition: TerminalItemTransition<'_>,
        pause_batch: bool,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        self.inner
            .finish_item_and_settle_batch(project_id, batch_id, transition, pause_batch, updated_at)
            .await
    }
    async fn set_item_skipped(
        &self,
        item_id: &ProductionBatchItemId,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        self.inner.set_item_skipped(item_id, updated_at).await
    }
    async fn append_requeue_item(
        &self,
        item: &ProductionBatchItem,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        self.inner.append_requeue_item(item, updated_at).await
    }
    async fn set_archived_at(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        archived_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        self.inner
            .set_archived_at(project_id, batch_id, archived_at, updated_at)
            .await
    }
    async fn delete_batch(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
    ) -> Result<bool, RepositoryError> {
        self.inner.delete_batch(project_id, batch_id).await
    }
    async fn recover_uncertain_dispatches(
        &self,
        updated_at: DateTime<Utc>,
    ) -> Result<Vec<ProductionBatchId>, RepositoryError> {
        self.inner.recover_uncertain_dispatches(updated_at).await
    }
}

#[tokio::test]
async fn cancel_batch_waits_for_live_dispatch_then_cancels_the_bound_task() {
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        let mut adapter = ControlledComfy::new(ComfyBehavior::Success);
        adapter.hold_upload = true;
        let comfy = Arc::new(adapter);
        let gate = Arc::new(QueueGate::default());
        let (_directory, pool, services, batch, item) =
            fixture_with_gate(comfy.clone(), Some(gate.clone())).await;
        gate.dispatch_armed.store(true, Ordering::SeqCst);
        services
            .queue
            .start_for_test(PROJECT_ID, &batch)
            .await
            .unwrap();
        gate.dispatch_entered.notified().await;
        let queue = services.queue.clone();
        let cancel_batch = batch.clone();
        let cancel =
            tokio::spawn(async move { queue.cancel_batch(PROJECT_ID, &cancel_batch).await });
        gate.cancel_read_entered.notified().await;
        let dispatching = services.queue.get(PROJECT_ID, &batch).await.unwrap();
        assert_eq!(dispatching.batch.status, ProductionBatchStatus::Paused);
        assert_eq!(
            dispatching.items[0].status,
            ProductionBatchItemStatus::Dispatching
        );
        assert!(dispatching.items[0].task_id.is_none());
        assert!(!cancel.is_finished());

        gate.dispatch_release.notify_one();
        comfy.upload_entered.notified().await;
        let cancelled = cancel.await.unwrap().unwrap();
        assert_eq!(
            cancelled.items[0].status,
            ProductionBatchItemStatus::Dispatched
        );
        let task_id =
            ai_studio_lib::domain::TaskId::parse(cancelled.items[0].task_id.clone().unwrap())
                .unwrap();
        let task = SqliteTaskRepository::new(pool.clone())
            .find_by_id(&task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.status, TaskStatus::CancelRequested);
        comfy.upload_release.notify_one();
        wait_for_item_status(&pool, &item, ProductionBatchItemStatus::Cancelled).await;
        let task = SqliteTaskRepository::new(pool)
            .find_by_id(&task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.status, TaskStatus::Cancelled);
        assert!(!services.generation.has_live_execution(&task_id));
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cancel_batch_rechecks_binding_when_unbound_finish_loses_the_race() {
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        for bind in [false, true] {
            let comfy = Arc::new(ControlledComfy::new(ComfyBehavior::Offline));
            let gate = Arc::new(QueueGate::default());
            let (_directory, pool, services, batch, item) =
                fixture_with_gate(comfy, Some(gate.clone())).await;
            let repository = SqliteProductionQueueRepository::new(pool.clone());
            let batch_id = ProductionBatchId::parse(batch.clone()).unwrap();
            let item_id = ProductionBatchItemId::parse(item).unwrap();
            repository
                .set_batch_status(
                    PROJECT_ID,
                    &batch_id,
                    ProductionBatchStatus::Running,
                    Utc::now(),
                )
                .await
                .unwrap();
            repository
                .set_item_dispatching(&item_id, Utc::now())
                .await
                .unwrap();
            gate.finish_armed.store(true, Ordering::SeqCst);
            let queue = services.queue.clone();
            let cancel = tokio::spawn(async move { queue.cancel_batch(PROJECT_ID, &batch).await });
            gate.finish_entered.notified().await;
            if bind {
                sqlx::query(
                    "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id,
                     status, progress_mode, created_at)
                     VALUES ('tsk_cancel_bind', ?, ?, ?, ?, 'CREATED', 'indeterminate', ?)",
                )
                .bind(PROJECT_ID)
                .bind(WORKFLOW_ID)
                .bind(WORKFLOW_VERSION_ID)
                .bind(RECIPE_ID)
                .bind(Utc::now().to_rfc3339())
                .execute(&pool)
                .await
                .unwrap();
                assert!(repository
                    .link_item_task(&item_id, "tsk_cancel_bind", Utc::now())
                    .await
                    .unwrap());
            }
            gate.finish_release.notify_one();
            let cancelled = cancel.await.unwrap().unwrap();
            assert_eq!(cancelled.batch.status, ProductionBatchStatus::Completed);
            assert!(cancelled
                .items
                .iter()
                .all(|item| item.status == ProductionBatchItemStatus::Cancelled));
            if bind {
                let task = SqliteTaskRepository::new(pool)
                    .find_by_id(
                        &ai_studio_lib::domain::TaskId::parse("tsk_cancel_bind".to_owned())
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(task.status, TaskStatus::Cancelled);
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn pause_committed_before_claim_prevents_new_submission() {
    let comfy = Arc::new(ControlledComfy::new(ComfyBehavior::Success));
    let gate = Arc::new(QueueGate::default());
    let (_directory, _pool, services, batch, _item) =
        fixture_with_gate(comfy.clone(), Some(gate.clone())).await;
    gate.claim_armed.store(true, Ordering::SeqCst);
    services
        .queue
        .start_for_test(PROJECT_ID, &batch)
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        gate.claim_entered.notified(),
    )
    .await
    .unwrap();
    services.queue.pause(PROJECT_ID, &batch).await.unwrap();
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
    gate.claim_release.notify_one();
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        gate.claim_finished.notified(),
    )
    .await
    .unwrap();
    let detail = services.queue.get(PROJECT_ID, &batch).await.unwrap();
    assert_eq!(detail.batch.status, ProductionBatchStatus::Paused);
    assert_eq!(
        detail.items[0].status,
        ProductionBatchItemStatus::Pending,
        "a successful pause must fence an item not yet claimed"
    );
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn resume_during_old_worker_exit_keeps_pending_items_executing() {
    let mut adapter = ControlledComfy::new(ComfyBehavior::Success);
    adapter.hold_upload = true;
    let comfy = Arc::new(adapter);
    let gate = Arc::new(QueueGate::default());
    let (_directory, pool, services, batch, first) =
        fixture_with_gate(comfy.clone(), Some(gate.clone())).await;
    let detail = services.queue.get(PROJECT_ID, &batch).await.unwrap();
    let mut second = detail.items[0].clone();
    second.id = ProductionBatchItemId::new();
    second.ordinal = 1;
    SqliteProductionQueueRepository::new(pool.clone())
        .append_requeue_item(&second, Utc::now())
        .await
        .unwrap();
    services
        .queue
        .start_for_test(PROJECT_ID, &batch)
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        comfy.upload_entered.notified(),
    )
    .await
    .unwrap();
    services.queue.pause(PROJECT_ID, &batch).await.unwrap();
    gate.pause_read_armed.store(true, Ordering::SeqCst);
    // The second input upload should not be held after resuming.
    comfy.upload_release.notify_one();
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        gate.pause_read_entered.notified(),
    )
    .await
    .unwrap();
    services
        .queue
        .start_for_test(PROJECT_ID, &batch)
        .await
        .unwrap();
    gate.pause_read_release.notify_one();
    comfy.upload_release.notify_one();
    wait_for_item_status(&pool, &first, ProductionBatchItemStatus::Succeeded).await;
    wait_for_item_status(
        &pool,
        second.id.as_str(),
        ProductionBatchItemStatus::Succeeded,
    )
    .await;
    wait_for_batch_status(&pool, &batch, ProductionBatchStatus::Completed).await;
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 2);
    assert_eq!(count(&pool, "tasks").await, 2);
}
