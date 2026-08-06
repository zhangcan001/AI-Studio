use crate::application::asset_import_service::{AssetImportError, AssetImportService};
use crate::application::generation_input_preparer::{
    image_snapshot_value, GenerationInputPrepareError, GenerationInputPreparer,
    GenerationInputValue, PreparedGenerationInputs,
};
use crate::application::output_collector::{OutputCollector, OutputCollectorError};
use crate::application::ports::{
    AssetRepository, AssetStore, Clock, ComfyAdapter, ComfyAdapterError, ComfyExecutionEvent,
    ComfyHistory, ComfyHistoryStatus, GenerationDefinitionRepository, GenerationSnapshotRepository,
    MonotonicEventClock, NoopTaskUpdateSink, ProjectRepository, RepositoryError, TaskRepository,
    TaskUpdateSink,
};
use crate::application::task_execution_registry::TaskExecutionRegistry;
use crate::compiler::{CompileError, RecipeParser, WorkflowCompiler};
use crate::domain::{
    CompileRequest, GenerationSnapshot, ResolvedInputValue, SeedValue, Task, TaskDomainError,
    TaskError, TaskStateMachine, TaskStatus,
};
use serde_json::{Map, Number, Value};
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};
use tokio::sync::watch;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct CreateGenerationRequest {
    pub project_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, GenerationInputValue>,
}

#[derive(Debug, PartialEq)]
pub enum GenerationServiceError {
    DefinitionNotFound {
        workflow_version_id: String,
        recipe_id: String,
    },
    Repository(RepositoryError),
    Compile(CompileError),
    InputPrepare(GenerationInputPrepareError),
    Snapshot(String),
    Domain(TaskDomainError),
    Comfy(ComfyAdapterError),
    StreamDisconnected(String),
    OutputCollection(OutputCollectorError),
    AssetImport(AssetImportError),
    ExecutionFailed {
        code: String,
        message: String,
    },
}

impl fmt::Display for GenerationServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefinitionNotFound {
                workflow_version_id,
                recipe_id,
            } => write!(
                formatter,
                "GENERATION_DEFINITION_NOT_FOUND: workflow version {workflow_version_id} and recipe {recipe_id}"
            ),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Compile(error) => write!(formatter, "{error}"),
            Self::InputPrepare(error) => write!(formatter, "{error}"),
            Self::Snapshot(message) => write!(formatter, "SNAPSHOT_ERROR: {message}"),
            Self::Domain(error) => write!(formatter, "TASK_DOMAIN_ERROR: {error}"),
            Self::Comfy(error) => write!(formatter, "{error}"),
            Self::StreamDisconnected(message) => {
                write!(formatter, "COMFY_STREAM_DISCONNECTED: {message}")
            }
            Self::OutputCollection(error) => write!(formatter, "{error}"),
            Self::AssetImport(error) => write!(formatter, "{error}"),
            Self::ExecutionFailed { code, message } => write!(formatter, "{code}: {message}"),
        }
    }
}

impl Error for GenerationServiceError {}

impl From<RepositoryError> for GenerationServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<TaskDomainError> for GenerationServiceError {
    fn from(error: TaskDomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<ComfyAdapterError> for GenerationServiceError {
    fn from(error: ComfyAdapterError) -> Self {
        Self::Comfy(error)
    }
}

pub struct GenerationService {
    task_repository: Arc<dyn TaskRepository>,
    snapshot_repository: Arc<dyn GenerationSnapshotRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    comfy_adapter: Arc<dyn ComfyAdapter>,
    output_collector: Arc<OutputCollector>,
    asset_import_service: Arc<AssetImportService>,
    generation_input_preparer: Arc<GenerationInputPreparer>,
    clock: Arc<dyn Clock>,
    task_update_sink: Arc<dyn TaskUpdateSink>,
    execution_registry: TaskExecutionRegistry,
    compiler: WorkflowCompiler,
}

enum CancelResolution {
    KeepWaiting,
    Cancelled,
    Success(ComfyHistory),
    Failed(TaskError),
}

impl GenerationService {
    pub fn new(
        task_repository: Arc<dyn TaskRepository>,
        snapshot_repository: Arc<dyn GenerationSnapshotRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        comfy_adapter: Arc<dyn ComfyAdapter>,
        project_repository: Arc<dyn ProjectRepository>,
        asset_store: Arc<dyn AssetStore>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let clock: Arc<dyn Clock> = Arc::new(MonotonicEventClock::new(clock));
        Self {
            task_repository,
            snapshot_repository,
            definition_repository,
            output_collector: Arc::new(OutputCollector::new(comfy_adapter.clone())),
            asset_import_service: Arc::new(AssetImportService::new(
                project_repository.clone(),
                asset_store.clone(),
                asset_repository.clone(),
                clock.clone(),
            )),
            generation_input_preparer: Arc::new(GenerationInputPreparer::new(
                asset_repository,
                asset_store,
                comfy_adapter.clone(),
            )),
            comfy_adapter,
            clock,
            task_update_sink: Arc::new(NoopTaskUpdateSink),
            execution_registry: TaskExecutionRegistry::default(),
            compiler: WorkflowCompiler,
        }
    }

    pub fn with_task_update_sink(mut self, task_update_sink: Arc<dyn TaskUpdateSink>) -> Self {
        self.task_update_sink = task_update_sink;
        self
    }

    pub fn with_execution_registry(mut self, execution_registry: TaskExecutionRegistry) -> Self {
        self.execution_registry = execution_registry;
        self
    }

    #[allow(dead_code)]
    pub async fn execute(
        &self,
        request: CreateGenerationRequest,
    ) -> Result<Task, GenerationServiceError> {
        let (request, definition, task) = self.prepare_task(request).await?;
        let (cancel_signal, _guard) = self.execution_registry.register(task.id.clone());
        self.execute_prepared(request, definition, task, cancel_signal)
            .await
    }

    pub async fn start_generation(
        self: &Arc<Self>,
        request: CreateGenerationRequest,
    ) -> Result<Task, GenerationServiceError> {
        let (request, definition, task) = self.prepare_task(request).await?;
        let (cancel_signal, guard) = self.execution_registry.register(task.id.clone());
        let service = Arc::clone(self);
        let background_task = task.clone();
        tokio::spawn(async move {
            let _guard = guard;
            if let Err(error) = service
                .execute_prepared(request, definition, background_task, cancel_signal)
                .await
            {
                tracing::error!(error = %error, "background generation failed");
            }
        });
        Ok(task)
    }

    async fn prepare_task(
        &self,
        request: CreateGenerationRequest,
    ) -> Result<
        (
            CreateGenerationRequest,
            crate::application::ports::GenerationDefinition,
            Task,
        ),
        GenerationServiceError,
    > {
        let definition = self
            .definition_repository
            .find(&request.workflow_version_id, &request.recipe_id)
            .await?
            .ok_or_else(|| GenerationServiceError::DefinitionNotFound {
                workflow_version_id: request.workflow_version_id.clone(),
                recipe_id: request.recipe_id.clone(),
            })?;
        let created_at = self.clock.now();
        let task = Task::new(
            request.project_id.clone(),
            definition.workflow_id.clone(),
            definition.workflow_version_id.clone(),
            definition.recipe_id.clone(),
            created_at,
        );
        let created_event = task.created_event();
        self.task_repository.create(&task, &created_event).await?;
        self.task_update_sink.publish(&task);
        Ok((request, definition, task))
    }

    async fn execute_prepared(
        &self,
        request: CreateGenerationRequest,
        definition: crate::application::ports::GenerationDefinition,
        mut task: Task,
        mut cancel_signal: watch::Receiver<bool>,
    ) -> Result<Task, GenerationServiceError> {
        let project_id = request.project_id.clone();
        self.transition_and_persist(&mut task, TaskStatus::Validating)
            .await?;
        if self.cancel_checkpoint(&mut task, &cancel_signal).await? {
            return Ok(task);
        }

        let recipe = match RecipeParser::parse(&definition.recipe_yaml) {
            Ok(recipe) => recipe,
            Err(error) => {
                let error = CompileError::from(error);
                return Err(self
                    .fail_and_preserve(
                        &mut task,
                        task_error_from_compile(&error),
                        GenerationServiceError::Compile(error),
                    )
                    .await);
            }
        };
        let workflow =
            match crate::domain::WorkflowDocument::parse(definition.workflow_json.clone()) {
                Ok(workflow) => workflow,
                Err(error) => {
                    let error = CompileError::from(error);
                    return Err(self
                        .fail_and_preserve(
                            &mut task,
                            task_error_from_compile(&error),
                            GenerationServiceError::Compile(error),
                        )
                        .await);
                }
            };
        let preflight_request =
            CompileRequest::new(GenerationInputPreparer::preflight_values(&request.values));
        if let Err(error) = self
            .compiler
            .compile(&workflow, &recipe, &preflight_request)
        {
            return Err(self
                .fail_and_preserve(
                    &mut task,
                    task_error_from_compile(&error),
                    GenerationServiceError::Compile(error),
                )
                .await);
        }
        if let Err(error) = self
            .generation_input_preparer
            .validate_asset_references(&project_id, &request.values)
            .await
        {
            return Err(self
                .fail_and_preserve(
                    &mut task,
                    task_error_from_input_prepare(&error),
                    GenerationServiceError::InputPrepare(error),
                )
                .await);
        }
        if self.cancel_checkpoint(&mut task, &cancel_signal).await? {
            return Ok(task);
        }

        self.transition_and_persist(&mut task, TaskStatus::Preparing)
            .await?;
        if self.cancel_checkpoint(&mut task, &cancel_signal).await? {
            return Ok(task);
        }

        let prepared = match self
            .generation_input_preparer
            .prepare(&project_id, &task.id, &request.values)
            .await
        {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(self
                    .fail_and_preserve(
                        &mut task,
                        task_error_from_input_prepare(&error),
                        GenerationServiceError::InputPrepare(error),
                    )
                    .await);
            }
        };
        if self.cancel_checkpoint(&mut task, &cancel_signal).await? {
            return Ok(task);
        }
        let compile_request = CompileRequest::new(prepared.compiler_values.clone());
        let compile_result = match self.compiler.compile(&workflow, &recipe, &compile_request) {
            Ok(result) => result,
            Err(error) => {
                return Err(self
                    .fail_and_preserve(
                        &mut task,
                        task_error_from_compile(&error),
                        GenerationServiceError::Compile(error),
                    )
                    .await);
            }
        };
        if self.cancel_checkpoint(&mut task, &cancel_signal).await? {
            return Ok(task);
        }

        let snapshot = match GenerationSnapshot::new(
            task.id.clone(),
            compile_result.workflow.clone(),
            definition.recipe_yaml.clone(),
            input_values_to_json(&request.values),
            resolved_inputs_to_json(&compile_result.resolved_inputs, &prepared),
            self.clock.now(),
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let original = GenerationServiceError::Snapshot(error.to_string());
                return Err(self
                    .fail_and_preserve(
                        &mut task,
                        TaskError {
                            code: "SNAPSHOT_INVALID".to_owned(),
                            message: error.to_string(),
                            raw: None,
                        },
                        original,
                    )
                    .await);
            }
        };
        if let Err(error) = self.snapshot_repository.insert(&snapshot).await {
            let original = GenerationServiceError::Repository(error.clone());
            return Err(self
                .fail_and_preserve(
                    &mut task,
                    TaskError {
                        code: "SNAPSHOT_PERSISTENCE_ERROR".to_owned(),
                        message: error.to_string(),
                        raw: None,
                    },
                    original,
                )
                .await);
        }
        if self.cancel_checkpoint(&mut task, &cancel_signal).await? {
            return Ok(task);
        }

        let client_id = Uuid::new_v4().to_string();
        let prompt_id = Uuid::new_v4().to_string();
        let submission_event =
            task.prepare_submission(prompt_id.clone(), client_id.clone(), self.clock.now())?;
        self.task_repository
            .persist_runtime_update(&task, &submission_event)
            .await?;
        self.task_update_sink.publish(&task);

        if self.cancel_checkpoint(&mut task, &cancel_signal).await? {
            return Ok(task);
        }

        let mut subscription = match self.comfy_adapter.subscribe_events(&client_id).await {
            Ok(subscription) => subscription,
            Err(error) => {
                let original = GenerationServiceError::Comfy(error.clone());
                return Err(self
                    .fail_and_preserve(&mut task, task_error_from_adapter(&error), original)
                    .await);
            }
        };

        if self.cancel_checkpoint(&mut task, &cancel_signal).await? {
            return Ok(task);
        }

        let submission = match self
            .comfy_adapter
            .submit_workflow(&client_id, &prompt_id, compile_result.workflow)
            .await
        {
            Ok(submission) => submission,
            Err(error) => {
                let original = GenerationServiceError::Comfy(error.clone());
                return Err(self
                    .fail_and_preserve(&mut task, task_error_from_adapter(&error), original)
                    .await);
            }
        };

        if submission.prompt_id != prompt_id {
            let error = ComfyAdapterError::Protocol(format!(
                "POST /prompt prompt_id mismatch: requested {prompt_id}, received {}",
                submission.prompt_id
            ));
            let original = GenerationServiceError::Comfy(error.clone());
            return Err(self
                .fail_and_preserve(&mut task, task_error_from_adapter(&error), original)
                .await);
        }

        let persisted_after_submit = self.task_repository.find_by_id(&task.id).await?;
        match persisted_after_submit {
            Some(current) if current.status == TaskStatus::CancelRequested => {
                task = current;
            }
            Some(_) => {
                task.set_queue_number(submission.number)?;
                self.transition_and_persist(&mut task, TaskStatus::Queued)
                    .await?;
            }
            None => {
                return Err(GenerationServiceError::Repository(
                    RepositoryError::NotFound {
                        entity: "task".to_owned(),
                        id: task.id.to_string(),
                    },
                ));
            }
        }

        let mut cancel_action_sent = false;
        loop {
            if !cancel_action_sent && *cancel_signal.borrow() {
                match self.handle_submitted_cancel(&mut task, &prompt_id).await? {
                    CancelResolution::KeepWaiting => {
                        cancel_action_sent = true;
                    }
                    CancelResolution::Cancelled => {
                        self.cancel_checkpoint(&mut task, &cancel_signal).await?;
                        return Ok(task);
                    }
                    CancelResolution::Success(history) => {
                        return self
                            .complete_success(
                                &mut task,
                                &recipe,
                                &project_id,
                                &prompt_id,
                                Some(&history),
                            )
                            .await;
                    }
                    CancelResolution::Failed(error) => {
                        let message = error.message.clone();
                        return Err(self
                            .fail_and_preserve(
                                &mut task,
                                error,
                                GenerationServiceError::ExecutionFailed {
                                    code: "EXECUTION_ERROR".to_owned(),
                                    message,
                                },
                            )
                            .await);
                    }
                }
            }

            let event_result = tokio::select! {
                changed = cancel_signal.changed(), if !cancel_action_sent => {
                    if changed.is_err() {
                        cancel_action_sent = true;
                    }
                    continue;
                }
                event = subscription.next_event() => event,
            };

            let event = match event_result {
                Ok(Some(event)) => event,
                Ok(None) => {
                    let message = "ComfyUI WebSocket closed after prompt submission".to_owned();
                    return Err(
                        match self
                            .persist_stream_disconnect(&mut task, message.clone())
                            .await
                        {
                            Ok(()) => GenerationServiceError::StreamDisconnected(message),
                            Err(error) => error,
                        },
                    );
                }
                Err(error) => {
                    let stream_error = match &error {
                        ComfyAdapterError::StreamDisconnected(message) => {
                            GenerationServiceError::StreamDisconnected(message.clone())
                        }
                        _ => GenerationServiceError::Comfy(error.clone()),
                    };
                    return Err(
                        match self
                            .persist_stream_disconnect(&mut task, error.to_string())
                            .await
                        {
                            Ok(()) => stream_error,
                            Err(error) => error,
                        },
                    );
                }
            };

            if event.prompt_id() != prompt_id {
                tracing::debug!(
                    task_id = %task.id,
                    expected_prompt_id = %prompt_id,
                    received_prompt_id = %event.prompt_id(),
                    "ignoring execution event for another ComfyUI task"
                );
                continue;
            }

            match event {
                ComfyExecutionEvent::ExecutionStarted { .. } => {
                    if task.status == TaskStatus::Queued {
                        self.transition_and_persist(&mut task, TaskStatus::Running)
                            .await?;
                    }
                }
                ComfyExecutionEvent::NodeStarted { node_id, .. } => {
                    if task.status == TaskStatus::Running {
                        let event = task.update_node_progress(node_id, self.clock.now())?;
                        self.task_repository
                            .persist_runtime_update(&task, &event)
                            .await?;
                        self.task_update_sink.publish(&task);
                    }
                }
                ComfyExecutionEvent::Progress {
                    node_id,
                    current,
                    total,
                    ..
                } => {
                    if task.status == TaskStatus::Running {
                        if let Some(event) =
                            task.update_step_progress(current, total, node_id, self.clock.now())?
                        {
                            self.task_repository
                                .persist_runtime_update(&task, &event)
                                .await?;
                            self.task_update_sink.publish(&task);
                        }
                    }
                }
                ComfyExecutionEvent::ExecutionSucceeded { .. } => {
                    if *cancel_signal.borrow() {
                        if let Some(current) = self.task_repository.find_by_id(&task.id).await? {
                            if current.status == TaskStatus::CancelRequested {
                                task = current;
                            }
                        }
                    }
                    if task.status == TaskStatus::CancelRequested {
                        return self
                            .complete_success(&mut task, &recipe, &project_id, &prompt_id, None)
                            .await;
                    }
                    if task.status != TaskStatus::Running {
                        continue;
                    }
                    return self
                        .complete_success(&mut task, &recipe, &project_id, &prompt_id, None)
                        .await;
                }
                ComfyExecutionEvent::ExecutionError {
                    node_id,
                    message,
                    raw,
                    ..
                } => {
                    self.refresh_task_from_database(&mut task).await?;
                    let message = if let Some(node_id) = node_id {
                        format!("node {node_id}: {message}")
                    } else {
                        message
                    };
                    let original = GenerationServiceError::ExecutionFailed {
                        code: "EXECUTION_ERROR".to_owned(),
                        message: message.clone(),
                    };
                    return Err(self
                        .fail_and_preserve(
                            &mut task,
                            TaskError {
                                code: "EXECUTION_ERROR".to_owned(),
                                message,
                                raw: Some(raw),
                            },
                            original,
                        )
                        .await);
                }
                ComfyExecutionEvent::ExecutionInterrupted { node_id, raw, .. } => {
                    self.refresh_task_from_database(&mut task).await?;
                    if task.status == TaskStatus::CancelRequested {
                        self.cancel_checkpoint(&mut task, &cancel_signal).await?;
                        return Ok(task);
                    }
                    let message = node_id
                        .map(|node_id| format!("execution interrupted at node {node_id}"))
                        .unwrap_or_else(|| "ComfyUI interrupted execution".to_owned());
                    let original = GenerationServiceError::ExecutionFailed {
                        code: "EXECUTION_INTERRUPTED".to_owned(),
                        message: message.clone(),
                    };
                    return Err(self
                        .fail_and_preserve(
                            &mut task,
                            TaskError {
                                code: "EXECUTION_INTERRUPTED".to_owned(),
                                message,
                                raw: Some(raw),
                            },
                            original,
                        )
                        .await);
                }
            }
        }
    }

    async fn cancel_checkpoint(
        &self,
        task: &mut Task,
        cancel_signal: &watch::Receiver<bool>,
    ) -> Result<bool, GenerationServiceError> {
        if !*cancel_signal.borrow() {
            return Ok(false);
        }

        let Some(current) = self.task_repository.find_by_id(&task.id).await? else {
            return Err(GenerationServiceError::Repository(
                RepositoryError::NotFound {
                    entity: "task".to_owned(),
                    id: task.id.to_string(),
                },
            ));
        };
        match current.status {
            TaskStatus::CancelRequested => {
                let mut cancelled = current;
                let event = cancelled.cancel(self.clock.now())?;
                self.task_repository
                    .persist_transition(&cancelled, &event, TaskStatus::CancelRequested)
                    .await?;
                self.task_update_sink.publish(&cancelled);
                *task = cancelled;
                Ok(true)
            }
            status if status.is_terminal() => {
                *task = current;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn refresh_task_from_database(
        &self,
        task: &mut Task,
    ) -> Result<(), GenerationServiceError> {
        *task = self
            .task_repository
            .find_by_id(&task.id)
            .await?
            .ok_or_else(|| {
                GenerationServiceError::Repository(RepositoryError::NotFound {
                    entity: "task".to_owned(),
                    id: task.id.to_string(),
                })
            })?;
        Ok(())
    }

    async fn handle_submitted_cancel(
        &self,
        task: &mut Task,
        prompt_id: &str,
    ) -> Result<CancelResolution, GenerationServiceError> {
        self.refresh_task_from_database(task).await?;
        if task.status != TaskStatus::CancelRequested {
            return Ok(CancelResolution::KeepWaiting);
        }

        let _cancel_result = self.comfy_adapter.cancel_prompt(prompt_id).await?;
        self.reconcile_cancel_request(prompt_id).await
    }

    async fn reconcile_cancel_request(
        &self,
        prompt_id: &str,
    ) -> Result<CancelResolution, GenerationServiceError> {
        let history = match self.comfy_adapter.get_history(prompt_id).await {
            Ok(history) => Some(history),
            Err(ComfyAdapterError::HistoryNotFound(_)) => None,
            Err(error) => return Err(GenerationServiceError::Comfy(error)),
        };

        if let Some(history) = history {
            return Ok(match classify_history_status(&history.status) {
                HistoryResolution::Success => CancelResolution::Success(history),
                HistoryResolution::Interrupted => CancelResolution::Cancelled,
                HistoryResolution::Failed => CancelResolution::Failed(history_error(&history)),
                HistoryResolution::Unknown => self.cancel_resolution_from_queue(prompt_id).await?,
            });
        }

        self.cancel_resolution_from_queue(prompt_id).await
    }

    async fn cancel_resolution_from_queue(
        &self,
        prompt_id: &str,
    ) -> Result<CancelResolution, GenerationServiceError> {
        let queue = self.comfy_adapter.get_queue_state().await?;
        if queue.running_prompt_ids.iter().any(|id| id == prompt_id)
            || queue.pending_prompt_ids.iter().any(|id| id == prompt_id)
        {
            Ok(CancelResolution::KeepWaiting)
        } else {
            Ok(CancelResolution::Cancelled)
        }
    }

    async fn complete_success(
        &self,
        task: &mut Task,
        recipe: &crate::domain::Recipe,
        project_id: &str,
        prompt_id: &str,
        history: Option<&ComfyHistory>,
    ) -> Result<Task, GenerationServiceError> {
        if task.status == TaskStatus::CancelRequested {
            let event = task.record_cancel_not_effective(self.clock.now())?;
            self.task_repository
                .persist_runtime_update(task, &event)
                .await?;
            self.task_update_sink.publish(task);
        }
        if task.status != TaskStatus::Collecting {
            self.transition_and_persist(task, TaskStatus::Collecting)
                .await?;
        }

        let images = match history {
            Some(history) => {
                self.output_collector
                    .collect_from_history(recipe, history)
                    .await
            }
            None => self.output_collector.collect(recipe, prompt_id).await,
        };
        let images = match images {
            Ok(images) => images,
            Err(error) => {
                let original = GenerationServiceError::OutputCollection(error);
                return Err(self
                    .fail_and_preserve(task, task_error_from_output(&original), original)
                    .await);
            }
        };
        if let Err(error) = self
            .asset_import_service
            .import(project_id, &task.id, &images)
            .await
        {
            let original = GenerationServiceError::AssetImport(error);
            return Err(self
                .fail_and_preserve(task, task_error_from_output(&original), original)
                .await);
        }

        let previous_status = task.status;
        let event = task.succeed(self.clock.now())?;
        if let Err(error) = self
            .task_repository
            .persist_transition(task, &event, previous_status)
            .await
        {
            tracing::error!(
                task_id = %task.id,
                error = %error,
                "assets imported but SUCCEEDED task persistence failed"
            );
            return Err(error.into());
        }
        self.task_update_sink.publish(task);
        Ok(task.clone())
    }

    async fn transition_and_persist(
        &self,
        task: &mut Task,
        target: TaskStatus,
    ) -> Result<(), GenerationServiceError> {
        let previous_status = task.status;
        let event = TaskStateMachine::transition(task, target, self.clock.now())?;
        self.task_repository
            .persist_transition(task, &event, previous_status)
            .await?;
        self.task_update_sink.publish(task);
        Ok(())
    }

    async fn fail_and_preserve(
        &self,
        task: &mut Task,
        error: TaskError,
        original: GenerationServiceError,
    ) -> GenerationServiceError {
        let previous_status = task.status;
        let event = match task.fail(error, self.clock.now()) {
            Ok(event) => event,
            Err(error) => return GenerationServiceError::Domain(error),
        };
        match self
            .task_repository
            .persist_transition(task, &event, previous_status)
            .await
        {
            Ok(_) => {
                self.task_update_sink.publish(task);
                original
            }
            Err(error) => {
                tracing::error!(
                    task_id = %task.id,
                    error = %error,
                    "failed to persist FAILED task state"
                );
                GenerationServiceError::Repository(error)
            }
        }
    }

    async fn persist_stream_disconnect(
        &self,
        task: &mut Task,
        message: String,
    ) -> Result<(), GenerationServiceError> {
        let event = match task.record_stream_disconnected(message, self.clock.now()) {
            Ok(event) => event,
            Err(error) => return Err(GenerationServiceError::Domain(error)),
        };
        self.task_repository
            .persist_runtime_update(task, &event)
            .await
            .map(|_| {
                self.task_update_sink.publish(task);
            })
            .map_err(GenerationServiceError::Repository)
    }
}

pub(crate) enum HistoryResolution {
    Success,
    Interrupted,
    Failed,
    Unknown,
}

pub(crate) fn classify_history_status(status: &ComfyHistoryStatus) -> HistoryResolution {
    let status_str = status
        .status_str
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let messages = status
        .messages
        .as_ref()
        .map(|messages| messages.to_string().to_ascii_lowercase())
        .unwrap_or_default();

    if status_str.contains("interrupt") || messages.contains("execution_interrupted") {
        return HistoryResolution::Interrupted;
    }
    if status_str.contains("error")
        || status_str.contains("fail")
        || messages.contains("execution_error")
    {
        return HistoryResolution::Failed;
    }
    if status.completed == Some(false) {
        return HistoryResolution::Unknown;
    }
    if status.completed == Some(true)
        || status_str.contains("success")
        || status_str.contains("completed")
    {
        return HistoryResolution::Success;
    }
    HistoryResolution::Unknown
}

fn history_error(history: &ComfyHistory) -> TaskError {
    let interrupted = matches!(
        classify_history_status(&history.status),
        HistoryResolution::Interrupted
    );
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
            .unwrap_or_else(|| {
                if interrupted {
                    "ComfyUI history reported an interrupted execution".to_owned()
                } else {
                    "ComfyUI history reported an execution error".to_owned()
                }
            }),
        raw: history.status.messages.clone(),
    }
}

fn task_error_from_compile(error: &CompileError) -> TaskError {
    TaskError {
        code: error.code().to_owned(),
        message: error.to_string(),
        raw: None,
    }
}

fn task_error_from_adapter(error: &ComfyAdapterError) -> TaskError {
    let (code, message, raw) = match error {
        ComfyAdapterError::Offline(message) => ("COMFY_OFFLINE", message.clone(), None),
        ComfyAdapterError::Timeout(message) => ("COMFY_TIMEOUT", message.clone(), None),
        ComfyAdapterError::Incompatible(message) | ComfyAdapterError::Protocol(message) => {
            ("COMFY_PROTOCOL_ERROR", message.clone(), None)
        }
        ComfyAdapterError::WorkflowValidation {
            message,
            node_errors,
        } => (
            "WORKFLOW_VALIDATION_FAILED",
            message.clone(),
            Some(node_errors.clone()),
        ),
        ComfyAdapterError::StreamDisconnected(message) => {
            ("COMFY_STREAM_DISCONNECTED", message.clone(), None)
        }
        ComfyAdapterError::HistoryNotFound(message) => ("HISTORY_NOT_FOUND", message.clone(), None),
        ComfyAdapterError::OutputDownload(message) => {
            ("OUTPUT_DOWNLOAD_FAILED", message.clone(), None)
        }
        ComfyAdapterError::OutputTooLarge(message) => ("OUTPUT_TOO_LARGE", message.clone(), None),
        ComfyAdapterError::ImageUpload(message) => {
            ("COMFY_IMAGE_UPLOAD_FAILED", message.clone(), None)
        }
    };
    TaskError {
        code: code.to_owned(),
        message,
        raw,
    }
}

fn task_error_from_output(error: &GenerationServiceError) -> TaskError {
    let (code, message) = match error {
        GenerationServiceError::OutputCollection(error) => (error.code(), error.to_string()),
        GenerationServiceError::AssetImport(error) => (error.code(), error.to_string()),
        _ => ("OUTPUT_IMPORT_FAILED", error.to_string()),
    };
    TaskError {
        code: code.to_owned(),
        message,
        raw: None,
    }
}

fn task_error_from_input_prepare(error: &GenerationInputPrepareError) -> TaskError {
    TaskError {
        code: error.code().to_owned(),
        message: error.to_string(),
        raw: None,
    }
}

fn input_values_to_json(values: &BTreeMap<String, GenerationInputValue>) -> Value {
    let object = values
        .iter()
        .map(|(key, value)| (key.clone(), input_value_to_json(value)))
        .collect::<Map<_, _>>();
    Value::Object(object)
}

fn input_value_to_json(value: &GenerationInputValue) -> Value {
    match value {
        GenerationInputValue::Text(value) => Value::String(value.clone()),
        GenerationInputValue::Integer(value) => Value::Number(Number::from(*value)),
        GenerationInputValue::Seed(SeedValue::Random) => Value::String("random".to_owned()),
        GenerationInputValue::Seed(SeedValue::Fixed(value)) => Value::Number(Number::from(*value)),
        GenerationInputValue::ImageAsset(asset_id) => serde_json::json!({
            "type": "image_asset",
            "assetId": asset_id.as_str(),
        }),
    }
}

fn resolved_inputs_to_json(
    values: &BTreeMap<String, ResolvedInputValue>,
    prepared: &PreparedGenerationInputs,
) -> Value {
    let object = values
        .iter()
        .map(|(key, value)| {
            let value = match value {
                ResolvedInputValue::String(value) => Value::String(value.clone()),
                ResolvedInputValue::Integer(value) => Value::Number(Number::from(*value)),
                ResolvedInputValue::Seed(value) => Value::Number(Number::from(*value)),
                ResolvedInputValue::Image(_) => prepared
                    .images
                    .get(key)
                    .map(image_snapshot_value)
                    .unwrap_or_else(|| Value::String("image".to_owned())),
            };
            (key.clone(), value)
        })
        .collect::<Map<_, _>>();
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::{
        ComfyHistory, ComfyNodeOutput, ComfyOutputData, ComfyOutputFile,
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn output_failures_are_stable_task_error_codes() {
        let error = GenerationServiceError::OutputCollection(OutputCollectorError::OutputMissing {
            output_id: "generated_image".to_owned(),
            node_id: "9".to_owned(),
        });
        assert_eq!(task_error_from_output(&error).code, "OUTPUT_MISSING");

        let error = GenerationServiceError::AssetImport(AssetImportError::OutputImportFailed {
            message: "invalid png".to_owned(),
        });
        assert_eq!(task_error_from_output(&error).code, "OUTPUT_IMPORT_FAILED");
    }

    #[test]
    fn workflow_validation_node_errors_are_preserved_on_failed_task() {
        let node_errors = json!({
            "123": {
                "errors": [{
                    "type": "value_bigger_than_max",
                    "message": "Value 21 bigger than max 20"
                }]
            }
        });
        let adapter_error = ComfyAdapterError::WorkflowValidation {
            message: "Prompt outputs failed validation".to_owned(),
            node_errors: node_errors.clone(),
        };
        let task_error = task_error_from_adapter(&adapter_error);
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let mut task = Task::new("project-1", "workflow-1", "version-1", "recipe-1", now);

        task.fail(task_error, now + chrono::Duration::seconds(1))
            .expect("task should fail");

        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(
            task.error.expect("failed task should have error").raw,
            Some(node_errors)
        );
    }

    #[derive(Clone)]
    struct FakeClock {
        values: Arc<std::sync::Mutex<Vec<chrono::DateTime<Utc>>>>,
    }

    impl FakeClock {
        fn new(values: Vec<chrono::DateTime<Utc>>) -> Self {
            Self {
                values: Arc::new(std::sync::Mutex::new(values)),
            }
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> chrono::DateTime<Utc> {
            self.values
                .lock()
                .unwrap()
                .pop()
                .unwrap_or_else(|| Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap())
        }
    }

    #[test]
    fn monotonic_clock_prevents_backwards_and_equal_event_times() {
        let first = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
        let second = first - chrono::Duration::seconds(1);
        let source = Arc::new(FakeClock::new(vec![second, first, first]));
        let clock = MonotonicEventClock::new(source);
        let one = clock.now();
        let two = clock.now();
        let three = clock.now();
        assert!(two > one);
        assert!(three > two);
    }

    #[allow(dead_code)]
    fn _history_fixture() -> ComfyHistory {
        ComfyHistory {
            prompt_id: "prompt-1".to_owned(),
            status: Default::default(),
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

    #[allow(dead_code)]
    fn _output_fixture() -> ComfyOutputData {
        ComfyOutputData {
            bytes: Vec::new(),
            content_type: None,
        }
    }
}
