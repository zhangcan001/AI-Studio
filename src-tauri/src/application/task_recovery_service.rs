use crate::application::asset_import_service::AssetImportService;
use crate::application::output_collector::OutputCollector;
use crate::application::ports::{
    AssetRepository, AssetStore, Clock, ComfyAdapter, ComfyAdapterError, ComfyHistory,
    GenerationSnapshotRepository, RepositoryError, TaskRepository, TaskUpdateSink,
};
use crate::compiler::RecipeParser;
use crate::domain::{Task, TaskDomainError, TaskError, TaskEventType, TaskStatus};
use serde::Serialize;
use std::{collections::HashSet, error::Error, fmt, sync::Arc};
use tokio::sync::Mutex;

pub struct TaskRecoveryService {
    task_repository: Arc<dyn TaskRepository>,
    snapshot_repository: Arc<dyn GenerationSnapshotRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    comfy_adapter: Arc<dyn ComfyAdapter>,
    output_collector: Arc<OutputCollector>,
    asset_import_service: Arc<AssetImportService>,
    clock: Arc<dyn Clock>,
    task_update_sink: Arc<dyn TaskUpdateSink>,
    gate: Mutex<()>,
}

impl TaskRecoveryService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_repository: Arc<dyn TaskRepository>,
        snapshot_repository: Arc<dyn GenerationSnapshotRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        comfy_adapter: Arc<dyn ComfyAdapter>,
        project_repository: Arc<dyn crate::application::ports::ProjectRepository>,
        asset_store: Arc<dyn AssetStore>,
        clock: Arc<dyn Clock>,
        task_update_sink: Arc<dyn TaskUpdateSink>,
    ) -> Self {
        Self {
            task_repository,
            snapshot_repository,
            asset_repository: asset_repository.clone(),
            output_collector: Arc::new(OutputCollector::new(comfy_adapter.clone())),
            asset_import_service: Arc::new(AssetImportService::new(
                project_repository,
                asset_store,
                asset_repository,
                clock.clone(),
            )),
            comfy_adapter,
            clock,
            task_update_sink,
            gate: Mutex::new(()),
        }
    }

    pub async fn reconcile_active(&self) -> Result<RecoveryReport, TaskRecoveryError> {
        let _gate = self.gate.lock().await;
        let tasks = self.task_repository.list_active().await?;
        if tasks.is_empty() {
            return Ok(RecoveryReport::default());
        }

        let mut report = RecoveryReport {
            examined: tasks.len() as u32,
            ..RecoveryReport::default()
        };

        let mut local_tasks = Vec::new();
        let mut external_tasks = Vec::new();
        for task in tasks {
            self.record_recovery_started(&task).await?;
            if task.prompt_id.is_some() {
                external_tasks.push(task);
            } else {
                local_tasks.push(task);
            }
        }

        for task in local_tasks {
            let outcome = self.reconcile_local_task(task).await?;
            add_outcome(&mut report, outcome);
        }

        if external_tasks.is_empty() {
            return Ok(report);
        }

        let availability = self.comfy_adapter.health_check().await;
        for task in external_tasks {
            let outcome = match &availability {
                Err(error) if is_offline_error(error) => {
                    self.record_deferred(&task, "COMFY_OFFLINE").await?;
                    RecoveryOutcome::Deferred
                }
                Err(error) => {
                    self.record_unresolved(&task, &format!("COMFY_UNAVAILABLE: {error}"))
                        .await?;
                    RecoveryOutcome::Unresolved
                }
                Ok(_) => self.reconcile_external_task(task).await?,
            };
            add_outcome(&mut report, outcome);
        }

        Ok(report)
    }

    async fn reconcile_local_task(
        &self,
        mut task: Task,
    ) -> Result<RecoveryOutcome, TaskRecoveryError> {
        if task.status == TaskStatus::CancelRequested {
            self.finish_cancelled(&mut task).await?;
            self.record_succeeded(&task, "cancelled without an external prompt")
                .await?;
            return Ok(RecoveryOutcome::Succeeded);
        }

        if matches!(
            task.status,
            TaskStatus::Created | TaskStatus::Validating | TaskStatus::Preparing
        ) {
            self.fail_task(
                &mut task,
                TaskError {
                    code: "APP_RESTARTED_BEFORE_SUBMISSION".to_owned(),
                    message: "AI Studio restarted before the workflow was submitted to ComfyUI"
                        .to_owned(),
                    raw: None,
                },
            )
            .await?;
            self.record_succeeded(&task, "task failed before external submission")
                .await?;
            return Ok(RecoveryOutcome::Succeeded);
        }

        self.record_unresolved(&task, "ACTIVE_TASK_MISSING_PROMPT_ID")
            .await?;
        Ok(RecoveryOutcome::Unresolved)
    }

    async fn reconcile_external_task(
        &self,
        mut task: Task,
    ) -> Result<RecoveryOutcome, TaskRecoveryError> {
        let prompt_id = match task.prompt_id.clone() {
            Some(prompt_id) => prompt_id,
            None => return self.reconcile_local_task(task).await,
        };
        let history = match self.comfy_adapter.get_history(&prompt_id).await {
            Ok(history) => Some(history),
            Err(ComfyAdapterError::HistoryNotFound(_)) => None,
            Err(error) if is_offline_error(&error) => {
                self.record_deferred(&task, "COMFY_OFFLINE").await?;
                return Ok(RecoveryOutcome::Deferred);
            }
            Err(error) => {
                self.record_submission_uncertain(&task, &format!("HISTORY_CHECK_FAILED: {error}"))
                    .await?;
                return Ok(RecoveryOutcome::Unresolved);
            }
        };

        if let Some(history) = history {
            match crate::application::generation_service::classify_history_status(&history.status) {
                crate::application::generation_service::HistoryResolution::Success => {
                    self.recover_success(&mut task, &prompt_id, &history)
                        .await?;
                    self.record_succeeded(&task, "history confirms completed execution")
                        .await?;
                    return Ok(RecoveryOutcome::Succeeded);
                }
                crate::application::generation_service::HistoryResolution::Interrupted => {
                    self.recover_interrupted(&mut task, &history).await?;
                    self.record_succeeded(&task, "history confirms interrupted execution")
                        .await?;
                    return Ok(RecoveryOutcome::Succeeded);
                }
                crate::application::generation_service::HistoryResolution::Failed => {
                    self.fail_task(&mut task, history_error(&history)).await?;
                    self.record_succeeded(&task, "history confirms failed execution")
                        .await?;
                    return Ok(RecoveryOutcome::Failed);
                }
                crate::application::generation_service::HistoryResolution::Unknown => {}
            }
        }

        if task.status == TaskStatus::CancelRequested {
            return self.reconcile_cancel_requested(&mut task, &prompt_id).await;
        }

        let queue = match self.comfy_adapter.get_queue_state().await {
            Ok(queue) => queue,
            Err(error) if is_offline_error(&error) => {
                self.record_deferred(&task, "COMFY_OFFLINE").await?;
                return Ok(RecoveryOutcome::Deferred);
            }
            Err(error) => {
                self.record_submission_uncertain(&task, &format!("QUEUE_CHECK_FAILED: {error}"))
                    .await?;
                return Ok(RecoveryOutcome::Unresolved);
            }
        };

        if queue.pending_prompt_ids.iter().any(|id| id == &prompt_id) {
            if task.status == TaskStatus::Preparing {
                self.transition(&mut task, TaskStatus::Queued).await?;
            }
            self.record_succeeded(&task, "prompt remains pending in ComfyUI queue")
                .await?;
            return Ok(RecoveryOutcome::Succeeded);
        }

        if queue.running_prompt_ids.iter().any(|id| id == &prompt_id) {
            if task.status == TaskStatus::Queued {
                self.transition(&mut task, TaskStatus::Running).await?;
            }
            self.record_succeeded(&task, "prompt remains running in ComfyUI queue")
                .await?;
            return Ok(RecoveryOutcome::Succeeded);
        }

        self.record_submission_uncertain(&task, "prompt is absent from ComfyUI history and queue")
            .await?;
        Ok(RecoveryOutcome::Unresolved)
    }

    async fn reconcile_cancel_requested(
        &self,
        task: &mut Task,
        prompt_id: &str,
    ) -> Result<RecoveryOutcome, TaskRecoveryError> {
        if let Err(error) = self.comfy_adapter.cancel_prompt(prompt_id).await {
            if is_offline_error(&error) {
                self.record_deferred(task, "COMFY_OFFLINE").await?;
                return Ok(RecoveryOutcome::Deferred);
            }
            self.record_unresolved(task, &format!("CANCEL_CHECK_FAILED: {error}"))
                .await?;
            return Ok(RecoveryOutcome::Unresolved);
        }

        let history = match self.comfy_adapter.get_history(prompt_id).await {
            Ok(history) => Some(history),
            Err(ComfyAdapterError::HistoryNotFound(_)) => None,
            Err(error) if is_offline_error(&error) => {
                self.record_deferred(task, "COMFY_OFFLINE").await?;
                return Ok(RecoveryOutcome::Deferred);
            }
            Err(error) => {
                self.record_unresolved(task, &format!("HISTORY_CHECK_FAILED: {error}"))
                    .await?;
                return Ok(RecoveryOutcome::Unresolved);
            }
        };

        if let Some(history) = history {
            match crate::application::generation_service::classify_history_status(&history.status) {
                crate::application::generation_service::HistoryResolution::Success => {
                    self.recover_success(task, prompt_id, &history).await?;
                    self.record_succeeded(task, "cancel request was too late")
                        .await?;
                    return Ok(RecoveryOutcome::Succeeded);
                }
                crate::application::generation_service::HistoryResolution::Interrupted => {
                    self.finish_cancelled(task).await?;
                    self.record_succeeded(task, "history confirms cancellation")
                        .await?;
                    return Ok(RecoveryOutcome::Succeeded);
                }
                crate::application::generation_service::HistoryResolution::Failed => {
                    self.fail_task(task, history_error(&history)).await?;
                    self.record_succeeded(task, "history confirms execution failure")
                        .await?;
                    return Ok(RecoveryOutcome::Failed);
                }
                crate::application::generation_service::HistoryResolution::Unknown => {}
            }
        }

        let queue = match self.comfy_adapter.get_queue_state().await {
            Ok(queue) => queue,
            Err(error) if is_offline_error(&error) => {
                self.record_deferred(task, "COMFY_OFFLINE").await?;
                return Ok(RecoveryOutcome::Deferred);
            }
            Err(error) => {
                self.record_unresolved(task, &format!("QUEUE_CHECK_FAILED: {error}"))
                    .await?;
                return Ok(RecoveryOutcome::Unresolved);
            }
        };
        if queue.pending_prompt_ids.iter().any(|id| id == prompt_id)
            || queue.running_prompt_ids.iter().any(|id| id == prompt_id)
        {
            self.record_unresolved(task, "cancel requested but prompt remains in ComfyUI queue")
                .await?;
            return Ok(RecoveryOutcome::Unresolved);
        }

        self.finish_cancelled(task).await?;
        self.record_succeeded(task, "prompt is absent after idempotent cancellation")
            .await?;
        Ok(RecoveryOutcome::Succeeded)
    }

    async fn recover_success(
        &self,
        task: &mut Task,
        prompt_id: &str,
        history: &ComfyHistory,
    ) -> Result<(), TaskRecoveryError> {
        if task.status == TaskStatus::CancelRequested {
            let event = task.record_cancel_not_effective(self.clock.now())?;
            self.task_repository
                .persist_runtime_update(task, &event)
                .await?;
            self.task_update_sink.publish(task);
        }
        if !matches!(task.status, TaskStatus::Collecting | TaskStatus::Succeeded) {
            self.transition(task, TaskStatus::Collecting).await?;
        }
        if task.status == TaskStatus::Succeeded {
            return Ok(());
        }

        let mappings = self.asset_repository.list_output_mappings(&task.id).await?;
        let assets = if mappings.is_empty() {
            self.asset_repository.list_by_source_task(&task.id).await?
        } else {
            Vec::new()
        };
        if mappings.is_empty() && !assets.is_empty() {
            // Legacy tasks have no output mapping. Their existing source-task assets
            // remain the idempotency fallback and must not be re-collected.
        } else {
            let existing_outputs: HashSet<(String, usize)> = mappings
                .iter()
                .map(|mapping| (mapping.output_id.clone(), mapping.ordinal as usize))
                .collect();
            let snapshot = self.snapshot_repository.find_by_task_id(&task.id).await?;
            if let Some(snapshot) = snapshot {
                let recipe = RecipeParser::parse(&snapshot.recipe_yaml).map_err(|error| {
                    TaskRecoveryError::Unresolved(format!("snapshot recipe is invalid: {error}"))
                })?;
                let outputs = self
                    .output_collector
                    .collect_outputs_from_history_excluding(&recipe, history, &existing_outputs)
                    .await
                    .map_err(|error| TaskRecoveryError::OutputCollection(error.to_string()))?;
                if !outputs.is_empty() {
                    self.asset_import_service
                        .import_outputs(&task.project_id, &task.id, outputs)
                        .await
                        .map_err(|error| TaskRecoveryError::AssetImport(error.to_string()))?;
                }
            } else if mappings.is_empty() {
                return Err(TaskRecoveryError::Unresolved(
                    "generation snapshot is missing".to_owned(),
                ));
            }
        }

        let _ = prompt_id;
        let previous_status = task.status;
        let event = task.succeed(self.clock.now())?;
        self.task_repository
            .persist_transition(task, &event, previous_status)
            .await?;
        self.task_update_sink.publish(task);
        Ok(())
    }

    async fn recover_interrupted(
        &self,
        task: &mut Task,
        history: &ComfyHistory,
    ) -> Result<(), TaskRecoveryError> {
        if task.status == TaskStatus::CancelRequested {
            self.finish_cancelled(task).await
        } else {
            self.fail_task(task, history_error(history)).await
        }
    }

    async fn finish_cancelled(&self, task: &mut Task) -> Result<(), TaskRecoveryError> {
        if task.status == TaskStatus::Cancelled {
            return Ok(());
        }
        let previous_status = task.status;
        let event = task.cancel(self.clock.now())?;
        self.task_repository
            .persist_transition(task, &event, previous_status)
            .await?;
        self.task_update_sink.publish(task);
        Ok(())
    }

    async fn fail_task(&self, task: &mut Task, error: TaskError) -> Result<(), TaskRecoveryError> {
        if task.status.is_terminal() {
            return Ok(());
        }
        let previous_status = task.status;
        let event = task.fail(error, self.clock.now())?;
        self.task_repository
            .persist_transition(task, &event, previous_status)
            .await?;
        self.task_update_sink.publish(task);
        Ok(())
    }

    async fn transition(
        &self,
        task: &mut Task,
        status: TaskStatus,
    ) -> Result<(), TaskRecoveryError> {
        let previous_status = task.status;
        let event = crate::domain::TaskStateMachine::transition(task, status, self.clock.now())?;
        self.task_repository
            .persist_transition(task, &event, previous_status)
            .await?;
        self.task_update_sink.publish(task);
        Ok(())
    }

    async fn record_recovery_started(&self, task: &Task) -> Result<(), TaskRecoveryError> {
        self.record_recovery_event(task, TaskEventType::TaskRecoveryStarted, None)
            .await
    }

    async fn record_succeeded(&self, task: &Task, reason: &str) -> Result<(), TaskRecoveryError> {
        self.record_recovery_event(
            task,
            TaskEventType::TaskRecoverySucceeded,
            Some(serde_json::json!({ "reason": reason })),
        )
        .await
    }

    async fn record_deferred(&self, task: &Task, reason: &str) -> Result<(), TaskRecoveryError> {
        self.record_recovery_event(
            task,
            TaskEventType::TaskRecoveryDeferred,
            Some(serde_json::json!({ "reason": reason })),
        )
        .await
    }

    async fn record_unresolved(&self, task: &Task, reason: &str) -> Result<(), TaskRecoveryError> {
        self.record_recovery_event(
            task,
            TaskEventType::TaskRecoveryUnresolved,
            Some(serde_json::json!({ "reason": reason })),
        )
        .await
    }

    async fn record_submission_uncertain(
        &self,
        task: &Task,
        reason: &str,
    ) -> Result<(), TaskRecoveryError> {
        self.record_recovery_event(
            task,
            TaskEventType::TaskRecoveryUnresolved,
            Some(serde_json::json!({
                "code": "SUBMISSION_STATE_UNCERTAIN",
                "reason": reason,
                "promptId": task.prompt_id,
            })),
        )
        .await
    }

    async fn record_recovery_event(
        &self,
        task: &Task,
        event_type: TaskEventType,
        payload: Option<serde_json::Value>,
    ) -> Result<(), TaskRecoveryError> {
        let event = task.record_recovery_event(event_type, payload, self.clock.now())?;
        self.task_repository
            .persist_runtime_update(task, &event)
            .await?;
        self.task_update_sink.publish(task);
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReport {
    pub examined: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub deferred: u32,
    pub unresolved: u32,
}

#[derive(Clone, Copy)]
enum RecoveryOutcome {
    Succeeded,
    Failed,
    Deferred,
    Unresolved,
}

fn add_outcome(report: &mut RecoveryReport, outcome: RecoveryOutcome) {
    match outcome {
        RecoveryOutcome::Succeeded => report.succeeded += 1,
        RecoveryOutcome::Deferred => report.deferred += 1,
        RecoveryOutcome::Unresolved => report.unresolved += 1,
        RecoveryOutcome::Failed => report.failed += 1,
    }
}

#[derive(Debug)]
pub enum TaskRecoveryError {
    Repository(RepositoryError),
    Domain(TaskDomainError),
    Unresolved(String),
    OutputCollection(String),
    AssetImport(String),
}

impl fmt::Display for TaskRecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Domain(error) => write!(formatter, "TASK_DOMAIN_ERROR: {error}"),
            Self::Unresolved(message) => write!(formatter, "TASK_RECOVERY_UNRESOLVED: {message}"),
            Self::OutputCollection(message) => {
                write!(
                    formatter,
                    "TASK_RECOVERY_OUTPUT_COLLECTION_FAILED: {message}"
                )
            }
            Self::AssetImport(message) => {
                write!(formatter, "TASK_RECOVERY_ASSET_IMPORT_FAILED: {message}")
            }
        }
    }
}

impl Error for TaskRecoveryError {}

impl From<RepositoryError> for TaskRecoveryError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<TaskDomainError> for TaskRecoveryError {
    fn from(error: TaskDomainError) -> Self {
        Self::Domain(error)
    }
}

fn is_offline_error(error: &ComfyAdapterError) -> bool {
    matches!(
        error,
        ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_)
    )
}

fn history_error(history: &ComfyHistory) -> TaskError {
    let interrupted = history
        .status
        .status_str
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("interrupt")
        || history
            .status
            .messages
            .as_ref()
            .map(|messages| messages.to_string().to_ascii_lowercase())
            .is_some_and(|messages| messages.contains("execution_interrupted"));
    TaskError {
        code: if interrupted {
            "EXECUTION_INTERRUPTED".to_owned()
        } else {
            "EXECUTION_ERROR".to_owned()
        },
        message: history
            .status
            .messages
            .as_ref()
            .map(|messages| format!("ComfyUI history reported: {messages}"))
            .unwrap_or_else(|| "ComfyUI history reported an execution failure".to_owned()),
        raw: history.status.messages.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::TaskRecoveryService;
    use crate::application::ports::{
        AssetRepository, CancelPromptResult, Clock, ComfyAdapter, ComfyAdapterError,
        ComfyEventSubscription, ComfyHealth, ComfyHistory, ComfyHistoryStatus, ComfyImageUpload,
        ComfyNodeOutput, ComfyOutputData, ComfyOutputFile, ComfyOutputStream, ComfyQueueState,
        ComfySavedResult, ComfyUploadedImage, GenerationSnapshotRepository, PromptSubmission,
        SystemStats, TaskRepository, TaskUpdateSink,
    };
    use crate::domain::{
        Asset, AssetId, GenerationSnapshot, Task, TaskEventType, TaskStateMachine, TaskStatus,
    };
    use crate::infrastructure::database::{
        initialize,
        repositories::{
            test_support, SqliteAssetRepository, SqliteGenerationSnapshotRepository,
            SqliteTaskRepository,
        },
        SqliteProjectRepository,
    };
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use async_trait::async_trait;
    use chrono::{DateTime, Duration, TimeZone, Utc};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};
    use tempfile::{tempdir, TempDir};

    const RECIPE_YAML: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_t2i/recipe.yaml"
    ));

    const VIDEO_RECIPE_YAML: &str = r#"
schema_version: 1
id: simple_video
name: Simple Video
workflow:
  file: workflow_api.json
inputs: {}
bindings: []
outputs:
  - id: generated_video
    type: video
    node: "11"
    required: true
"#;

    #[derive(Clone, Default)]
    struct AdapterCounters {
        health: usize,
        cancel: usize,
        submit: usize,
        upload: usize,
        download: usize,
        history: usize,
        queue: usize,
        open_stream: usize,
    }

    #[derive(Clone)]
    struct RecoveryAdapter {
        online: bool,
        history: Option<ComfyHistory>,
        queue: ComfyQueueState,
        counters: Arc<Mutex<AdapterCounters>>,
    }

    #[async_trait]
    impl ComfyAdapter for RecoveryAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            self.counters.lock().unwrap().health += 1;
            if !self.online {
                return Err(ComfyAdapterError::Offline("test offline".to_owned()));
            }
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
            self.counters.lock().unwrap().upload += 1;
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
            self.counters.lock().unwrap().cancel += 1;
            Ok(CancelPromptResult::CancellationRequested)
        }

        async fn get_queue_state(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
            self.counters.lock().unwrap().queue += 1;
            Ok(self.queue.clone())
        }

        async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            self.counters.lock().unwrap().history += 1;
            self.history
                .clone()
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
            self.counters.lock().unwrap().download += 1;
            Ok(ComfyOutputData {
                bytes: png_bytes(),
                content_type: Some("image/png".to_owned()),
            })
        }

        async fn open_output_stream(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<Box<dyn ComfyOutputStream>, ComfyAdapterError> {
            self.counters.lock().unwrap().open_stream += 1;
            Ok(Box::new(RecoveryVideoStream { sent: false }))
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            prompt_id: &str,
            _workflow: Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            self.counters.lock().unwrap().submit += 1;
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
            Err(ComfyAdapterError::Incompatible(
                "recovery must not subscribe".to_owned(),
            ))
        }
    }

    struct RecoveryVideoStream {
        sent: bool,
    }

    #[async_trait]
    impl ComfyOutputStream for RecoveryVideoStream {
        fn content_type(&self) -> Option<&str> {
            Some("video/mp4")
        }

        fn content_length(&self) -> Option<u64> {
            Some(12)
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ComfyAdapterError> {
            if self.sent {
                return Ok(None);
            }
            self.sent = true;
            let mut bytes = vec![0; 12];
            bytes[4..8].copy_from_slice(b"ftyp");
            Ok(Some(bytes))
        }
    }

    #[derive(Clone, Copy)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 1, 0).unwrap()
        }
    }

    #[derive(Clone, Copy, Default)]
    struct NoopSink;

    impl TaskUpdateSink for NoopSink {
        fn publish(&self, _task: &Task) {}
    }

    struct Harness {
        _directory: TempDir,
        task_repository: Arc<SqliteTaskRepository>,
        snapshot_repository: Arc<SqliteGenerationSnapshotRepository>,
        asset_repository: Arc<SqliteAssetRepository>,
        service: TaskRecoveryService,
        adapter: Arc<RecoveryAdapter>,
    }

    async fn setup(adapter: RecoveryAdapter) -> Harness {
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
        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool));
        let asset_store = Arc::new(FileSystemAssetStore::new());
        let adapter = Arc::new(adapter);
        let service = TaskRecoveryService::new(
            task_repository.clone(),
            snapshot_repository.clone(),
            asset_repository.clone(),
            adapter.clone(),
            project_repository,
            asset_store,
            Arc::new(FixedClock),
            Arc::new(NoopSink),
        );
        Harness {
            _directory: directory,
            task_repository,
            snapshot_repository,
            asset_repository,
            service,
            adapter,
        }
    }

    fn adapter(
        online: bool,
        history: Option<ComfyHistory>,
        queue: ComfyQueueState,
    ) -> RecoveryAdapter {
        RecoveryAdapter {
            online,
            history,
            queue,
            counters: Arc::new(Mutex::new(AdapterCounters::default())),
        }
    }

    async fn create_task(repository: &SqliteTaskRepository) -> Task {
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );
        repository
            .create(&task, &task.created_event())
            .await
            .expect("task fixture should persist");
        task
    }

    async fn local_task(repository: &SqliteTaskRepository, target: TaskStatus) -> Task {
        let mut task = create_task(repository).await;
        match target {
            TaskStatus::Created => {}
            TaskStatus::Validating => {
                persist_transition(repository, &mut task, TaskStatus::Validating, 1).await;
            }
            TaskStatus::Preparing => {
                persist_transition(repository, &mut task, TaskStatus::Validating, 1).await;
                persist_transition(repository, &mut task, TaskStatus::Preparing, 2).await;
            }
            TaskStatus::CancelRequested => {
                let event = task
                    .request_cancel(task.created_at + Duration::seconds(1))
                    .unwrap();
                repository
                    .persist_transition(&task, &event, TaskStatus::Created)
                    .await
                    .unwrap();
            }
            _ => panic!("unsupported local task fixture status: {target:?}"),
        }
        task
    }

    async fn local_external_state_task(
        repository: &SqliteTaskRepository,
        target: TaskStatus,
    ) -> Task {
        let mut task = create_task(repository).await;
        persist_transition(repository, &mut task, TaskStatus::Validating, 1).await;
        persist_transition(repository, &mut task, TaskStatus::Preparing, 2).await;
        match target {
            TaskStatus::Queued => {
                persist_transition(repository, &mut task, TaskStatus::Queued, 3).await;
            }
            TaskStatus::Running => {
                persist_transition(repository, &mut task, TaskStatus::Queued, 3).await;
                persist_transition(repository, &mut task, TaskStatus::Running, 4).await;
            }
            TaskStatus::Collecting => {
                persist_transition(repository, &mut task, TaskStatus::Collecting, 3).await;
            }
            _ => panic!("unsupported missing-prompt fixture status: {target:?}"),
        }
        task
    }

    async fn persist_transition(
        repository: &SqliteTaskRepository,
        task: &mut Task,
        target: TaskStatus,
        seconds: i64,
    ) {
        let previous = task.status;
        let at = task.created_at + Duration::seconds(seconds);
        let event = TaskStateMachine::transition(task, target, at)
            .expect("fixture transition should succeed");
        repository
            .persist_transition(task, &event, previous)
            .await
            .expect("fixture transition should persist");
    }

    async fn submitted_task(repository: &SqliteTaskRepository, target: TaskStatus) -> Task {
        let mut task = create_task(repository).await;
        persist_transition(repository, &mut task, TaskStatus::Validating, 1).await;
        persist_transition(repository, &mut task, TaskStatus::Preparing, 2).await;
        let prepared = task
            .prepare_submission(
                "prompt-test",
                "client-test",
                task.created_at + Duration::seconds(3),
            )
            .unwrap();
        repository
            .persist_runtime_update(&task, &prepared)
            .await
            .unwrap();
        if target == TaskStatus::Preparing {
            return task;
        }
        task.set_queue_number(Some(1)).unwrap();
        persist_transition(repository, &mut task, TaskStatus::Queued, 4).await;
        if target == TaskStatus::Queued {
            return task;
        }
        persist_transition(repository, &mut task, TaskStatus::Running, 5).await;
        if target == TaskStatus::Running {
            return task;
        }
        if target == TaskStatus::CancelRequested {
            let event = task
                .request_cancel(task.created_at + Duration::seconds(6))
                .unwrap();
            repository
                .persist_transition(&task, &event, TaskStatus::Running)
                .await
                .unwrap();
            return task;
        }
        persist_transition(repository, &mut task, TaskStatus::Collecting, 6).await;
        if target == TaskStatus::Collecting {
            return task;
        }
        task
    }

    async fn snapshot_for(repository: &SqliteGenerationSnapshotRepository, task: &Task) {
        snapshot_for_recipe(repository, task, RECIPE_YAML).await;
    }

    async fn snapshot_for_recipe(
        repository: &SqliteGenerationSnapshotRepository,
        task: &Task,
        recipe_yaml: &str,
    ) {
        let snapshot = GenerationSnapshot::new(
            task.id.clone(),
            json!({}),
            recipe_yaml,
            json!({}),
            json!({ "seed": 123 }),
            task.created_at + Duration::seconds(3),
        )
        .unwrap();
        repository.insert(&snapshot).await.unwrap();
    }

    fn success_history() -> ComfyHistory {
        ComfyHistory {
            prompt_id: "prompt-test".to_owned(),
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
                    saved_results: Vec::new(),
                },
            )]),
        }
    }

    fn video_success_history() -> ComfyHistory {
        ComfyHistory {
            prompt_id: "prompt-test".to_owned(),
            status: ComfyHistoryStatus {
                status_str: Some("success".to_owned()),
                completed: Some(true),
                messages: None,
            },
            outputs: BTreeMap::from([(
                "11".to_owned(),
                ComfyNodeOutput {
                    images: vec![ComfyOutputFile {
                        filename: "ComfyUI_00001.mp4".to_owned(),
                        subfolder: String::new(),
                        folder_type: "output".to_owned(),
                    }],
                    saved_results: vec![ComfySavedResult {
                        file: ComfyOutputFile {
                            filename: "ComfyUI_00001.mp4".to_owned(),
                            subfolder: String::new(),
                            folder_type: "output".to_owned(),
                        },
                        animated: Some(true),
                    }],
                },
            )]),
        }
    }

    fn error_history() -> ComfyHistory {
        ComfyHistory {
            prompt_id: "prompt-test".to_owned(),
            status: ComfyHistoryStatus {
                status_str: Some("error".to_owned()),
                completed: Some(false),
                messages: Some(json!({ "execution_error": "test failure" })),
            },
            outputs: BTreeMap::new(),
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

    async fn insert_existing_asset(repository: &SqliteAssetRepository, task: &Task) {
        let asset = Asset::new_generated_image(
            AssetId::new(),
            "project-1",
            "Generated Image 1",
            "ComfyUI_00001.png",
            "existing.png",
            "hash",
            "image/png",
            2,
            2,
            4,
            task.id.clone(),
            json!({}),
            task.created_at + Duration::seconds(7),
        )
        .unwrap();
        repository.insert_many(&[asset]).await.unwrap();
    }

    #[tokio::test]
    async fn offline_local_tasks_recover_without_comfy_calls() {
        let harness = setup(adapter(
            false,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let created = local_task(&harness.task_repository, TaskStatus::Created).await;
        let validating = local_task(&harness.task_repository, TaskStatus::Validating).await;
        let preparing = local_task(&harness.task_repository, TaskStatus::Preparing).await;
        let cancel_requested =
            local_task(&harness.task_repository, TaskStatus::CancelRequested).await;

        let report = harness.service.reconcile_active().await.unwrap();

        assert_eq!(report.examined, 4);
        assert_eq!(report.succeeded, 4);
        assert_eq!(report.deferred, 0);
        assert_eq!(report.unresolved, 0);
        let created = harness
            .task_repository
            .find_by_id(&created.id)
            .await
            .unwrap()
            .unwrap();
        let validating = harness
            .task_repository
            .find_by_id(&validating.id)
            .await
            .unwrap()
            .unwrap();
        let preparing = harness
            .task_repository
            .find_by_id(&preparing.id)
            .await
            .unwrap()
            .unwrap();
        let cancel_requested = harness
            .task_repository
            .find_by_id(&cancel_requested.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(created.status, TaskStatus::Failed);
        assert_eq!(
            created.error.unwrap().code,
            "APP_RESTARTED_BEFORE_SUBMISSION"
        );
        assert_eq!(validating.status, TaskStatus::Failed);
        assert_eq!(
            validating.error.unwrap().code,
            "APP_RESTARTED_BEFORE_SUBMISSION"
        );
        assert_eq!(preparing.status, TaskStatus::Failed);
        assert_eq!(
            preparing.error.unwrap().code,
            "APP_RESTARTED_BEFORE_SUBMISSION"
        );
        assert_eq!(cancel_requested.status, TaskStatus::Cancelled);

        let counters = harness.adapter.counters.lock().unwrap().clone();
        assert_eq!(counters.health, 0);
        assert_eq!(counters.history, 0);
        assert_eq!(counters.queue, 0);
        assert_eq!(counters.cancel, 0);
        assert_eq!(counters.submit, 0);
        assert_eq!(counters.upload, 0);
    }

    #[tokio::test]
    async fn mixed_offline_recovery_resolves_local_and_defers_external() {
        let harness = setup(adapter(
            false,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let local_failed = local_task(&harness.task_repository, TaskStatus::Created).await;
        let local_cancelled =
            local_task(&harness.task_repository, TaskStatus::CancelRequested).await;
        let external = submitted_task(&harness.task_repository, TaskStatus::Running).await;

        let report = harness.service.reconcile_active().await.unwrap();

        assert_eq!(report.examined, 3);
        assert_eq!(report.succeeded, 2);
        assert_eq!(report.deferred, 1);
        assert_eq!(report.unresolved, 0);
        assert_eq!(
            harness
                .task_repository
                .find_by_id(&local_failed.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::Failed
        );
        assert_eq!(
            harness
                .task_repository
                .find_by_id(&local_cancelled.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::Cancelled
        );
        assert_eq!(
            harness
                .task_repository
                .find_by_id(&external.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::Running
        );

        let external_events = harness
            .task_repository
            .list_events(&external.id)
            .await
            .unwrap();
        assert!(external_events.iter().any(|event| {
            event.event_type == TaskEventType::TaskRecoveryDeferred
                && event.payload == Some(json!({ "reason": "COMFY_OFFLINE" }))
        }));
        for task_id in [&local_failed.id, &local_cancelled.id] {
            let events = harness.task_repository.list_events(task_id).await.unwrap();
            assert!(!events
                .iter()
                .any(|event| event.event_type == TaskEventType::TaskRecoveryDeferred));
        }

        let counters = harness.adapter.counters.lock().unwrap().clone();
        assert_eq!(counters.health, 1);
        assert_eq!(counters.history, 0);
        assert_eq!(counters.queue, 0);
        assert_eq!(counters.cancel, 0);
        assert_eq!(counters.submit, 0);
        assert_eq!(counters.upload, 0);
    }

    #[tokio::test]
    async fn missing_prompt_in_external_state_is_unresolved_without_comfy_calls() {
        let harness = setup(adapter(
            false,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let queued = local_external_state_task(&harness.task_repository, TaskStatus::Queued).await;
        let running =
            local_external_state_task(&harness.task_repository, TaskStatus::Running).await;
        let collecting =
            local_external_state_task(&harness.task_repository, TaskStatus::Collecting).await;

        let report = harness.service.reconcile_active().await.unwrap();

        assert_eq!(report.examined, 3);
        assert_eq!(report.unresolved, 3);
        assert_eq!(report.deferred, 0);
        for task in [&queued, &running, &collecting] {
            let found = harness
                .task_repository
                .find_by_id(&task.id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(found.status, task.status);
            let events = harness.task_repository.list_events(&task.id).await.unwrap();
            assert!(events.iter().any(|event| {
                event.event_type == TaskEventType::TaskRecoveryUnresolved
                    && event.payload == Some(json!({ "reason": "ACTIVE_TASK_MISSING_PROMPT_ID" }))
            }));
            assert!(!events
                .iter()
                .any(|event| event.event_type == TaskEventType::TaskRecoveryDeferred));
        }
        let counters = harness.adapter.counters.lock().unwrap().clone();
        assert_eq!(counters.health, 0);
        assert_eq!(counters.history, 0);
        assert_eq!(counters.queue, 0);
        assert_eq!(counters.cancel, 0);
        assert_eq!(counters.submit, 0);
        assert_eq!(counters.upload, 0);
    }

    #[tokio::test]
    async fn multiple_external_tasks_share_one_health_check() {
        let harness = setup(adapter(
            true,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let first = submitted_task(&harness.task_repository, TaskStatus::Running).await;
        let second = submitted_task(&harness.task_repository, TaskStatus::Running).await;

        let report = harness.service.reconcile_active().await.unwrap();

        assert_eq!(report.examined, 2);
        assert_eq!(report.unresolved, 2);
        let counters = harness.adapter.counters.lock().unwrap().clone();
        assert_eq!(counters.health, 1);
        assert_eq!(counters.history, 2);
        assert_eq!(counters.queue, 2);
        assert_eq!(counters.submit, 0);
        assert_eq!(counters.upload, 0);
        let events = harness
            .task_repository
            .list_events(&first.id)
            .await
            .unwrap();
        assert!(events.iter().any(|event| {
            event.event_type == TaskEventType::TaskRecoveryUnresolved
                && event
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("code"))
                    == Some(&json!("SUBMISSION_STATE_UNCERTAIN"))
        }));
        assert_eq!(first.prompt_id, second.prompt_id);
    }

    #[tokio::test]
    async fn restart_created_without_prompt_fails_without_resubmit() {
        let harness = setup(adapter(
            true,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = create_task(&harness.task_repository).await;
        let report = harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(found.status, TaskStatus::Failed);
        assert_eq!(found.error.unwrap().code, "APP_RESTARTED_BEFORE_SUBMISSION");
        assert_eq!(harness.adapter.counters.lock().unwrap().submit, 0);
        assert_eq!(harness.adapter.counters.lock().unwrap().upload, 0);
    }

    #[tokio::test]
    async fn restart_cancel_requested_without_prompt_becomes_cancelled() {
        let harness = setup(adapter(
            true,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let mut task = create_task(&harness.task_repository).await;
        let event = task
            .request_cancel(task.created_at + Duration::seconds(1))
            .unwrap();
        harness
            .task_repository
            .persist_transition(&task, &event, TaskStatus::Created)
            .await
            .unwrap();
        harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, TaskStatus::Cancelled);
        assert!(found.error.is_none());
    }

    #[tokio::test]
    async fn history_success_imports_missing_asset_from_snapshot_recipe() {
        let harness = setup(adapter(
            true,
            Some(success_history()),
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Running).await;
        snapshot_for(&harness.snapshot_repository, &task).await;
        let report = harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(found.status, TaskStatus::Succeeded);
        assert_eq!(
            harness
                .asset_repository
                .list_by_source_task(&task.id)
                .await
                .unwrap()
                .len(),
            1
        );
        let counters = harness.adapter.counters.lock().unwrap().clone();
        assert_eq!(counters.submit, 0);
        assert_eq!(counters.upload, 0);
        assert_eq!(counters.download, 1);
    }

    #[tokio::test]
    async fn history_success_imports_video_and_output_mapping_after_restart() {
        let harness = setup(adapter(
            true,
            Some(video_success_history()),
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Running).await;
        snapshot_for_recipe(&harness.snapshot_repository, &task, VIDEO_RECIPE_YAML).await;

        let report = harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        let assets = harness
            .asset_repository
            .list_by_source_task(&task.id)
            .await
            .unwrap();
        let mappings = harness
            .asset_repository
            .list_output_mappings(&task.id)
            .await
            .unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(found.status, TaskStatus::Succeeded);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].asset_type, crate::domain::AssetType::Video);
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].output_id, "generated_video");
        assert_eq!(mappings[0].ordinal, 0);
        assert!(std::path::Path::new(&assets[0].storage_path).is_file());
        let counters = harness.adapter.counters.lock().unwrap().clone();
        assert_eq!(counters.open_stream, 1);
        assert_eq!(counters.download, 0);
        assert_eq!(counters.submit, 0);
    }

    #[tokio::test]
    async fn history_success_with_existing_asset_does_not_duplicate_import() {
        let harness = setup(adapter(
            true,
            Some(success_history()),
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Collecting).await;
        insert_existing_asset(&harness.asset_repository, &task).await;
        harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, TaskStatus::Succeeded);
        assert_eq!(
            harness
                .asset_repository
                .list_by_source_task(&task.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(harness.adapter.counters.lock().unwrap().download, 0);
    }

    #[tokio::test]
    async fn history_success_with_existing_mapping_does_not_download_again() {
        let harness = setup(adapter(
            true,
            Some(success_history()),
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Collecting).await;
        let asset = Asset::new_generated_image(
            AssetId::new(),
            "project-1",
            "Generated Image 1",
            "ComfyUI_00001.png",
            "existing-mapped.png",
            "hash-mapped",
            "image/png",
            2,
            2,
            4,
            task.id.clone(),
            json!({}),
            task.created_at + Duration::seconds(7),
        )
        .unwrap();
        let mapping = crate::application::ports::TaskOutputAssetMapping {
            task_id: task.id.clone(),
            output_id: "generated_image".to_owned(),
            ordinal: 0,
            asset_id: asset.id.clone(),
            created_at: asset.created_at,
        };
        harness
            .asset_repository
            .insert_generated_outputs(&[asset], &[mapping])
            .await
            .unwrap();
        snapshot_for(&harness.snapshot_repository, &task).await;

        harness.service.reconcile_active().await.unwrap();
        assert_eq!(
            harness
                .asset_repository
                .list_output_mappings(&task.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(harness.adapter.counters.lock().unwrap().download, 0);
    }

    #[tokio::test]
    async fn history_error_marks_task_failed() {
        let harness = setup(adapter(
            true,
            Some(error_history()),
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Running).await;
        let report = harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.failed, 1);
        assert_eq!(found.status, TaskStatus::Failed);
        assert_eq!(found.error.unwrap().code, "EXECUTION_ERROR");
    }

    #[tokio::test]
    async fn queue_pending_advances_preparing_without_resubmit() {
        let harness = setup(adapter(
            true,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: vec!["prompt-test".to_owned()],
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Preparing).await;
        harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, TaskStatus::Queued);
        assert_eq!(harness.adapter.counters.lock().unwrap().submit, 0);
    }

    #[tokio::test]
    async fn queue_running_advances_queued_without_state_regression() {
        let harness = setup(adapter(
            true,
            None,
            ComfyQueueState {
                running_prompt_ids: vec!["prompt-test".to_owned()],
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Queued).await;
        harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, TaskStatus::Running);
        assert_eq!(harness.adapter.counters.lock().unwrap().submit, 0);
    }

    #[tokio::test]
    async fn offline_recovery_is_deferred_and_status_is_unchanged() {
        let harness = setup(adapter(
            false,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Running).await;
        let report = harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.deferred, 1);
        assert_eq!(found.status, TaskStatus::Running);
        assert_eq!(harness.adapter.counters.lock().unwrap().history, 0);
        let events = harness.task_repository.list_events(&task.id).await.unwrap();
        assert!(events
            .iter()
            .any(|event| { event.event_type == TaskEventType::TaskRecoveryDeferred }));
    }

    #[tokio::test]
    async fn absent_history_and_queue_is_unresolved_and_unchanged() {
        let harness = setup(adapter(
            true,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::Running).await;
        let report = harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.unresolved, 1);
        assert_eq!(found.status, TaskStatus::Running);
        let events = harness.task_repository.list_events(&task.id).await.unwrap();
        assert!(events
            .iter()
            .any(|event| { event.event_type == TaskEventType::TaskRecoveryUnresolved }));
        let counters = harness.adapter.counters.lock().unwrap().clone();
        assert_eq!(counters.submit, 0);
        assert_eq!(counters.upload, 0);
    }

    #[tokio::test]
    async fn cancel_requested_recovery_retries_cancel_without_resubmit() {
        let harness = setup(adapter(
            true,
            None,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        ))
        .await;
        let task = submitted_task(&harness.task_repository, TaskStatus::CancelRequested).await;
        let report = harness.service.reconcile_active().await.unwrap();
        let found = harness
            .task_repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(found.status, TaskStatus::Cancelled);
        let counters = harness.adapter.counters.lock().unwrap().clone();
        assert_eq!(counters.cancel, 1);
        assert_eq!(counters.submit, 0);
        assert_eq!(counters.upload, 0);
    }
}
