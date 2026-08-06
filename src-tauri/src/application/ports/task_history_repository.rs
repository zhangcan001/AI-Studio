use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::RepositoryError;
use crate::domain::{Task, TaskStatus};
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskHistoryFilter {
    All,
    Active,
    Succeeded,
    Failed,
    Cancelled,
}

impl TaskHistoryFilter {
    pub fn statuses(self) -> Option<&'static [TaskStatus]> {
        match self {
            Self::All => None,
            Self::Active => Some(&[
                TaskStatus::Created,
                TaskStatus::Validating,
                TaskStatus::Preparing,
                TaskStatus::Queued,
                TaskStatus::Running,
                TaskStatus::CancelRequested,
                TaskStatus::Collecting,
            ]),
            Self::Succeeded => Some(&[TaskStatus::Succeeded]),
            Self::Failed => Some(&[TaskStatus::Failed]),
            Self::Cancelled => Some(&[TaskStatus::Cancelled]),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskHistoryRecord {
    pub task: Task,
    pub workflow_name: String,
    pub output_count: u32,
}

#[async_trait]
pub trait TaskHistoryRepository: Send + Sync {
    async fn list_page(
        &self,
        project_id: &str,
        filter: TaskHistoryFilter,
        cursor: Option<PageCursor>,
        limit: u32,
    ) -> Result<PageResult<TaskHistoryRecord>, RepositoryError>;

    async fn find_detail(
        &self,
        project_id: &str,
        task_id: &crate::domain::TaskId,
    ) -> Result<Option<TaskHistoryRecord>, RepositoryError>;
}
