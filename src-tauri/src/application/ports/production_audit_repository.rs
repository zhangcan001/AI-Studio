use super::RepositoryError;
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct ProductionAuditGraph {
    pub runs: Vec<ProductionAuditRunRecord>,
    pub stages: Vec<ProductionAuditStageRecord>,
    pub batches: Vec<ProductionAuditBatchRecord>,
    pub batch_items: Vec<ProductionAuditBatchItemRecord>,
    pub stage_items: Vec<ProductionAuditStageItemRecord>,
    pub tasks: Vec<ProductionAuditTaskRecord>,
    pub snapshots: Vec<ProductionAuditSnapshotRecord>,
    pub preparation_snapshots: Vec<ProductionAuditPreparationSnapshotRecord>,
    pub assets: Vec<ProductionAuditAssetRecord>,
    pub task_outputs: Vec<ProductionAuditTaskOutputRecord>,
    pub shots: Vec<ProductionAuditShotRecord>,
    pub shot_links: Vec<ProductionAuditShotLinkRecord>,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditRunRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditStageRecord {
    pub id: String,
    pub run_id: String,
    pub ordinal: i64,
    pub stage_type: String,
    pub status: String,
    pub production_batch_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditBatchRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditBatchItemRecord {
    pub id: String,
    pub batch_id: String,
    pub ordinal: i64,
    pub status: String,
    pub task_id: Option<String>,
    pub retry_of_item_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditStageItemRecord {
    pub id: String,
    pub stage_id: String,
    pub ordinal: i64,
    pub status: String,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub asset_id: Option<String>,
    pub parent_stage_item_id: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditTaskRecord {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditSnapshotRecord {
    pub id: String,
    pub task_id: String,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditPreparationSnapshotRecord {
    pub id: String,
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    pub context_hash: String,
    pub production_batch_id: String,
    pub production_batch_item_id: String,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditSnapshotDetailRecord {
    pub id: String,
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    pub context_hash: String,
    pub production_batch_id: String,
    pub production_batch_item_id: String,
    pub snapshot_json: String,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditAssetRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub source_task_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditTaskOutputRecord {
    pub task_id: String,
    pub output_id: String,
    pub ordinal: i64,
    pub asset_id: String,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditShotRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub selected_image_asset_id: Option<String>,
    pub selected_video_asset_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionAuditShotLinkRecord {
    pub id: String,
    pub shot_id: String,
    pub stage: String,
    pub task_id: Option<String>,
    pub production_batch_item_id: Option<String>,
    pub created_at: String,
}

#[async_trait]
pub trait ProductionAuditRepository: Send + Sync {
    async fn load_project_graph(
        &self,
        project_id: &str,
    ) -> Result<ProductionAuditGraph, RepositoryError>;

    async fn find_snapshot_detail(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
    ) -> Result<Option<ProductionAuditSnapshotDetailRecord>, RepositoryError>;
}
