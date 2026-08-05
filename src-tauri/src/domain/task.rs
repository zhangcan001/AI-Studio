use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    pub fn new() -> Self {
        Self(format!("tsk_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, TaskDomainError> {
        let value = value.into();
        if value.starts_with("tsk_") && value.len() > "tsk_".len() {
            Ok(Self(value))
        } else {
            Err(TaskDomainError::invalid_id("task", value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    Created,
    Validating,
    Preparing,
    Queued,
    Running,
    Collecting,
    Succeeded,
    Failed,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Validating => "VALIDATING",
            Self::Preparing => "PREPARING",
            Self::Queued => "QUEUED",
            Self::Running => "RUNNING",
            Self::Collecting => "COLLECTING",
            Self::Succeeded => "SUCCEEDED",
            Self::Failed => "FAILED",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, TaskDomainError> {
        match value {
            "CREATED" => Ok(Self::Created),
            "VALIDATING" => Ok(Self::Validating),
            "PREPARING" => Ok(Self::Preparing),
            "QUEUED" => Ok(Self::Queued),
            "RUNNING" => Ok(Self::Running),
            "COLLECTING" => Ok(Self::Collecting),
            "SUCCEEDED" => Ok(Self::Succeeded),
            "FAILED" => Ok(Self::Failed),
            _ => Err(TaskDomainError::invalid_status(value)),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }

    pub(crate) fn event_type(self) -> Option<TaskEventType> {
        match self {
            Self::Created => Some(TaskEventType::TaskCreated),
            Self::Validating => Some(TaskEventType::TaskValidating),
            Self::Preparing => Some(TaskEventType::TaskPreparing),
            Self::Queued => Some(TaskEventType::TaskQueued),
            Self::Running => Some(TaskEventType::TaskRunning),
            Self::Collecting => Some(TaskEventType::TaskCollecting),
            Self::Succeeded => Some(TaskEventType::TaskSucceeded),
            Self::Failed => Some(TaskEventType::TaskFailed),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskProgress {
    Indeterminate,
    Node {
        node_id: String,
    },
    Step {
        current: u64,
        total: u64,
        node_id: Option<String>,
    },
}

impl TaskProgress {
    pub fn node(node_id: impl Into<String>) -> Result<Self, TaskDomainError> {
        let node_id = node_id.into();
        if node_id.trim().is_empty() {
            return Err(TaskDomainError::invalid_progress(
                "node progress requires a non-empty node_id",
            ));
        }
        Ok(Self::Node { node_id })
    }

    pub fn step(
        current: u64,
        total: u64,
        node_id: Option<String>,
    ) -> Result<Self, TaskDomainError> {
        if total == 0 {
            return Err(TaskDomainError::invalid_progress(
                "step progress total must be greater than zero",
            ));
        }
        if current > total {
            return Err(TaskDomainError::invalid_progress(format!(
                "step progress current {current} cannot exceed total {total}"
            )));
        }
        Ok(Self::Step {
            current,
            total,
            node_id,
        })
    }

    pub fn mode(&self) -> &'static str {
        match self {
            Self::Indeterminate => "indeterminate",
            Self::Node { .. } => "node",
            Self::Step { .. } => "step",
        }
    }

    pub fn validate(&self) -> Result<(), TaskDomainError> {
        match self {
            Self::Indeterminate => Ok(()),
            Self::Node { node_id } if node_id.trim().is_empty() => Err(
                TaskDomainError::invalid_progress("node progress requires a non-empty node_id"),
            ),
            Self::Node { .. } => Ok(()),
            Self::Step { current, total, .. } => {
                if *total == 0 || current > total {
                    return Err(TaskDomainError::invalid_progress(format!(
                        "invalid step progress {current}/{total}"
                    )));
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskError {
    pub code: String,
    pub message: String,
    pub raw: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: TaskId,
    pub project_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub status: TaskStatus,
    pub prompt_id: Option<String>,
    pub queue_number: Option<i64>,
    pub progress: TaskProgress,
    pub current_node_id: Option<String>,
    pub error: Option<TaskError>,
    pub created_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl Task {
    pub fn new(
        project_id: impl Into<String>,
        workflow_id: impl Into<String>,
        workflow_version_id: impl Into<String>,
        recipe_id: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: TaskId::new(),
            project_id: project_id.into(),
            workflow_id: workflow_id.into(),
            workflow_version_id: workflow_version_id.into(),
            recipe_id: recipe_id.into(),
            status: TaskStatus::Created,
            prompt_id: None,
            queue_number: None,
            progress: TaskProgress::Indeterminate,
            current_node_id: None,
            error: None,
            created_at,
            queued_at: None,
            started_at: None,
            finished_at: None,
        }
    }

    pub fn created_event(&self) -> NewTaskEvent {
        NewTaskEvent {
            id: new_event_id(),
            task_id: self.id.clone(),
            event_type: TaskEventType::TaskCreated,
            payload: None,
            created_at: self.created_at,
        }
    }

    pub fn prepare_submission(
        &mut self,
        prompt_id: impl Into<String>,
        client_id: impl Into<String>,
        at: DateTime<Utc>,
    ) -> Result<NewTaskEvent, TaskDomainError> {
        if self.status != TaskStatus::Preparing {
            return Err(TaskDomainError::invalid_transition(
                self.status,
                TaskStatus::Preparing,
            ));
        }
        if self.prompt_id.is_some() {
            return Err(TaskDomainError::invalid_task(
                "submission prompt_id has already been prepared",
            ));
        }
        if at < self.created_at {
            return Err(TaskDomainError::invalid_timestamp(
                "submission preparation time must not precede created_at",
            ));
        }

        let prompt_id = prompt_id.into();
        let client_id = client_id.into();
        if prompt_id.trim().is_empty() || client_id.trim().is_empty() {
            return Err(TaskDomainError::invalid_task(
                "submission prompt_id and client_id must not be empty",
            ));
        }

        let mut next = self.clone();
        next.prompt_id = Some(prompt_id.clone());
        next.validate()?;
        *self = next;

        Ok(NewTaskEvent {
            id: new_event_id(),
            task_id: self.id.clone(),
            event_type: TaskEventType::TaskSubmissionPrepared,
            payload: Some(serde_json::json!({
                "promptId": prompt_id,
                "clientId": client_id,
            })),
            created_at: at,
        })
    }

    pub fn set_queue_number(&mut self, queue_number: Option<i64>) -> Result<(), TaskDomainError> {
        if !matches!(self.status, TaskStatus::Preparing | TaskStatus::Queued) {
            return Err(TaskDomainError::invalid_task(format!(
                "queue_number cannot be changed while task is {}",
                self.status.as_str()
            )));
        }
        if queue_number.is_some_and(|number| number < 0) {
            return Err(TaskDomainError::invalid_task(
                "queue_number must not be negative",
            ));
        }

        let mut next = self.clone();
        next.queue_number = queue_number;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn update_node_progress(
        &mut self,
        node_id: impl Into<String>,
        at: DateTime<Utc>,
    ) -> Result<NewTaskEvent, TaskDomainError> {
        if self.status != TaskStatus::Running {
            return Err(TaskDomainError::invalid_task(
                "node progress requires a RUNNING task",
            ));
        }
        if at < self.created_at {
            return Err(TaskDomainError::invalid_timestamp(
                "node progress time must not precede created_at",
            ));
        }

        let node_id = node_id.into();
        let progress = TaskProgress::node(node_id.clone())?;
        let mut next = self.clone();
        next.progress = progress;
        next.current_node_id = Some(node_id.clone());
        next.validate()?;
        *self = next;

        Ok(NewTaskEvent {
            id: new_event_id(),
            task_id: self.id.clone(),
            event_type: TaskEventType::TaskNodeStarted,
            payload: Some(serde_json::json!({"nodeId": node_id})),
            created_at: at,
        })
    }

    pub fn update_step_progress(
        &mut self,
        current: u64,
        total: u64,
        node_id: Option<String>,
        at: DateTime<Utc>,
    ) -> Result<Option<NewTaskEvent>, TaskDomainError> {
        if self.status != TaskStatus::Running {
            return Err(TaskDomainError::invalid_task(
                "step progress requires a RUNNING task",
            ));
        }
        if at < self.created_at {
            return Err(TaskDomainError::invalid_timestamp(
                "step progress time must not precede created_at",
            ));
        }

        let progress = TaskProgress::step(current, total, node_id.clone())?;
        if self.progress == progress && self.current_node_id == node_id {
            return Ok(None);
        }

        let mut next = self.clone();
        next.progress = progress;
        next.current_node_id = node_id.clone();
        next.validate()?;
        *self = next;

        Ok(Some(NewTaskEvent {
            id: new_event_id(),
            task_id: self.id.clone(),
            event_type: TaskEventType::TaskProgressUpdated,
            payload: Some(serde_json::json!({
                "current": current,
                "total": total,
                "nodeId": node_id,
            })),
            created_at: at,
        }))
    }

    pub fn record_stream_disconnected(
        &mut self,
        message: impl Into<String>,
        at: DateTime<Utc>,
    ) -> Result<NewTaskEvent, TaskDomainError> {
        if !matches!(
            self.status,
            TaskStatus::Queued | TaskStatus::Running | TaskStatus::Collecting
        ) {
            return Err(TaskDomainError::invalid_task(
                "stream disconnection requires an active submitted task",
            ));
        }
        if at < self.created_at {
            return Err(TaskDomainError::invalid_timestamp(
                "stream disconnection time must not precede created_at",
            ));
        }

        let message = message.into();
        if message.trim().is_empty() {
            return Err(TaskDomainError::invalid_task(
                "stream disconnection message must not be empty",
            ));
        }

        Ok(NewTaskEvent {
            id: new_event_id(),
            task_id: self.id.clone(),
            event_type: TaskEventType::TaskStreamDisconnected,
            payload: Some(serde_json::json!({"message": message})),
            created_at: at,
        })
    }

    pub fn fail(
        &mut self,
        error: TaskError,
        at: DateTime<Utc>,
    ) -> Result<NewTaskEvent, TaskDomainError> {
        if error.code.trim().is_empty() || error.message.trim().is_empty() {
            return Err(TaskDomainError::invalid_task(
                "failed task requires a non-empty error code and message",
            ));
        }

        let mut next = self.clone();
        next.error = Some(error);
        let event = TaskStateMachine::transition(&mut next, TaskStatus::Failed, at)?;
        *self = next;
        Ok(event)
    }

    pub fn validate(&self) -> Result<(), TaskDomainError> {
        for (field, value) in [
            ("project_id", self.project_id.as_str()),
            ("workflow_id", self.workflow_id.as_str()),
            ("workflow_version_id", self.workflow_version_id.as_str()),
            ("recipe_id", self.recipe_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(TaskDomainError::invalid_task(format!(
                    "{field} must not be empty"
                )));
            }
        }

        self.progress.validate()?;

        if self
            .prompt_id
            .as_deref()
            .is_some_and(|prompt_id| prompt_id.trim().is_empty())
        {
            return Err(TaskDomainError::invalid_task(
                "prompt_id must not be empty when present",
            ));
        }
        if self.queue_number.is_some_and(|number| number < 0) {
            return Err(TaskDomainError::invalid_task(
                "queue_number must not be negative",
            ));
        }

        if self.queued_at.is_some_and(|at| at < self.created_at)
            || self.started_at.is_some_and(|at| at < self.created_at)
            || self.finished_at.is_some_and(|at| at < self.created_at)
        {
            return Err(TaskDomainError::invalid_timestamp(
                "task timestamps must not precede created_at",
            ));
        }

        if let (Some(queued_at), Some(started_at)) = (self.queued_at, self.started_at) {
            if started_at < queued_at {
                return Err(TaskDomainError::invalid_timestamp(
                    "started_at must not precede queued_at",
                ));
            }
        }

        if let Some(finished_at) = self.finished_at {
            let lower_bound = self.started_at.unwrap_or(self.created_at);
            if finished_at < lower_bound {
                return Err(TaskDomainError::invalid_timestamp(
                    "finished_at must not precede started_at or created_at",
                ));
            }
        }

        match self.status {
            TaskStatus::Queued if self.queued_at.is_none() => {
                return Err(TaskDomainError::invalid_task(
                    "QUEUED task must have queued_at",
                ));
            }
            TaskStatus::Running | TaskStatus::Collecting
                if self.queued_at.is_none() || self.started_at.is_none() =>
            {
                return Err(TaskDomainError::invalid_task(
                    "RUNNING or COLLECTING task must have queued_at and started_at",
                ));
            }
            TaskStatus::Succeeded
                if self.queued_at.is_none()
                    || self.started_at.is_none()
                    || self.finished_at.is_none() =>
            {
                return Err(TaskDomainError::invalid_task(
                    "SUCCEEDED task must have queued_at, started_at, and finished_at",
                ));
            }
            TaskStatus::Failed if self.finished_at.is_none() => {
                return Err(TaskDomainError::invalid_task(
                    "FAILED task must have finished_at",
                ));
            }
            _ => {}
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskEventType {
    TaskCreated,
    TaskValidating,
    TaskPreparing,
    TaskQueued,
    TaskRunning,
    TaskCollecting,
    TaskSucceeded,
    TaskFailed,
    TaskSubmissionPrepared,
    TaskNodeStarted,
    TaskProgressUpdated,
    TaskStreamDisconnected,
}

impl TaskEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TaskCreated => "TASK_CREATED",
            Self::TaskValidating => "TASK_VALIDATING",
            Self::TaskPreparing => "TASK_PREPARING",
            Self::TaskQueued => "TASK_QUEUED",
            Self::TaskRunning => "TASK_RUNNING",
            Self::TaskCollecting => "TASK_COLLECTING",
            Self::TaskSucceeded => "TASK_SUCCEEDED",
            Self::TaskFailed => "TASK_FAILED",
            Self::TaskSubmissionPrepared => "TASK_SUBMISSION_PREPARED",
            Self::TaskNodeStarted => "TASK_NODE_STARTED",
            Self::TaskProgressUpdated => "TASK_PROGRESS_UPDATED",
            Self::TaskStreamDisconnected => "TASK_STREAM_DISCONNECTED",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, TaskDomainError> {
        match value {
            "TASK_CREATED" => Ok(Self::TaskCreated),
            "TASK_VALIDATING" => Ok(Self::TaskValidating),
            "TASK_PREPARING" => Ok(Self::TaskPreparing),
            "TASK_QUEUED" => Ok(Self::TaskQueued),
            "TASK_RUNNING" => Ok(Self::TaskRunning),
            "TASK_COLLECTING" => Ok(Self::TaskCollecting),
            "TASK_SUCCEEDED" => Ok(Self::TaskSucceeded),
            "TASK_FAILED" => Ok(Self::TaskFailed),
            "TASK_SUBMISSION_PREPARED" => Ok(Self::TaskSubmissionPrepared),
            "TASK_NODE_STARTED" => Ok(Self::TaskNodeStarted),
            "TASK_PROGRESS_UPDATED" => Ok(Self::TaskProgressUpdated),
            "TASK_STREAM_DISCONNECTED" => Ok(Self::TaskStreamDisconnected),
            _ => Err(TaskDomainError::invalid_event_type(value)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewTaskEvent {
    pub id: String,
    pub task_id: TaskId,
    pub event_type: TaskEventType,
    pub payload: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredTaskEvent {
    pub id: String,
    pub task_id: TaskId,
    pub sequence: u64,
    pub event_type: TaskEventType,
    pub payload: Option<Value>,
    pub created_at: DateTime<Utc>,
}

pub struct TaskStateMachine;

impl TaskStateMachine {
    pub fn transition(
        task: &mut Task,
        target: TaskStatus,
        at: DateTime<Utc>,
    ) -> Result<NewTaskEvent, TaskDomainError> {
        if task.status.is_terminal() || !is_allowed_transition(task.status, target) {
            return Err(TaskDomainError::invalid_transition(task.status, target));
        }

        if at < task.created_at {
            return Err(TaskDomainError::invalid_timestamp(
                "transition time must not precede created_at",
            ));
        }

        let mut next = task.clone();
        match target {
            TaskStatus::Queued => {
                next.queued_at = Some(at);
            }
            TaskStatus::Running => {
                let queued_at = task.queued_at.ok_or_else(|| {
                    TaskDomainError::invalid_timestamp(
                        "RUNNING transition requires a prior queued_at",
                    )
                })?;
                if at < queued_at {
                    return Err(TaskDomainError::invalid_timestamp(
                        "started_at must not precede queued_at",
                    ));
                }
                next.started_at = Some(at);
            }
            TaskStatus::Succeeded | TaskStatus::Failed => {
                let lower_bound = task.started_at.unwrap_or(task.created_at);
                if at < lower_bound {
                    return Err(TaskDomainError::invalid_timestamp(
                        "finished_at must not precede started_at or created_at",
                    ));
                }
                next.finished_at = Some(at);
            }
            _ => {}
        }

        next.status = target;
        next.validate()?;
        let event_type = target
            .event_type()
            .ok_or_else(|| TaskDomainError::invalid_task("target status has no event type"))?;
        *task = next;

        Ok(NewTaskEvent {
            id: new_event_id(),
            task_id: task.id.clone(),
            event_type,
            payload: None,
            created_at: at,
        })
    }
}

fn is_allowed_transition(from: TaskStatus, to: TaskStatus) -> bool {
    if to == TaskStatus::Failed
        && matches!(
            from,
            TaskStatus::Created
                | TaskStatus::Validating
                | TaskStatus::Preparing
                | TaskStatus::Queued
                | TaskStatus::Running
                | TaskStatus::Collecting
        )
    {
        return true;
    }

    matches!(
        (from, to),
        (TaskStatus::Created, TaskStatus::Validating)
            | (TaskStatus::Validating, TaskStatus::Preparing)
            | (TaskStatus::Preparing, TaskStatus::Queued)
            | (TaskStatus::Queued, TaskStatus::Running)
            | (TaskStatus::Running, TaskStatus::Collecting)
            | (TaskStatus::Collecting, TaskStatus::Succeeded)
    )
}

fn new_event_id() -> String {
    format!("evt_{}", Uuid::new_v4())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskDomainError {
    InvalidId { kind: String, value: String },
    InvalidStatus { value: String },
    InvalidEventType { value: String },
    InvalidProgress { message: String },
    InvalidTimestamp { message: String },
    InvalidTransition { from: TaskStatus, to: TaskStatus },
    InvalidTask { message: String },
}

impl TaskDomainError {
    pub(crate) fn invalid_id(kind: &str, value: String) -> Self {
        Self::InvalidId {
            kind: kind.to_owned(),
            value,
        }
    }

    fn invalid_status(value: &str) -> Self {
        Self::InvalidStatus {
            value: value.to_owned(),
        }
    }

    fn invalid_event_type(value: &str) -> Self {
        Self::InvalidEventType {
            value: value.to_owned(),
        }
    }

    fn invalid_progress(message: impl Into<String>) -> Self {
        Self::InvalidProgress {
            message: message.into(),
        }
    }

    fn invalid_timestamp(message: impl Into<String>) -> Self {
        Self::InvalidTimestamp {
            message: message.into(),
        }
    }

    fn invalid_transition(from: TaskStatus, to: TaskStatus) -> Self {
        Self::InvalidTransition { from, to }
    }

    fn invalid_task(message: impl Into<String>) -> Self {
        Self::InvalidTask {
            message: message.into(),
        }
    }
}

impl fmt::Display for TaskDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId { kind, value } => {
                write!(formatter, "invalid {kind} id \"{value}\"")
            }
            Self::InvalidStatus { value } => write!(formatter, "unknown task status \"{value}\""),
            Self::InvalidEventType { value } => {
                write!(formatter, "unknown task event type \"{value}\"")
            }
            Self::InvalidProgress { message }
            | Self::InvalidTimestamp { message }
            | Self::InvalidTask { message } => formatter.write_str(message),
            Self::InvalidTransition { from, to } => {
                write!(
                    formatter,
                    "invalid task transition {} -> {}",
                    from.as_str(),
                    to.as_str()
                )
            }
        }
    }
}

impl Error for TaskDomainError {}

#[cfg(test)]
mod tests {
    use super::{Task, TaskDomainError, TaskEventType, TaskProgress, TaskStateMachine, TaskStatus};
    use chrono::{Duration, TimeZone, Utc};

    fn created_task() -> Task {
        Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        )
    }

    #[test]
    fn new_task_has_expected_initial_state() {
        let task = created_task();

        assert!(task.id.as_str().starts_with("tsk_"));
        assert_eq!(task.status, TaskStatus::Created);
        assert!(task.prompt_id.is_none());
        assert!(task.queue_number.is_none());
        assert_eq!(task.progress, TaskProgress::Indeterminate);
        assert!(task.current_node_id.is_none());
        assert!(task.error.is_none());
        assert!(task.queued_at.is_none());
        assert!(task.started_at.is_none());
        assert!(task.finished_at.is_none());
    }

    #[test]
    fn normal_state_flow_produces_matching_events_and_timestamps() {
        let mut task = created_task();
        let base = task.created_at;
        let transitions = [
            (TaskStatus::Validating, TaskEventType::TaskValidating),
            (TaskStatus::Preparing, TaskEventType::TaskPreparing),
            (TaskStatus::Queued, TaskEventType::TaskQueued),
            (TaskStatus::Running, TaskEventType::TaskRunning),
            (TaskStatus::Collecting, TaskEventType::TaskCollecting),
            (TaskStatus::Succeeded, TaskEventType::TaskSucceeded),
        ];

        for (index, (status, event_type)) in transitions.into_iter().enumerate() {
            let at = base + Duration::seconds((index + 1) as i64);
            let event = TaskStateMachine::transition(&mut task, status, at)
                .expect("normal transition should succeed");
            assert_eq!(event.event_type, event_type);
            assert_eq!(event.task_id, task.id);
            assert_eq!(event.created_at, at);
        }

        assert_eq!(task.status, TaskStatus::Succeeded);
        assert_eq!(task.queued_at, Some(base + Duration::seconds(3)));
        assert_eq!(task.started_at, Some(base + Duration::seconds(4)));
        assert_eq!(task.finished_at, Some(base + Duration::seconds(6)));
    }

    #[test]
    fn invalid_and_terminal_transitions_fail() {
        let mut task = created_task();
        let at = task.created_at + Duration::seconds(1);

        assert!(matches!(
            TaskStateMachine::transition(&mut task, TaskStatus::Running, at),
            Err(TaskDomainError::InvalidTransition { .. })
        ));

        TaskStateMachine::transition(&mut task, TaskStatus::Validating, at)
            .expect("validating should succeed");
        assert!(matches!(
            TaskStateMachine::transition(&mut task, TaskStatus::Queued, at + Duration::seconds(1)),
            Err(TaskDomainError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn all_active_states_can_fail_but_terminal_states_cannot_transition() {
        let statuses = [
            TaskStatus::Created,
            TaskStatus::Validating,
            TaskStatus::Preparing,
            TaskStatus::Queued,
            TaskStatus::Running,
            TaskStatus::Collecting,
        ];

        for status in statuses {
            let mut task = created_task();
            let base = task.created_at;
            match status {
                TaskStatus::Created => {}
                TaskStatus::Validating => {
                    TaskStateMachine::transition(&mut task, status, base + Duration::seconds(1))
                        .unwrap();
                }
                TaskStatus::Preparing => {
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Validating,
                        base + Duration::seconds(1),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Preparing,
                        base + Duration::seconds(2),
                    )
                    .unwrap();
                }
                TaskStatus::Queued => {
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Validating,
                        base + Duration::seconds(1),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Preparing,
                        base + Duration::seconds(2),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Queued,
                        base + Duration::seconds(3),
                    )
                    .unwrap();
                }
                TaskStatus::Running => {
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Validating,
                        base + Duration::seconds(1),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Preparing,
                        base + Duration::seconds(2),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Queued,
                        base + Duration::seconds(3),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Running,
                        base + Duration::seconds(4),
                    )
                    .unwrap();
                }
                TaskStatus::Collecting => {
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Validating,
                        base + Duration::seconds(1),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Preparing,
                        base + Duration::seconds(2),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Queued,
                        base + Duration::seconds(3),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Running,
                        base + Duration::seconds(4),
                    )
                    .unwrap();
                    TaskStateMachine::transition(
                        &mut task,
                        TaskStatus::Collecting,
                        base + Duration::seconds(5),
                    )
                    .unwrap();
                }
                TaskStatus::Succeeded | TaskStatus::Failed => unreachable!(),
            }

            TaskStateMachine::transition(
                &mut task,
                TaskStatus::Failed,
                base + Duration::seconds(20),
            )
            .expect("active task should be allowed to fail");
            assert_eq!(task.status, TaskStatus::Failed);
            assert!(TaskStateMachine::transition(
                &mut task,
                TaskStatus::Validating,
                base + Duration::seconds(21),
            )
            .is_err());
        }
    }

    #[test]
    fn rejects_time_that_moves_backwards() {
        let mut task = created_task();
        let created_at = task.created_at;

        TaskStateMachine::transition(
            &mut task,
            TaskStatus::Validating,
            created_at + Duration::seconds(1),
        )
        .unwrap();
        TaskStateMachine::transition(
            &mut task,
            TaskStatus::Preparing,
            created_at + Duration::seconds(2),
        )
        .unwrap();
        TaskStateMachine::transition(
            &mut task,
            TaskStatus::Queued,
            created_at + Duration::seconds(3),
        )
        .unwrap();
        let error = TaskStateMachine::transition(
            &mut task,
            TaskStatus::Running,
            created_at + Duration::seconds(2),
        )
        .expect_err("transition time must not move backwards");

        assert!(matches!(error, TaskDomainError::InvalidTimestamp { .. }));
    }

    #[test]
    fn progress_invariants_are_enforced() {
        assert!(TaskProgress::step(20, 10, None).is_err());
        assert!(TaskProgress::step(1, 0, None).is_err());
        assert!(TaskProgress::node("").is_err());
        assert!(TaskProgress::step(10, 10, Some("3".to_owned())).is_ok());
    }

    #[test]
    fn db_status_and_event_values_are_strict() {
        assert_eq!(
            TaskStatus::try_from_db("RUNNING").unwrap(),
            TaskStatus::Running
        );
        assert_eq!(
            TaskEventType::try_from_db("TASK_CREATED").unwrap(),
            TaskEventType::TaskCreated
        );
        assert!(TaskStatus::try_from_db("FOOBAR").is_err());
        assert!(TaskEventType::try_from_db("EVENT_UNKNOWN").is_err());
    }
}
