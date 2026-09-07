use super::RepositoryError;
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct WorkflowBenchmarkExperimentRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub media_type: String,
    pub status: String,
    pub base_values_json: String,
    pub asset_ids_json: String,
    pub winner_candidate_id: Option<String>,
    pub production_batch_id: Option<String>,
    pub seed_strategy: String,
    pub fixed_seed: Option<String>,
    pub repeat_count: i64,
    pub recommendation_type: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct WorkflowBenchmarkCandidateRecord {
    pub id: String,
    pub position: i64,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub preset_id: Option<String>,
    pub preset_name: Option<String>,
    pub label: String,
    pub values_json: String,
    pub asset_ids_json: String,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub workflow_id: Option<String>,
    pub workflow_version: Option<String>,
    pub workflow_sha256: Option<String>,
    pub recipe_version: Option<String>,
    pub recipe_sha256: Option<String>,
    pub runtime_package: Option<String>,
    pub runtime_profile: Option<String>,
}

#[derive(Clone, Debug)]
pub struct WorkflowBenchmarkRunRecord {
    pub id: String,
    pub candidate_id: String,
    pub run_number: i64,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub output_asset_id: Option<String>,
    pub generation_execution_id: Option<String>,
    pub compiled_workflow_sha256: Option<String>,
    pub runtime_profile: Option<String>,
    pub concurrency_class: Option<String>,
    pub queue_wait_ms: Option<i64>,
    pub prepare_ms: Option<i64>,
    pub submit_ms: Option<i64>,
    pub comfy_execution_ms: Option<i64>,
    pub collect_ms: Option<i64>,
    pub total_ms: Option<i64>,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub output_file_size: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct WorkflowBenchmarkQualityRecord {
    pub candidate_id: String,
    pub prompt_adherence: Option<i64>,
    pub visual_quality: Option<i64>,
    pub motion_quality: Option<i64>,
    pub reference_consistency: Option<i64>,
    pub overall: Option<i64>,
    pub note: Option<String>,
}

#[derive(Clone, Debug)]
pub struct WorkflowBenchmarkReviewRecord {
    pub production_batch_item_id: String,
    pub review_status: String,
    pub review_note: String,
}

#[derive(Clone, Debug)]
pub struct WorkflowBenchmarkSnapshot {
    pub experiment: WorkflowBenchmarkExperimentRecord,
    pub candidates: Vec<WorkflowBenchmarkCandidateRecord>,
    pub runs: Vec<WorkflowBenchmarkRunRecord>,
    pub quality: Vec<WorkflowBenchmarkQualityRecord>,
    pub reviews: Vec<WorkflowBenchmarkReviewRecord>,
}

#[derive(Clone, Debug)]
pub struct WorkflowBenchmarkDraft {
    pub experiment: WorkflowBenchmarkExperimentRecord,
    pub candidates: Vec<WorkflowBenchmarkCandidateRecord>,
    pub repeat_count: u32,
}

#[derive(Clone, Debug)]
pub struct WorkflowBenchmarkQueueLink {
    pub production_batch_item_id: String,
    pub candidate_position: u32,
    pub run_number: u32,
    pub values_json: String,
}

#[async_trait]
pub trait WorkflowBenchmarkRepository: Send + Sync {
    async fn list_experiments(
        &self,
        project_id: &str,
        limit: i64,
    ) -> Result<Vec<WorkflowBenchmarkExperimentRecord>, RepositoryError>;

    async fn load_experiment_snapshot(
        &self,
        project_id: &str,
        experiment_id: &str,
        updated_at: &str,
    ) -> Result<Option<WorkflowBenchmarkSnapshot>, RepositoryError>;

    async fn create_draft_atomic(
        &self,
        draft: &WorkflowBenchmarkDraft,
    ) -> Result<(), RepositoryError>;

    async fn candidate_belongs_to_experiment(
        &self,
        project_id: &str,
        experiment_id: &str,
        candidate_id: &str,
    ) -> Result<bool, RepositoryError>;

    async fn set_winner(
        &self,
        project_id: &str,
        experiment_id: &str,
        candidate_id: Option<&str>,
        updated_at: &str,
    ) -> Result<bool, RepositoryError>;

    async fn set_recommendation(
        &self,
        project_id: &str,
        experiment_id: &str,
        recommendation_type: Option<&str>,
        updated_at: &str,
    ) -> Result<bool, RepositoryError>;

    async fn save_quality(
        &self,
        candidate_id: &str,
        prompt_adherence: Option<i64>,
        visual_quality: Option<i64>,
        motion_quality: Option<i64>,
        reference_consistency: Option<i64>,
        overall: Option<i64>,
        note: Option<&str>,
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn clone_experiment_atomic(
        &self,
        project_id: &str,
        experiment_id: &str,
        new_id: &str,
        name: &str,
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn verify_frozen_assets(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<(), RepositoryError>;

    async fn link_queue_atomic(
        &self,
        experiment_id: &str,
        batch_id: &str,
        links: &[WorkflowBenchmarkQueueLink],
        updated_at: &str,
    ) -> Result<(), RepositoryError>;

    async fn mark_queue_link_failed(
        &self,
        experiment_id: &str,
        batch_id: &str,
        updated_at: &str,
    ) -> Result<(), RepositoryError>;

    async fn set_status(
        &self,
        experiment_id: &str,
        status: &str,
        updated_at: &str,
    ) -> Result<(), RepositoryError>;

    async fn delete_atomic(
        &self,
        project_id: &str,
        experiment_id: &str,
    ) -> Result<bool, RepositoryError>;
}
