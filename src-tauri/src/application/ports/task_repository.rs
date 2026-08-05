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

    async fn list_recent(&self, limit: u32) -> Result<Vec<Task>, RepositoryError>;

    async fn list_events(&self, task_id: &TaskId) -> Result<Vec<StoredTaskEvent>, RepositoryError>;
}
