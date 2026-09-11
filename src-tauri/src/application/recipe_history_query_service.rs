use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::{
    RecipeHistoryQuery, RecipeHistoryQueryRecord, RecipeHistoryQueryRepository, RepositoryError,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{error::Error, fmt, sync::Arc};

pub const DEFAULT_TASK_LIMIT: u32 = 20;
pub const MAX_TASK_LIMIT: u32 = 100;

#[derive(Debug)]
pub enum RecipeHistoryQueryError {
    InvalidIdentity,
    NotFound {
        workflow_version_id: String,
        recipe_id: String,
    },
    Repository(RepositoryError),
}

impl fmt::Display for RecipeHistoryQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentity => {
                write!(formatter, "workflowVersionId and recipeId are required")
            }
            Self::NotFound {
                workflow_version_id,
                recipe_id,
            } => write!(
                formatter,
                "recipe history identity was not found: {workflow_version_id}:{recipe_id}"
            ),
            Self::Repository(error) => error.fmt(formatter),
        }
    }
}

impl Error for RecipeHistoryQueryError {}

impl From<RepositoryError> for RecipeHistoryQueryError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

pub struct RecipeHistoryQueryService {
    repository: Arc<dyn RecipeHistoryQueryRepository>,
}

impl RecipeHistoryQueryService {
    pub fn new(repository: Arc<dyn RecipeHistoryQueryRepository>) -> Self {
        Self { repository }
    }

    pub async fn get_exact_pair(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        task_cursor: Option<PageCursor>,
        task_limit: Option<u32>,
    ) -> Result<RecipeHistoryView, RecipeHistoryQueryError> {
        let workflow_version_id = normalize_identity(workflow_version_id)?;
        let recipe_id = normalize_identity(recipe_id)?;
        let record = self
            .repository
            .get_exact_pair(RecipeHistoryQuery {
                workflow_version_id: workflow_version_id.clone(),
                recipe_id: recipe_id.clone(),
                task_cursor,
                task_limit: task_limit
                    .unwrap_or(DEFAULT_TASK_LIMIT)
                    .clamp(1, MAX_TASK_LIMIT),
            })
            .await?
            .ok_or(RecipeHistoryQueryError::NotFound {
                workflow_version_id,
                recipe_id,
            })?;
        Ok(RecipeHistoryView::from(record))
    }
}

fn normalize_identity(value: &str) -> Result<String, RecipeHistoryQueryError> {
    let value = value.trim();
    (!value.is_empty())
        .then(|| value.to_owned())
        .ok_or(RecipeHistoryQueryError::InvalidIdentity)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryView {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub workflow_version: String,
    pub recipe_version: String,
    pub schema_version: u32,
    pub recipe_sha256: String,
    pub created_at: DateTime<Utc>,
    pub is_current_version: bool,
    pub is_promoted: bool,
    pub promoted_at: Option<DateTime<Utc>>,
    pub archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub workflow_library_state: String,
    pub task_count: u64,
    pub active_task_count: u64,
    pub succeeded_task_count: u64,
    pub failed_task_count: u64,
    pub cancelled_task_count: u64,
    pub last_finished_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub executed_project_count: u64,
    pub referenced_project_count: u64,
    pub preset_count: u64,
    pub project_template_count: u64,
    pub production_run_template_count: u64,
    pub queue_item_count: u64,
    pub shot_stage_count: u64,
    pub experiment_count: u64,
    pub benchmark_run_count: u64,
    pub project_binding_count: u64,
    pub task_page: RecipeHistoryTaskPageView,
    pub queue_items: Vec<RecipeHistoryQueueView>,
    pub presets: Vec<RecipeHistoryPresetView>,
    pub project_templates: Vec<RecipeHistoryProjectTemplateView>,
    pub production_run_templates: Vec<RecipeHistoryProductionRunTemplateView>,
    pub project_bindings: Vec<RecipeHistoryBindingView>,
    pub shots: Vec<RecipeHistoryShotView>,
    pub experiments: Vec<RecipeHistoryExperimentView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryTaskPageView {
    pub items: Vec<RecipeHistoryTaskView>,
    pub next_cursor: Option<PageCursor>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryTaskView {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryQueueView {
    pub batch_id: String,
    pub batch_name: String,
    pub project_id: String,
    pub project_name: String,
    pub batch_status: String,
    pub item_id: String,
    pub item_status: String,
    pub task_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryPresetView {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub project_name: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryProjectTemplateView {
    pub id: String,
    pub name: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryProductionRunTemplateView {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub project_name: String,
    pub recipe_role: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryBindingView {
    pub project_id: String,
    pub project_name: String,
    pub stage: String,
    pub mode: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryShotView {
    pub shot_id: String,
    pub project_id: String,
    pub project_name: String,
    pub stage: String,
    pub updated_at: DateTime<Utc>,
    pub task_id: Option<String>,
    pub production_batch_item_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistoryExperimentView {
    pub experiment_id: String,
    pub experiment_name: String,
    pub project_id: String,
    pub project_name: String,
    pub candidate_id: String,
    pub run_count: u64,
    pub created_at: DateTime<Utc>,
}

impl From<RecipeHistoryQueryRecord> for RecipeHistoryView {
    fn from(record: RecipeHistoryQueryRecord) -> Self {
        let definition = record.definition;
        let counts = record.counts;
        Self {
            workflow_id: definition.workflow_id,
            workflow_version_id: definition.workflow_version_id,
            recipe_id: definition.recipe_id,
            workflow_version: definition.workflow_version,
            recipe_version: definition.recipe_version,
            schema_version: definition.schema_version,
            recipe_sha256: definition.recipe_sha256,
            created_at: definition.created_at,
            is_current_version: definition.is_current_version,
            is_promoted: definition.is_promoted,
            promoted_at: definition.promoted_at,
            archived: definition.archived,
            archived_at: definition.archived_at,
            workflow_library_state: definition.workflow_library_state,
            task_count: counts.task_count,
            active_task_count: counts.active_task_count,
            succeeded_task_count: counts.succeeded_task_count,
            failed_task_count: counts.failed_task_count,
            cancelled_task_count: counts.cancelled_task_count,
            last_finished_at: counts.last_finished_at,
            last_attempt_at: counts.last_attempt_at,
            executed_project_count: counts.executed_project_count,
            referenced_project_count: counts.referenced_project_count,
            preset_count: counts.preset_count,
            project_template_count: counts.project_template_count,
            production_run_template_count: counts.production_run_template_count,
            queue_item_count: counts.queue_item_count,
            shot_stage_count: counts.shot_stage_count,
            experiment_count: counts.experiment_count,
            benchmark_run_count: counts.benchmark_run_count,
            project_binding_count: counts.project_binding_count,
            task_page: task_page_view(record.tasks),
            queue_items: record.queue_items.into_iter().map(Into::into).collect(),
            presets: record.presets.into_iter().map(Into::into).collect(),
            project_templates: record
                .project_templates
                .into_iter()
                .map(Into::into)
                .collect(),
            production_run_templates: record
                .production_run_templates
                .into_iter()
                .map(Into::into)
                .collect(),
            project_bindings: record
                .project_bindings
                .into_iter()
                .map(Into::into)
                .collect(),
            shots: record.shots.into_iter().map(Into::into).collect(),
            experiments: record.experiments.into_iter().map(Into::into).collect(),
        }
    }
}

fn task_page_view(
    page: PageResult<crate::application::ports::RecipeHistoryTaskRecord>,
) -> RecipeHistoryTaskPageView {
    RecipeHistoryTaskPageView {
        items: page.items.into_iter().map(Into::into).collect(),
        next_cursor: page.next_cursor,
    }
}

macro_rules! impl_view_from {
    ($source:ty, $target:ty, { $($field:ident),+ $(,)? }) => {
        impl From<$source> for $target {
            fn from(value: $source) -> Self {
                Self { $($field: value.$field),+ }
            }
        }
    };
}

impl_view_from!(crate::application::ports::RecipeHistoryTaskRecord, RecipeHistoryTaskView,
    { id, project_id, project_name, status, created_at, queued_at, started_at, finished_at });
impl_view_from!(crate::application::ports::RecipeHistoryQueueRecord, RecipeHistoryQueueView,
    { batch_id, batch_name, project_id, project_name, batch_status, item_id, item_status, task_id, created_at, updated_at });
impl_view_from!(crate::application::ports::RecipeHistoryPresetRecord, RecipeHistoryPresetView,
    { id, name, project_id, project_name, updated_at });
impl_view_from!(crate::application::ports::RecipeHistoryProjectTemplateRecord, RecipeHistoryProjectTemplateView,
    { id, name, updated_at });
impl_view_from!(crate::application::ports::RecipeHistoryProductionRunTemplateRecord, RecipeHistoryProductionRunTemplateView,
    { id, name, project_id, project_name, recipe_role, updated_at });
impl_view_from!(crate::application::ports::RecipeHistoryBindingRecord, RecipeHistoryBindingView,
    { project_id, project_name, stage, mode, updated_at });
impl_view_from!(crate::application::ports::RecipeHistoryShotRecord, RecipeHistoryShotView,
    { shot_id, project_id, project_name, stage, updated_at, task_id, production_batch_item_id });
impl_view_from!(crate::application::ports::RecipeHistoryExperimentRecord, RecipeHistoryExperimentView,
    { experiment_id, experiment_name, project_id, project_name, candidate_id, run_count, created_at });

#[cfg(test)]
mod tests {
    use super::{
        RecipeHistoryQueryError, RecipeHistoryQueryService, DEFAULT_TASK_LIMIT, MAX_TASK_LIMIT,
    };
    use crate::application::pagination::{PageCursor, PageResult};
    use crate::application::ports::*;
    use async_trait::async_trait;
    use chrono::Utc;
    use std::sync::{Arc, Mutex};

    struct FakeRepository {
        seen: Arc<Mutex<Vec<RecipeHistoryQuery>>>,
        record: Option<RecipeHistoryQueryRecord>,
    }

    #[async_trait]
    impl RecipeHistoryQueryRepository for FakeRepository {
        async fn get_exact_pair(
            &self,
            query: RecipeHistoryQuery,
        ) -> Result<Option<RecipeHistoryQueryRecord>, RepositoryError> {
            self.seen.lock().unwrap().push(query);
            Ok(self.record.clone())
        }
    }

    fn record() -> RecipeHistoryQueryRecord {
        RecipeHistoryQueryRecord {
            definition: RecipeHistoryDefinitionRecord {
                workflow_id: "workflow".to_owned(),
                workflow_version_id: "version".to_owned(),
                recipe_id: "recipe".to_owned(),
                workflow_version: "1.0.0".to_owned(),
                recipe_version: "1.0.0".to_owned(),
                schema_version: 1,
                recipe_sha256: "sha".to_owned(),
                created_at: Utc::now(),
                is_current_version: false,
                is_promoted: true,
                promoted_at: None,
                archived: true,
                archived_at: None,
                workflow_library_state: "REMOVED".to_owned(),
            },
            counts: RecipeHistoryCounts {
                task_count: 1,
                active_task_count: 0,
                succeeded_task_count: 1,
                failed_task_count: 0,
                cancelled_task_count: 0,
                last_finished_at: None,
                last_attempt_at: None,
                executed_project_count: 1,
                referenced_project_count: 1,
                preset_count: 0,
                project_template_count: 0,
                production_run_template_count: 0,
                queue_item_count: 0,
                shot_stage_count: 0,
                experiment_count: 0,
                benchmark_run_count: 0,
                project_binding_count: 0,
            },
            tasks: PageResult {
                items: Vec::new(),
                next_cursor: None,
            },
            queue_items: Vec::new(),
            presets: Vec::new(),
            project_templates: Vec::new(),
            production_run_templates: Vec::new(),
            project_bindings: Vec::new(),
            shots: Vec::new(),
            experiments: Vec::new(),
        }
    }

    #[tokio::test]
    async fn validates_exact_identity_and_bounds_task_limit() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let service = RecipeHistoryQueryService::new(Arc::new(FakeRepository {
            seen: seen.clone(),
            record: Some(record()),
        }));
        assert!(matches!(
            service.get_exact_pair(" ", "recipe", None, None).await,
            Err(RecipeHistoryQueryError::InvalidIdentity)
        ));
        let view = service
            .get_exact_pair(" version ", " recipe ", None, Some(500))
            .await
            .unwrap();
        assert!(view.archived);
        assert!(view.is_promoted);
        let query = &seen.lock().unwrap()[0];
        assert_eq!(query.workflow_version_id, "version");
        assert_eq!(query.recipe_id, "recipe");
        assert_eq!(query.task_limit, MAX_TASK_LIMIT);
        assert_eq!(DEFAULT_TASK_LIMIT, 20);
        assert!(PageCursor::for_item(Utc::now(), "task").id == "task");
    }
}
