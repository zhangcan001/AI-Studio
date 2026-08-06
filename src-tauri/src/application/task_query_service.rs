use crate::application::ports::{
    AssetRepository, RepositoryError, TaskRepository, TaskUpdatePayload,
};
use crate::domain::{Task, TaskId};
use std::{error::Error, fmt, sync::Arc};

pub struct TaskQueryService {
    task_repository: Arc<dyn TaskRepository>,
    asset_repository: Arc<dyn AssetRepository>,
}

impl TaskQueryService {
    pub fn new(
        task_repository: Arc<dyn TaskRepository>,
        asset_repository: Arc<dyn AssetRepository>,
    ) -> Self {
        Self {
            task_repository,
            asset_repository,
        }
    }

    pub async fn get(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<Option<TaskView>, TaskQueryError> {
        validate_project_id(project_id)?;
        let task_id = TaskId::parse(task_id.to_owned())
            .map_err(|error| TaskQueryError::InvalidTaskId(error.to_string()))?;
        let Some(task) = self.task_repository.find_by_id(&task_id).await? else {
            return Ok(None);
        };
        if task.project_id != project_id {
            return Ok(None);
        }
        Ok(Some(self.view(task).await?))
    }

    pub async fn list_recent(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<TaskView>, TaskQueryError> {
        validate_project_id(project_id)?;
        let tasks = self
            .task_repository
            .list_recent(project_id, limit.min(50))
            .await?;
        let mut views = Vec::with_capacity(tasks.len());
        for task in tasks {
            views.push(self.view(task).await?);
        }
        Ok(views)
    }

    pub async fn view(&self, task: Task) -> Result<TaskView, TaskQueryError> {
        let assets = self.asset_repository.list_by_source_task(&task.id).await?;
        let mut view = TaskUpdatePayload::from_task(&task);
        view.output_asset_ids = assets
            .into_iter()
            .filter(|asset| asset.project_id == task.project_id)
            .map(|asset| asset.id.as_str().to_owned())
            .collect();
        Ok(view)
    }
}

pub type TaskView = TaskUpdatePayload;

#[derive(Debug)]
pub enum TaskQueryError {
    InvalidProjectId(String),
    InvalidTaskId(String),
    Repository(RepositoryError),
}

impl fmt::Display for TaskQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectId(message) => write!(formatter, "INVALID_PROJECT_ID: {message}"),
            Self::InvalidTaskId(message) => write!(formatter, "INVALID_TASK_ID: {message}"),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

fn validate_project_id(project_id: &str) -> Result<(), TaskQueryError> {
    if project_id.trim().is_empty() {
        return Err(TaskQueryError::InvalidProjectId(
            "project id must not be empty".to_owned(),
        ));
    }
    Ok(())
}

impl Error for TaskQueryError {}

impl From<RepositoryError> for TaskQueryError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}
