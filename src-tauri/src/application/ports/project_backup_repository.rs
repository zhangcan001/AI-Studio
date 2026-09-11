use super::{ProjectRecord, RepositoryError};
use crate::application::project_backup_service::{
    BackupDocument, BackupSnapshot, ConsistencyRestoreIds, ProductionStructureIds, RestoredAsset,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ProjectBackupAssetSource {
    pub id: String,
    pub asset_type: String,
    pub category: Option<String>,
    pub name: String,
    pub original_name: Option<String>,
    pub sha256: String,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub file_size: Option<i64>,
    pub source_task_id: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub storage_path: String,
    pub thumbnail_path: Option<String>,
}

#[derive(Clone)]
pub struct ProjectBackupSnapshot {
    pub(crate) document: BackupDocument,
    pub(crate) assets: Vec<ProjectBackupAssetSource>,
}

pub struct ProjectBackupRestorePlan {
    pub(crate) project: ProjectRecord,
    pub(crate) document: BackupDocument,
    pub(crate) task_ids: HashMap<String, String>,
    pub(crate) asset_ids: HashMap<String, String>,
    pub(crate) snapshot_ids: HashMap<String, String>,
    pub(crate) preset_ids: HashMap<String, String>,
    pub(crate) prompt_ids: HashMap<String, String>,
    pub(crate) prompt_version_ids: HashMap<String, String>,
    pub(crate) batch_ids: HashMap<String, String>,
    pub(crate) item_ids: HashMap<String, String>,
    pub(crate) preparation_snapshot_ids: HashMap<String, String>,
    pub(crate) benchmark_experiment_ids: HashMap<String, String>,
    pub(crate) benchmark_candidate_ids: HashMap<String, String>,
    pub(crate) production_run_ids: HashMap<String, String>,
    pub(crate) production_stage_ids: HashMap<String, String>,
    pub(crate) production_stage_item_ids: HashMap<String, String>,
    pub(crate) production_run_template_ids: HashMap<String, String>,
    pub(crate) benchmark_run_ids: HashMap<String, String>,
    pub(crate) benchmark_quality_score_ids: HashMap<String, String>,
    pub(crate) tag_ids: HashMap<String, String>,
    pub(crate) reference_anchor_ids: HashMap<String, String>,
    pub(crate) production_structure_ids: ProductionStructureIds,
    pub(crate) handoff_ids: HashMap<String, String>,
    pub(crate) script_source_ids: HashMap<String, String>,
    pub(crate) script_draft_ids: HashMap<String, String>,
    pub(crate) script_revision_ids: HashMap<String, String>,
    pub(crate) consistency_ids: ConsistencyRestoreIds,
    pub(crate) shot_ids: HashMap<String, String>,
    pub(crate) shot_generation_link_ids: HashMap<String, String>,
    pub(crate) restored_assets: Vec<RestoredAsset>,
    pub(crate) restored_snapshots: Vec<BackupSnapshot>,
}

#[async_trait]
pub trait ProjectBackupRepository: Send + Sync {
    async fn load_export_snapshot(
        &self,
        project_id: &str,
    ) -> Result<ProjectBackupSnapshot, RepositoryError>;

    async fn find_missing_workflows(
        &self,
        document: &BackupDocument,
    ) -> Result<Vec<String>, RepositoryError>;

    async fn restore_atomic(&self, plan: ProjectBackupRestorePlan) -> Result<(), RepositoryError>;
}

pub trait ProjectBackupRepositorySource {
    fn into_repository(self) -> Arc<dyn ProjectBackupRepository>;
}

impl ProjectBackupRepositorySource for Arc<dyn ProjectBackupRepository> {
    fn into_repository(self) -> Arc<dyn ProjectBackupRepository> {
        self
    }
}

impl<T> ProjectBackupRepositorySource for Arc<T>
where
    T: ProjectBackupRepository + 'static,
{
    fn into_repository(self) -> Arc<dyn ProjectBackupRepository> {
        self
    }
}
