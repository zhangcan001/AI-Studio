use ai_studio_lib::application::{
    generation_service::GenerationService,
    ports::{
        AssetRepository, AssetStore, Clock, ComfyAdapter, ComfyAdapterError,
        ComfyEventSubscription, ComfyHealth, ComfyHistory, ComfyOutputData, ComfyOutputFile,
        GenerationDefinitionRepository, GenerationSnapshotRepository, NoopTaskUpdateSink,
        ProductionQueueRepository, ProjectRepository, PromptSubmission, ShotBatchRepository,
        SystemStats, TaskRepository, WorkflowBenchmarkCandidateRecord, WorkflowBenchmarkDraft,
        WorkflowBenchmarkExperimentRecord, WorkflowBenchmarkRepository,
    },
    production_queue_service::ProductionQueueService,
    task_recovery_service::TaskRecoveryService,
    workflow_benchmark_service::{WorkflowBenchmarkError, WorkflowBenchmarkService},
};
use ai_studio_lib::infrastructure::{
    database::{
        initialize, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
        SqliteGenerationSnapshotRepository, SqlitePresetRepository,
        SqliteProductionQueueRepository, SqliteProjectRepository, SqliteTaskRepository,
        SqliteWorkflowBenchmarkRepository,
    },
    filesystem::FileSystemAssetStore,
};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};

const PROJECT_ID: &str = "prj_default";
const WORKFLOW_ID: &str = "workflow-1";
const WORKFLOW_VERSION_ID: &str = "workflow-version-1";
const RECIPE_ID: &str = "recipe-1";
const EXPERIMENT_ID: &str = "benchmark-queue-failure";
const NOW: &str = "2026-01-02T00:00:00Z";

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap()
    }
}

struct UnusedComfy;

#[async_trait]
impl ComfyAdapter for UnusedComfy {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "Comfy is not used".to_owned(),
        ))
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "Comfy is not used".to_owned(),
        ))
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "Comfy is not used".to_owned(),
        ))
    }

    async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "Comfy is not used".to_owned(),
        ))
    }

    async fn download_output(
        &self,
        _file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "Comfy is not used".to_owned(),
        ))
    }

    async fn submit_workflow(
        &self,
        _client_id: &str,
        _prompt_id: &str,
        _workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "Comfy is not used".to_owned(),
        ))
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "Comfy is not used".to_owned(),
        ))
    }
}

async fn seed_database(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES (?, 'Benchmark test project', NULL, 'C:/project', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(NOW)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("project fixture should insert");
    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES (?, 'Benchmark test workflow', 'test', 'image', ?, ?, ?)",
    )
    .bind(WORKFLOW_ID)
    .bind(WORKFLOW_VERSION_ID)
    .bind(NOW)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("workflow fixture should insert");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES (?, ?, '1', '{}', 'workflow-sha', ?)",
    )
    .bind(WORKFLOW_VERSION_ID)
    .bind(WORKFLOW_ID)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("workflow version fixture should insert");
    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
         VALUES (?, ?, '1', 1, 'not a valid recipe', 'recipe-sha', ?)",
    )
    .bind(RECIPE_ID)
    .bind(WORKFLOW_VERSION_ID)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("recipe fixture should insert");
}

fn candidate(id: &str, position: i64) -> WorkflowBenchmarkCandidateRecord {
    WorkflowBenchmarkCandidateRecord {
        id: id.to_owned(),
        position,
        workflow_version_id: WORKFLOW_VERSION_ID.to_owned(),
        recipe_id: RECIPE_ID.to_owned(),
        preset_id: None,
        preset_name: None,
        label: format!("Candidate {position}"),
        values_json: "{}".to_owned(),
        asset_ids_json: "[]".to_owned(),
        production_batch_item_id: None,
        task_id: None,
        workflow_id: Some(WORKFLOW_ID.to_owned()),
        workflow_version: Some("1".to_owned()),
        workflow_sha256: Some("workflow-sha".to_owned()),
        recipe_version: Some("1".to_owned()),
        recipe_sha256: Some("recipe-sha".to_owned()),
        runtime_package: None,
        runtime_profile: None,
    }
}

async fn setup() -> (TempDir, SqlitePool, WorkflowBenchmarkService) {
    let directory = tempdir().expect("temporary directory should exist");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("database should initialize");
    seed_database(&pool).await;

    let benchmark_repository = Arc::new(SqliteWorkflowBenchmarkRepository::new(pool.clone()));
    benchmark_repository
        .create_draft_atomic(&WorkflowBenchmarkDraft {
            experiment: WorkflowBenchmarkExperimentRecord {
                id: EXPERIMENT_ID.to_owned(),
                project_id: PROJECT_ID.to_owned(),
                name: "Queue failure regression".to_owned(),
                media_type: "IMAGE".to_owned(),
                status: "DRAFT".to_owned(),
                base_values_json: "{}".to_owned(),
                asset_ids_json: "[]".to_owned(),
                winner_candidate_id: None,
                production_batch_id: None,
                seed_strategy: "FIXED_SEED".to_owned(),
                fixed_seed: Some("42".to_owned()),
                repeat_count: 1,
                recommendation_type: None,
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
            },
            candidates: vec![candidate("candidate-1", 0), candidate("candidate-2", 1)],
            repeat_count: 1,
        })
        .await
        .expect("benchmark draft fixture should insert");

    let clock: Arc<dyn Clock> = Arc::new(FixedClock);
    let project_repository: Arc<dyn ProjectRepository> =
        Arc::new(SqliteProjectRepository::new(pool.clone()));
    let asset_repository: Arc<dyn AssetRepository> =
        Arc::new(SqliteAssetRepository::new(pool.clone()));
    let task_repository: Arc<dyn TaskRepository> =
        Arc::new(SqliteTaskRepository::new(pool.clone()));
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> =
        Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
    let definition_repository: Arc<dyn GenerationDefinitionRepository> =
        Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let comfy: Arc<dyn ComfyAdapter> = Arc::new(UnusedComfy);
    let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
    let generation_service = Arc::new(GenerationService::new(
        task_repository.clone(),
        snapshot_repository.clone(),
        definition_repository.clone(),
        comfy.clone(),
        project_repository.clone(),
        asset_store.clone(),
        asset_repository.clone(),
        clock.clone(),
    ));
    let recovery_service = Arc::new(TaskRecoveryService::new(
        task_repository.clone(),
        snapshot_repository,
        asset_repository,
        comfy,
        project_repository,
        asset_store,
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));
    let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let queue_repository_port: Arc<dyn ProductionQueueRepository> = queue_repository.clone();
    let shot_batch_repository: Arc<dyn ShotBatchRepository> = queue_repository;
    let queue_service = Arc::new(ProductionQueueService::new(
        queue_repository_port,
        task_repository,
        definition_repository.clone(),
        generation_service,
        shot_batch_repository,
        recovery_service,
        clock.clone(),
    ));
    let preset_repository = Arc::new(SqlitePresetRepository::new(pool.clone()));
    let service = WorkflowBenchmarkService::new(
        benchmark_repository,
        definition_repository,
        preset_repository,
        queue_service,
        clock,
    );
    (directory, pool, service)
}

#[tokio::test]
async fn queue_creation_failure_marks_benchmark_failed_to_queue() {
    let (_directory, pool, service) = setup().await;
    let error = service
        .queue_existing(PROJECT_ID, EXPERIMENT_ID, false)
        .await
        .expect_err("invalid queue recipe should fail queue creation");
    assert!(
        matches!(error, WorkflowBenchmarkError::Queue(_)),
        "unexpected queue_existing error: {error:?}"
    );
    let status =
        sqlx::query_scalar::<_, String>("SELECT status FROM benchmark_experiments WHERE id = ?")
            .bind(EXPERIMENT_ID)
            .fetch_one(&pool)
            .await
            .expect("benchmark status should be queryable");
    assert_eq!(status, "FAILED_TO_QUEUE");
    let queue_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
        .fetch_one(&pool)
        .await
        .expect("queue count should be queryable");
    assert_eq!(
        queue_count, 0,
        "failed queue creation must not persist a batch"
    );
}
