use super::RepositoryError;
use crate::domain::{NewTaskEvent, StoredTaskEvent, Task, TaskId, TaskStatus};
use async_trait::async_trait;

#[async_trait]
pub trait TaskRepository: Send + Sync {
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
