use crate::application::ports::{
    AssetRepository, GenerationDefinitionRepository, RepositoryError, TaskOutputAssetMapping,
    TaskRepository, TaskUpdatePayload,
};
use crate::compiler::RecipeParser;
use crate::domain::{Asset, Task, TaskId};
use std::{error::Error, fmt, sync::Arc};

pub struct TaskQueryService {
    task_repository: Arc<dyn TaskRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
}

impl TaskQueryService {
    pub fn new(
        task_repository: Arc<dyn TaskRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
    ) -> Self {
        Self {
            task_repository,
            asset_repository,
            definition_repository,
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
        let assets = self.list_output_assets(&task).await?;
        let mut view = TaskUpdatePayload::from_task(&task);
        view.output_asset_ids = assets
            .into_iter()
            .filter(|asset| asset.project_id == task.project_id)
            .map(|asset| asset.id.as_str().to_owned())
            .collect();
        Ok(view)
    }

    async fn list_output_assets(&self, task: &Task) -> Result<Vec<Asset>, TaskQueryError> {
        let mut mapped = self.asset_repository.list_mapped_assets(&task.id).await?;
        if mapped.is_empty() {
            return Ok(self.asset_repository.list_by_source_task(&task.id).await?);
        }

        let output_order = self.output_order(task).await?;
        mapped.sort_by(|(left, _), (right, _)| compare_mappings(left, right, &output_order));
        Ok(mapped.into_iter().map(|(_, asset)| asset).collect())
    }

    async fn output_order(&self, task: &Task) -> Result<Vec<String>, TaskQueryError> {
        let Some(definition) = self
            .definition_repository
            .find(&task.workflow_version_id, &task.recipe_id)
            .await?
        else {
            return Ok(Vec::new());
        };
        Ok(RecipeParser::parse(&definition.recipe_yaml)
            .map(|recipe| recipe.outputs.into_iter().map(|output| output.id).collect())
            .unwrap_or_default())
    }
}

fn compare_mappings(
    left: &TaskOutputAssetMapping,
    right: &TaskOutputAssetMapping,
    output_order: &[String],
) -> std::cmp::Ordering {
    let left_rank = output_order
        .iter()
        .position(|output_id| output_id == &left.output_id)
        .unwrap_or(usize::MAX);
    let right_rank = output_order
        .iter()
        .position(|output_id| output_id == &right.output_id)
        .unwrap_or(usize::MAX);
    left_rank
        .cmp(&right_rank)
        .then_with(|| left.ordinal.cmp(&right.ordinal))
        .then_with(|| left.output_id.cmp(&right.output_id))
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

#[cfg(test)]
mod tests {
    use super::compare_mappings;
    use crate::application::ports::TaskOutputAssetMapping;
    use crate::domain::{AssetId, TaskId};
    use chrono::Utc;

    fn mapping(output_id: &str, ordinal: u32, asset_id: &str) -> TaskOutputAssetMapping {
        TaskOutputAssetMapping {
            task_id: TaskId::parse("tsk_order").unwrap(),
            output_id: output_id.to_owned(),
            ordinal,
            asset_id: AssetId::parse(asset_id).unwrap(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn output_mappings_follow_recipe_order_then_ordinal() {
        let image = mapping("preview", 0, "ast_image");
        let video = mapping("final_video", 0, "ast_video");
        let second_video = mapping("final_video", 1, "ast_video_2");
        let order = vec!["preview".to_owned(), "final_video".to_owned()];
        assert_eq!(
            compare_mappings(&image, &video, &order),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_mappings(&video, &second_video, &order),
            std::cmp::Ordering::Less
        );
    }
}
