use super::RepositoryError;
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct ProductionRunRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub current_stage_ordinal: i64,
    pub template_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProductionStageRecord {
    pub id: String,
    pub run_id: String,
    pub ordinal: i64,
    pub stage_type: String,
    pub status: String,
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub production_batch_id: Option<String>,
    pub frozen_config_json: String,
    pub prompt: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProductionStageItemRecord {
    pub id: String,
    pub stage_id: String,
    pub ordinal: i64,
    pub status: String,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub task_status: Option<String>,
    pub asset_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub reference_index: Option<i64>,
    pub attempt: i64,
    pub submission_idempotency_key: Option<String>,
    pub parent_stage_item_id: Option<String>,
    pub frozen_values_json: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProductionRunTemplateRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub krea2_workflow_version_id: Option<String>,
    pub krea2_recipe_id: Option<String>,
    pub krea2_preset_id: Option<String>,
    pub default_image_count: i64,
    pub h3_workflow_version_id: Option<String>,
    pub h3_recipe_id: Option<String>,
    pub h3_profile: Option<String>,
    pub default_duration_seconds: Option<i64>,
    pub default_width: Option<i64>,
    pub default_height: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct SelectedReferenceRecord {
    pub asset_id: Option<String>,
    pub reference_index: i64,
}

#[derive(Clone, Debug)]
pub struct ProductionRunSnapshot {
    pub run: ProductionRunRecord,
    pub stages: Vec<ProductionStageRecord>,
    pub items: Vec<ProductionStageItemRecord>,
}

#[derive(Clone, Debug, Default)]
pub struct ProductionStageStats {
    pub total: i64,
    pub succeeded: i64,
    pub terminal: i64,
    pub pending: i64,
    pub active: i64,
}

#[derive(Clone, Debug)]
pub struct ProductionBatchTarget {
    pub id: String,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct ProductionStageItemDraft {
    pub ordinal: i64,
    pub status: String,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub asset_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub reference_index: Option<i64>,
    pub attempt: i64,
    pub parent_stage_item_id: Option<String>,
    pub frozen_values_json: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[async_trait]
pub trait ProductionOrchestratorRepository: Send + Sync {
    async fn template_exists(
        &self,
        project_id: &str,
        template_id: &str,
    ) -> Result<bool, RepositoryError>;

    async fn create_run_atomic(
        &self,
        run: &ProductionRunRecord,
        stages: &[ProductionStageRecord],
    ) -> Result<(), RepositoryError>;

    async fn list_runs(
        &self,
        project_id: &str,
        limit: i64,
    ) -> Result<Vec<ProductionRunRecord>, RepositoryError>;

    async fn load_run(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<Option<ProductionRunRecord>, RepositoryError>;

    async fn load_run_by_id(
        &self,
        run_id: &str,
    ) -> Result<Option<ProductionRunRecord>, RepositoryError>;

    async fn load_run_snapshot(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<Option<ProductionRunSnapshot>, RepositoryError>;

    async fn load_stage(
        &self,
        run_id: &str,
        ordinal: i64,
    ) -> Result<Option<ProductionStageRecord>, RepositoryError>;

    async fn load_selected_assets(
        &self,
        selection_stage_id: &str,
    ) -> Result<Vec<SelectedReferenceRecord>, RepositoryError>;

    async fn validate_generated_assets(
        &self,
        project_id: &str,
        stage_id: &str,
        asset_ids: &[String],
    ) -> Result<Vec<String>, RepositoryError>;

    async fn attach_image_batch_atomic(
        &self,
        run_id: &str,
        stage_id: &str,
        batch_id: &str,
        items: &[ProductionStageItemDraft],
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn save_selection_atomic(
        &self,
        run_id: &str,
        selection_stage_id: &str,
        items: &[ProductionStageItemDraft],
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn prepare_h3_atomic(
        &self,
        run_id: &str,
        stage_id: &str,
        batch_id: &str,
        items: &[ProductionStageItemDraft],
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn find_retry_source_item(
        &self,
        stage_id: &str,
        batch_id: &str,
    ) -> Result<Option<String>, RepositoryError>;

    async fn retry_metadata(&self, stage_id: &str) -> Result<(i64, i64), RepositoryError>;

    async fn prepare_retry_atomic(
        &self,
        stage_id: &str,
        items: &[ProductionStageItemDraft],
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn load_cancel_targets(
        &self,
        run_id: &str,
    ) -> Result<(Vec<String>, Vec<ProductionBatchTarget>), RepositoryError>;

    async fn cancel_run_atomic(&self, run_id: &str, now: &str) -> Result<(), RepositoryError>;

    async fn save_template(
        &self,
        template: &ProductionRunTemplateRecord,
    ) -> Result<(), RepositoryError>;

    async fn list_templates(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProductionRunTemplateRecord>, RepositoryError>;

    async fn load_template(
        &self,
        project_id: &str,
        template_id: &str,
    ) -> Result<Option<ProductionRunTemplateRecord>, RepositoryError>;

    async fn sync_run_persistence(&self, run_id: &str, now: &str) -> Result<(), RepositoryError>;

    async fn load_batch_status(&self, batch_id: &str) -> Result<Option<String>, RepositoryError>;

    async fn stage_stats(
        &self,
        stage_id: &str,
        batch_id: Option<&str>,
    ) -> Result<ProductionStageStats, RepositoryError>;

    async fn mark_stage_running(
        &self,
        run_id: &str,
        ordinal: i64,
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn update_stage_status(
        &self,
        run_id: &str,
        ordinal: i64,
        status: &str,
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn update_run_status(
        &self,
        run_id: &str,
        status: &str,
        ordinal: i64,
        finished_at: Option<&str>,
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn reset_run_waiting(&self, run_id: &str, now: &str) -> Result<(), RepositoryError>;
}
