//! DEV-061B Agent B hardening tests for production-package partial truth.

use ai_studio_lib::application::{
    asset_video_prompt_service::AssetVideoPromptService,
    generation_service::GenerationService,
    h3_local_import_service::H3LocalImportService,
    ports::{
        AssetRepository, AssetStore, Clock, ComfyAdapter, ComfyAdapterError,
        ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyOutputData,
        ComfyOutputFile, GenerationDefinitionRepository, GenerationSnapshotRepository,
        NoopTaskUpdateSink, ProductionQueueRepository, ProjectRepository, PromptSubmission,
        SystemStats, TaskRepository,
    },
    production_package_inspector::{ProductionPackageInspector, PRODUCTION_PACKAGE_TYPE},
    production_package_service::{
        ProductionPackageCreateStatus, ProductionPackageH3Config, ProductionPackageService,
    },
    production_queue_service::ProductionQueueService,
    source_asset_import_service::SourceAssetImportService,
    task_recovery_service::TaskRecoveryService,
};
use ai_studio_lib::infrastructure::{
    database::{
        initialize, SqliteAssetRepository, SqliteAssetVideoPromptRepository,
        SqliteGenerationDefinitionRepository, SqliteGenerationSnapshotRepository,
        SqliteProductionQueueRepository, SqliteProjectRepository, SqliteTaskRepository,
    },
    filesystem::FileSystemAssetStore,
    time::SystemClock,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};
use tempfile::tempdir;

const PROJECT_ID: &str = "prj_default";
const WORKFLOW_ID: &str = "dev061b-workflow";
const WORKFLOW_VERSION_ID: &str = "dev061b-workflow-version";
const RECIPE_ID: &str = "dev061b-recipe";
const CREATED_AT: &str = "2026-08-28T00:00:00Z";
const WORKFLOW_JSON: &str = r#"{"6":{"inputs":{"text":""},"class_type":"CLIPTextEncode"},"9":{"inputs":{"images":["6",0]},"class_type":"SaveImage"}}"#;
const RECIPE_YAML: &str = r#"
schema_version: 1
id: dev061b-recipe
name: DEV-061B Fixture Recipe
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  seed:
    type: seed
    label: Seed
    default: random
    min: 0
    max: 4294967295
bindings: []
outputs: []
"#;

struct EmptyComfyEvents;

#[async_trait]
impl ComfyEventSubscription for EmptyComfyEvents {
    async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
        Ok(None)
    }
}

#[derive(Clone)]
struct NoSubmitComfyAdapter {
    submit_calls: Arc<AtomicUsize>,
}

impl NoSubmitComfyAdapter {
    fn new() -> Self {
        Self {
            submit_calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ComfyAdapter for NoSubmitComfyAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Ok(ComfyHealth {
            system: SystemStats {
                comfyui_version: Some("dev061b-test".to_owned()),
                python_version: None,
                os: None,
                ram_total: None,
                ram_free: None,
                devices: Vec::new(),
            },
        })
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Ok(SystemStats {
            comfyui_version: Some("dev061b-test".to_owned()),
            python_version: None,
            os: None,
            ram_total: None,
            ram_free: None,
            devices: Vec::new(),
        })
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        Ok(json!({}))
    }

    async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        Err(ComfyAdapterError::HistoryNotFound(prompt_id.to_owned()))
    }

    async fn download_output(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        Err(ComfyAdapterError::OutputDownload(file.filename.clone()))
    }

    async fn submit_workflow(
        &self,
        _client_id: &str,
        _prompt_id: &str,
        _workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        self.submit_calls.fetch_add(1, Ordering::SeqCst);
        Err(ComfyAdapterError::Incompatible(
            "DEV-061B test must not submit workflows".to_owned(),
        ))
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Ok(Box::new(EmptyComfyEvents))
    }
}

async fn seed_database(pool: &SqlitePool, project_root: &Path) {
    SqliteProjectRepository::new(pool.clone())
        .ensure_default_project(
            PROJECT_ID,
            "DEV-061B package project",
            &project_root.to_path_buf(),
            Utc::now(),
        )
        .await
        .expect("project fixture should be created");

    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES (?, ?, 'video', 'fl2va', NULL, ?, ?)",
    )
    .bind(WORKFLOW_ID)
    .bind("DEV-061B H3 fixture")
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("workflow fixture should be inserted");

    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES (?, ?, '1', ?, 'dev061b-workflow-sha', ?)",
    )
    .bind(WORKFLOW_VERSION_ID)
    .bind(WORKFLOW_ID)
    .bind(WORKFLOW_JSON)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("workflow version fixture should be inserted");

    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml,
          recipe_sha256, created_at)
         VALUES (?, ?, '1', 1, ?, 'dev061b-recipe-sha', ?)",
    )
    .bind(RECIPE_ID)
    .bind(WORKFLOW_VERSION_ID)
    .bind(RECIPE_YAML)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("recipe fixture should be inserted");

    sqlx::query("UPDATE workflows SET current_version_id = ? WHERE id = ?")
        .bind(WORKFLOW_VERSION_ID)
        .bind(WORKFLOW_ID)
        .execute(pool)
        .await
        .expect("current workflow version should be set");
}

async fn write_text_package(root: &Path, item_count: usize) {
    let items = (1..=item_count)
        .map(|ordinal| {
            json!({
                "id": format!("DEV061B-TXT-{ordinal:03}"),
                "name": format!("Text shot {ordinal}"),
                "videoPrompt": format!("DEV-061B frozen prompt {ordinal}"),
                "mode": "TEXT_ONLY"
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({
        "schemaVersion": 1,
        "packageType": PRODUCTION_PACKAGE_TYPE,
        "name": "DEV-061B text package",
        "defaults": {"durationSeconds": 5, "width": 864, "height": 480},
        "items": items
    });
    tokio::fs::write(
        root.join("production-package.json"),
        serde_json::to_vec(&manifest).expect("manifest should serialize"),
    )
    .await
    .expect("manifest should write");
}

fn build_service(pool: &SqlitePool, comfy: Arc<NoSubmitComfyAdapter>) -> ProductionPackageService {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let project_repository: Arc<dyn ProjectRepository> =
        Arc::new(SqliteProjectRepository::new(pool.clone()));
    let asset_repository: Arc<dyn AssetRepository> =
        Arc::new(SqliteAssetRepository::new(pool.clone()));
    let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
    let task_repository: Arc<dyn TaskRepository> =
        Arc::new(SqliteTaskRepository::new(pool.clone()));
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> =
        Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
    let definition_repository: Arc<dyn GenerationDefinitionRepository> =
        Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let comfy_adapter: Arc<dyn ComfyAdapter> = comfy;
    let generation_service = Arc::new(GenerationService::new(
        task_repository.clone(),
        snapshot_repository.clone(),
        definition_repository.clone(),
        comfy_adapter.clone(),
        project_repository.clone(),
        asset_store.clone(),
        asset_repository.clone(),
        clock.clone(),
    ));
    let task_recovery_service = Arc::new(TaskRecoveryService::new(
        task_repository.clone(),
        snapshot_repository,
        asset_repository.clone(),
        comfy_adapter,
        project_repository.clone(),
        asset_store.clone(),
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));
    let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let production_queue_service = Arc::new(ProductionQueueService::new(
        queue_repository.clone(),
        task_repository,
        definition_repository,
        generation_service,
        queue_repository.clone(),
        task_recovery_service,
        clock.clone(),
    ));
    let asset_video_prompt_service = Arc::new(AssetVideoPromptService::new(
        Arc::new(SqliteAssetVideoPromptRepository::new(pool.clone())),
        asset_repository.clone(),
        clock.clone(),
    ));
    let source_asset_import_service = Arc::new(SourceAssetImportService::new(
        project_repository,
        asset_store,
        asset_repository,
        clock.clone(),
    ));
    let h3_local_import_service = Arc::new(H3LocalImportService::new(
        source_asset_import_service.clone(),
        asset_video_prompt_service,
        production_queue_service.clone(),
        clock.clone(),
    ));
    ProductionPackageService::new(
        ProductionPackageInspector::new(),
        h3_local_import_service,
        source_asset_import_service,
        production_queue_service,
        ProductionPackageH3Config {
            workflow_version_id: WORKFLOW_VERSION_ID.to_owned(),
            recipe_id: RECIPE_ID.to_owned(),
            fl2va_workflow_version_id: None,
            fl2va_recipe_id: None,
            ref2va_workflow_version_id: None,
            ref2va_recipe_id: None,
            quality_profile: None,
            quality_recipes: Vec::new(),
        },
        clock,
    )
}

async fn selected_ids(
    service: &ProductionPackageService,
    package_root: PathBuf,
) -> (String, Vec<String>) {
    let (session_id, inspection) = service
        .inspect_session(PROJECT_ID, package_root)
        .await
        .expect("package should inspect");
    let ids = inspection
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    (session_id, ids)
}

#[tokio::test]
async fn complete_500_item_package_is_five_frozen_batches_without_tasks_or_comfy_submit() {
    let directory = tempdir().expect("temporary workspace should exist");
    let pool = initialize(&directory.path().join("dev061b-complete.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;
    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    write_text_package(&package_root, 500).await;

    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let service = build_service(&pool, comfy.clone());
    let inspect_started = Instant::now();
    let (session_id, ids) = selected_ids(&service, package_root).await;
    let inspect_ms = inspect_started.elapsed().as_millis();
    let create_started = Instant::now();
    let result = service
        .create_batches(&session_id, &ids)
        .await
        .expect("all chunks should be created");
    let create_ms = create_started.elapsed().as_millis();

    assert_eq!(result.status, ProductionPackageCreateStatus::Complete);
    assert_eq!(result.status.as_str(), "COMPLETE");
    assert_eq!(result.requested_count, 500);
    assert_eq!(result.created_count, 500);
    assert_eq!(result.remaining_count, 0);
    assert!(result.remaining_item_ids.is_empty());
    assert_eq!(result.batch_count, 5);
    assert_eq!(result.batches.len(), 5);
    assert!(result.batches.iter().all(|batch| batch.item_count == 100));
    assert!(!result.auto_started);

    let frozen_values = sqlx::query_scalar::<_, String>(
        "SELECT values_json FROM production_batch_items ORDER BY created_at, id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("frozen queue values should be readable");
    let frozen_values =
        serde_json::from_str::<Value>(&frozen_values).expect("values should be JSON");
    assert!(frozen_values["prompt"]["value"]
        .as_str()
        .is_some_and(|value| value.starts_with("DEV-061B frozen prompt ")));
    assert_eq!(frozen_values["width"]["value"], 864);
    assert_eq!(frozen_values["height"]["value"], 480);
    assert_eq!(frozen_values["duration_seconds"]["value"], 5);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE project_id = ?")
            .bind(PROJECT_ID)
            .fetch_one(&pool)
            .await
            .expect("task count should be readable"),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
            .fetch_one(&pool)
            .await
            .expect("batch count should be readable"),
        5
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batch_items")
            .fetch_one(&pool)
            .await
            .expect("batch item count should be readable"),
        500
    );
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);

    // Rebuild the package stack before reading queue state. The repository
    // calls below model the bounded queue overview/detail reads used after an
    // application restart rather than relying on the previous service's heap.
    drop(service);
    let reload_started = Instant::now();
    let _reloaded_service = build_service(&pool, comfy.clone());
    let queue_repository = SqliteProductionQueueRepository::new(pool.clone());
    let reloaded_batches = queue_repository
        .list(PROJECT_ID)
        .await
        .expect("reloaded queue batches should be readable");
    assert_eq!(reloaded_batches.len(), 5);
    let mut reloaded_item_count = 0;
    for batch in &reloaded_batches {
        let detail = queue_repository
            .find_detail(PROJECT_ID, &batch.id)
            .await
            .expect("reloaded batch detail should be readable")
            .expect("every listed batch should have detail");
        assert_eq!(detail.items.len(), 100);
        reloaded_item_count += detail.items.len();
    }
    let queue_reload_ms = reload_started.elapsed().as_millis();
    assert_eq!(reloaded_item_count, 500);
    eprintln!(
        "DEV061B_PERF PACKAGE_INSPECT_500_MS={inspect_ms} PACKAGE_CREATE_500_MS={create_ms} QUEUE_RELOAD_5_BATCH_MS={queue_reload_ms}"
    );
}

#[tokio::test]
async fn later_chunk_failure_returns_partial_truth_consumes_session_and_reinspects_authoritatively()
{
    let directory = tempdir().expect("temporary workspace should exist");
    let pool = initialize(&directory.path().join("dev061b-partial.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;
    sqlx::query(
        "CREATE TRIGGER dev061b_fail_second_chunk
         BEFORE INSERT ON production_batches
         WHEN NEW.name LIKE '%2/5'
         BEGIN SELECT RAISE(ABORT, 'DEV-061B forced later chunk failure'); END",
    )
    .execute(&pool)
    .await
    .expect("failure trigger should install");

    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    write_text_package(&package_root, 500).await;
    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let service = build_service(&pool, comfy.clone());
    let (session_id, ids) = selected_ids(&service, package_root.clone()).await;
    let result = service
        .create_batches(&session_id, &ids)
        .await
        .expect("a later chunk failure should be represented as partial");

    assert_eq!(result.status, ProductionPackageCreateStatus::Partial);
    assert_eq!(result.status.as_str(), "PARTIAL");
    assert_eq!(result.requested_count, 500);
    assert_eq!(result.created_count, 100);
    assert_eq!(result.remaining_count, 400);
    assert_eq!(result.remaining_item_ids, ids[100..]);
    assert_eq!(result.batch_count, 1);
    assert_eq!(result.batches[0].item_count, 100);
    assert_eq!(result.item_mappings.len(), 100);
    assert!(!result.auto_started);

    let duplicate = service.create_batches(&session_id, &ids).await;
    assert!(matches!(
        duplicate,
        Err(ai_studio_lib::application::production_package_service::ProductionPackageError::SessionNotFound)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
            .fetch_one(&pool)
            .await
            .expect("persisted batch count should be readable"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batch_items")
            .fetch_one(&pool)
            .await
            .expect("persisted batch item count should be readable"),
        100
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE project_id = ?")
            .bind(PROJECT_ID)
            .fetch_one(&pool)
            .await
            .expect("task count should be readable"),
        0
    );
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);

    // The next inspection reads the changed manifest instead of reusing the
    // consumed 500-item snapshot, which is the authoritative retry seam.
    write_text_package(&package_root, 400).await;
    let (_, reinspected) = service
        .inspect_session(PROJECT_ID, package_root)
        .await
        .expect("remaining work should be re-inspectable");
    assert_eq!(reinspected.item_count, 400);
    assert_eq!(reinspected.items[0].id, "DEV061B-TXT-001");
    assert_eq!(reinspected.items.last().unwrap().id, "DEV061B-TXT-400");
}

#[tokio::test]
async fn partial_resume_uses_only_remaining_items_and_restarts_chunk_provenance() {
    let directory = tempdir().expect("partial resume workspace should exist");
    let pool = initialize(&directory.path().join("dev061b-resume.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;

    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    write_text_package(&package_root, 150).await;

    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let service = build_service(&pool, comfy);
    let (initial_session_id, ids) = selected_ids(&service, package_root.clone()).await;
    let initial_result = service
        .create_batches(&initial_session_id, &ids[..100].to_vec())
        .await
        .expect("the initial 100 items should be created");
    assert_eq!(initial_result.requested_count, 100);
    assert_eq!(initial_result.created_count, 100);
    assert_eq!(initial_result.remaining_count, 0);

    let queue_repository = SqliteProductionQueueRepository::new(pool.clone());
    let old_bindings = queue_repository
        .list_package_bindings(PROJECT_ID)
        .await
        .expect("initial package binding should be readable");
    assert_eq!(old_bindings.len(), 1);
    let old_binding = old_bindings[0].clone();

    let (resume_session_id, resume_ids) = selected_ids(&service, package_root).await;
    let result = service
        .create_batches(&resume_session_id, &resume_ids)
        .await
        .expect("the remaining 50 items should be created");

    assert_eq!(result.status, ProductionPackageCreateStatus::Complete);
    assert_eq!(result.requested_count, 50);
    assert_eq!(result.created_count, 50);
    assert_eq!(result.remaining_count, 0);
    assert!(result.remaining_item_ids.is_empty());
    assert_eq!(result.batch_count, 1);
    assert_eq!(result.item_count, 50);
    assert_eq!(
        result
            .item_mappings
            .iter()
            .map(|mapping| mapping.package_item_id.as_str())
            .collect::<Vec<_>>(),
        ids[100..]
    );

    let bindings = queue_repository
        .list_package_bindings(PROJECT_ID)
        .await
        .expect("resumed package bindings should be readable");
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0], old_binding);
    assert_eq!(bindings[1].chunk_index, 0);
    assert_eq!(bindings[1].chunk_count, 1);
    assert_eq!(bindings[1].package_item_ids, ids[100..]);
}

#[tokio::test]
async fn partial_resume_failure_reports_remaining_chunks_and_preserves_old_bindings() {
    let directory = tempdir().expect("partial failure workspace should exist");
    let pool = initialize(&directory.path().join("dev061b-resume-partial.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;

    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    write_text_package(&package_root, 350).await;

    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let service = build_service(&pool, comfy);
    let (initial_session_id, ids) = selected_ids(&service, package_root.clone()).await;
    service
        .create_batches(&initial_session_id, &ids[..100].to_vec())
        .await
        .expect("the initial 100 items should be created");

    let queue_repository = SqliteProductionQueueRepository::new(pool.clone());
    let old_bindings = queue_repository
        .list_package_bindings(PROJECT_ID)
        .await
        .expect("initial package binding should be readable");
    assert_eq!(old_bindings.len(), 1);
    let old_binding = old_bindings[0].clone();

    sqlx::query(
        "CREATE TRIGGER dev061b_fail_second_resume_chunk
         BEFORE INSERT ON production_batches
         WHEN NEW.name LIKE '%2/3'
         BEGIN SELECT RAISE(ABORT, 'DEV-061B forced resume chunk failure'); END",
    )
    .execute(&pool)
    .await
    .expect("resume failure trigger should install");

    let (resume_session_id, resume_ids) = selected_ids(&service, package_root).await;
    let result = service
        .create_batches(&resume_session_id, &resume_ids)
        .await
        .expect("a later resume chunk failure should be represented as partial");

    assert_eq!(result.status, ProductionPackageCreateStatus::Partial);
    assert_eq!(result.requested_count, 250);
    assert_eq!(result.created_count, 100);
    assert_eq!(result.remaining_count, 150);
    assert_eq!(result.remaining_item_ids, ids[200..]);
    assert_eq!(result.batch_count, 1);
    assert_eq!(result.batches[0].batch_name, "DEV-061B text package · 1/3");
    assert_eq!(result.item_mappings.len(), 100);
    assert_eq!(
        result
            .item_mappings
            .iter()
            .map(|mapping| mapping.package_item_id.as_str())
            .collect::<Vec<_>>(),
        ids[100..200]
    );

    let bindings = queue_repository
        .list_package_bindings(PROJECT_ID)
        .await
        .expect("partial resume bindings should be readable");
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0], old_binding);
    assert_eq!(bindings[1].chunk_index, 0);
    assert_eq!(bindings[1].chunk_count, 3);
    assert_eq!(bindings[1].package_item_ids, ids[100..200]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
            .fetch_one(&pool)
            .await
            .expect("batch count should be readable"),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batch_items")
            .fetch_one(&pool)
            .await
            .expect("batch item count should be readable"),
        200
    );
}

#[tokio::test]
async fn all_bound_package_does_not_create_a_duplicate_batch() {
    let directory = tempdir().expect("duplicate protection workspace should exist");
    let pool = initialize(&directory.path().join("dev061b-duplicate.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;

    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    write_text_package(&package_root, 3).await;

    let service = build_service(&pool, Arc::new(NoSubmitComfyAdapter::new()));
    let (initial_session_id, ids) = selected_ids(&service, package_root.clone()).await;
    service
        .create_batches(&initial_session_id, &ids)
        .await
        .expect("the package should be created once");
    let batch_count_before =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
            .fetch_one(&pool)
            .await
            .expect("initial batch count should be readable");

    let (duplicate_session_id, duplicate_ids) = selected_ids(&service, package_root).await;
    let duplicate = service
        .create_batches(&duplicate_session_id, &duplicate_ids)
        .await;
    assert!(matches!(
        duplicate,
        Err(ai_studio_lib::application::production_package_service::ProductionPackageError::InvalidInput(message))
            if message == "all selected production package items are already bound to production batches"
    ));
    let batch_count_after = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
        .fetch_one(&pool)
        .await
        .expect("duplicate batch count should be readable");
    assert_eq!(batch_count_after, batch_count_before);
}
