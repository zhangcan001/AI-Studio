use crate::application::ports::{Clock, RepositoryError, TaskRepository, TaskUpdateSink};
use crate::application::task_execution_registry::TaskExecutionRegistry;
use crate::domain::{Task, TaskDomainError, TaskId, TaskStatus};
use std::{error::Error, fmt, sync::Arc};

pub struct TaskCancellationService {
    task_repository: Arc<dyn TaskRepository>,
    execution_registry: TaskExecutionRegistry,
    clock: Arc<dyn Clock>,
    task_update_sink: Arc<dyn TaskUpdateSink>,
}

impl TaskCancellationService {
    pub fn new(
        task_repository: Arc<dyn TaskRepository>,
        execution_registry: TaskExecutionRegistry,
        clock: Arc<dyn Clock>,
        task_update_sink: Arc<dyn TaskUpdateSink>,
    ) -> Self {
        Self {
            task_repository,
            execution_registry,
            clock,
            task_update_sink,
        }
    }

    pub async fn request_cancel(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<Task, TaskCancellationError> {
        if project_id.trim().is_empty() {
            return Err(TaskCancellationError::InvalidProjectId(
                "project id must not be empty".to_owned(),
            ));
        }
        let task_id = TaskId::parse(task_id.to_owned())
            .map_err(|error| TaskCancellationError::InvalidTaskId(error.to_string()))?;
        let Some(task) = self.task_repository.find_by_id(&task_id).await? else {
            return Err(TaskCancellationError::NotFound(task_id.to_string()));
        };
        if task.project_id != project_id {
            return Err(TaskCancellationError::NotFound(task_id.to_string()));
        }

        match task.status {
            TaskStatus::CancelRequested | TaskStatus::Cancelled => {
                self.execution_registry.signal_cancel(&task.id);
                Ok(task)
            }
            TaskStatus::Created
            | TaskStatus::Validating
            | TaskStatus::Preparing
            | TaskStatus::Queued
            | TaskStatus::Running => self.persist_request(task).await,
            TaskStatus::Collecting | TaskStatus::Succeeded | TaskStatus::Failed => {
                Err(TaskCancellationError::NotCancellable {
                    task_id: task.id.to_string(),
                    status: task.status,
                })
            }
        }
    }

    /// Hides a finished task record (soft delete). Running tasks must be
    /// cancelled first. Generated files and library assets are never removed.
    pub async fn hide_record(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<(), TaskCancellationError> {
        let task_id = TaskId::parse(task_id.to_owned())
            .map_err(|error| TaskCancellationError::InvalidTaskId(error.to_string()))?;
        let Some(task) = self.task_repository.find_by_id(&task_id).await? else {
            return Err(TaskCancellationError::NotFound(task_id.to_string()));
        };
        if task.project_id != project_id {
            return Err(TaskCancellationError::NotFound(task_id.to_string()));
        }
        if !matches!(
            task.status,
            TaskStatus::Succeeded | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return Err(TaskCancellationError::NotCancellable {
                task_id: task_id.to_string(),
                status: task.status,
            });
        }
        self.task_repository
            .hide_finished(project_id, &task_id, self.clock.now())
            .await?;
        Ok(())
    }

    async fn persist_request(&self, mut task: Task) -> Result<Task, TaskCancellationError> {
        let mut last_error = None;
        for _attempt in 0..4 {
            let previous_status = task.status;
            let mut next = task.clone();
            let event = next.request_cancel(self.clock.now())?;
            match self
                .task_repository
                .persist_transition(&next, &event, previous_status)
                .await
            {
                Ok(_) => {
                    self.task_update_sink.publish(&next);
                    self.execution_registry.signal_cancel(&next.id);
                    return Ok(next);
                }
                Err(error) => {
                    let Some(current) = self.task_repository.find_by_id(&next.id).await? else {
                        return Err(TaskCancellationError::NotFound(next.id.to_string()));
                    };
                    match current.status {
                        TaskStatus::CancelRequested
                        | TaskStatus::Cancelled
                        | TaskStatus::Succeeded
                        | TaskStatus::Failed => {
                            self.execution_registry.signal_cancel(&current.id);
                            return Ok(current);
                        }
                        TaskStatus::Collecting => {
                            return Err(TaskCancellationError::NotCancellable {
                                task_id: current.id.to_string(),
                                status: current.status,
                            });
                        }
                        TaskStatus::Created
                        | TaskStatus::Validating
                        | TaskStatus::Preparing
                        | TaskStatus::Queued
                        | TaskStatus::Running
                            if current.status != previous_status =>
                        {
                            task = current;
                            last_error = Some(error);
                            continue;
                        }
                        _ => return Err(TaskCancellationError::Repository(error)),
                    }
                }
            }
        }
        Err(TaskCancellationError::Repository(
            last_error.unwrap_or_else(|| {
                RepositoryError::database("task cancel transition retry exhausted")
            }),
        ))
    }
}

#[derive(Debug)]
pub enum TaskCancellationError {
    InvalidProjectId(String),
    InvalidTaskId(String),
    NotFound(String),
    NotCancellable { task_id: String, status: TaskStatus },
    Repository(RepositoryError),
    Domain(TaskDomainError),
}

impl fmt::Display for TaskCancellationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectId(message) => write!(formatter, "INVALID_PROJECT_ID: {message}"),
            Self::InvalidTaskId(message) => write!(formatter, "INVALID_TASK_ID: {message}"),
            Self::NotFound(task_id) => {
                write!(formatter, "TASK_NOT_FOUND: task {task_id} was not found")
            }
            Self::NotCancellable { task_id, status } => write!(
                formatter,
                "TASK_NOT_CANCELLABLE: task {task_id} cannot be cancelled from {}",
                status.as_str()
            ),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Domain(error) => write!(formatter, "TASK_DOMAIN_ERROR: {error}"),
        }
    }
}

impl Error for TaskCancellationError {}

impl From<RepositoryError> for TaskCancellationError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<TaskDomainError> for TaskCancellationError {
    fn from(error: TaskDomainError) -> Self {
        Self::Domain(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskCancellationError, TaskCancellationService};
    use crate::application::ports::{Clock, TaskRepository, TaskUpdateSink};
    use crate::application::task_execution_registry::TaskExecutionRegistry;
    use crate::domain::{Task, TaskEventType, TaskStatus};
    use crate::infrastructure::database::{
        initialize,
        repositories::{test_support, SqliteTaskRepository},
    };
    use chrono::{TimeZone, Utc};
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Clone, Copy)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> chrono::DateTime<Utc> {
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 10).unwrap()
        }
    }

    #[derive(Clone, Default)]
    struct RecordingSink(Arc<Mutex<Vec<TaskStatus>>>);

    impl TaskUpdateSink for RecordingSink {
        fn publish(&self, task: &Task) {
            self.0.lock().unwrap().push(task.status);
        }
    }

    async fn setup() -> (tempfile::TempDir, Arc<SqliteTaskRepository>, Task) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = Arc::new(SqliteTaskRepository::new(pool));
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
            .unwrap();
        (directory, repository, task)
    }

    #[tokio::test]
    async fn hide_record_requires_finished_task_and_hides_it_from_lists() {
        let (_directory, repository, task) = setup().await;
        let service = TaskCancellationService::new(
            repository.clone(),
            TaskExecutionRegistry::default(),
            Arc::new(FixedClock),
            Arc::new(RecordingSink::default()),
        );
        assert!(matches!(
            service.hide_record("project-1", task.id.as_str()).await,
            Err(TaskCancellationError::NotCancellable { .. })
        ));
        let requested = service
            .request_cancel("project-1", task.id.as_str())
            .await
            .unwrap();
        let mut cancelled = requested.clone();
        let event = cancelled.cancel(FixedClock.now()).unwrap();
        repository
            .persist_transition(&cancelled, &event, TaskStatus::CancelRequested)
            .await
            .unwrap();
        service
            .hide_record("project-1", task.id.as_str())
            .await
            .unwrap();
        assert!(repository
            .list_recent("project-1", 10)
            .await
            .unwrap()
            .is_empty());
        assert!(repository.find_by_id(&task.id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn request_cancel_persists_event_and_signals_worker() {
        let (_directory, repository, task) = setup().await;
        let registry = TaskExecutionRegistry::default();
        let (mut signal, guard) = registry.register(task.id.clone());
        let sink = Arc::new(RecordingSink::default());
        let service = TaskCancellationService::new(
            repository.clone(),
            registry.clone(),
            Arc::new(FixedClock),
            sink.clone(),
        );

        let cancelled = service
            .request_cancel(&task.project_id, task.id.as_str())
            .await
            .unwrap();
        assert_eq!(cancelled.status, TaskStatus::CancelRequested);
        assert!(signal.changed().await.is_ok());
        assert!(*signal.borrow());
        assert_eq!(
            repository
                .find_by_id(&task.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::CancelRequested
        );
        assert_eq!(
            repository
                .list_events(&task.id)
                .await
                .unwrap()
                .last()
                .unwrap()
                .event_type,
            TaskEventType::TaskCancelRequested
        );
        assert_eq!(
            sink.0.lock().unwrap().as_slice(),
            &[TaskStatus::CancelRequested]
        );
        drop(guard);
    }

    #[tokio::test]
    async fn terminal_task_is_not_changed_by_cancel_request() {
        let (_directory, repository, mut task) = setup().await;
        let error = task
            .fail(
                crate::domain::TaskError {
                    code: "TEST".to_owned(),
                    message: "terminal".to_owned(),
                    raw: None,
                },
                task.created_at + chrono::Duration::seconds(1),
            )
            .unwrap();
        repository
            .persist_transition(&task, &error, TaskStatus::Created)
            .await
            .unwrap();
        let service = TaskCancellationService::new(
            repository.clone(),
            TaskExecutionRegistry::default(),
            Arc::new(FixedClock),
            Arc::new(RecordingSink::default()),
        );

        assert!(matches!(
            service
                .request_cancel(&task.project_id, task.id.as_str())
                .await,
            Err(TaskCancellationError::NotCancellable {
                status: TaskStatus::Failed,
                ..
            })
        ));
        assert_eq!(
            repository
                .find_by_id(&task.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::Failed
        );
    }

    #[tokio::test]
    async fn cross_project_cancel_is_not_found_without_signaling_or_persisting() {
        let (_directory, repository, task) = setup().await;
        let registry = TaskExecutionRegistry::default();
        let (signal, _guard) = registry.register(task.id.clone());
        let sink = Arc::new(RecordingSink::default());
        let service = TaskCancellationService::new(
            repository.clone(),
            registry,
            Arc::new(FixedClock),
            sink.clone(),
        );

        assert!(matches!(
            service.request_cancel("project-2", task.id.as_str()).await,
            Err(TaskCancellationError::NotFound(_))
        ));
        assert!(!*signal.borrow());
        assert!(sink.0.lock().unwrap().is_empty());
        assert_eq!(
            repository
                .find_by_id(&task.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::Created
        );
    }

    #[tokio::test]
    async fn request_cancel_retries_when_task_advances_to_another_cancellable_status() {
        use crate::application::ports::RepositoryError;
        use crate::domain::{NewTaskEvent, StoredTaskEvent, TaskId, TaskStateMachine};
        use async_trait::async_trait;
        use std::sync::atomic::{AtomicBool, Ordering};

        struct AdvancingTaskRepo {
            inner: Arc<SqliteTaskRepository>,
            raced_once: AtomicBool,
        }

        #[async_trait]
        impl TaskRepository for AdvancingTaskRepo {
            async fn create(
                &self,
                task: &Task,
                created_event: &NewTaskEvent,
            ) -> Result<StoredTaskEvent, RepositoryError> {
                self.inner.create(task, created_event).await
            }

            async fn persist_transition(
                &self,
                task: &Task,
                event: &NewTaskEvent,
                expected_previous_status: TaskStatus,
            ) -> Result<StoredTaskEvent, RepositoryError> {
                if expected_previous_status == TaskStatus::Created
                    && !self.raced_once.swap(true, Ordering::SeqCst)
                {
                    if let Some(mut moved) = self.inner.find_by_id(&task.id).await? {
                        let moved_at = moved.created_at + chrono::Duration::seconds(1);
                        let moved_event = TaskStateMachine::transition(
                            &mut moved,
                            TaskStatus::Validating,
                            moved_at,
                        )
                        .map_err(|e| RepositoryError::database(e.to_string()))?;
                        self.inner
                            .persist_transition(&moved, &moved_event, TaskStatus::Created)
                            .await?;
                    }
                }
                self.inner
                    .persist_transition(task, event, expected_previous_status)
                    .await
            }

            async fn persist_runtime_update(
                &self,
                task: &Task,
                event: &NewTaskEvent,
            ) -> Result<StoredTaskEvent, RepositoryError> {
                self.inner.persist_runtime_update(task, event).await
            }

            async fn find_by_id(&self, task_id: &TaskId) -> Result<Option<Task>, RepositoryError> {
                self.inner.find_by_id(task_id).await
            }

            async fn find_by_submission_idempotency_key(
                &self,
                project_id: &str,
                key: &str,
            ) -> Result<Option<Task>, RepositoryError> {
                self.inner
                    .find_by_submission_idempotency_key(project_id, key)
                    .await
            }

            async fn list_recent(
                &self,
                project_id: &str,
                limit: u32,
            ) -> Result<Vec<Task>, RepositoryError> {
                self.inner.list_recent(project_id, limit).await
            }

            async fn list_active(&self) -> Result<Vec<Task>, RepositoryError> {
                self.inner.list_active().await
            }

            async fn list_events(
                &self,
                task_id: &TaskId,
            ) -> Result<Vec<StoredTaskEvent>, RepositoryError> {
                self.inner.list_events(task_id).await
            }
        }

        let (_directory, repository, task) = setup().await;
        let repo = Arc::new(AdvancingTaskRepo {
            inner: repository.clone(),
            raced_once: AtomicBool::new(false),
        });
        let registry = TaskExecutionRegistry::default();
        let (mut signal, _guard) = registry.register(task.id.clone());
        let service = TaskCancellationService::new(
            repo,
            registry,
            Arc::new(FixedClock),
            Arc::new(RecordingSink::default()),
        );

        let cancelled = service
            .request_cancel(&task.project_id, task.id.as_str())
            .await
            .unwrap();
        assert_eq!(cancelled.status, TaskStatus::CancelRequested);
        assert!(signal.changed().await.is_ok());
        assert!(*signal.borrow());
        assert_eq!(
            repository
                .find_by_id(&task.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::CancelRequested
        );
    }
}
