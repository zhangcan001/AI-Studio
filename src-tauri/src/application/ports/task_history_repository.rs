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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskHistoryTimeFilter {
    #[default]
    All,
    Today,
    Last7Days,
    Last30Days,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskHistoryQuery {
    pub project_id: String,
    pub filter: TaskHistoryFilter,
    pub workflow_id: Option<String>,
    pub keyword: Option<String>,
    pub time_filter: TaskHistoryTimeFilter,
    pub cursor: Option<PageCursor>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskHistoryWorkflowOption {
    pub workflow_id: String,
    pub workflow_name: String,
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
        query: TaskHistoryQuery,
    ) -> Result<PageResult<TaskHistoryRecord>, RepositoryError>;

    async fn list_workflow_options(
        &self,
        project_id: &str,
    ) -> Result<Vec<TaskHistoryWorkflowOption>, RepositoryError>;

    async fn find_detail(
        &self,
        project_id: &str,
        task_id: &crate::domain::TaskId,
    ) -> Result<Option<TaskHistoryRecord>, RepositoryError>;
}
