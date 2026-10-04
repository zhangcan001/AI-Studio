use super::RepositoryError;
use crate::domain::{NewTaskEvent, StoredTaskEvent, Task, TaskId, TaskStatus, TaskTelemetry};
use async_trait::async_trait;

/// Read-only persisted diagnostic facts, not an executable Task. Intentionally
/// excludes prompt, workflow source, paths, raw error and write/state operations.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskDiagnosticFacts {
    pub id: TaskId,
    pub project_id: String,
    pub status: TaskStatus,
    pub error_code: Option<String>,
    pub telemetry: TaskTelemetry,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub queued_at: Option<chrono::DateTime<chrono::Utc>>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<&Task> for TaskDiagnosticFacts {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            project_id: task.project_id.clone(),
            status: task.status,
            error_code: task.error.as_ref().map(|error| error.code.clone()),
            telemetry: task.telemetry.clone(),
            created_at: task.created_at,
            queued_at: task.queued_at,
            started_at: task.started_at,
            finished_at: task.finished_at,
        }
    }
}

#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// Production implementations read facts without weakening executable Task validation.
    async fn list_recent_diagnostic_facts(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<TaskDiagnosticFacts>, RepositoryError> {
        Ok(self
            .list_recent(project_id, limit.min(50))
            .await?
            .iter()
            .map(TaskDiagnosticFacts::from)
            .collect())
    }

    async fn find_diagnostic_facts(
        &self,
        project_id: &str,
        task_id: &TaskId,
    ) -> Result<Option<TaskDiagnosticFacts>, RepositoryError> {
        Ok(self
            .find_by_id(task_id)
            .await?
            .filter(|task| task.project_id == project_id)
            .as_ref()
            .map(TaskDiagnosticFacts::from))
    }
    async fn create(
        &self,
        task: &Task,
        created_event: &NewTaskEvent,
    ) -> Result<StoredTaskEvent, RepositoryError>;

    async fn persist_transition(
        &self,
        task: &Task,
        event: &NewTaskEvent,
        expected_previous_status: TaskStatus,
    ) -> Result<StoredTaskEvent, RepositoryError>;

    async fn persist_runtime_update(
        &self,
        task: &Task,
        event: &NewTaskEvent,
    ) -> Result<StoredTaskEvent, RepositoryError>;

    async fn find_by_id(&self, task_id: &TaskId) -> Result<Option<Task>, RepositoryError>;

    /// Loads a set of tasks for list hydration.  Lightweight repositories may
    /// use the compatible single-row fallback; production repositories should
    /// override this with one set-based query.
    async fn find_many_by_ids(&self, task_ids: &[TaskId]) -> Result<Vec<Task>, RepositoryError> {
        let mut tasks = Vec::with_capacity(task_ids.len());
        for task_id in task_ids {
            if let Some(task) = self.find_by_id(task_id).await? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    /// Finds the original task for a caller-owned submission idempotency key.
    /// Implementations must use the indexed task identity column; task event
    /// history is retained for audit only and is not a production lookup path.
    async fn find_by_submission_idempotency_key(
        &self,
        project_id: &str,
        key: &str,
    ) -> Result<Option<Task>, RepositoryError>;

    async fn list_recent(&self, project_id: &str, limit: u32)
        -> Result<Vec<Task>, RepositoryError>;

    async fn list_active(&self) -> Result<Vec<Task>, RepositoryError>;

    async fn list_events(&self, task_id: &TaskId) -> Result<Vec<StoredTaskEvent>, RepositoryError>;
}
