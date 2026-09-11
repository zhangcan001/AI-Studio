use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryQuery {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub task_cursor: Option<PageCursor>,
    pub task_limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryTaskRecord {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryQueueRecord {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryPresetRecord {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub project_name: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryProjectTemplateRecord {
    pub id: String,
    pub name: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryProductionRunTemplateRecord {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub project_name: String,
    pub recipe_role: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryBindingRecord {
    pub project_id: String,
    pub project_name: String,
    pub stage: String,
    pub mode: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryShotRecord {
    pub shot_id: String,
    pub project_id: String,
    pub project_name: String,
    pub stage: String,
    pub updated_at: DateTime<Utc>,
    pub task_id: Option<String>,
    pub production_batch_item_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryExperimentRecord {
    pub experiment_id: String,
    pub experiment_name: String,
    pub project_id: String,
    pub project_name: String,
    pub candidate_id: String,
    pub run_count: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryDefinitionRecord {
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryCounts {
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeHistoryQueryRecord {
    pub definition: RecipeHistoryDefinitionRecord,
    pub counts: RecipeHistoryCounts,
    pub tasks: PageResult<RecipeHistoryTaskRecord>,
    pub queue_items: Vec<RecipeHistoryQueueRecord>,
    pub presets: Vec<RecipeHistoryPresetRecord>,
    pub project_templates: Vec<RecipeHistoryProjectTemplateRecord>,
    pub production_run_templates: Vec<RecipeHistoryProductionRunTemplateRecord>,
    pub project_bindings: Vec<RecipeHistoryBindingRecord>,
    pub shots: Vec<RecipeHistoryShotRecord>,
    pub experiments: Vec<RecipeHistoryExperimentRecord>,
}

#[async_trait]
pub trait RecipeHistoryQueryRepository: Send + Sync {
    async fn get_exact_pair(
        &self,
        query: RecipeHistoryQuery,
    ) -> Result<Option<RecipeHistoryQueryRecord>, RepositoryError>;
}
