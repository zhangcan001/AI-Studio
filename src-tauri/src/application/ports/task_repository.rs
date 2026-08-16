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

    /// Finds the original task for a caller-owned submission idempotency key.
    ///
    /// The key is stored in the immutable submission-prepared event so this
    /// lookup remains available after restart without adding another mutable
    /// execution table.
    async fn find_by_submission_idempotency_key(
        &self,
        project_id: &str,
        key: &str,
    ) -> Result<Option<Task>, RepositoryError> {
        let candidates = self.list_recent(project_id, 1_000).await?;
        for task in candidates {
            let events = self.list_events(&task.id).await?;
            if events.iter().any(|event| {
                event.event_type == crate::domain::TaskEventType::TaskSubmissionPrepared
                    && event
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.get("submissionIdempotencyKey"))
                        .and_then(serde_json::Value::as_str)
                        == Some(key)
            }) {
                return Ok(Some(task));
            }
        }
        Ok(None)
    }

    async fn list_recent(&self, project_id: &str, limit: u32)
        -> Result<Vec<Task>, RepositoryError>;

    async fn list_active(&self) -> Result<Vec<Task>, RepositoryError>;

    async fn list_events(&self, task_id: &TaskId) -> Result<Vec<StoredTaskEvent>, RepositoryError>;
}
