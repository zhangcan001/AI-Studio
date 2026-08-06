use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::generation_service::{CreateGenerationRequest, GenerationService};
use crate::application::ports::{
    AssetRepository, AssetStore, CancelPromptResult, Clock, ComfyAdapter, ComfyAdapterError,
    ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyHistoryStatus,
    ComfyImageUpload, ComfyNodeOutput, ComfyOutputData, ComfyOutputFile, ComfyQueueState,
    ComfyUploadedImage, PromptSubmission, SystemStats, TaskRepository, TaskUpdateSink,
};
use crate::application::task_cancellation_service::TaskCancellationService;
use crate::application::task_execution_registry::TaskExecutionRegistry;
use crate::domain::{Asset, AssetId, SeedValue, Task, TaskId, TaskStatus};
use crate::infrastructure::database::{
    initialize,
    repositories::{
        test_support, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
        SqliteGenerationSnapshotRepository, SqliteTaskRepository,
    },
    SqliteProjectRepository,
};
use crate::infrastructure::filesystem::FileSystemAssetStore;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use tempfile::{tempdir, TempDir};
use tokio::sync::watch;

const T2I_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/simple_t2i/recipe.yaml"
));
const T2I_WORKFLOW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/simple_t2i/workflow_api.json"
));
const I2I_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/simple_i2i/recipe.yaml"
));
const I2I_WORKFLOW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/simple_i2i/workflow_api.json"
));

#[derive(Clone)]
struct Gate {
    reached: watch::Sender<bool>,
    release: watch::Sender<bool>,
}

impl Gate {
    fn new() -> Self {
        let (reached, _) = watch::channel(false);
        let (release, _) = watch::channel(false);
        Self { reached, release }
    }

    fn mark_reached(&self) {
        self.reached.send_replace(true);
    }

    fn release(&self) {
        self.release.send_replace(true);
    }

    async fn wait_until_reached(&self) {
        let mut receiver = self.reached.subscribe();
        if !*receiver.borrow() {
            receiver
                .changed()
                .await
                .expect("gate should remain observable");
        }
    }

    async fn wait_for_release(&self) {
        let mut receiver = self.release.subscribe();
        if !*receiver.borrow() {
            receiver
                .changed()
                .await
                .expect("gate should remain observable");
        }
    }
}

#[derive(Clone)]
struct AdapterControl {
    upload: Gate,
    subscribe: Gate,
    first_event: Gate,
    terminal_event: Gate,
}

impl Default for AdapterControl {
    fn default() -> Self {
        Self {
            upload: Gate::new(),
            subscribe: Gate::new(),
            first_event: Gate::new(),
            terminal_event: Gate::new(),
        }
    }
}

#[derive(Clone)]
struct AdapterBehavior {
    events: VecDeque<Result<Option<ComfyExecutionEvent>, ComfyAdapterError>>,
    history: Option<ComfyHistory>,
    queue: ComfyQueueState,
    hold_upload: bool,
    hold_subscribe: bool,
    hold_first_event: bool,
    hold_terminal_event: bool,
}

#[derive(Clone)]
struct ControlledAdapter {
    behavior: Arc<Mutex<AdapterBehavior>>,
    prompt_id: Arc<Mutex<Option<String>>>,
    actions: Arc<Mutex<Vec<String>>>,
    control: AdapterControl,
}

struct ControlledSubscription {
    events: VecDeque<Result<Option<ComfyExecutionEvent>, ComfyAdapterError>>,
    prompt_id: Arc<Mutex<Option<String>>>,
    control: AdapterControl,
    first_event: bool,
    hold_terminal_event: bool,
}

impl ControlledAdapter {
    fn new(behavior: AdapterBehavior) -> Self {
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            prompt_id: Arc::new(Mutex::new(None)),
            actions: Arc::new(Mutex::new(Vec::new())),
            control: AdapterControl::default(),
        }
    }

    fn action_count(&self, action: &str) -> usize {
        self.actions
            .lock()
            .unwrap()
            .iter()
            .filter(|item| item.as_str() == action)
            .count()
    }

    fn prompt_id(&self) -> Option<String> {
        self.prompt_id.lock().unwrap().clone()
    }
}

#[async_trait]
impl ComfyEventSubscription for ControlledSubscription {
    async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
        if self.first_event {
            self.first_event = false;
            let hold = self.control.first_event.clone();
            hold.mark_reached();
            hold.wait_for_release().await;
        }

        let next = self.events.front().cloned().unwrap_or(Ok(None));
        if let Ok(Some(event)) = &next {
            let is_terminal = matches!(
                event,
                ComfyExecutionEvent::ExecutionSucceeded { .. }
                    | ComfyExecutionEvent::ExecutionInterrupted { .. }
            );
            if is_terminal && self.hold_terminal_event {
                let hold = self.control.terminal_event.clone();
                hold.mark_reached();
                hold.wait_for_release().await;
            }
        }

        let next = self.events.pop_front().unwrap_or(Ok(None))?;
        let prompt_id = self.prompt_id.lock().unwrap().clone();
        Ok(next.map(|event| replace_prompt(event, prompt_id.as_deref())))
    }
}

#[async_trait]
impl ComfyAdapter for ControlledAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Ok(ComfyHealth {
            system: SystemStats {
                comfyui_version: Some("test".to_owned()),
                python_version: None,
                os: None,
                ram_total: None,
                ram_free: None,
                devices: Vec::new(),
            },
        })
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Ok(SystemStats {
            comfyui_version: Some("test".to_owned()),
            python_version: None,
            os: None,
            ram_total: None,
            ram_free: None,
            devices: Vec::new(),
        })
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        Ok(json!({}))
    }

    async fn upload_image(
        &self,
        _upload: ComfyImageUpload,
    ) -> Result<ComfyUploadedImage, ComfyAdapterError> {
        self.actions.lock().unwrap().push("upload_image".to_owned());
        let hold = {
            let behavior = self.behavior.lock().unwrap();
            behavior.hold_upload
        };
        if hold {
            self.control.upload.mark_reached();
            self.control.upload.wait_for_release().await;
        }
        Ok(ComfyUploadedImage {
            name: "uploaded.png".to_owned(),
            subfolder: String::new(),
            folder_type: "input".to_owned(),
        })
    }

    async fn cancel_prompt(
        &self,
        _prompt_id: &str,
    ) -> Result<CancelPromptResult, ComfyAdapterError> {
        self.actions
            .lock()
            .unwrap()
            .push("cancel_prompt".to_owned());
        Ok(CancelPromptResult::CancellationRequested)
    }

    async fn get_queue_state(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
        let prompt_id = self.prompt_id();
        let queue = self.behavior.lock().unwrap().queue.clone();
        Ok(ComfyQueueState {
            running_prompt_ids: replace_current(queue.running_prompt_ids, prompt_id.as_deref()),
            pending_prompt_ids: replace_current(queue.pending_prompt_ids, prompt_id.as_deref()),
        })
    }

    async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        self.actions.lock().unwrap().push("get_history".to_owned());
        let history = self.behavior.lock().unwrap().history.clone();
        history
            .map(|mut history| {
                history.prompt_id = prompt_id.to_owned();
                history
            })
            .ok_or_else(|| ComfyAdapterError::HistoryNotFound(prompt_id.to_owned()))
    }

    async fn download_output(
        &self,
        _file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        self.actions
            .lock()
            .unwrap()
            .push("download_output".to_owned());
        Ok(ComfyOutputData {
            bytes: png_bytes(),
            content_type: Some("image/png".to_owned()),
        })
    }

    async fn submit_workflow(
        &self,
        _client_id: &str,
        prompt_id: &str,
        _workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        self.actions
            .lock()
            .unwrap()
            .push("submit_workflow".to_owned());
        *self.prompt_id.lock().unwrap() = Some(prompt_id.to_owned());
        Ok(PromptSubmission {
            prompt_id: prompt_id.to_owned(),
            number: Some(1),
            node_errors: json!({}),
        })
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        self.actions
            .lock()
            .unwrap()
            .push("subscribe_events".to_owned());
        let hold = {
            let behavior = self.behavior.lock().unwrap();
            behavior.hold_subscribe
        };
        if hold {
            self.control.subscribe.mark_reached();
            self.control.subscribe.wait_for_release().await;
        }
        let behavior = self.behavior.lock().unwrap().clone();
        Ok(Box::new(ControlledSubscription {
            events: behavior.events,
            prompt_id: self.prompt_id.clone(),
            control: self.control.clone(),
            first_event: behavior.hold_first_event,
            hold_terminal_event: behavior.hold_terminal_event,
        }))
    }
}

fn replace_current(ids: Vec<String>, prompt_id: Option<&str>) -> Vec<String> {
    ids.into_iter()
        .map(|id| {
            if id == "CURRENT" {
                prompt_id.unwrap_or("CURRENT").to_owned()
            } else {
                id
            }
        })
        .collect()
}

fn replace_prompt(event: ComfyExecutionEvent, prompt_id: Option<&str>) -> ComfyExecutionEvent {
    let prompt_id = prompt_id.unwrap_or_default().to_owned();
    match event {
        ComfyExecutionEvent::ExecutionStarted { .. } => {
            ComfyExecutionEvent::ExecutionStarted { prompt_id }
        }
        ComfyExecutionEvent::ExecutionSucceeded { .. } => {
            ComfyExecutionEvent::ExecutionSucceeded { prompt_id }
        }
        ComfyExecutionEvent::ExecutionInterrupted { node_id, raw, .. } => {
            ComfyExecutionEvent::ExecutionInterrupted {
                prompt_id,
                node_id,
                raw,
            }
        }
        other => other,
    }
}

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 1, 0).unwrap()
    }
}

#[derive(Clone, Default)]
struct NoopSink;

impl TaskUpdateSink for NoopSink {
    fn publish(&self, _task: &Task) {}
}

struct Harness {
    _directory: TempDir,
    task_repository: Arc<SqliteTaskRepository>,
    service: Arc<GenerationService>,
    cancellation: TaskCancellationService,
    registry: TaskExecutionRegistry,
    adapter: Arc<ControlledAdapter>,
    source_asset: Option<AssetId>,
}

impl Harness {
    async fn new(i2i: bool, behavior: AdapterBehavior) -> Self {
        let directory = tempdir().expect("temporary directory should exist");
        let root = directory.path().join("project");
        std::fs::create_dir_all(&root).expect("project directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET root_path = ? WHERE id = 'project-1'")
            .bind(root.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("project root should update");
        sqlx::query(
            "UPDATE workflow_versions SET api_workflow_json = ? WHERE id = 'workflow-version-1'",
        )
        .bind(if i2i { I2I_WORKFLOW } else { T2I_WORKFLOW })
        .execute(&pool)
        .await
        .expect("workflow fixture should update");
        sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
            .bind(if i2i { I2I_RECIPE } else { T2I_RECIPE })
            .execute(&pool)
            .await
            .expect("recipe fixture should update");

        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let asset_store = Arc::new(FileSystemAssetStore::new());
        let source_asset = if i2i {
            let asset_id = AssetId::new();
            let bytes = png_bytes();
            let stored = asset_store
                .write_source_image(&root, &asset_id, "png", &bytes)
                .await
                .expect("source image should be stored");
            let asset = Asset::new_source_image(
                asset_id.clone(),
                "project-1",
                "reference.png",
                "reference.png",
                stored.path.to_string_lossy().to_string(),
                format!("{:x}", Sha256::digest(&bytes)),
                "image/png",
                2,
                2,
                bytes.len() as u64,
                json!({}),
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
            )
            .expect("source asset should be valid");
            asset_repository
                .insert_many(&[asset])
                .await
                .expect("source asset should persist");
            Some(asset_id)
        } else {
            None
        };

        let adapter = Arc::new(ControlledAdapter::new(behavior));
        let registry = TaskExecutionRegistry::default();
        let service = Arc::new(
            GenerationService::new(
                task_repository.clone(),
                snapshot_repository,
                definition_repository,
                adapter.clone(),
                project_repository,
                asset_store,
                asset_repository,
                Arc::new(FixedClock),
            )
            .with_task_update_sink(Arc::new(NoopSink))
            .with_execution_registry(registry.clone()),
        );
        let cancellation = TaskCancellationService::new(
            task_repository.clone(),
            registry.clone(),
            Arc::new(FixedClock),
            Arc::new(NoopSink),
        );
        Self {
            _directory: directory,
            task_repository,
            service,
            cancellation,
            registry,
            adapter,
            source_asset,
        }
    }

    fn request(&self) -> CreateGenerationRequest {
        let mut values = BTreeMap::from([
            (
                "prompt".to_owned(),
                GenerationInputValue::Text("cancellation test".to_owned()),
            ),
            ("steps".to_owned(), GenerationInputValue::Integer(2)),
            (
                "seed".to_owned(),
                GenerationInputValue::Seed(SeedValue::Fixed(123)),
            ),
        ]);
        if let Some(asset_id) = &self.source_asset {
            values.insert(
                "reference_image".to_owned(),
                GenerationInputValue::ImageAsset(asset_id.clone()),
            );
        }
        CreateGenerationRequest {
            project_id: "project-1".to_owned(),
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values,
        }
    }
}

fn behavior(
    events: Vec<ComfyExecutionEvent>,
    history: Option<ComfyHistory>,
    queue: ComfyQueueState,
) -> AdapterBehavior {
    AdapterBehavior {
        events: events.into_iter().map(|event| Ok(Some(event))).collect(),
        history,
        queue,
        hold_upload: false,
        hold_subscribe: false,
        hold_first_event: false,
        hold_terminal_event: false,
    }
}

fn success_history() -> ComfyHistory {
    ComfyHistory {
        prompt_id: "CURRENT".to_owned(),
        status: ComfyHistoryStatus {
            status_str: Some("success".to_owned()),
            completed: Some(true),
            messages: None,
        },
        outputs: BTreeMap::from([(
            "9".to_owned(),
            ComfyNodeOutput {
                images: vec![ComfyOutputFile {
                    filename: "ComfyUI_00001.png".to_owned(),
                    subfolder: String::new(),
                    folder_type: "output".to_owned(),
                }],
            },
        )]),
    }
}

fn png_bytes() -> Vec<u8> {
    let image = RgbImage::from_pixel(2, 2, Rgb([1, 2, 3]));
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("png should encode");
    bytes.into_inner()
}

async fn wait_for_status(
    repository: &SqliteTaskRepository,
    task_id: &TaskId,
    expected: TaskStatus,
) -> Task {
    for _ in 0..2_000 {
        if let Some(task) = repository.find_by_id(task_id).await.unwrap() {
            if task.status == expected {
                return task;
            }
        }
        tokio::task::yield_now().await;
    }
    panic!("task did not reach {}", expected.as_str());
}

async fn wait_for_action(adapter: &ControlledAdapter, action: &str) {
    for _ in 0..2_000 {
        if adapter.action_count(action) > 0 {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("adapter action {action} was not observed");
}

#[tokio::test]
async fn cancel_before_post_never_submits_prompt() {
    let harness = Harness::new(
        false,
        behavior(
            Vec::new(),
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ),
    )
    .await;
    harness.adapter.behavior.lock().unwrap().hold_subscribe = true;
    let task = harness
        .service
        .start_generation(harness.request())
        .await
        .unwrap();
    harness.adapter.control.subscribe.wait_until_reached().await;
    assert_eq!(
        harness
            .cancellation
            .request_cancel(task.id.as_str())
            .await
            .unwrap()
            .status,
        TaskStatus::CancelRequested
    );
    harness.adapter.control.subscribe.release();
    let finished = wait_for_status(&harness.task_repository, &task.id, TaskStatus::Cancelled).await;
    assert_eq!(finished.status, TaskStatus::Cancelled);
    assert_eq!(harness.adapter.action_count("submit_workflow"), 0);
    assert!(!harness.registry.contains(&task.id));
}

#[tokio::test]
async fn cancel_after_upload_stops_before_snapshot_and_post() {
    let harness = Harness::new(
        true,
        behavior(
            Vec::new(),
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ),
    )
    .await;
    harness.adapter.behavior.lock().unwrap().hold_upload = true;
    let task = harness
        .service
        .start_generation(harness.request())
        .await
        .unwrap();
    harness.adapter.control.upload.wait_until_reached().await;
    harness
        .cancellation
        .request_cancel(task.id.as_str())
        .await
        .unwrap();
    harness.adapter.control.upload.release();
    let finished = wait_for_status(&harness.task_repository, &task.id, TaskStatus::Cancelled).await;
    assert_eq!(finished.status, TaskStatus::Cancelled);
    assert_eq!(harness.adapter.action_count("upload_image"), 1);
    assert_eq!(harness.adapter.action_count("submit_workflow"), 0);
    assert!(!harness.registry.contains(&task.id));
}

#[tokio::test]
async fn cancel_queued_task_becomes_cancelled_without_interrupting_unknown_work() {
    let harness = Harness::new(
        false,
        behavior(
            vec![ComfyExecutionEvent::ExecutionStarted {
                prompt_id: "CURRENT".to_owned(),
            }],
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ),
    )
    .await;
    harness.adapter.behavior.lock().unwrap().hold_first_event = true;
    let task = harness
        .service
        .start_generation(harness.request())
        .await
        .unwrap();
    wait_for_status(&harness.task_repository, &task.id, TaskStatus::Queued).await;
    harness
        .adapter
        .control
        .first_event
        .wait_until_reached()
        .await;
    harness
        .cancellation
        .request_cancel(task.id.as_str())
        .await
        .unwrap();
    let finished = wait_for_status(&harness.task_repository, &task.id, TaskStatus::Cancelled).await;
    assert_eq!(finished.status, TaskStatus::Cancelled);
    assert_eq!(harness.adapter.action_count("cancel_prompt"), 1);
    assert_eq!(harness.adapter.action_count("submit_workflow"), 1);
    assert!(!harness.registry.contains(&task.id));
}

#[tokio::test]
async fn cancel_running_waits_for_execution_interrupted_then_cancels() {
    let harness = Harness::new(
        false,
        behavior(
            vec![
                ComfyExecutionEvent::ExecutionStarted {
                    prompt_id: "CURRENT".to_owned(),
                },
                ComfyExecutionEvent::ExecutionInterrupted {
                    prompt_id: "CURRENT".to_owned(),
                    node_id: Some("3".to_owned()),
                    raw: json!({"type": "execution_interrupted"}),
                },
            ],
            None,
            ComfyQueueState {
                running_prompt_ids: vec!["CURRENT".to_owned()],
                pending_prompt_ids: Vec::new(),
            },
        ),
    )
    .await;
    harness.adapter.behavior.lock().unwrap().hold_terminal_event = true;
    let task = harness
        .service
        .start_generation(harness.request())
        .await
        .unwrap();
    wait_for_status(&harness.task_repository, &task.id, TaskStatus::Running).await;
    harness
        .adapter
        .control
        .terminal_event
        .wait_until_reached()
        .await;
    harness
        .cancellation
        .request_cancel(task.id.as_str())
        .await
        .unwrap();
    wait_for_action(&harness.adapter, "cancel_prompt").await;
    harness.adapter.control.terminal_event.release();
    let finished = wait_for_status(&harness.task_repository, &task.id, TaskStatus::Cancelled).await;
    assert_eq!(finished.status, TaskStatus::Cancelled);
    assert_eq!(harness.adapter.action_count("cancel_prompt"), 1);
    assert!(!harness.registry.contains(&task.id));
}

#[tokio::test]
async fn cancellation_racing_success_preserves_result_and_records_not_effective() {
    let harness = Harness::new(
        false,
        behavior(
            vec![
                ComfyExecutionEvent::ExecutionStarted {
                    prompt_id: "CURRENT".to_owned(),
                },
                ComfyExecutionEvent::ExecutionSucceeded {
                    prompt_id: "CURRENT".to_owned(),
                },
            ],
            Some(success_history()),
            ComfyQueueState {
                running_prompt_ids: vec!["CURRENT".to_owned()],
                pending_prompt_ids: Vec::new(),
            },
        ),
    )
    .await;
    harness.adapter.behavior.lock().unwrap().hold_terminal_event = true;
    let task = harness
        .service
        .start_generation(harness.request())
        .await
        .unwrap();
    wait_for_status(&harness.task_repository, &task.id, TaskStatus::Running).await;
    harness
        .adapter
        .control
        .terminal_event
        .wait_until_reached()
        .await;
    harness
        .cancellation
        .request_cancel(task.id.as_str())
        .await
        .unwrap();
    let finished = wait_for_status(&harness.task_repository, &task.id, TaskStatus::Succeeded).await;
    let events = harness.task_repository.list_events(&task.id).await.unwrap();
    assert_eq!(finished.status, TaskStatus::Succeeded);
    assert!(events
        .iter()
        .any(|event| { event.event_type == crate::domain::TaskEventType::TaskCancelNotEffective }));
    assert!(harness.adapter.action_count("download_output") > 0);
    assert!(!harness.registry.contains(&task.id));
}
