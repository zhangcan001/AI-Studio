use crate::domain::{Task, TaskError, TaskProgress};
use chrono::{DateTime, Utc};
use serde::Serialize;

pub const TASK_UPDATED_EVENT: &str = "task://updated";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdatePayload {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub prompt_id: Option<String>,
    pub queue_number: Option<i64>,
    pub progress: TaskProgressPayload,
    pub error: Option<TaskErrorPayload>,
    pub created_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub output_asset_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressPayload {
    pub mode: String,
    pub current: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskErrorPayload {
    pub code: String,
    pub message: String,
}

impl TaskUpdatePayload {
    pub fn from_task(task: &Task) -> Self {
        let (mode, current, total) = match &task.progress {
            TaskProgress::Indeterminate => ("indeterminate".to_owned(), None, None),
            TaskProgress::Node { .. } => ("node".to_owned(), None, None),
            TaskProgress::Step { current, total, .. } => {
                ("step".to_owned(), Some(*current), Some(*total))
            }
        };

        Self {
            id: task.id.as_str().to_owned(),
            project_id: task.project_id.clone(),
            status: task.status.as_str().to_owned(),
            prompt_id: task.prompt_id.clone(),
            queue_number: task.queue_number,
            progress: TaskProgressPayload {
                mode,
                current,
                total,
            },
            error: task.error.as_ref().map(TaskErrorPayload::from),
            created_at: task.created_at,
            queued_at: task.queued_at,
            started_at: task.started_at,
            finished_at: task.finished_at,
            output_asset_ids: Vec::new(),
        }
    }
}

impl From<&TaskError> for TaskErrorPayload {
    fn from(error: &TaskError) -> Self {
        Self {
            code: error.code.clone(),
            message: error.message.clone(),
        }
    }
}

pub trait TaskUpdateSink: Send + Sync {
    fn publish(&self, task: &Task);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopTaskUpdateSink;

impl TaskUpdateSink for NoopTaskUpdateSink {
    fn publish(&self, _task: &Task) {}
}

#[cfg(test)]
mod tests {
    use super::TaskUpdatePayload;
    use crate::domain::Task;
    use chrono::Utc;

    #[test]
    fn task_update_payload_exposes_project_context_without_runtime_details() {
        let task = Task::new(
            "prj_test",
            "workflow",
            "workflow-version",
            "recipe",
            Utc::now(),
        );
        let payload = serde_json::to_value(TaskUpdatePayload::from_task(&task)).unwrap();
        assert_eq!(payload["projectId"], "prj_test");
        assert!(payload.get("workflowJson").is_none());
        assert!(payload.get("storagePath").is_none());
        assert!(payload.get("rawError").is_none());
    }
}
