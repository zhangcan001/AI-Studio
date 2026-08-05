use crate::application::ports::{
    ComfyAdapter, ComfyAdapterError, ComfyExecutionEvent, GenerationDefinitionRepository,
    GenerationSnapshotRepository, RepositoryError, TaskRepository,
};
use crate::compiler::{CompileError, RecipeParser, WorkflowCompiler};
use crate::domain::{
    CompileRequest, GenerationSnapshot, InputValue, ResolvedInputValue, SeedValue, Task,
    TaskDomainError, TaskError, TaskStateMachine, TaskStatus,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{Map, Number, Value};
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct CreateGenerationRequest {
    pub project_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, InputValue>,
}

#[derive(Debug, PartialEq)]
pub enum GenerationServiceError {
    DefinitionNotFound {
        workflow_version_id: String,
        recipe_id: String,
    },
    Repository(RepositoryError),
    Compile(CompileError),
    Snapshot(String),
    Domain(TaskDomainError),
    Comfy(ComfyAdapterError),
    StreamDisconnected(String),
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
            Self::Snapshot(message) => write!(formatter, "SNAPSHOT_ERROR: {message}"),
            Self::Domain(error) => write!(formatter, "TASK_DOMAIN_ERROR: {error}"),
            Self::Comfy(error) => write!(formatter, "{error}"),
            Self::StreamDisconnected(message) => {
                write!(formatter, "COMFY_STREAM_DISCONNECTED: {message}")
            }
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

pub struct GenerationService {
    task_repository: Arc<dyn TaskRepository>,
    snapshot_repository: Arc<dyn GenerationSnapshotRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    comfy_adapter: Arc<dyn ComfyAdapter>,
    compiler: WorkflowCompiler,
}

impl GenerationService {
    pub fn new(
        task_repository: Arc<dyn TaskRepository>,
        snapshot_repository: Arc<dyn GenerationSnapshotRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        comfy_adapter: Arc<dyn ComfyAdapter>,
    ) -> Self {
        Self {
            task_repository,
            snapshot_repository,
            definition_repository,
            comfy_adapter,
            compiler: WorkflowCompiler,
        }
    }

    pub async fn execute(
        &self,
        request: CreateGenerationRequest,
        created_at: DateTime<Utc>,
    ) -> Result<Task, GenerationServiceError> {
        let definition = self
            .definition_repository
            .find(&request.workflow_version_id, &request.recipe_id)
            .await?
            .ok_or_else(|| GenerationServiceError::DefinitionNotFound {
                workflow_version_id: request.workflow_version_id.clone(),
                recipe_id: request.recipe_id.clone(),
            })?;

        let mut task = Task::new(
            request.project_id,
            definition.workflow_id.clone(),
            definition.workflow_version_id.clone(),
            definition.recipe_id.clone(),
            created_at,
        );
        let created_event = task.created_event();
        self.task_repository.create(&task, &created_event).await?;
        let mut clock = ServiceClock::new(created_at);

        self.transition_and_persist(&mut task, TaskStatus::Validating, &mut clock)
            .await?;

        let recipe = match RecipeParser::parse(&definition.recipe_yaml) {
            Ok(recipe) => recipe,
            Err(error) => {
                let error = CompileError::from(error);
                return Err(self
                    .fail_and_preserve(
                        &mut task,
                        task_error_from_compile(&error),
                        clock.next(),
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
                            clock.next(),
                            GenerationServiceError::Compile(error),
                        )
                        .await);
                }
            };
        let compile_request = CompileRequest::new(request.values.clone());
        let compile_result = match self.compiler.compile(&workflow, &recipe, &compile_request) {
            Ok(result) => result,
            Err(error) => {
                return Err(self
                    .fail_and_preserve(
                        &mut task,
                        task_error_from_compile(&error),
                        clock.next(),
                        GenerationServiceError::Compile(error),
                    )
                    .await);
            }
        };

        self.transition_and_persist(&mut task, TaskStatus::Preparing, &mut clock)
            .await?;

        let snapshot = match GenerationSnapshot::new(
            task.id.clone(),
            compile_result.workflow.clone(),
            definition.recipe_yaml.clone(),
            input_values_to_json(&request.values),
            resolved_inputs_to_json(&compile_result.resolved_inputs),
            clock.next(),
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
                        clock.next(),
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
                    clock.next(),
                    original,
                )
                .await);
        }

        let client_id = Uuid::new_v4().to_string();
        let prompt_id = Uuid::new_v4().to_string();
        let submission_event =
            task.prepare_submission(prompt_id.clone(), client_id.clone(), clock.next())?;
        self.task_repository
            .persist_runtime_update(&task, &submission_event)
            .await?;

        let mut subscription = match self.comfy_adapter.subscribe_events(&client_id).await {
            Ok(subscription) => subscription,
            Err(error) => {
                let original = GenerationServiceError::Comfy(error.clone());
                return Err(self
                    .fail_and_preserve(
                        &mut task,
                        task_error_from_adapter(&error),
                        clock.next(),
                        original,
                    )
                    .await);
            }
        };

        let submission = match self
            .comfy_adapter
            .submit_workflow(&client_id, &prompt_id, compile_result.workflow)
            .await
        {
            Ok(submission) => submission,
            Err(error) => {
                let original = GenerationServiceError::Comfy(error.clone());
                return Err(self
                    .fail_and_preserve(
                        &mut task,
                        task_error_from_adapter(&error),
                        clock.next(),
                        original,
                    )
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
                .fail_and_preserve(
                    &mut task,
                    task_error_from_adapter(&error),
                    clock.next(),
                    original,
                )
                .await);
        }

        task.set_queue_number(submission.number)?;
        let previous_status = task.status;
        let queued_event =
            TaskStateMachine::transition(&mut task, TaskStatus::Queued, clock.next())?;
        if let Err(error) = self
            .task_repository
            .persist_transition(&task, &queued_event, previous_status)
            .await
        {
            tracing::error!(
                task_id = %task.id,
                prompt_id = %prompt_id,
                client_id = %client_id,
                error = %error,
                "ComfyUI accepted workflow but QUEUED persistence failed; not retrying POST"
            );
            return Err(error.into());
        }

        loop {
            let event = match subscription.next_event().await {
                Ok(Some(event)) => event,
                Ok(None) => {
                    let message = "ComfyUI WebSocket closed after prompt submission".to_owned();
                    return Err(
                        match self
                            .persist_stream_disconnect(&mut task, message.clone(), &mut clock)
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
                            .persist_stream_disconnect(&mut task, error.to_string(), &mut clock)
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
                        self.transition_and_persist(&mut task, TaskStatus::Running, &mut clock)
                            .await?;
                    }
                }
                ComfyExecutionEvent::NodeStarted { node_id, .. } => {
                    if task.status == TaskStatus::Running {
                        let event = task.update_node_progress(node_id, clock.next())?;
                        self.task_repository
                            .persist_runtime_update(&task, &event)
                            .await?;
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
                            task.update_step_progress(current, total, node_id, clock.next())?
                        {
                            self.task_repository
                                .persist_runtime_update(&task, &event)
                                .await?;
                        }
                    }
                }
                ComfyExecutionEvent::ExecutionSucceeded { .. } => {
                    if task.status == TaskStatus::Running {
                        self.transition_and_persist(&mut task, TaskStatus::Collecting, &mut clock)
                            .await?;
                        return Ok(task);
                    }
                }
                ComfyExecutionEvent::ExecutionError {
                    node_id,
                    message,
                    raw,
                    ..
                } => {
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
                            clock.next(),
                            original,
                        )
                        .await);
                }
                ComfyExecutionEvent::ExecutionInterrupted { node_id, raw, .. } => {
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
                            clock.next(),
                            original,
                        )
                        .await);
                }
            }
        }
    }

    async fn transition_and_persist(
        &self,
        task: &mut Task,
        target: TaskStatus,
        clock: &mut ServiceClock,
    ) -> Result<(), GenerationServiceError> {
        let previous_status = task.status;
        let event = TaskStateMachine::transition(task, target, clock.next())?;
        self.task_repository
            .persist_transition(task, &event, previous_status)
            .await?;
        Ok(())
    }

    async fn fail_and_preserve(
        &self,
        task: &mut Task,
        error: TaskError,
        at: DateTime<Utc>,
        original: GenerationServiceError,
    ) -> GenerationServiceError {
        let previous_status = task.status;
        let event = match task.fail(error, at) {
            Ok(event) => event,
            Err(error) => return GenerationServiceError::Domain(error),
        };
        match self
            .task_repository
            .persist_transition(task, &event, previous_status)
            .await
        {
            Ok(_) => original,
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
        clock: &mut ServiceClock,
    ) -> Result<(), GenerationServiceError> {
        let event = match task.record_stream_disconnected(message, clock.next()) {
            Ok(event) => event,
            Err(error) => return Err(GenerationServiceError::Domain(error)),
        };
        match self
            .task_repository
            .persist_runtime_update(task, &event)
            .await
        {
            Ok(_) => Ok(()),
            Err(error) => Err(GenerationServiceError::Repository(error)),
        }
    }
}

struct ServiceClock {
    current: DateTime<Utc>,
}

impl ServiceClock {
    fn new(current: DateTime<Utc>) -> Self {
        Self { current }
    }

    fn next(&mut self) -> DateTime<Utc> {
        self.current += Duration::microseconds(1);
        self.current
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
        ComfyAdapterError::Incompatible(message) => ("COMFY_PROTOCOL_ERROR", message.clone(), None),
        ComfyAdapterError::Protocol(message) => ("COMFY_PROTOCOL_ERROR", message.clone(), None),
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
    };
    TaskError {
        code: code.to_owned(),
        message,
        raw,
    }
}

fn input_values_to_json(values: &BTreeMap<String, InputValue>) -> Value {
    let object = values
        .iter()
        .map(|(key, value)| (key.clone(), input_value_to_json(value)))
        .collect::<Map<_, _>>();
    Value::Object(object)
}

fn input_value_to_json(value: &InputValue) -> Value {
    match value {
        InputValue::String(value) => Value::String(value.clone()),
        InputValue::Integer(value) => Value::Number(Number::from(*value)),
        InputValue::Seed(SeedValue::Random) => Value::String("random".to_owned()),
        InputValue::Seed(SeedValue::Fixed(value)) => Value::Number(Number::from(*value)),
    }
}

fn resolved_inputs_to_json(values: &BTreeMap<String, ResolvedInputValue>) -> Value {
    let object = values
        .iter()
        .map(|(key, value)| {
            let value = match value {
                ResolvedInputValue::String(value) => Value::String(value.clone()),
                ResolvedInputValue::Integer(value) => Value::Number(Number::from(*value)),
                ResolvedInputValue::Seed(value) => Value::Number(Number::from(*value)),
            };
            (key.clone(), value)
        })
        .collect::<Map<_, _>>();
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use super::{CreateGenerationRequest, GenerationService, GenerationServiceError};
    use crate::application::ports::{
        ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth,
        GenerationDefinition, GenerationDefinitionRepository, GenerationSnapshotRepository,
        PromptSubmission, RepositoryError, TaskRepository,
    };
    use crate::domain::{
        GenerationSnapshot, InputValue, NewTaskEvent, SeedValue, StoredTaskEvent, Task, TaskId,
        TaskStatus,
    };
    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};
    use serde_json::{json, Value};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    const RECIPE_YAML: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_t2i/recipe.yaml"
    ));
    const WORKFLOW_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_t2i/workflow_api.json"
    ));

    type SharedLog = Arc<Mutex<Vec<String>>>;

    #[derive(Clone, Default)]
    struct FakeTaskRepository {
        state: Arc<Mutex<FakeTaskState>>,
        log: SharedLog,
    }

    #[derive(Default)]
    struct FakeTaskState {
        task: Option<Task>,
        events: Vec<StoredTaskEvent>,
    }

    impl FakeTaskRepository {
        fn new(log: SharedLog) -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeTaskState::default())),
                log,
            }
        }

        fn task(&self) -> Task {
            self.state
                .lock()
                .expect("task state lock")
                .task
                .clone()
                .expect("task should be stored")
        }

        fn events(&self) -> Vec<StoredTaskEvent> {
            self.state.lock().expect("task state lock").events.clone()
        }

        fn store_event(state: &mut FakeTaskState, event: &NewTaskEvent) -> StoredTaskEvent {
            let stored = StoredTaskEvent {
                id: event.id.clone(),
                task_id: event.task_id.clone(),
                sequence: state.events.len() as u64 + 1,
                event_type: event.event_type,
                payload: event.payload.clone(),
                created_at: event.created_at,
            };
            state.events.push(stored.clone());
            stored
        }
    }

    #[async_trait]
    impl TaskRepository for FakeTaskRepository {
        async fn create(
            &self,
            task: &Task,
            created_event: &NewTaskEvent,
        ) -> Result<StoredTaskEvent, RepositoryError> {
            self.log
                .lock()
                .expect("log lock")
                .push("task_create".to_owned());
            let mut state = self.state.lock().expect("task state lock");
            if state.task.is_some() {
                return Err(RepositoryError::integrity("duplicate fake task"));
            }
            state.task = Some(task.clone());
            Ok(Self::store_event(&mut state, created_event))
        }

        async fn persist_transition(
            &self,
            task: &Task,
            event: &NewTaskEvent,
            expected_previous_status: TaskStatus,
        ) -> Result<StoredTaskEvent, RepositoryError> {
            self.log
                .lock()
                .expect("log lock")
                .push(format!("transition_{}", task.status.as_str()));
            let mut state = self.state.lock().expect("task state lock");
            let stored_task = state
                .task
                .as_ref()
                .ok_or_else(|| RepositoryError::not_found("task", task.id.as_str()))?;
            if stored_task.status != expected_previous_status {
                return Err(RepositoryError::integrity("stale fake transition"));
            }
            state.task = Some(task.clone());
            Ok(Self::store_event(&mut state, event))
        }

        async fn persist_runtime_update(
            &self,
            task: &Task,
            event: &NewTaskEvent,
        ) -> Result<StoredTaskEvent, RepositoryError> {
            self.log
                .lock()
                .expect("log lock")
                .push(format!("runtime_{}", event.event_type.as_str()));
            let mut state = self.state.lock().expect("task state lock");
            let stored_task = state
                .task
                .as_ref()
                .ok_or_else(|| RepositoryError::not_found("task", task.id.as_str()))?;
            if stored_task.status != task.status {
                return Err(RepositoryError::integrity("stale fake runtime update"));
            }
            state.task = Some(task.clone());
            Ok(Self::store_event(&mut state, event))
        }

        async fn find_by_id(&self, task_id: &TaskId) -> Result<Option<Task>, RepositoryError> {
            Ok(self
                .state
                .lock()
                .expect("task state lock")
                .task
                .clone()
                .filter(|task| task.id == *task_id))
        }

        async fn list_recent(&self, _limit: u32) -> Result<Vec<Task>, RepositoryError> {
            Ok(self
                .state
                .lock()
                .expect("task state lock")
                .task
                .clone()
                .into_iter()
                .collect())
        }

        async fn list_events(
            &self,
            task_id: &TaskId,
        ) -> Result<Vec<StoredTaskEvent>, RepositoryError> {
            Ok(self
                .events()
                .into_iter()
                .filter(|event| event.task_id == *task_id)
                .collect())
        }
    }

    #[derive(Clone, Default)]
    struct FakeSnapshotRepository {
        snapshots: Arc<Mutex<Vec<GenerationSnapshot>>>,
        log: SharedLog,
    }

    #[async_trait]
    impl GenerationSnapshotRepository for FakeSnapshotRepository {
        async fn insert(&self, snapshot: &GenerationSnapshot) -> Result<(), RepositoryError> {
            self.log
                .lock()
                .expect("log lock")
                .push("snapshot_insert".to_owned());
            self.snapshots
                .lock()
                .expect("snapshot lock")
                .push(snapshot.clone());
            Ok(())
        }

        async fn find_by_task_id(
            &self,
            task_id: &TaskId,
        ) -> Result<Option<GenerationSnapshot>, RepositoryError> {
            Ok(self
                .snapshots
                .lock()
                .expect("snapshot lock")
                .iter()
                .find(|snapshot| snapshot.task_id == *task_id)
                .cloned())
        }
    }

    #[derive(Clone)]
    struct FakeDefinitionRepository {
        definition: Option<GenerationDefinition>,
    }

    #[async_trait]
    impl GenerationDefinitionRepository for FakeDefinitionRepository {
        async fn find(
            &self,
            _workflow_version_id: &str,
            _recipe_id: &str,
        ) -> Result<Option<GenerationDefinition>, RepositoryError> {
            Ok(self.definition.clone())
        }
    }

    struct FakeSubscription {
        events: VecDeque<Result<Option<ComfyExecutionEvent>, ComfyAdapterError>>,
        prompt_id: Arc<Mutex<Option<String>>>,
    }

    #[async_trait]
    impl ComfyEventSubscription for FakeSubscription {
        async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
            let event = self.events.pop_front().unwrap_or(Ok(None))?;
            Ok(event.map(|event| replace_current_prompt_id(event, &self.prompt_id)))
        }
    }

    #[derive(Clone)]
    struct FakeComfyAdapter {
        events: VecDeque<Result<Option<ComfyExecutionEvent>, ComfyAdapterError>>,
        subscribe_error: Option<ComfyAdapterError>,
        submit_error: Option<ComfyAdapterError>,
        prompt_id: Arc<Mutex<Option<String>>>,
        log: SharedLog,
        submit_calls: Arc<Mutex<Vec<(String, String, Value)>>>,
    }

    impl FakeComfyAdapter {
        fn happy(events: Vec<ComfyExecutionEvent>, log: SharedLog) -> Self {
            Self {
                events: events.into_iter().map(|event| Ok(Some(event))).collect(),
                subscribe_error: None,
                submit_error: None,
                prompt_id: Arc::new(Mutex::new(None)),
                log,
                submit_calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_submit_error(error: ComfyAdapterError, log: SharedLog) -> Self {
            let mut adapter = Self::happy(Vec::new(), log);
            adapter.submit_error = Some(error);
            adapter
        }

        fn with_subscribe_error(error: ComfyAdapterError, log: SharedLog) -> Self {
            let mut adapter = Self::happy(Vec::new(), log);
            adapter.subscribe_error = Some(error);
            adapter
        }

        fn submit_count(&self) -> usize {
            self.submit_calls.lock().expect("submit lock").len()
        }
    }

    #[async_trait]
    impl ComfyAdapter for FakeComfyAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_system_stats(
            &self,
        ) -> Result<crate::application::ports::SystemStats, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn submit_workflow(
            &self,
            client_id: &str,
            prompt_id: &str,
            workflow: Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            self.log
                .lock()
                .expect("log lock")
                .push("submit_workflow".to_owned());
            self.submit_calls.lock().expect("submit lock").push((
                client_id.to_owned(),
                prompt_id.to_owned(),
                workflow,
            ));
            *self.prompt_id.lock().expect("prompt lock") = Some(prompt_id.to_owned());
            if let Some(error) = &self.submit_error {
                return Err(error.clone());
            }
            Ok(PromptSubmission {
                prompt_id: prompt_id.to_owned(),
                number: Some(11),
                node_errors: json!({}),
            })
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            self.log
                .lock()
                .expect("log lock")
                .push("subscribe_events".to_owned());
            if let Some(error) = &self.subscribe_error {
                return Err(error.clone());
            }
            Ok(Box::new(FakeSubscription {
                events: self.events.clone(),
                prompt_id: self.prompt_id.clone(),
            }))
        }
    }

    fn definition(recipe_yaml: &str) -> GenerationDefinition {
        GenerationDefinition {
            workflow_id: "workflow-1".to_owned(),
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            workflow_json: serde_json::from_str(WORKFLOW_JSON).expect("workflow fixture"),
            recipe_yaml: recipe_yaml.to_owned(),
        }
    }

    fn request() -> CreateGenerationRequest {
        CreateGenerationRequest {
            project_id: "project-1".to_owned(),
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values: std::collections::BTreeMap::from([
                ("prompt".to_owned(), InputValue::String("hello".to_owned())),
                ("steps".to_owned(), InputValue::Integer(20)),
                (
                    "seed".to_owned(),
                    InputValue::Seed(SeedValue::Fixed(123_456)),
                ),
            ]),
        }
    }

    fn current_event(event: ComfyExecutionEvent) -> ComfyExecutionEvent {
        event
    }

    fn base_time() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn replace_current_prompt_id(
        event: ComfyExecutionEvent,
        prompt_id: &Arc<Mutex<Option<String>>>,
    ) -> ComfyExecutionEvent {
        let Some(prompt_id) = prompt_id.lock().expect("prompt lock").clone() else {
            return event;
        };
        match event {
            ComfyExecutionEvent::ExecutionStarted { prompt_id: value } if value == "CURRENT" => {
                ComfyExecutionEvent::ExecutionStarted { prompt_id }
            }
            ComfyExecutionEvent::NodeStarted {
                prompt_id: value,
                node_id,
            } if value == "CURRENT" => ComfyExecutionEvent::NodeStarted { prompt_id, node_id },
            ComfyExecutionEvent::Progress {
                prompt_id: value,
                node_id,
                current,
                total,
            } if value == "CURRENT" => ComfyExecutionEvent::Progress {
                prompt_id,
                node_id,
                current,
                total,
            },
            ComfyExecutionEvent::ExecutionSucceeded { prompt_id: value } if value == "CURRENT" => {
                ComfyExecutionEvent::ExecutionSucceeded { prompt_id }
            }
            ComfyExecutionEvent::ExecutionError {
                prompt_id: value,
                node_id,
                message,
                raw,
            } if value == "CURRENT" => ComfyExecutionEvent::ExecutionError {
                prompt_id,
                node_id,
                message,
                raw,
            },
            ComfyExecutionEvent::ExecutionInterrupted {
                prompt_id: value,
                node_id,
                raw,
            } if value == "CURRENT" => ComfyExecutionEvent::ExecutionInterrupted {
                prompt_id,
                node_id,
                raw,
            },
            event => event,
        }
    }

    fn service(
        definition: GenerationDefinition,
        adapter: FakeComfyAdapter,
        log: SharedLog,
    ) -> (
        GenerationService,
        FakeTaskRepository,
        FakeSnapshotRepository,
    ) {
        let task_repository = FakeTaskRepository::new(log.clone());
        let snapshot_repository = FakeSnapshotRepository {
            snapshots: Arc::new(Mutex::new(Vec::new())),
            log: log.clone(),
        };
        let definition_repository = FakeDefinitionRepository {
            definition: Some(definition),
        };
        let service = GenerationService::new(
            Arc::new(task_repository.clone()),
            Arc::new(snapshot_repository.clone()),
            Arc::new(definition_repository),
            Arc::new(adapter),
        );
        (service, task_repository, snapshot_repository)
    }

    #[tokio::test]
    async fn happy_path_persists_snapshot_and_stops_at_collecting() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let adapter = FakeComfyAdapter::happy(
            vec![
                current_event(ComfyExecutionEvent::Progress {
                    prompt_id: "OTHER".to_owned(),
                    node_id: Some("other".to_owned()),
                    current: 999,
                    total: 999,
                }),
                current_event(ComfyExecutionEvent::ExecutionStarted {
                    prompt_id: "CURRENT".to_owned(),
                }),
                current_event(ComfyExecutionEvent::NodeStarted {
                    prompt_id: "CURRENT".to_owned(),
                    node_id: "3".to_owned(),
                }),
                current_event(ComfyExecutionEvent::Progress {
                    prompt_id: "CURRENT".to_owned(),
                    node_id: Some("3".to_owned()),
                    current: 1,
                    total: 20,
                }),
                current_event(ComfyExecutionEvent::Progress {
                    prompt_id: "CURRENT".to_owned(),
                    node_id: Some("3".to_owned()),
                    current: 1,
                    total: 20,
                }),
                current_event(ComfyExecutionEvent::Progress {
                    prompt_id: "CURRENT".to_owned(),
                    node_id: Some("3".to_owned()),
                    current: 20,
                    total: 20,
                }),
                current_event(ComfyExecutionEvent::NodeStarted {
                    prompt_id: "CURRENT".to_owned(),
                    node_id: "9".to_owned(),
                }),
                current_event(ComfyExecutionEvent::ExecutionSucceeded {
                    prompt_id: "CURRENT".to_owned(),
                }),
            ],
            log.clone(),
        );
        let submit_calls = adapter.submit_calls.clone();
        let (service, task_repository, snapshot_repository) =
            service(definition(RECIPE_YAML), adapter, log.clone());

        let task = service
            .execute(request(), base_time())
            .await
            .expect("happy path should complete");
        assert_eq!(task.status, TaskStatus::Collecting);
        assert!(task.error.is_none());
        assert!(task.prompt_id.is_some());
        assert_eq!(task.queue_number, Some(11));

        let events = task_repository.events();
        let event_types = events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![
                "TASK_CREATED",
                "TASK_VALIDATING",
                "TASK_PREPARING",
                "TASK_SUBMISSION_PREPARED",
                "TASK_QUEUED",
                "TASK_RUNNING",
                "TASK_NODE_STARTED",
                "TASK_PROGRESS_UPDATED",
                "TASK_PROGRESS_UPDATED",
                "TASK_NODE_STARTED",
                "TASK_COLLECTING",
            ]
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type.as_str() == "TASK_PROGRESS_UPDATED")
                .count(),
            2
        );
        assert_eq!(
            snapshot_repository
                .snapshots
                .lock()
                .expect("snapshot lock")
                .len(),
            1
        );
        let snapshot = snapshot_repository.snapshots.lock().expect("snapshot lock")[0].clone();
        assert_eq!(snapshot.user_inputs_json["seed"], 123_456);
        assert_eq!(snapshot.resolved_inputs_json["seed"], 123_456);

        let calls = log.lock().expect("log lock");
        let snapshot_index = calls
            .iter()
            .position(|call| call == "snapshot_insert")
            .unwrap();
        let subscribe_index = calls
            .iter()
            .position(|call| call == "subscribe_events")
            .unwrap();
        let submit_index = calls
            .iter()
            .position(|call| call == "submit_workflow")
            .unwrap();
        assert!(snapshot_index < subscribe_index && subscribe_index < submit_index);
        let submission_prompt_id = &submit_calls.lock().expect("submit lock")[0].1;
        assert_eq!(
            task.prompt_id.as_deref(),
            Some(submission_prompt_id.as_str())
        );
    }

    #[tokio::test]
    async fn compile_failure_fails_task_without_snapshot_or_submit() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let adapter = FakeComfyAdapter::happy(Vec::new(), log.clone());
        let submit_calls = adapter.submit_calls.clone();
        let (service, task_repository, snapshot_repository) =
            service(definition("not valid recipe"), adapter, log);

        let error = service
            .execute(request(), base_time())
            .await
            .expect_err("compile failure should be returned");
        assert!(matches!(error, GenerationServiceError::Compile(_)));
        assert_eq!(task_repository.task().status, TaskStatus::Failed);
        assert_eq!(
            task_repository
                .task()
                .error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("RECIPE_PARSE_ERROR")
        );
        assert!(snapshot_repository
            .snapshots
            .lock()
            .expect("snapshot lock")
            .is_empty());
        assert_eq!(submit_calls.lock().expect("submit lock").len(), 0);
    }

    #[tokio::test]
    async fn workflow_validation_failure_keeps_snapshot_and_fails_task() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let adapter = FakeComfyAdapter::with_submit_error(
            ComfyAdapterError::WorkflowValidation {
                message: "missing model".to_owned(),
                node_errors: json!({"3": {"errors": ["model missing"]}}),
            },
            log.clone(),
        );
        let (service, task_repository, snapshot_repository) =
            service(definition(RECIPE_YAML), adapter, log);

        let error = service
            .execute(request(), base_time())
            .await
            .expect_err("workflow validation should be returned");
        assert!(matches!(
            error,
            GenerationServiceError::Comfy(ComfyAdapterError::WorkflowValidation { .. })
        ));
        assert_eq!(task_repository.task().status, TaskStatus::Failed);
        assert_eq!(
            task_repository
                .task()
                .error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("WORKFLOW_VALIDATION_FAILED")
        );
        assert_eq!(
            snapshot_repository
                .snapshots
                .lock()
                .expect("snapshot lock")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn websocket_connect_failure_happens_before_post_and_fails_task() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let adapter = FakeComfyAdapter::with_subscribe_error(
            ComfyAdapterError::StreamDisconnected("connection refused".to_owned()),
            log.clone(),
        );
        let submit_calls = adapter.submit_calls.clone();
        let (service, task_repository, _snapshot_repository) =
            service(definition(RECIPE_YAML), adapter, log);

        let error = service
            .execute(request(), base_time())
            .await
            .expect_err("websocket connection failure should be returned");
        assert!(matches!(
            error,
            GenerationServiceError::Comfy(ComfyAdapterError::StreamDisconnected(_))
        ));
        assert_eq!(task_repository.task().status, TaskStatus::Failed);
        assert_eq!(submit_calls.lock().expect("submit lock").len(), 0);
    }

    #[tokio::test]
    async fn post_submit_disconnect_keeps_running_task_and_records_event() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut adapter = FakeComfyAdapter::happy(
            vec![current_event(ComfyExecutionEvent::ExecutionStarted {
                prompt_id: "CURRENT".to_owned(),
            })],
            log,
        );
        adapter
            .events
            .push_back(Err(ComfyAdapterError::StreamDisconnected(
                "socket lost".to_owned(),
            )));
        let (service, task_repository, _snapshot_repository) = service(
            definition(RECIPE_YAML),
            adapter,
            Arc::new(Mutex::new(Vec::new())),
        );

        let error = service
            .execute(request(), base_time())
            .await
            .expect_err("post-submit disconnect should be surfaced");
        assert!(matches!(
            error,
            GenerationServiceError::StreamDisconnected(_)
        ));
        assert_eq!(task_repository.task().status, TaskStatus::Running);
        assert!(task_repository
            .events()
            .iter()
            .any(|event| event.event_type.as_str() == "TASK_STREAM_DISCONNECTED"));
        assert!(task_repository.task().error.is_none());
    }

    #[tokio::test]
    async fn execution_error_is_failed_and_preserves_raw_event() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let adapter = FakeComfyAdapter::happy(
            vec![
                current_event(ComfyExecutionEvent::ExecutionStarted {
                    prompt_id: "CURRENT".to_owned(),
                }),
                current_event(ComfyExecutionEvent::ExecutionError {
                    prompt_id: "CURRENT".to_owned(),
                    node_id: Some("3".to_owned()),
                    message: "CUDA out of memory".to_owned(),
                    raw: json!({"exception_type": "OOM"}),
                }),
            ],
            log.clone(),
        );
        let (service, task_repository, _snapshot_repository) =
            service(definition(RECIPE_YAML), adapter, log);

        let error = service
            .execute(request(), base_time())
            .await
            .expect_err("execution error should be returned");
        assert!(matches!(
            error,
            GenerationServiceError::ExecutionFailed { ref code, .. } if code == "EXECUTION_ERROR"
        ));
        assert_eq!(task_repository.task().status, TaskStatus::Failed);
        assert_eq!(
            task_repository.task().error.as_ref().unwrap().raw,
            Some(json!({"exception_type": "OOM"}))
        );
    }

    #[tokio::test]
    async fn execution_interrupted_is_failed_with_dedicated_error_code() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let adapter = FakeComfyAdapter::happy(
            vec![
                current_event(ComfyExecutionEvent::ExecutionStarted {
                    prompt_id: "CURRENT".to_owned(),
                }),
                current_event(ComfyExecutionEvent::ExecutionInterrupted {
                    prompt_id: "CURRENT".to_owned(),
                    node_id: Some("3".to_owned()),
                    raw: json!({"reason": "user"}),
                }),
            ],
            log.clone(),
        );
        let (service, task_repository, _snapshot_repository) =
            service(definition(RECIPE_YAML), adapter, log);

        let error = service
            .execute(request(), base_time())
            .await
            .expect_err("interrupted execution should be returned");
        assert!(matches!(
            error,
            GenerationServiceError::ExecutionFailed { ref code, .. }
                if code == "EXECUTION_INTERRUPTED"
        ));
        assert_eq!(task_repository.task().status, TaskStatus::Failed);
        assert_eq!(
            task_repository
                .task()
                .error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("EXECUTION_INTERRUPTED")
        );
    }
}
