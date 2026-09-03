//! DEV-059 Agent C contract tests.
//!
//! The service is wired by Main after all package-owned modules land. These
//! source-level tests keep the ownership boundary verifiable in the interim:
//! the adapter must use the existing H3/session path and may not grow a second
//! queue, executor, or direct Comfy transport.

use ai_studio_lib::application::{
    asset_video_prompt_service::AssetVideoPromptService,
    comfy_service::{ComfyRuntime, ComfyService},
    generation_service::GenerationService,
    h3_local_import_service::H3LocalImportService,
    ports::{
        AssetRepository, AssetStore, AssetVideoPromptRepository, Clock, ComfyAdapter,
        ComfyAdapterError, ComfyConnectionConfig, ComfyEventSubscription, ComfyExecutionEvent,
        ComfyHealth, ComfyHistory, ComfyOutputData, ComfyOutputFile,
        GenerationDefinitionRepository, GenerationSnapshotRepository, NoopTaskUpdateSink,
        ProductionQueueRepository, ProjectRepository, ProjectWorkflowBindingRepository,
        PromptSubmission, ShotBatchRepository, SystemStats, TaskRepository, WorkflowLibrarySource,
        WorkflowLibrarySourceError, WorkflowPackageFiles, WorkflowPackageLoad,
        WorkflowPackageStore, WorkflowRunRepository, WorkflowRuntimeRepository,
        WorkflowRuntimeStateRepository,
    },
    production_package_inspector::{
        ProductionPackageInspector, ProductionPackageItemStatus, PRODUCTION_PACKAGE_TYPE,
    },
    production_package_service::{ProductionPackageH3Config, ProductionPackageService},
    production_queue_service::ProductionQueueService,
    production_start_admission_service::ProductionStartAdmissionService,
    project_workflow_binding_service::ProjectWorkflowBindingService,
    source_asset_import_service::SourceAssetImportService,
    task_recovery_service::TaskRecoveryService,
    workflow_library_service::WorkflowLibraryService,
    workflow_lifecycle_service::WorkflowLifecycleService,
    workflow_onboarding_service::WorkflowOnboardingService,
};
use ai_studio_lib::infrastructure::{
    database::{
        initialize, SqliteAssetRepository, SqliteAssetVideoPromptRepository,
        SqliteGenerationDefinitionRepository, SqliteGenerationSnapshotRepository,
        SqliteProductionQueueRepository, SqliteProjectRepository,
        SqliteProjectWorkflowBindingRepository, SqliteTaskRepository,
        SqliteWorkflowLibraryRepository, SqliteWorkflowRunRepository,
        SqliteWorkflowRuntimeRepository, SqliteWorkflowRuntimeStateRepository,
    },
    filesystem::{FileSystemAssetStore, FileSystemWorkflowPackageStore},
    time::SystemClock,
};
use async_trait::async_trait;
use chrono::Utc;
use image::{DynamicImage, ImageFormat};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tempfile::tempdir;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const DEV059_PROJECT_ID: &str = "prj_default";
const DEV059_WORKFLOW_ID: &str = "dev059-workflow";
const DEV059_WORKFLOW_VERSION_ID: &str = "dev059-workflow-version";
const DEV059_RECIPE_ID: &str = "dev059-recipe";
const DEV059_CREATED_AT: &str = "2026-08-28T00:00:00Z";

const DEV059_WORKFLOW_JSON: &str = r#"{"6":{"inputs":{"text":""},"class_type":"CLIPTextEncode"},"9":{"inputs":{"images":["6",0]},"class_type":"SaveImage"}}"#;

const DEV059_RECIPE_YAML: &str = r#"
schema_version: 1
id: dev059-recipe
name: DEV-059 Fixture Recipe
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

const DEV080_WORKFLOW_ID: &str = "wfl_dev080_package_custom";
const DEV080_WORKFLOW_VERSION_ID: &str = "wfv_dev080_package_custom";
const DEV080_RECIPE_ID: &str = "rcp_dev080_package_custom";
const DEV080_WORKFLOW_JSON: &str = r#"{"1":{"inputs":{},"class_type":"SaveVideo"}}"#;
const DEV080_RECIPE_YAML: &str = r#"
schema_version: 1
id: rcp_dev080_package_custom
name: DEV-080 custom package video Recipe
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  duration_seconds:
    type: integer
    label: Duration
    required: true
    default: 5
    min: 1
    max: 15
    step: 1
  width:
    type: integer
    label: Width
    required: true
    default: 960
    min: 608
    max: 2048
    step: 1
  height:
    type: integer
    label: Height
    required: true
    default: 544
    min: 352
    max: 1152
    step: 1
  seed:
    type: seed
    label: Seed
    default: random
    min: 0
    max: 4294967295
bindings: []
outputs:
  - id: video
    type: video
    node: "1"
    required: true
"#;

#[derive(Clone)]
struct StaticWorkflowSource {
    package: WorkflowPackageFiles,
}

#[async_trait]
impl WorkflowLibrarySource for StaticWorkflowSource {
    async fn load_packages(&self) -> Result<Vec<WorkflowPackageLoad>, WorkflowLibrarySourceError> {
        Ok(vec![WorkflowPackageLoad::Loaded(self.package.clone())])
    }
}

fn dev080_runtime_package() -> WorkflowPackageFiles {
    WorkflowPackageFiles {
        package_name: "dev080_custom_package".to_owned(),
        package_source_path: None,
        manifest_yaml: format!(
            "schema_version: 1\nid: {DEV080_WORKFLOW_ID}\nname: DEV-080 custom package\nworkflow_version: 1\nrecipe_version: 1\ncategory: video\nmode: fl2va\n"
        ),
        recipe_yaml: DEV080_RECIPE_YAML.to_owned(),
        workflow_json: DEV080_WORKFLOW_JSON.to_owned(),
    }
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn repo_root() -> PathBuf {
    Path::new(ROOT)
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf()
}

fn service_source() -> String {
    fs::read_to_string(repo_root().join("src-tauri/src/application/production_package_service.rs"))
        .expect("DEV-059 package service should exist")
        .replace("\r\n", "\n")
}

fn assert_contains_all(source: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            source.contains(needle),
            "missing DEV-059 contract fragment: {needle}"
        );
    }
}

fn assert_contains_none(source: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            !source.contains(needle),
            "forbidden DEV-059 contract fragment: {needle}"
        );
    }
}

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
    accept_submission: Arc<AtomicBool>,
    submitted_workflow: Arc<Mutex<Option<Value>>>,
}

impl NoSubmitComfyAdapter {
    fn new() -> Self {
        Self {
            submit_calls: Arc::new(AtomicUsize::new(0)),
            accept_submission: Arc::new(AtomicBool::new(false)),
            submitted_workflow: Arc::new(Mutex::new(None)),
        }
    }

    fn runtime() -> Self {
        let adapter = Self::new();
        adapter.accept_submission.store(true, Ordering::SeqCst);
        adapter
    }

    fn submitted_workflow(&self) -> Value {
        self.submitted_workflow
            .lock()
            .expect("submitted workflow lock should not be poisoned")
            .clone()
            .expect("fake Comfy executor should receive one workflow")
    }
}

#[async_trait]
impl ComfyAdapter for NoSubmitComfyAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Ok(ComfyHealth {
            system: SystemStats {
                comfyui_version: Some("dev059-test".to_owned()),
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
            comfyui_version: Some("dev059-test".to_owned()),
            python_version: None,
            os: None,
            ram_total: None,
            ram_free: None,
            devices: Vec::new(),
        })
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        if self.accept_submission.load(Ordering::SeqCst) {
            Ok(json!({"SaveVideo": {}}))
        } else {
            Ok(json!({}))
        }
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
        prompt_id: &str,
        workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        self.submit_calls.fetch_add(1, Ordering::SeqCst);
        if self.accept_submission.load(Ordering::SeqCst) {
            *self
                .submitted_workflow
                .lock()
                .expect("submitted workflow lock should not be poisoned") = Some(workflow);
            return Ok(PromptSubmission {
                prompt_id: prompt_id.to_owned(),
                number: Some(1),
                node_errors: json!({}),
            });
        }
        Err(ComfyAdapterError::Incompatible(
            "DEV-059 package test must not submit workflows".to_owned(),
        ))
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Ok(Box::new(EmptyComfyEvents))
    }
}

fn png_bytes() -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    DynamicImage::new_rgba8(1, 1)
        .write_to(&mut output, ImageFormat::Png)
        .expect("DEV-059 PNG fixture should encode");
    output.into_inner()
}

async fn seed_dev059_database(pool: &SqlitePool, project_root: &Path) {
    let project_repository = SqliteProjectRepository::new(pool.clone());
    project_repository
        .ensure_default_project(
            DEV059_PROJECT_ID,
            "DEV-059 package project",
            &project_root.to_path_buf(),
            Utc::now(),
        )
        .await
        .expect("DEV-059 project fixture should be created");

    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES (?, ?, 'video', 'fl2va', NULL, ?, ?)",
    )
    .bind(DEV059_WORKFLOW_ID)
    .bind("DEV-059 H3 fixture")
    .bind(DEV059_CREATED_AT)
    .bind(DEV059_CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV-059 workflow fixture should be inserted");

    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES (?, ?, '1', ?, 'dev059-workflow-sha', ?)",
    )
    .bind(DEV059_WORKFLOW_VERSION_ID)
    .bind(DEV059_WORKFLOW_ID)
    .bind(DEV059_WORKFLOW_JSON)
    .bind(DEV059_CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV-059 workflow version fixture should be inserted");

    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml,
          recipe_sha256, created_at)
         VALUES (?, ?, '1', 1, ?, 'dev059-recipe-sha', ?)",
    )
    .bind(DEV059_RECIPE_ID)
    .bind(DEV059_WORKFLOW_VERSION_ID)
    .bind(DEV059_RECIPE_YAML)
    .bind(DEV059_CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV-059 recipe fixture should be inserted");

    sqlx::query("UPDATE workflows SET current_version_id = ? WHERE id = ?")
        .bind(DEV059_WORKFLOW_VERSION_ID)
        .bind(DEV059_WORKFLOW_ID)
        .execute(pool)
        .await
        .expect("DEV-059 current workflow version should be set");
}

async fn seed_dev080_custom_package_binding(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES (?, 'DEV-080 custom package workflow', 'video', 'fl2va', NULL, ?, ?)",
    )
    .bind(DEV080_WORKFLOW_ID)
    .bind(DEV059_CREATED_AT)
    .bind(DEV059_CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV-080 custom workflow should be inserted");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES (?, ?, '1', ?, ?, ?)",
    )
    .bind(DEV080_WORKFLOW_VERSION_ID)
    .bind(DEV080_WORKFLOW_ID)
    .bind(DEV080_WORKFLOW_JSON)
    .bind(sha256_hex(DEV080_WORKFLOW_JSON))
    .bind(DEV059_CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV-080 custom workflow version should be inserted");
    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml,
          recipe_sha256, created_at)
         VALUES (?, ?, '1', 1, ?, ?, ?)",
    )
    .bind(DEV080_RECIPE_ID)
    .bind(DEV080_WORKFLOW_VERSION_ID)
    .bind(DEV080_RECIPE_YAML)
    .bind(sha256_hex(DEV080_RECIPE_YAML))
    .bind(DEV059_CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV-080 custom Recipe should be inserted");
    sqlx::query("UPDATE workflows SET current_version_id = ? WHERE id = ?")
        .bind(DEV080_WORKFLOW_VERSION_ID)
        .bind(DEV080_WORKFLOW_ID)
        .execute(pool)
        .await
        .expect("DEV-080 custom workflow should become current");
    sqlx::query(
        "INSERT INTO project_workflow_bindings
         (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at)
         VALUES (?, 'VIDEO', 'DEFAULT', ?, ?, ?, ?)",
    )
    .bind(DEV059_PROJECT_ID)
    .bind(DEV080_WORKFLOW_VERSION_ID)
    .bind(DEV080_RECIPE_ID)
    .bind(DEV059_CREATED_AT)
    .bind(DEV059_CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV-080 project workflow binding should be inserted");
}

async fn write_dev059_package(root: &Path) {
    let frames = root.join("frames");
    fs::create_dir_all(&frames).expect("DEV-059 package frames directory should be created");
    let image = png_bytes();
    let mut items = Vec::new();
    for ordinal in 1..=3 {
        let relative_path = format!("frames/shot-{ordinal:03}.png");
        fs::write(root.join(&relative_path), &image).expect("DEV-059 PNG fixture should write");
        items.push(json!({
            "id": format!("DEV059-SH{ordinal:03}"),
            "name": format!("Shot {ordinal}"),
            "videoPrompt": format!("DEV-059 camera move {ordinal}"),
            "mode": "I2V",
            "firstFrame": relative_path
        }));
    }

    let manifest = json!({
        "schemaVersion": 1,
        "packageType": PRODUCTION_PACKAGE_TYPE,
        "name": "DEV-059 I2V package",
        "defaults": {
            "durationSeconds": 5,
            "width": 864,
            "height": 480
        },
        "items": items
    });
    tokio::fs::write(
        root.join("production-package.json"),
        serde_json::to_vec_pretty(&manifest).expect("DEV-059 manifest should serialize"),
    )
    .await
    .expect("DEV-059 manifest should write");
}

async fn write_dev059_text_package(root: &Path, item_count: usize) {
    let items = (1..=item_count)
        .map(|ordinal| {
            json!({
                "id": format!("DEV059-TXT-{ordinal:03}"),
                "name": format!("Text shot {ordinal}"),
                "videoPrompt": format!("DEV-059 text camera move {ordinal}"),
                "mode": "TEXT_ONLY"
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({
        "schemaVersion": 1,
        "packageType": PRODUCTION_PACKAGE_TYPE,
        "name": "DEV-059 500 text package",
        "defaults": {
            "durationSeconds": 5,
            "width": 864,
            "height": 480
        },
        "items": items
    });
    tokio::fs::write(
        root.join("production-package.json"),
        serde_json::to_vec(&manifest).expect("DEV-059 text manifest should serialize"),
    )
    .await
    .expect("DEV-059 text manifest should write");
}

async fn write_dev059_mode_package(root: &Path) {
    let frames = root.join("frames");
    fs::create_dir_all(&frames).expect("DEV-059 mode frames directory should be created");
    let image = png_bytes();
    for file_name in [
        "first.png",
        "last.png",
        "reference-a.png",
        "reference-b.png",
        "reference-c.png",
    ] {
        fs::write(frames.join(file_name), &image).expect("DEV-059 mode PNG should write");
    }

    let manifest = json!({
        "schemaVersion": 1,
        "packageType": PRODUCTION_PACKAGE_TYPE,
        "name": "DEV-059 mode package",
        "defaults": {
            "durationSeconds": 5,
            "width": 864,
            "height": 480
        },
        "items": [
            {
                "id": "DEV059-FIRST-LAST",
                "name": "First and last frame shot",
                "videoPrompt": "DEV-059 first-last camera move",
                "mode": "FIRST_LAST",
                "firstFrame": "frames/first.png",
                "lastFrame": "frames/last.png"
            },
            {
                "id": "DEV059-REFERENCES",
                "name": "Reference image shot",
                "videoPrompt": "DEV-059 reference camera move",
                "mode": "REFERENCE_IMAGES",
                "referenceImages": [
                    "frames/reference-a.png",
                    "frames/reference-b.png",
                    "frames/reference-c.png"
                ]
            }
        ]
    });
    tokio::fs::write(
        root.join("production-package.json"),
        serde_json::to_vec(&manifest).expect("DEV-059 mode manifest should serialize"),
    )
    .await
    .expect("DEV-059 mode manifest should write");
}

fn build_dev059_package_service(
    pool: &SqlitePool,
    comfy: Arc<NoSubmitComfyAdapter>,
) -> ProductionPackageService {
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
        snapshot_repository.clone(),
        asset_repository.clone(),
        comfy_adapter.clone(),
        project_repository.clone(),
        asset_store.clone(),
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));

    let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let production_queue_repository: Arc<dyn ProductionQueueRepository> = queue_repository.clone();
    let shot_batch_repository: Arc<dyn ShotBatchRepository> = queue_repository.clone();
    let production_queue_service = Arc::new(ProductionQueueService::new(
        production_queue_repository,
        task_repository,
        definition_repository,
        generation_service,
        shot_batch_repository,
        task_recovery_service,
        clock.clone(),
    ));
    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let runtime_state_repository: Arc<dyn WorkflowRuntimeStateRepository> =
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone()));
    let project_workflow_binding_repository: Arc<dyn ProjectWorkflowBindingRepository> =
        Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone()));
    let project_workflow_binding_service = Arc::new(ProjectWorkflowBindingService::new(
        project_workflow_binding_repository,
        project_repository.clone(),
        runtime_repository,
        runtime_state_repository,
        clock.clone(),
    ));

    let asset_video_prompt_repository: Arc<dyn AssetVideoPromptRepository> =
        Arc::new(SqliteAssetVideoPromptRepository::new(pool.clone()));
    let asset_video_prompt_service = Arc::new(AssetVideoPromptService::new(
        asset_video_prompt_repository,
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
            workflow_version_id: DEV059_WORKFLOW_VERSION_ID.to_owned(),
            recipe_id: DEV059_RECIPE_ID.to_owned(),
            fl2va_workflow_version_id: None,
            fl2va_recipe_id: None,
            ref2va_workflow_version_id: None,
            ref2va_recipe_id: None,
            quality_profile: None,
            quality_recipes: Vec::new(),
        },
        clock,
    )
    .with_project_workflow_binding_service(project_workflow_binding_service)
}

fn build_dev080_admission_service(
    pool: &SqlitePool,
    comfy: Arc<NoSubmitComfyAdapter>,
) -> Arc<ProductionStartAdmissionService> {
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
    let comfy_service = Arc::new(ComfyService::from_runtime(Arc::new(ComfyRuntime::new(
        comfy_adapter.clone(),
        ComfyConnectionConfig::default(),
    ))));

    let source: Arc<dyn WorkflowLibrarySource> = Arc::new(StaticWorkflowSource {
        package: dev080_runtime_package(),
    });
    let workflow_library_service = Arc::new(WorkflowLibraryService::new(
        source.clone(),
        Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone())),
        clock.clone(),
    ));
    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let runtime_state_repository: Arc<dyn WorkflowRuntimeStateRepository> =
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone()));
    let workflow_run_repository: Arc<dyn WorkflowRunRepository> =
        Arc::new(SqliteWorkflowRunRepository::new(pool.clone()));
    let package_store: Arc<dyn WorkflowPackageStore> =
        Arc::new(FileSystemWorkflowPackageStore::new(
            "dev080-runtime-library".into(),
            "dev080-staging".into(),
        ));
    let onboarding_service = Arc::new(
        WorkflowOnboardingService::new(
            source.clone(),
            comfy_adapter.clone(),
            workflow_library_service.clone(),
            workflow_run_repository,
            package_store.clone(),
            clock.clone(),
        )
        .with_runtime_state(runtime_repository.clone(), runtime_state_repository.clone()),
    );
    let lifecycle_service = Arc::new(WorkflowLifecycleService::new(
        source,
        workflow_library_service,
        onboarding_service,
        runtime_repository,
        runtime_state_repository,
        package_store,
        clock.clone(),
    ));

    let generation_service = Arc::new(GenerationService::new(
        task_repository.clone(),
        snapshot_repository,
        definition_repository.clone(),
        comfy_adapter.clone(),
        project_repository.clone(),
        asset_store.clone(),
        asset_repository.clone(),
        clock.clone(),
    ));
    let task_recovery_service = Arc::new(TaskRecoveryService::new(
        task_repository.clone(),
        Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone())),
        asset_repository,
        comfy_adapter,
        project_repository,
        asset_store,
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));
    let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let production_queue_repository: Arc<dyn ProductionQueueRepository> = queue_repository.clone();
    let shot_batch_repository: Arc<dyn ShotBatchRepository> = queue_repository;
    let queue = Arc::new(ProductionQueueService::new(
        production_queue_repository,
        task_repository,
        definition_repository,
        generation_service,
        shot_batch_repository,
        task_recovery_service,
        clock,
    ));

    Arc::new(ProductionStartAdmissionService::new(
        queue,
        comfy_service,
        lifecycle_service,
    ))
}

#[test]
fn package_service_exposes_inspection_session_and_mapping_contract() {
    let source = service_source();
    assert_contains_all(
        &source,
        &[
            "pub async fn inspect_session",
            "pub async fn create_batches",
            "ProductionPackageInspection",
            "ProductionPackageCreateBatchesResult",
            "ProductionPackageItemMapping",
            "selected_item_ids: &[String]",
            "MAX_PRODUCTION_PACKAGE_ITEMS: usize = 500",
            "let chunk_size = MAX_PRODUCTION_PACKAGE_ITEMS.min(100)",
            "auto_start: false",
            "H3LocalImportMode::ProjectFolder",
            "SourceAssetImportService",
            "ProductionQueueService",
        ],
    );
}

#[test]
fn package_service_keeps_execution_and_generation_boundaries_closed() {
    let source = service_source();
    assert_contains_all(
        &source,
        &[
            "PACKAGE_MEDIA_CHANGED",
            "PACKAGE_DUPLICATE_ITEM_ID",
            "ProductionPackageInspectionError",
            "ProductionPackageItemStatus::Blocked",
            "FL2VA_IMAGE_TO_VIDEO",
            "FL2VA_FIRST_LAST",
            "REF2VA_IMAGE",
        ],
    );
    assert_contains_none(
        &source,
        &[
            "reqwest::",
            "submit_prompt",
            "ComfyClient",
            "generation_service.start",
            "production_queue_service.start(",
            "ShotService",
            "SeriesService",
            "ScriptImportService",
        ],
    );
}

#[test]
fn package_paths_are_revalidated_before_h3_commit() {
    let source = service_source();
    assert_contains_all(
        &source,
        &[
            "current.manifest_sha256 != session.inspection.manifest_sha256",
            "ensure_item_unchanged(item, current_item)",
            "revalidate_media(&session.root_path, media)",
            "sha256(&bytes)",
            "media.relative_path.clone()",
        ],
    );
}

#[test]
fn package_mapping_is_read_back_from_the_existing_queue_batch() {
    let source = service_source();
    assert_contains_all(
        &source,
        &[
            ".production_queue_service",
            ".get(project_id, &result.batch_id)",
            "batch_item_id: batch_item.id.as_str().to_owned()",
            "imported_asset_ids: asset_ids_from_values",
            "item_mappings.extend(batch.item_mappings.iter().cloned())",
        ],
    );
}

#[tokio::test]
async fn real_sqlite_temp_package_round_trip_creates_one_ready_i2v_batch_without_execution() {
    let directory = tempdir().expect("DEV-059 temp workspace should exist");
    let pool = initialize(&directory.path().join("dev059.db"))
        .await
        .expect("DEV-059 SQLite database should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("DEV-059 project storage should exist");
    seed_dev059_database(&pool, &project_root).await;

    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("DEV-059 package root should exist");
    write_dev059_package(&package_root).await;

    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let package_service = build_dev059_package_service(&pool, comfy.clone());
    let tasks_before =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE project_id = ?")
            .bind(DEV059_PROJECT_ID)
            .fetch_one(&pool)
            .await
            .expect("DEV-059 task count should be readable");

    let (session_id, inspection) = package_service
        .inspect_session(DEV059_PROJECT_ID, package_root)
        .await
        .expect("DEV-059 package should inspect");
    assert_eq!(inspection.item_count, 3);
    assert_eq!(inspection.ready_count, 3);
    assert_eq!(inspection.blocked_count, 0);
    assert!(inspection.items.iter().all(|item| {
        item.status == ProductionPackageItemStatus::Ready
            && item.mode == "FL2VA_IMAGE_TO_VIDEO"
            && item.duration_seconds == 5
            && item.width == 864
            && item.height == 480
            && item.first_frame.is_some()
    }));

    let selected_item_ids = inspection
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let result = package_service
        .create_batches(&session_id, &selected_item_ids)
        .await
        .expect("DEV-059 package should create a batch");

    assert_eq!(result.batch_count, 1);
    assert_eq!(result.item_count, 3);
    assert_eq!(result.batches.len(), 1);
    assert_eq!(result.batches[0].item_count, 3);
    assert!(!result.auto_started);
    assert_eq!(
        result
            .item_mappings
            .iter()
            .map(|mapping| mapping.package_item_id.as_str())
            .collect::<Vec<_>>(),
        selected_item_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert!(result
        .item_mappings
        .iter()
        .all(|mapping| mapping.imported_asset_ids.len() == 1));

    let tasks_after =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE project_id = ?")
            .bind(DEV059_PROJECT_ID)
            .fetch_one(&pool)
            .await
            .expect("DEV-059 task count should remain readable");
    let batch_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
        .fetch_one(&pool)
        .await
        .expect("DEV-059 batch count should be readable");
    let batch_item_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batch_items")
            .fetch_one(&pool)
            .await
            .expect("DEV-059 batch item count should be readable");
    let imported_asset_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM assets WHERE project_id = ? AND category = 'source_image'",
    )
    .bind(DEV059_PROJECT_ID)
    .fetch_one(&pool)
    .await
    .expect("DEV-059 imported asset count should be readable");

    assert_eq!(tasks_before, 0);
    assert_eq!(tasks_after, 0);
    assert_eq!(batch_count, 1);
    assert_eq!(batch_item_count, 3);
    assert_eq!(imported_asset_count, 3);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dev080_package_uses_the_current_project_video_default_and_freezes_it_in_the_batch() {
    let directory = tempdir().expect("DEV-080 temp workspace should exist");
    let pool = initialize(&directory.path().join("dev080.db"))
        .await
        .expect("DEV-080 SQLite database should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("DEV-080 project storage should exist");
    seed_dev059_database(&pool, &project_root).await;
    seed_dev080_custom_package_binding(&pool).await;

    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("DEV-080 package root should exist");
    write_dev059_text_package(&package_root, 1).await;

    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let package_service = build_dev059_package_service(&pool, comfy.clone());
    let (session_id, inspection) = package_service
        .inspect_session(DEV059_PROJECT_ID, package_root)
        .await
        .expect("DEV-080 package should inspect");
    assert_eq!(inspection.items[0].mode, "FL2VA_TEXT_TO_VIDEO");

    let result = package_service
        .create_batches(&session_id, &[inspection.items[0].id.clone()])
        .await
        .expect("DEV-080 package should create a batch");
    assert_eq!(result.item_count, 1);

    let pair = sqlx::query_as::<_, (String, String)>(
        "SELECT workflow_version_id, recipe_id
         FROM production_batch_items
         ORDER BY ordinal",
    )
    .fetch_one(&pool)
    .await
    .expect("DEV-080 frozen batch pair should be readable");
    assert_eq!(
        pair,
        (
            DEV080_WORKFLOW_VERSION_ID.to_owned(),
            DEV080_RECIPE_ID.to_owned()
        )
    );
    assert_ne!(pair.0, DEV059_WORKFLOW_VERSION_ID);
    assert_ne!(pair.1, DEV059_RECIPE_ID);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dev080_unavailable_project_binding_blocks_before_package_side_effects() {
    let directory = tempdir().expect("DEV-080 blocked temp workspace should exist");
    let pool = initialize(&directory.path().join("dev080-blocked.db"))
        .await
        .expect("DEV-080 blocked SQLite database should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("DEV-080 blocked project storage should exist");
    seed_dev059_database(&pool, &project_root).await;
    seed_dev080_custom_package_binding(&pool).await;
    sqlx::query(
        "INSERT INTO workflow_runtime_states
         (workflow_version_id, enabled, archived, updated_at)
         VALUES (?, 0, 0, ?)",
    )
    .bind(DEV080_WORKFLOW_VERSION_ID)
    .bind(DEV059_CREATED_AT)
    .execute(&pool)
    .await
    .expect("DEV-080 unavailable runtime state should be inserted");

    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("DEV-080 blocked package root should exist");
    write_dev059_text_package(&package_root, 1).await;
    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let package_service = build_dev059_package_service(&pool, comfy.clone());
    let (session_id, inspection) = package_service
        .inspect_session(DEV059_PROJECT_ID, package_root)
        .await
        .expect("DEV-080 blocked package should inspect");

    let error = package_service
        .create_batches(&session_id, &[inspection.items[0].id.clone()])
        .await
        .expect_err("unavailable configured project workflow must block");
    assert_eq!(
        error.code(),
        "PROJECT_WORKFLOW_UNAVAILABLE_FOR_PACKAGE_MODE"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dev080_custom_package_reaches_fake_comfy_after_start_admission() {
    let directory = tempdir().expect("DEV-080 runtime temp workspace should exist");
    let pool = initialize(&directory.path().join("dev080-runtime.db"))
        .await
        .expect("DEV-080 runtime SQLite database should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("DEV-080 runtime project storage should exist");
    seed_dev059_database(&pool, &project_root).await;
    seed_dev080_custom_package_binding(&pool).await;

    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("DEV-080 runtime package root should exist");
    write_dev059_text_package(&package_root, 1).await;
    let comfy = Arc::new(NoSubmitComfyAdapter::runtime());
    let package_service = build_dev059_package_service(&pool, comfy.clone());
    let (session_id, inspection) = package_service
        .inspect_session(DEV059_PROJECT_ID, package_root)
        .await
        .expect("DEV-080 runtime package should inspect");
    let result = package_service
        .create_batches(&session_id, &[inspection.items[0].id.clone()])
        .await
        .expect("DEV-080 runtime package should create a batch");
    let batch_id = result.batches[0].batch_id.clone();

    let admission = build_dev080_admission_service(&pool, comfy.clone());
    admission
        .start(DEV059_PROJECT_ID, &batch_id)
        .await
        .expect("DEV-080 custom package should pass exact start admission");
    for _ in 0..200 {
        if comfy.submit_calls.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 1);
    let submitted_workflow = comfy.submitted_workflow();
    assert_eq!(
        submitted_workflow["1"]["class_type"],
        Value::String("SaveVideo".to_owned())
    );
    let pair = sqlx::query_as::<_, (String, String)>(
        "SELECT workflow_version_id, recipe_id
         FROM production_batch_items
         ORDER BY ordinal",
    )
    .fetch_one(&pool)
    .await
    .expect("DEV-080 runtime frozen pair should be readable");
    assert_eq!(
        pair,
        (
            DEV080_WORKFLOW_VERSION_ID.to_owned(),
            DEV080_RECIPE_ID.to_owned()
        )
    );
    assert_ne!(pair.0, DEV059_WORKFLOW_VERSION_ID);
    assert_ne!(pair.1, DEV059_RECIPE_ID);
}

#[tokio::test]
async fn real_sqlite_500_item_text_package_chunks_at_the_queue_cap_without_execution() {
    let directory = tempdir().expect("DEV-059 500-item temp workspace should exist");
    let pool = initialize(&directory.path().join("dev059-500.db"))
        .await
        .expect("DEV-059 500-item SQLite database should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("DEV-059 500-item project storage should exist");
    seed_dev059_database(&pool, &project_root).await;

    let package_root = directory.path().join("text-package");
    fs::create_dir_all(&package_root).expect("DEV-059 500-item package root should exist");
    write_dev059_text_package(&package_root, 500).await;

    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let package_service = build_dev059_package_service(&pool, comfy.clone());
    let (session_id, inspection) = package_service
        .inspect_session(DEV059_PROJECT_ID, package_root)
        .await
        .expect("DEV-059 500-item package should inspect");
    assert_eq!(inspection.item_count, 500);
    assert_eq!(inspection.ready_count, 500);
    assert_eq!(inspection.blocked_count, 0);
    assert!(inspection.items.iter().all(|item| {
        item.status == ProductionPackageItemStatus::Ready
            && item.mode == "FL2VA_TEXT_TO_VIDEO"
            && item.duration_seconds == 5
            && item.width == 864
            && item.height == 480
    }));

    let selected_item_ids = inspection
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let result = package_service
        .create_batches(&session_id, &selected_item_ids)
        .await
        .expect("DEV-059 500-item package should create batches");

    assert_eq!(result.item_count, 500);
    assert_eq!(result.batch_count, 5);
    assert_eq!(result.batches.len(), 5);
    assert!(result.batches.iter().all(|batch| batch.item_count == 100));
    assert!(!result.auto_started);
    assert_eq!(
        result
            .item_mappings
            .iter()
            .map(|mapping| mapping.package_item_id.as_str())
            .collect::<Vec<_>>(),
        selected_item_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert!(result
        .batches
        .iter()
        .all(|batch| batch.item_mappings.len() == 100));

    let tasks = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE project_id = ?")
        .bind(DEV059_PROJECT_ID)
        .fetch_one(&pool)
        .await
        .expect("DEV-059 500-item task count should be readable");
    let batch_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
        .fetch_one(&pool)
        .await
        .expect("DEV-059 500-item batch count should be readable");
    let batch_item_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batch_items")
            .fetch_one(&pool)
            .await
            .expect("DEV-059 500-item batch item count should be readable");

    assert_eq!(tasks, 0);
    assert_eq!(batch_count, 5);
    assert_eq!(batch_item_count, 500);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn real_sqlite_mode_package_preserves_first_last_and_reference_inputs_without_execution() {
    let directory = tempdir().expect("DEV-059 mode temp workspace should exist");
    let pool = initialize(&directory.path().join("dev059-modes.db"))
        .await
        .expect("DEV-059 mode SQLite database should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("DEV-059 mode project storage should exist");
    seed_dev059_database(&pool, &project_root).await;

    let package_root = directory.path().join("mode-package");
    fs::create_dir_all(&package_root).expect("DEV-059 mode package root should exist");
    write_dev059_mode_package(&package_root).await;

    let comfy = Arc::new(NoSubmitComfyAdapter::new());
    let package_service = build_dev059_package_service(&pool, comfy.clone());
    let (session_id, inspection) = package_service
        .inspect_session(DEV059_PROJECT_ID, package_root)
        .await
        .expect("DEV-059 mode package should inspect");
    assert_eq!(inspection.item_count, 2);
    assert_eq!(inspection.ready_count, 2);
    assert_eq!(inspection.blocked_count, 0);
    assert_eq!(inspection.items[0].id, "DEV059-FIRST-LAST");
    assert_eq!(inspection.items[0].mode, "FL2VA_FIRST_LAST");
    assert_eq!(inspection.items[1].id, "DEV059-REFERENCES");
    assert_eq!(inspection.items[1].mode, "REF2VA_IMAGE");

    let selected_item_ids = inspection
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let result = package_service
        .create_batches(&session_id, &selected_item_ids)
        .await
        .expect("DEV-059 mode package should create a batch");
    assert_eq!(result.batch_count, 1);
    assert_eq!(result.item_count, 2);
    assert_eq!(result.batches[0].item_count, 2);
    assert!(!result.auto_started);
    assert_eq!(
        result
            .item_mappings
            .iter()
            .map(|mapping| mapping.package_item_id.as_str())
            .collect::<Vec<_>>(),
        vec!["DEV059-FIRST-LAST", "DEV059-REFERENCES"]
    );
    assert_eq!(result.item_mappings[0].imported_asset_ids.len(), 2);
    assert_eq!(result.item_mappings[1].imported_asset_ids.len(), 3);

    let values_json = sqlx::query_scalar::<_, String>(
        "SELECT values_json FROM production_batch_items ORDER BY ordinal",
    )
    .fetch_all(&pool)
    .await
    .expect("DEV-059 mode queue values should be readable")
    .into_iter()
    .map(|value| serde_json::from_str::<Value>(&value).expect("queue values should be JSON"))
    .collect::<Vec<_>>();
    assert_eq!(values_json.len(), 2);
    assert!(values_json[0].get("first_frame").is_some());
    assert!(values_json[0].get("last_frame").is_some());
    assert!(values_json[0].get("reference_images").is_none());

    let reference_asset_ids = values_json[1]["reference_images"]["assetIds"]
        .as_array()
        .expect("reference_images should contain assetIds")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("reference asset id should be a string")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(reference_asset_ids.len(), 3);
    let stored_reference_assets = sqlx::query_as::<_, (String, String)>(
        "SELECT id, original_name
         FROM assets
         WHERE project_id = ? AND category = 'source_image'
           AND original_name LIKE 'reference-%.png'
         ORDER BY original_name ASC",
    )
    .bind(DEV059_PROJECT_ID)
    .fetch_all(&pool)
    .await
    .expect("DEV-059 reference assets should be readable");
    assert_eq!(
        stored_reference_assets
            .iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "reference-000.png",
            "reference-001.png",
            "reference-002.png"
        ]
    );
    assert_eq!(
        reference_asset_ids,
        stored_reference_assets
            .iter()
            .map(|(asset_id, _)| asset_id.clone())
            .collect::<Vec<_>>()
    );

    let tasks = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE project_id = ?")
        .bind(DEV059_PROJECT_ID)
        .fetch_one(&pool)
        .await
        .expect("DEV-059 mode task count should be readable");
    assert_eq!(tasks, 0);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
}
