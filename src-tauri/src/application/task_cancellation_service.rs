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

    pub async fn request_cancel(&self, task_id: &str) -> Result<Task, TaskCancellationError> {
        let task_id = TaskId::parse(task_id.to_owned())
            .map_err(|error| TaskCancellationError::InvalidTaskId(error.to_string()))?;
        let Some(task) = self.task_repository.find_by_id(&task_id).await? else {
            return Err(TaskCancellationError::NotFound(task_id.to_string()));
        };

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

    async fn persist_request(&self, task: Task) -> Result<Task, TaskCancellationError> {
        let previous_status = task.status;
        let mut next = task;
        let event = next.request_cancel(self.clock.now())?;
        match self
            .task_repository
            .persist_transition(&next, &event, previous_status)
            .await
        {
            Ok(_) => {
                self.task_update_sink.publish(&next);
                self.execution_registry.signal_cancel(&next.id);
                Ok(next)
            }
            Err(error) => {
                if let Some(current) = self.task_repository.find_by_id(&next.id).await? {
                    if matches!(
                        current.status,
                        TaskStatus::CancelRequested
                            | TaskStatus::Cancelled
                            | TaskStatus::Succeeded
                            | TaskStatus::Failed
                    ) {
                        self.execution_registry.signal_cancel(&current.id);
                        return Ok(current);
                    }
                }
                Err(TaskCancellationError::Repository(error))
            }
        }
    }
}

#[derive(Debug)]
pub enum TaskCancellationError {
    InvalidTaskId(String),
    NotFound(String),
    NotCancellable { task_id: String, status: TaskStatus },
    Repository(RepositoryError),
    Domain(TaskDomainError),
}

impl fmt::Display for TaskCancellationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

        let cancelled = service.request_cancel(task.id.as_str()).await.unwrap();
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
            service.request_cancel(task.id.as_str()).await,
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
}
