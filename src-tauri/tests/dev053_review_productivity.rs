//! DEV-053 review productivity contract checks.
//!
//! The productivity read is intentionally owned by the application service;
//! this file checks the command boundary and the bulk seams it consumes
//! without constructing a second queue or a real ComfyUI runtime.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use ai_studio_lib::{
    application::{
        generation_service::GenerationService,
        ports::{
            AssetRepository, AssetStore, Clock, ComfyAdapter, ComfyAdapterError,
            ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth, ComfyHistory,
            ComfyOutputData, ComfyOutputFile, GenerationDefinition, GenerationDefinitionRepository,
            GenerationSnapshotRepository, NoopTaskUpdateSink, ProductionItemReviewRepository,
            ProductionQueueRepository, ProjectRecord, ProjectRepository, RepositoryError,
            ShotBatchRepository, TaskRepository,
        },
        production_item_review_service::ProductionItemReviewService,
        production_queue_service::ProductionQueueService,
        task_recovery_service::TaskRecoveryService,
    },
    domain::{
        Asset, AssetId, AssetType, GenerationSnapshot, NewTaskEvent, PreparationSnapshotRecord,
        ProductionBatch, ProductionBatchDetail, ProductionBatchId, ProductionBatchItem,
        ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus,
        ProductionReviewStatus, ShotStage, StoredTaskEvent, Task, TaskId, TaskStatus,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn repo_root() -> PathBuf {
    Path::new(ROOT)
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf()
}

fn read_repo(path: &str) -> String {
    fs::read_to_string(repo_root().join(path)).expect("DEV-053 source should be readable")
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing section start: {start}"));
    let rest = &source[start_index..];
    let end_index = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing section end: {end}"));
    &rest[..end_index]
}

fn assert_contains_all(source: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            source.contains(needle),
            "missing contract fragment: {needle}"
        );
    }
}

#[test]
fn review_get_uses_the_bulk_productivity_facade() {
    let command = read_repo("src-tauri/src/commands/production_item_review.rs");
    let get = section(
        &command,
        "pub async fn production_item_review_get",
        "#[tauri::command",
    );
    assert_contains_all(
        get,
        &[
            "get_productivity_view",
            "ProductionBatchReviewView",
            ".map(Into::into)",
        ],
    );
    assert!(!get.contains(".get(&project_id, &batch_id)"));
    assert!(!command.contains("hydrate_productivity_view"));
    assert!(!command.contains("preparation_snapshot(project_id"));
    assert!(!command.contains("shot_service.list"));
}

#[test]
fn productivity_dto_contains_the_review_board_contract() {
    let command = read_repo("src-tauri/src/commands/production_item_review.rs");
    assert_contains_all(
        &command,
        &[
            "ProductionReviewCandidateAssetView",
            "ProductionReviewContextView",
            "candidate_assets",
            "shot_id",
            "stage",
            "selected_asset_id",
            "reviewable",
            "snapshot_available",
            "context_hash",
            "prompt_text",
            "negative_prompt",
            "workflow_version_id",
            "recipe_id",
            "reference_sets",
            "reference_assets",
            "sha256",
            "output_spec",
            "stage_input",
            "readiness_status",
            "asset_id",
            "asset_type",
            "mime_type",
            "thumbnail_available",
            "task_id",
            "review_result",
            "rename_all = \"camelCase\"",
        ],
    );
    assert!(!command.contains("Vec<u8>"));
    assert!(!command.contains("scalar_values"));
}

#[test]
fn facade_and_repositories_are_project_and_batch_scoped() {
    let service = read_repo("src-tauri/src/application/production_item_review_service.rs");
    let reviews = read_repo("src-tauri/src/application/ports/production_item_review_repository.rs");
    let shots = read_repo("src-tauri/src/application/ports/shot_batch_repository.rs");
    let assets = read_repo("src-tauri/src/application/ports/asset_repository.rs");
    assert_contains_all(
        &service,
        &[
            "get_productivity_view",
            "ProductionReviewProductivityView",
            "list_for_lineages",
            "ensure_for_items",
            "list_preparation_snapshots_for_batch",
            "list_shot_links_for_batch",
            "list_by_source_tasks",
        ],
    );
    assert_contains_all(
        &reviews,
        &["async fn list_for_lineages", "async fn ensure_for_items"],
    );
    assert_contains_all(
        &shots,
        &[
            "async fn list_preparation_snapshots_for_batch",
            "async fn list_shot_links_for_batch",
            "project_id",
            "production_batch_id",
        ],
    );
    assert_contains_all(&assets, &["async fn list_by_source_tasks", "task_ids"]);
}

#[test]
fn review_status_note_and_regeneration_boundaries_are_preserved() {
    let command = read_repo("src-tauri/src/commands/production_item_review.rs");
    let service = read_repo("src-tauri/src/application/production_item_review_service.rs");
    assert_contains_all(
        &command,
        &[
            "Unreviewed",
            "Approved",
            "Starred",
            "Regenerate",
            "Rejected",
            "auto_start: false",
            "regenerate_marked(&request.project_id, &request.batch_id, false)",
        ],
    );
    assert_contains_all(
        &service,
        &[
            "MAX_REVIEW_NOTE_BYTES: usize = 4 * 1024",
            "note.as_bytes().len() > MAX_REVIEW_NOTE_BYTES",
            "ensure_reviewable_item",
            "失败或未完成的 Task 不能设置审片状态或备注",
        ],
    );
    let regenerate = section(
        &command,
        "pub async fn production_item_review_regenerate(",
        "#[tauri::command",
    );
    assert!(!regenerate.contains("request.auto_start"));
}

#[test]
fn review_read_path_does_not_admit_or_start_generation() {
    let command = read_repo("src-tauri/src/commands/production_item_review.rs");
    let get = section(
        &command,
        "pub async fn production_item_review_get",
        "#[tauri::command",
    );
    for forbidden in [
        "production_queue_service.start",
        "generation_service",
        "submit_prompt",
        "preflight",
        "current_stage",
        "start_generation",
    ] {
        assert!(!get.contains(forbidden), "read path contains {forbidden}");
    }
}

#[derive(Default)]
struct ReviewCounters {
    task_single: AtomicUsize,
    task_bulk: AtomicUsize,
    asset_single: AtomicUsize,
    asset_bulk: AtomicUsize,
    review_find: AtomicUsize,
    review_list_batch: AtomicUsize,
    review_ensure_many: AtomicUsize,
    lineage_single: AtomicUsize,
    lineage_bulk: AtomicUsize,
    snapshot_single: AtomicUsize,
    snapshot_batch: AtomicUsize,
}

impl ReviewCounters {
    fn print(&self) {
        println!(
            "DEV-053 counts task(single={}, bulk={}), asset(source_single={}, bulk={}), review(find={}, list_batch={}, ensure_many={}), lineage(single={}, bulk={}), snapshot(single={}, batch={})",
            self.task_single.load(Ordering::SeqCst),
            self.task_bulk.load(Ordering::SeqCst),
            self.asset_single.load(Ordering::SeqCst),
            self.asset_bulk.load(Ordering::SeqCst),
            self.review_find.load(Ordering::SeqCst),
            self.review_list_batch.load(Ordering::SeqCst),
            self.review_ensure_many.load(Ordering::SeqCst),
            self.lineage_single.load(Ordering::SeqCst),
            self.lineage_bulk.load(Ordering::SeqCst),
            self.snapshot_single.load(Ordering::SeqCst),
            self.snapshot_batch.load(Ordering::SeqCst),
        );
    }
}

fn unsupported() -> RepositoryError {
    RepositoryError::database("DEV-053 counting fake method is not used")
}

struct CountingTasks {
    counters: Arc<ReviewCounters>,
    tasks: Vec<Task>,
}

#[async_trait]
impl TaskRepository for CountingTasks {
    async fn create(&self, _: &Task, _: &NewTaskEvent) -> Result<StoredTaskEvent, RepositoryError> {
        Err(unsupported())
    }
    async fn persist_transition(
        &self,
        _: &Task,
        _: &NewTaskEvent,
        _: TaskStatus,
    ) -> Result<StoredTaskEvent, RepositoryError> {
        Err(unsupported())
    }
    async fn persist_runtime_update(
        &self,
        _: &Task,
        _: &NewTaskEvent,
    ) -> Result<StoredTaskEvent, RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_id(&self, _: &TaskId) -> Result<Option<Task>, RepositoryError> {
        self.counters.task_single.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn find_many_by_ids(&self, _: &[TaskId]) -> Result<Vec<Task>, RepositoryError> {
        self.counters.task_bulk.fetch_add(1, Ordering::SeqCst);
        Ok(self.tasks.clone())
    }
    async fn find_by_submission_idempotency_key(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<Task>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_recent(&self, _: &str, _: u32) -> Result<Vec<Task>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_active(&self) -> Result<Vec<Task>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_events(&self, _: &TaskId) -> Result<Vec<StoredTaskEvent>, RepositoryError> {
        Err(unsupported())
    }
}

struct CountingAssets {
    counters: Arc<ReviewCounters>,
    assets: Vec<Asset>,
}

#[async_trait]
impl AssetRepository for CountingAssets {
    async fn insert_many(&self, _: &[Asset]) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_id(&self, _: &AssetId) -> Result<Option<Asset>, RepositoryError> {
        self.counters.asset_single.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn list_by_source_task(&self, _: &TaskId) -> Result<Vec<Asset>, RepositoryError> {
        self.counters.asset_single.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
    async fn list_by_source_tasks(&self, _: &[TaskId]) -> Result<Vec<Asset>, RepositoryError> {
        self.counters.asset_bulk.fetch_add(1, Ordering::SeqCst);
        Ok(self.assets.clone())
    }
    async fn list_recent(&self, _: &str, _: u32) -> Result<Vec<Asset>, RepositoryError> {
        Err(unsupported())
    }
}

struct CountingReviews {
    counters: Arc<ReviewCounters>,
    reviews: Vec<ai_studio_lib::application::ports::ProductionItemReviewRecord>,
}

#[async_trait]
impl ProductionItemReviewRepository for CountingReviews {
    async fn list_for_batch(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<ai_studio_lib::application::ports::ProductionItemReviewRecord>, RepositoryError>
    {
        self.counters
            .review_list_batch
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.reviews.clone())
    }
    async fn list_for_lineage(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<ai_studio_lib::application::ports::ProductionItemReviewRecord>, RepositoryError>
    {
        self.counters.lineage_single.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
    async fn list_for_lineages(
        &self,
        _: &str,
        _: &[String],
    ) -> Result<Vec<ai_studio_lib::application::ports::ProductionItemReviewRecord>, RepositoryError>
    {
        self.counters.lineage_bulk.fetch_add(1, Ordering::SeqCst);
        Ok(self.reviews.clone())
    }
    async fn find_for_item(
        &self,
        _: &str,
        _: &str,
    ) -> Result<
        Option<ai_studio_lib::application::ports::ProductionItemReviewRecord>,
        RepositoryError,
    > {
        self.counters.review_find.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn ensure_for_item(
        &self,
        record: &ai_studio_lib::application::ports::ProductionItemReviewRecord,
    ) -> Result<ai_studio_lib::application::ports::ProductionItemReviewRecord, RepositoryError>
    {
        Ok(record.clone())
    }
    async fn ensure_for_items(
        &self,
        records: &[ai_studio_lib::application::ports::ProductionItemReviewRecord],
    ) -> Result<Vec<ai_studio_lib::application::ports::ProductionItemReviewRecord>, RepositoryError>
    {
        self.counters
            .review_ensure_many
            .fetch_add(1, Ordering::SeqCst);
        Ok(records.to_vec())
    }
    async fn insert(
        &self,
        _: &ai_studio_lib::application::ports::ProductionItemReviewRecord,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn set_status(
        &self,
        _: &str,
        _: &str,
        _: ProductionReviewStatus,
        _: DateTime<Utc>,
    ) -> Result<ai_studio_lib::application::ports::ProductionItemReviewRecord, RepositoryError>
    {
        Err(unsupported())
    }
    async fn set_note(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<ai_studio_lib::application::ports::ProductionItemReviewRecord, RepositoryError>
    {
        Err(unsupported())
    }
}

struct CountingShots {
    counters: Arc<ReviewCounters>,
    snapshot: PreparationSnapshotRecord,
    link: ai_studio_lib::application::ports::ProductionBatchShotLink,
}

#[async_trait]
impl ShotBatchRepository for CountingShots {
    async fn insert_batch_with_bindings(
        &self,
        _: &ProductionBatch,
        _: &[ProductionBatchItem],
        _: &[ai_studio_lib::application::ports::ShotBatchBinding],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn find_preparation_snapshot(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<PreparationSnapshotRecord>, RepositoryError> {
        self.counters.snapshot_single.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn list_preparation_snapshots_for_batch(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<PreparationSnapshotRecord>, RepositoryError> {
        self.counters.snapshot_batch.fetch_add(1, Ordering::SeqCst);
        Ok(vec![self.snapshot.clone()])
    }
    async fn list_shot_links_for_batch(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<ai_studio_lib::application::ports::ProductionBatchShotLink>, RepositoryError>
    {
        Ok(vec![self.link.clone()])
    }
    async fn bind_shot_item_task(
        &self,
        _: &str,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn append_requeue_item_with_binding(
        &self,
        _: &ProductionBatchItem,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn append_requeue_items_with_bindings(
        &self,
        _: &[ProductionBatchItem],
        _: DateTime<Utc>,
    ) -> Result<(Vec<String>, Vec<String>), RepositoryError> {
        Err(unsupported())
    }
    async fn has_active_shot_binding(&self, _: &str, _: &str) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn list_active_shot_bindings(
        &self,
        _: &str,
        _: ShotStage,
        _: &[String],
    ) -> Result<Vec<ai_studio_lib::application::ports::ActiveShotBatchBinding>, RepositoryError>
    {
        Err(unsupported())
    }
}

struct FakeQueue {
    detail: ProductionBatchDetail,
}

#[async_trait]
impl ProductionQueueRepository for FakeQueue {
    async fn insert(
        &self,
        _: &ProductionBatch,
        _: &[ProductionBatchItem],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn list(&self, _: &str) -> Result<Vec<ProductionBatch>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_running(&self) -> Result<Vec<ProductionBatch>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_active_items(
        &self,
    ) -> Result<Vec<ai_studio_lib::application::ports::ActiveProductionItem>, RepositoryError> {
        Err(unsupported())
    }
    async fn find_detail(
        &self,
        _: &str,
        _: &ProductionBatchId,
    ) -> Result<Option<ProductionBatchDetail>, RepositoryError> {
        Ok(Some(self.detail.clone()))
    }
    async fn set_batch_status(
        &self,
        _: &str,
        _: &ProductionBatchId,
        _: ProductionBatchStatus,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn set_item_dispatching(
        &self,
        _: &ProductionBatchItemId,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn cancel_pending_items(
        &self,
        _: &str,
        _: &ProductionBatchId,
        _: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        Err(unsupported())
    }
    async fn link_item_task(
        &self,
        _: &ProductionBatchItemId,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn finish_item(
        &self,
        _: &ProductionBatchItemId,
        _: ProductionBatchItemStatus,
        _: Option<&str>,
        _: Option<&str>,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn set_item_skipped(
        &self,
        _: &ProductionBatchItemId,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn append_requeue_item(
        &self,
        _: &ProductionBatchItem,
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn set_archived_at(
        &self,
        _: &str,
        _: &ProductionBatchId,
        _: Option<DateTime<Utc>>,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn delete_batch(&self, _: &str, _: &ProductionBatchId) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn recover_uncertain_dispatches(
        &self,
        _: DateTime<Utc>,
    ) -> Result<Vec<ProductionBatchId>, RepositoryError> {
        Err(unsupported())
    }
}

struct FakeDefinitions;

#[async_trait]
impl GenerationDefinitionRepository for FakeDefinitions {
    async fn find(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<GenerationDefinition>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_available(
        &self,
    ) -> Result<
        Vec<ai_studio_lib::application::ports::AvailableGenerationDefinition>,
        RepositoryError,
    > {
        Err(unsupported())
    }
}

struct FakeSnapshots;

#[async_trait]
impl GenerationSnapshotRepository for FakeSnapshots {
    async fn insert(&self, _: &GenerationSnapshot) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_task_id(
        &self,
        _: &TaskId,
    ) -> Result<Option<GenerationSnapshot>, RepositoryError> {
        Err(unsupported())
    }
}

struct FakeProjects;

#[async_trait]
impl ProjectRepository for FakeProjects {
    async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_id(&self, _: &str) -> Result<Option<ProjectRecord>, RepositoryError> {
        Err(unsupported())
    }
    async fn insert(&self, _: &ProjectRecord) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn update_metadata(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: DateTime<Utc>,
    ) -> Result<Option<ProjectRecord>, RepositoryError> {
        Err(unsupported())
    }
    async fn get_storage_root(&self, _: &str) -> Result<Option<PathBuf>, RepositoryError> {
        Err(unsupported())
    }
    async fn ensure_default_project(
        &self,
        _: &str,
        _: &str,
        _: &PathBuf,
        _: DateTime<Utc>,
    ) -> Result<ProjectRecord, RepositoryError> {
        Err(unsupported())
    }
}

struct NoopComfy;
struct EmptyComfyEvents;

#[async_trait]
impl ComfyEventSubscription for EmptyComfyEvents {
    async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
        Ok(None)
    }
}

#[async_trait]
impl ComfyAdapter for NoopComfy {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-053 read test must not call Comfy".to_owned(),
        ))
    }
    async fn get_system_stats(
        &self,
    ) -> Result<ai_studio_lib::application::ports::SystemStats, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-053 read test must not call Comfy".to_owned(),
        ))
    }
    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-053 read test must not call Comfy".to_owned(),
        ))
    }
    async fn get_history(&self, _: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-053 read test must not call Comfy".to_owned(),
        ))
    }
    async fn download_output(
        &self,
        _: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-053 read test must not call Comfy".to_owned(),
        ))
    }
    async fn submit_workflow(
        &self,
        _: &str,
        _: &str,
        _: Value,
    ) -> Result<ai_studio_lib::application::ports::PromptSubmission, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-053 read test must not submit".to_owned(),
        ))
    }
    async fn subscribe_events(
        &self,
        _: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Ok(Box::new(EmptyComfyEvents))
    }
}

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-27T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }
}

fn review_snapshot(item_id: &str, batch_id: &str) -> PreparationSnapshotRecord {
    let snapshot = serde_json::from_value(json!({
        "schemaVersion": 1,
        "projectId": "prj_dev053",
        "shotId": "shot_dev053",
        "stage": "image",
        "contextHash": "ctx_dev053",
        "resolvedAt": "2026-08-27T00:00:00Z",
        "preparedAt": "2026-08-27T00:00:00Z",
        "structure": {"series": null, "episode": null, "scene": null, "shot": {"id": "shot_dev053", "ordinal": 1, "name": "Shot"}},
        "profiles": {"characters": [], "scene": null, "props": [], "style": null},
        "referenceSets": [],
        "referenceAssets": [],
        "prompt": {"renderedText": "snapshot prompt", "negativePrompt": "snapshot negative", "orderedSegments": []},
        "workflow": {"workflowVersionId": "wf_dev053", "recipeId": "recipe_dev053", "scalarValues": {}},
        "outputSpec": {"width": 1024, "height": 576, "count": 1, "durationSeconds": null},
        "stageInput": {"selectedImageAssetId": null, "selectedImageSha256": null},
        "frozenGenerationValues": {},
        "readiness": {"status": "READY", "score": 100, "gates": [], "evaluatedAt": "2026-08-27T00:00:00Z"},
        "comfyCapabilityEvidence": {"workflowReady": true, "workflowTotal": 1, "runtimeBusy": false, "activeTaskCount": 0, "productionBusy": false, "issueCodes": []}
    })).expect("snapshot fixture should deserialize");
    PreparationSnapshotRecord {
        id: "snp_dev053".to_owned(),
        project_id: "prj_dev053".to_owned(),
        shot_id: "shot_dev053".to_owned(),
        stage: ShotStage::Image,
        context_hash: "ctx_dev053".to_owned(),
        production_batch_id: batch_id.to_owned(),
        production_batch_item_id: item_id.to_owned(),
        snapshot,
        created_at: FixedClock.now(),
    }
}

fn productivity_fixture() -> (
    ProductionItemReviewService,
    Arc<ReviewCounters>,
    String,
    String,
) {
    let counters = Arc::new(ReviewCounters::default());
    let project_id = "prj_dev053".to_owned();
    let batch_id = "pbt_dev053_100".to_owned();
    let now = FixedClock.now();
    let batch = ProductionBatch {
        id: ProductionBatchId::parse(batch_id.clone()).unwrap(),
        project_id: project_id.clone(),
        name: "DEV-053 100 item batch".to_owned(),
        status: ProductionBatchStatus::Completed,
        continue_on_failure: true,
        archived_at: None,
        created_at: now,
        updated_at: now,
    };
    let mut items = Vec::with_capacity(100);
    let mut tasks = Vec::with_capacity(100);
    let mut assets = Vec::with_capacity(100);
    let mut reviews = Vec::with_capacity(100);
    for ordinal in 0..100u32 {
        let item_id = format!("pbi_dev053_{ordinal:03}");
        let task_id = format!("tsk_dev053_{ordinal:03}");
        let asset_id = format!("ast_dev053_{ordinal:03}");
        let item_id_domain = ProductionBatchItemId::parse(item_id.clone()).unwrap();
        let task_id_domain = TaskId::parse(task_id.clone()).unwrap();
        items.push(ProductionBatchItem {
            id: item_id_domain.clone(),
            batch_id: batch.id.clone(),
            ordinal,
            workflow_version_id: "wf_dev053".to_owned(),
            recipe_id: "recipe_dev053".to_owned(),
            values_json: json!({"prompt": "legacy fallback prompt"}),
            status: ProductionBatchItemStatus::Succeeded,
            task_id: Some(task_id.clone()),
            retry_of_item_id: None,
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        });
        let mut task = Task::new(
            &project_id,
            "workflow_dev053",
            "wf_dev053",
            "recipe_dev053",
            now,
        );
        task.id = task_id_domain.clone();
        task.status = TaskStatus::Succeeded;
        tasks.push(task);
        assets.push(Asset {
            id: AssetId::parse(asset_id.clone()).unwrap(),
            project_id: project_id.clone(),
            asset_type: AssetType::Image,
            category: "generated_image".to_owned(),
            name: format!("candidate-{ordinal}"),
            original_name: format!("candidate-{ordinal}"),
            storage_path: format!("generated/{asset_id}.png"),
            thumbnail_path: Some(format!("thumbs/{asset_id}.jpg")),
            sha256: format!("sha-{ordinal}"),
            mime_type: "image/png".to_owned(),
            width: 1024,
            height: 576,
            duration_ms: None,
            file_size: 1,
            source_task_id: Some(task_id_domain),
            metadata_json: json!({}),
            created_at: now,
            updated_at: now,
        });
        reviews.push(
            ai_studio_lib::application::ports::ProductionItemReviewRecord {
                id: format!("pri_dev053_{ordinal:03}"),
                project_id: project_id.clone(),
                production_batch_id: batch_id.clone(),
                production_batch_item_id: item_id,
                task_id: Some(task_id),
                result_asset_id: Some(asset_id),
                review_status: if ordinal == 0 {
                    ProductionReviewStatus::Starred
                } else {
                    ProductionReviewStatus::Unreviewed
                },
                review_note: String::new(),
                version: 1,
                lineage_key: format!("lineage_dev053_{ordinal:03}"),
                parent_batch_id: None,
                parent_item_id: None,
                created_at: now,
                updated_at: now,
            },
        );
    }
    let first_item = items[0].id.as_str().to_owned();
    let first_asset = assets[0].id.as_str().to_owned();
    let snapshot = review_snapshot(&first_item, &batch_id);
    let link = ai_studio_lib::application::ports::ProductionBatchShotLink {
        production_batch_item_id: first_item.clone(),
        shot_id: "shot_dev053".to_owned(),
        stage: ShotStage::Image,
        selected_image_asset_id: Some(first_asset),
        selected_video_asset_id: None,
    };
    let queue_repository: Arc<dyn ProductionQueueRepository> = Arc::new(FakeQueue {
        detail: ProductionBatchDetail { batch, items },
    });
    let task_repository: Arc<dyn TaskRepository> = Arc::new(CountingTasks {
        counters: counters.clone(),
        tasks,
    });
    let asset_repository: Arc<dyn AssetRepository> = Arc::new(CountingAssets {
        counters: counters.clone(),
        assets,
    });
    let review_repository: Arc<dyn ProductionItemReviewRepository> = Arc::new(CountingReviews {
        counters: counters.clone(),
        reviews,
    });
    let shot_repository: Arc<dyn ShotBatchRepository> = Arc::new(CountingShots {
        counters: counters.clone(),
        snapshot,
        link,
    });
    let definition_repository: Arc<dyn GenerationDefinitionRepository> = Arc::new(FakeDefinitions);
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> = Arc::new(FakeSnapshots);
    let project_repository: Arc<dyn ProjectRepository> = Arc::new(FakeProjects);
    let comfy: Arc<dyn ComfyAdapter> = Arc::new(NoopComfy);
    let asset_store: Arc<dyn AssetStore> =
        Arc::new(ai_studio_lib::infrastructure::filesystem::FileSystemAssetStore::new());
    let clock: Arc<dyn Clock> = Arc::new(FixedClock);
    let generation_service = Arc::new(GenerationService::new(
        task_repository.clone(),
        snapshot_repository.clone(),
        definition_repository.clone(),
        comfy.clone(),
        project_repository.clone(),
        asset_store.clone(),
        asset_repository.clone(),
        clock.clone(),
    ));
    let recovery_service = Arc::new(TaskRecoveryService::new(
        task_repository.clone(),
        snapshot_repository,
        asset_repository.clone(),
        comfy,
        project_repository,
        asset_store,
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));
    let queue_service = Arc::new(ProductionQueueService::new(
        queue_repository.clone(),
        task_repository.clone(),
        definition_repository,
        generation_service,
        shot_repository.clone(),
        recovery_service,
        clock.clone(),
    ));
    let service = ProductionItemReviewService::new_with_shot_batch_repository(
        review_repository,
        queue_repository,
        queue_service,
        task_repository,
        asset_repository,
        shot_repository,
        clock,
    );
    (service, counters, project_id, batch_id)
}

#[tokio::test]
async fn productivity_facade_uses_bounded_bulk_reads_for_100_items() {
    let (service, counters, project_id, batch_id) = productivity_fixture();
    let view = service
        .get_productivity_view(&project_id, &batch_id)
        .await
        .expect("productivity facade should load");
    assert_eq!(view.total, 100);
    assert_eq!(view.items.len(), 100);
    assert_eq!(view.items[0].frozen_context.snapshot_available, true);
    assert_eq!(
        view.items[0].frozen_context.prompt_text.as_deref(),
        Some("snapshot prompt")
    );
    assert_eq!(view.items[0].shot_id.as_deref(), Some("shot_dev053"));
    assert_eq!(view.items[0].stage.as_deref(), Some("image"));
    assert!(view.items[0].candidate_assets[0].selected);
    assert_eq!(view.items[0].review_status.as_deref(), Some("STARRED"));
    assert_eq!(counters.task_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.task_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(counters.asset_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.asset_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(counters.review_find.load(Ordering::SeqCst), 0);
    assert_eq!(counters.review_list_batch.load(Ordering::SeqCst), 1);
    assert_eq!(counters.review_ensure_many.load(Ordering::SeqCst), 0);
    assert_eq!(counters.lineage_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.lineage_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(counters.snapshot_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.snapshot_batch.load(Ordering::SeqCst), 1);
    counters.print();
}
