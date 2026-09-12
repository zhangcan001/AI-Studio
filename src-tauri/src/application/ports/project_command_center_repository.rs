use super::RepositoryError;
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterData {
    pub project: Option<ProjectCommandCenterProjectRecord>,
    pub structure: ProjectCommandCenterStructureRecord,
    pub scenes: Vec<ProjectCommandCenterSceneRecord>,
    pub shots: Vec<ProjectCommandCenterShotRecord>,
    pub shot_configs: Vec<ProjectCommandCenterShotConfigRecord>,
    pub shot_links: Vec<ProjectCommandCenterShotLinkRecord>,
    pub queue_batches: Vec<ProjectCommandCenterQueueBatchRecord>,
    pub queue_items: Vec<ProjectCommandCenterQueueItemRecord>,
    pub task_counts: Vec<ProjectCommandCenterCountRecord>,
    pub asset_counts: Vec<ProjectCommandCenterAssetCountRecord>,
    pub consistency: ProjectCommandCenterConsistencyRecord,
    pub preparation: ProjectCommandCenterPreparationRecord,
    pub reference_anchors: Vec<ProjectCommandCenterReferenceAnchorRecord>,
    pub prompt_entries: Vec<ProjectCommandCenterPromptLibraryRecord>,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterProjectRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterStructureRecord {
    pub series_count: i64,
    pub episode_count: i64,
    pub scene_count: i64,
    pub assigned_shot_count: i64,
    pub unassigned_shot_count: i64,
    pub first_unassigned_shot_id: Option<String>,
    pub orphan_count: i64,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterSceneRecord {
    pub id: String,
    pub name: String,
    pub series_name: String,
    pub episode_name: String,
    pub total: i64,
    pub completed: i64,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterShotRecord {
    pub id: String,
    pub name: String,
    pub assigned: i64,
    pub selected_image_asset_id: Option<String>,
    pub selected_video_asset_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterShotConfigRecord {
    pub shot_id: String,
    pub stage: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterShotLinkRecord {
    pub shot_id: String,
    pub stage: String,
    pub task_id: Option<String>,
    pub task_status: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterQueueBatchRecord {
    pub id: String,
    pub status: String,
    pub archived_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterQueueItemRecord {
    pub id: String,
    pub batch_id: String,
    pub ordinal: i64,
    pub shot_id: Option<String>,
    pub task_id: Option<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub status: String,
    pub retry_of_item_id: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterCountRecord {
    pub status: String,
    pub count: i64,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterAssetCountRecord {
    pub asset_type: String,
    pub count: i64,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterConsistencyRecord {
    pub character_profiles: i64,
    pub scene_profiles: i64,
    pub prop_profiles: i64,
    pub style_profiles: i64,
    pub reference_sets: i64,
    pub shot_profile_bindings: i64,
    pub shot_reference_set_bindings: i64,
    pub scope_profile_bindings: i64,
    pub scope_reference_set_bindings: i64,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterPreparationRecord {
    pub snapshot_count: i64,
    pub prepared_image_items: i64,
    pub prepared_video_items: i64,
    pub active_prepared_items: i64,
    pub latest_prepared_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterReferenceAnchorRecord {
    pub kind: String,
    pub asset_count: i64,
}

#[derive(Clone, Debug)]
pub struct ProjectCommandCenterPromptLibraryRecord {
    pub id: String,
    pub name: String,
    pub version_count: i64,
    pub updated_at: String,
}

#[async_trait]
pub trait ProjectCommandCenterRepository: Send + Sync {
    async fn load_project_command_center_data(
        &self,
        project_id: &str,
    ) -> Result<ProjectCommandCenterData, RepositoryError>;
}
