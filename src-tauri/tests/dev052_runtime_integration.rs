//! DEV-052 runtime acceptance tests.
//!
//! These tests intentionally assemble the same application services that the
//! Tauri bootstrap wires together, but replace only ComfyUI with an in-process
//! adapter.  The database, repositories, resolver, readiness service, and
//! preparation admission path are real; no GPU or external ComfyUI process is
//! needed.

use ai_studio_lib::application::{
    comfy_preflight_service::ComfyPreflightService,
    comfy_service::{ComfyRuntime, ComfyService},
    diagnostics_service::DiagnosticsService,
    generation_input_preparer::GenerationInputValue,
    generation_service::GenerationService,
    ports::{
        AssetRepository, AssetStore, Clock, ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig,
        ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyOutputData,
        ComfyOutputFile, GenerationDefinitionRepository, GenerationSnapshotRepository,
        NoopTaskUpdateSink, ProductionQueueRepository, ProductionStructureRepository,
        ProjectRepository, PromptSubmission, ShotBatchBinding, ShotBatchRepository, ShotRepository,
        SystemStats, TaskRepository, WorkflowLibrarySource, WorkflowLibrarySourceError,
        WorkflowPackageFiles, WorkflowPackageLoad, WorkflowPackageStore, WorkflowRunRepository,
        WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
    },
    production_preparation_service::ProductionPreparationService,
    production_queue_service::{
        CreateProductionBatchItem, CreateProductionBatchRequest, ProductionQueueError,
        ProductionQueueService,
    },
    production_start_admission_service::{
        ProductionStartAdmissionError, ProductionStartAdmissionService,
        RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE, RUNTIME_ADMISSION_CAPABILITY_REFRESH_FAILED,
        RUNTIME_ADMISSION_COMFY_UNAVAILABLE, RUNTIME_ADMISSION_MISSING_NODES,
    },
    shot_batch_service::ShotBatchService,
    shot_context_resolver::ShotContextResolver,
    shot_readiness_service::ShotReadinessService,
    task_recovery_service::TaskRecoveryService,
    workflow_library_service::WorkflowLibraryService,
    workflow_lifecycle_service::WorkflowLifecycleService,
    workflow_onboarding_service::WorkflowOnboardingService,
};
use ai_studio_lib::domain::{
    AssetId, BindingRole, ComfyCapabilityEvidence, ContextSourceScope, PreparationSnapshotRecord,
    PreparationSnapshotV1, ProductionBatch, ProductionBatchItem, ProductionBatchItemId,
    ProductionBatchItemStatus, ProductionBatchStatus, ResolvedReferenceAsset, ResolvedShotContext,
    ResolvedStageInput, ShotStage,
};
use ai_studio_lib::infrastructure::database::repositories::SqliteConsistencyScopeRepository;
use ai_studio_lib::infrastructure::{
    database::{
        initialize, SqliteAssetRepository, SqliteConsistencyProfileRepository,
        SqliteGenerationDefinitionRepository, SqliteGenerationSnapshotRepository,
        SqliteProductionQueueRepository, SqliteProductionStructureRepository,
        SqliteProjectRepository, SqliteReferenceSetRepository, SqliteShotConsistencyRepository,
        SqliteShotRepository, SqliteTaskRepository, SqliteWorkflowLibraryRepository,
        SqliteWorkflowRunRepository, SqliteWorkflowRuntimeRepository,
        SqliteWorkflowRuntimeStateRepository,
    },
    filesystem::{FileSystemAssetStore, FileSystemWorkflowPackageStore},
    logging::LoggingStatus,
    time::SystemClock,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    collections::BTreeMap,
    fs,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tempfile::{tempdir, TempDir};
use tokio::sync::Notify;

const PROJECT_ID: &str = "prj_default";
const READY_SHOT_ID: &str = "shot_dev052_ready";
const INCOMPLETE_SHOT_ID: &str = "shot_dev052_incomplete";
const I2V_SHOT_ID: &str = "shot_dev052_i2v";
const SELECTED_ASSET_ID: &str = "ast_dev052_selected";
const CREATED_AT: &str = "2026-08-26T00:00:00Z";
const SERIES_ID: &str = "ser_dev052";
const EPISODE_ID: &str = "epi_dev052";
const SCENE_ID: &str = "scn_dev052";

const WORKFLOW_JSON: &str = r#"{
  "3": {
    "inputs": {
      "seed": 1,
      "steps": 20,
      "cfg": 8,
      "sampler_name": "euler",
      "scheduler": "normal",
      "denoise": 1.0,
      "model": ["4", 0],
      "positive": ["6", 0],
      "negative": ["7", 0],
      "latent_image": ["5", 0]
    },
    "class_type": "KSampler"
  },
  "4": {
    "inputs": {"ckpt_name": "model.safetensors"},
    "class_type": "CheckpointLoaderSimple"
  },
  "5": {
    "inputs": {"width": 512, "height": 512, "batch_size": 1},
    "class_type": "EmptyLatentImage"
  },
  "6": {
    "inputs": {"text": "original", "clip": ["4", 1]},
    "class_type": "CLIPTextEncode"
  },
  "7": {
    "inputs": {"text": "", "clip": ["4", 1]},
    "class_type": "CLIPTextEncode"
  },
  "8": {
    "inputs": {"samples": ["3", 0], "vae": ["4", 2]},
    "class_type": "VAEDecode"
  },
  "9": {
    "inputs": {"filename_prefix": "ComfyUI", "images": ["8", 0]},
    "class_type": "SaveImage"
  }
}"#;

const RECIPE_YAML: &str = r#"schema_version: 1
id: rcp_dev052_fixture
name: DEV-052 Fixture Image
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
outputs:
  - id: generated_image
    type: image
    node: "9"
    required: true
"#;

const SHARED_WORKFLOW_ID: &str = "wfl_dev078_shared_recipe";
const SHARED_RECIPE_A_VERSION: &str = "1.0.0";
const SHARED_RECIPE_B_VERSION: &str = "2.0.0";
const SHARED_RECIPE_A_YAML: &str = r#"schema_version: 1
id: rcp_dev078_recipe_a
name: DEV-078 Shared Recipe A
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  sampler:
    type: textarea
    label: Sampler
    required: false
    default: euler
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: sampler
    target:
      node: "3"
      input: sampler_name
outputs:
  - id: generated_image
    type: image
    node: "9"
    required: true
"#;
const SHARED_RECIPE_B_YAML: &str = r#"schema_version: 1
id: rcp_dev078_recipe_b
name: DEV-078 Shared Recipe B
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
outputs:
  - id: generated_image
    type: image
    node: "9"
    required: true
"#;

fn fixture_package() -> WorkflowPackageFiles {
    WorkflowPackageFiles {
        package_name: "dev052_fixture".to_owned(),
        package_source_path: None,
        manifest_yaml: "schema_version: 1\nid: wfl_dev052_fixture\nname: DEV-052 Fixture\nworkflow_version: 1.0.0\nrecipe_version: 1.0.0\ncategory: image\nmode: t2i\n".to_owned(),
        recipe_yaml: RECIPE_YAML.to_owned(),
        workflow_json: WORKFLOW_JSON.to_owned(),
    }
}

fn shared_recipe_packages() -> Vec<WorkflowPackageFiles> {
    [
        ("dev078_shared_recipe_a", SHARED_RECIPE_A_VERSION, SHARED_RECIPE_A_YAML),
        ("dev078_shared_recipe_b", SHARED_RECIPE_B_VERSION, SHARED_RECIPE_B_YAML),
    ]
    .into_iter()
    .map(|(package_name, recipe_version, recipe_yaml)| WorkflowPackageFiles {
        package_name: package_name.to_owned(),
        package_source_path: None,
        manifest_yaml: format!(
            "schema_version: 1\nid: {SHARED_WORKFLOW_ID}\nname: DEV-078 Shared Recipe\nworkflow_version: 1.0.0\nrecipe_version: {recipe_version}\ncategory: image\nmode: t2i\n"
        ),
        recipe_yaml: recipe_yaml.to_owned(),
        workflow_json: WORKFLOW_JSON.to_owned(),
    })
    .collect()
}

#[derive(Clone)]
struct StaticWorkflowSource {
    packages: Vec<WorkflowPackageFiles>,
    load_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl WorkflowLibrarySource for StaticWorkflowSource {
    async fn load_packages(&self) -> Result<Vec<WorkflowPackageLoad>, WorkflowLibrarySourceError> {
        self.load_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .packages
            .iter()
            .cloned()
            .map(WorkflowPackageLoad::Loaded)
            .collect())
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
struct CountingComfyAdapter {
    health_calls: Arc<AtomicUsize>,
    object_info_calls: Arc<AtomicUsize>,
    submit_calls: Arc<AtomicUsize>,
    offline: Arc<AtomicBool>,
    missing_nodes: Arc<AtomicBool>,
    sampler_option_incompatible: Arc<AtomicBool>,
    capability_refresh_failure: Arc<AtomicBool>,
    hold_health_check: Arc<AtomicBool>,
    health_check_released: Arc<AtomicBool>,
    health_check_started: Arc<AtomicBool>,
    health_check_notify: Arc<Notify>,
    health_check_release: Arc<Notify>,
    hold_submission: Arc<AtomicBool>,
    submission_released: Arc<AtomicBool>,
    submission_started: Arc<AtomicBool>,
    submission_notify: Arc<Notify>,
    submission_release: Arc<Notify>,
}

impl CountingComfyAdapter {
    fn new() -> Self {
        Self {
            health_calls: Arc::new(AtomicUsize::new(0)),
            object_info_calls: Arc::new(AtomicUsize::new(0)),
            submit_calls: Arc::new(AtomicUsize::new(0)),
            offline: Arc::new(AtomicBool::new(false)),
            missing_nodes: Arc::new(AtomicBool::new(false)),
            sampler_option_incompatible: Arc::new(AtomicBool::new(false)),
            capability_refresh_failure: Arc::new(AtomicBool::new(false)),
            hold_health_check: Arc::new(AtomicBool::new(false)),
            health_check_released: Arc::new(AtomicBool::new(true)),
            health_check_started: Arc::new(AtomicBool::new(false)),
            health_check_notify: Arc::new(Notify::new()),
            health_check_release: Arc::new(Notify::new()),
            hold_submission: Arc::new(AtomicBool::new(false)),
            submission_started: Arc::new(AtomicBool::new(false)),
            submission_notify: Arc::new(Notify::new()),
            submission_release: Arc::new(Notify::new()),
            submission_released: Arc::new(AtomicBool::new(true)),
        }
    }

    fn reset(&self) {
        self.health_calls.store(0, Ordering::SeqCst);
        self.object_info_calls.store(0, Ordering::SeqCst);
        self.submit_calls.store(0, Ordering::SeqCst);
        self.offline.store(false, Ordering::SeqCst);
        self.missing_nodes.store(false, Ordering::SeqCst);
        self.sampler_option_incompatible
            .store(false, Ordering::SeqCst);
        self.capability_refresh_failure
            .store(false, Ordering::SeqCst);
        self.hold_health_check.store(false, Ordering::SeqCst);
        self.health_check_released.store(true, Ordering::SeqCst);
        self.hold_submission.store(false, Ordering::SeqCst);
        self.submission_released.store(true, Ordering::SeqCst);
    }

    fn set_offline(&self, offline: bool) {
        self.offline.store(offline, Ordering::SeqCst);
    }

    fn set_capability_refresh_failure(&self, failed: bool) {
        self.capability_refresh_failure
            .store(failed, Ordering::SeqCst);
    }

    fn set_missing_nodes(&self, missing: bool) {
        self.missing_nodes.store(missing, Ordering::SeqCst);
    }

    fn set_recipe_specific_incompatibility(&self, incompatible: bool) {
        self.sampler_option_incompatible
            .store(incompatible, Ordering::SeqCst);
    }

    fn hold_health_check(&self) {
        self.health_check_started.store(false, Ordering::SeqCst);
        self.health_check_released.store(false, Ordering::SeqCst);
        self.hold_health_check.store(true, Ordering::SeqCst);
    }

    fn release_health_check(&self) {
        self.health_check_released.store(true, Ordering::SeqCst);
        self.hold_health_check.store(false, Ordering::SeqCst);
        self.health_check_release.notify_waiters();
    }

    async fn wait_for_health_check(&self) {
        if !self.health_check_started.load(Ordering::SeqCst) {
            self.health_check_notify.notified().await;
        }
    }

    fn hold_submission(&self) {
        self.submission_started.store(false, Ordering::SeqCst);
        self.submission_released.store(false, Ordering::SeqCst);
        self.hold_submission.store(true, Ordering::SeqCst);
    }

    fn release_submission(&self) {
        self.submission_released.store(true, Ordering::SeqCst);
        self.hold_submission.store(false, Ordering::SeqCst);
        self.submission_release.notify_waiters();
    }

    async fn wait_for_submission(&self) {
        if !self.submission_started.load(Ordering::SeqCst) {
            self.submission_notify.notified().await;
        }
    }
}

#[async_trait]
impl ComfyAdapter for CountingComfyAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        self.health_calls.fetch_add(1, Ordering::SeqCst);
        if self.offline.load(Ordering::SeqCst) {
            return Err(ComfyAdapterError::Offline(
                "DEV-078 test ComfyUI is offline".to_owned(),
            ));
        }
        if self.hold_health_check.load(Ordering::SeqCst) {
            self.health_check_started.store(true, Ordering::SeqCst);
            self.health_check_notify.notify_waiters();
            while self.hold_health_check.load(Ordering::SeqCst)
                && !self.health_check_released.load(Ordering::SeqCst)
            {
                self.health_check_release.notified().await;
            }
        }
        Ok(ComfyHealth {
            system: SystemStats {
                comfyui_version: Some("dev052-test".to_owned()),
                python_version: Some("3.12".to_owned()),
                os: Some("test".to_owned()),
                ram_total: None,
                ram_free: None,
                devices: Vec::new(),
            },
        })
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Ok(SystemStats {
            comfyui_version: Some("dev052-test".to_owned()),
            python_version: Some("3.12".to_owned()),
            os: Some("test".to_owned()),
            ram_total: None,
            ram_free: None,
            devices: Vec::new(),
        })
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        self.object_info_calls.fetch_add(1, Ordering::SeqCst);
        if self.capability_refresh_failure.load(Ordering::SeqCst) {
            return Err(ComfyAdapterError::Protocol(
                "DEV-078 test capability refresh failed".to_owned(),
            ));
        }
        let k_sampler = if self.sampler_option_incompatible.load(Ordering::SeqCst) {
            json!({
                "input": {"required": {
                    "sampler_name": [["ddim"], {}]
                }}
            })
        } else {
            json!({})
        };
        let mut object_info = json!({
            "KSampler": k_sampler,
            "CheckpointLoaderSimple": {},
            "EmptyLatentImage": {},
            "CLIPTextEncode": {},
            "VAEDecode": {},
            "SaveImage": {},
            "LoadImage": {}
        });
        if self.missing_nodes.load(Ordering::SeqCst) {
            object_info
                .as_object_mut()
                .expect("test object_info should be an object")
                .remove("KSampler");
        }
        Ok(object_info)
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
        if self.hold_submission.load(Ordering::SeqCst) {
            self.submission_started.store(true, Ordering::SeqCst);
            self.submission_notify.notify_waiters();
            while self.hold_submission.load(Ordering::SeqCst)
                && !self.submission_released.load(Ordering::SeqCst)
            {
                self.submission_release.notified().await;
            }
        }
        Err(ComfyAdapterError::Incompatible(
            "DEV-052 preparation must not submit workflows".to_owned(),
        ))
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Ok(Box::new(EmptyComfyEvents))
    }
}

struct Harness {
    _directory: TempDir,
    pool: SqlitePool,
    preparation: Arc<ProductionPreparationService>,
    queue_repository: Arc<SqliteProductionQueueRepository>,
    resolver: Arc<ShotContextResolver>,
    readiness: Arc<ShotReadinessService>,
    source_calls: Arc<AtomicUsize>,
    comfy: Arc<CountingComfyAdapter>,
    lifecycle: Arc<WorkflowLifecycleService>,
    admission: Arc<ProductionStartAdmissionService>,
    queue: Arc<ProductionQueueService>,
    workflow_version_id: String,
    recipe_id: String,
    recipe_ids_by_version: BTreeMap<String, String>,
}

async fn harness() -> Harness {
    harness_with_packages(vec![fixture_package()], false).await
}

async fn shared_recipe_harness() -> Harness {
    harness_with_packages(shared_recipe_packages(), true).await
}

async fn harness_with_packages(
    packages: Vec<WorkflowPackageFiles>,
    recipe_specific_incompatibility: bool,
) -> Harness {
    let directory = tempdir().expect("DEV-052 tempdir should exist");
    let pool = initialize(&directory.path().join("dev052-runtime.db"))
        .await
        .expect("DEV-052 migrations 001-024 should run");
    let workflow_library_root = directory.path().join("workflow-library");
    let workflow_staging_root = directory.path().join("workflow-staging");
    fs::create_dir_all(&workflow_library_root).expect("workflow library root should exist");
    fs::create_dir_all(&workflow_staging_root).expect("workflow staging root should exist");

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let project_repository: Arc<dyn ProjectRepository> =
        Arc::new(SqliteProjectRepository::new(pool.clone()));
    let project_root = directory.path().join("project");
    project_repository
        .ensure_default_project(PROJECT_ID, "DEV-052 Runtime", &project_root, Utc::now())
        .await
        .expect("project fixture should be created");

    let source_calls = Arc::new(AtomicUsize::new(0));
    let package_count = packages.len();
    let source: Arc<dyn WorkflowLibrarySource> = Arc::new(StaticWorkflowSource {
        packages,
        load_calls: source_calls.clone(),
    });
    let workflow_library_repository = Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone()));
    let workflow_library_service = Arc::new(WorkflowLibraryService::new(
        source.clone(),
        workflow_library_repository,
        clock.clone(),
    ));
    let sync = workflow_library_service
        .sync()
        .await
        .expect("fixture workflow package should sync");
    assert_eq!(
        sync.valid as usize, package_count,
        "fixture packages must pass runtime validation"
    );

    let definition_impl = Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let definitions = definition_impl
        .list_available()
        .await
        .expect("fixture generation definition should be available");
    assert_eq!(definitions.len(), package_count);
    let workflow_version_id = definitions[0].workflow_version_id.clone();
    let recipe_id = definitions[0].recipe_id.clone();
    let recipe_ids_by_version = definitions
        .iter()
        .map(|definition| {
            (
                definition.recipe_version.clone(),
                definition.recipe_id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(recipe_ids_by_version.len(), package_count);
    seed_hierarchy(&pool, &workflow_version_id, &recipe_id).await;

    let asset_repository: Arc<dyn AssetRepository> =
        Arc::new(SqliteAssetRepository::new(pool.clone()));
    let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
    let task_repository: Arc<dyn TaskRepository> =
        Arc::new(SqliteTaskRepository::new(pool.clone()));
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> =
        Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
    let definition_repository: Arc<dyn GenerationDefinitionRepository> = definition_impl.clone();
    let shot_repository: Arc<dyn ShotRepository> =
        Arc::new(SqliteShotRepository::new(pool.clone()));
    let structure_repository: Arc<dyn ProductionStructureRepository> =
        Arc::new(SqliteProductionStructureRepository::new(pool.clone()));
    let scope_repository: Arc<dyn ai_studio_lib::application::ports::ConsistencyScopeRepository> =
        Arc::new(SqliteConsistencyScopeRepository::new(pool.clone()));
    let profile_repository: Arc<
        dyn ai_studio_lib::application::ports::ConsistencyProfileRepository,
    > = Arc::new(SqliteConsistencyProfileRepository::new(pool.clone()));
    let reference_set_repository: Arc<
        dyn ai_studio_lib::application::ports::ReferenceSetRepository,
    > = Arc::new(SqliteReferenceSetRepository::new(pool.clone()));
    let shot_consistency_repository: Arc<
        dyn ai_studio_lib::application::ports::ShotConsistencyRepository,
    > = Arc::new(SqliteShotConsistencyRepository::new(pool.clone()));
    let resolver = Arc::new(ShotContextResolver::new(
        project_repository.clone(),
        structure_repository.clone(),
        shot_repository.clone(),
        scope_repository,
        profile_repository,
        reference_set_repository,
        shot_consistency_repository,
        asset_repository.clone(),
        clock.clone(),
    ));

    let comfy = Arc::new(CountingComfyAdapter::new());
    let comfy_adapter: Arc<dyn ComfyAdapter> = comfy.clone();
    let comfy_service = Arc::new(ComfyService::from_runtime(Arc::new(ComfyRuntime::new(
        comfy_adapter.clone(),
        ComfyConnectionConfig::default(),
    ))));
    let workflow_run_repository: Arc<dyn WorkflowRunRepository> =
        Arc::new(SqliteWorkflowRunRepository::new(pool.clone()));
    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let runtime_state_repository: Arc<dyn WorkflowRuntimeStateRepository> =
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone()));
    let package_store: Arc<dyn WorkflowPackageStore> = Arc::new(
        FileSystemWorkflowPackageStore::new(workflow_library_root, workflow_staging_root),
    );
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
    let workflow_lifecycle_service = Arc::new(WorkflowLifecycleService::new(
        source,
        workflow_library_service,
        onboarding_service,
        runtime_repository,
        runtime_state_repository,
        package_store,
        clock.clone(),
    ));

    let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let production_queue_repository: Arc<dyn ProductionQueueRepository> = queue_repository.clone();
    let shot_batch_repository: Arc<dyn ShotBatchRepository> = queue_repository.clone();
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
    let recovery_service = Arc::new(TaskRecoveryService::new(
        task_repository.clone(),
        snapshot_repository,
        asset_repository.clone(),
        comfy_adapter.clone(),
        project_repository.clone(),
        asset_store,
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));
    let queue_service = Arc::new(ProductionQueueService::new(
        production_queue_repository.clone(),
        task_repository.clone(),
        definition_repository.clone(),
        generation_service,
        shot_batch_repository.clone(),
        recovery_service,
        clock.clone(),
    ));
    let admission = Arc::new(ProductionStartAdmissionService::new(
        queue_service.clone(),
        comfy_service.clone(),
        workflow_lifecycle_service.clone(),
    ));
    let diagnostics_service = Arc::new(DiagnosticsService::new(
        pool.clone(),
        task_repository,
        comfy_service.clone(),
        workflow_lifecycle_service.clone(),
        queue_service.clone(),
        directory.path().join("logs"),
        LoggingStatus {
            available: false,
            retention_days: 7,
        },
    ));
    let comfy_preflight_service = Arc::new(ComfyPreflightService::new(
        comfy_service.clone(),
        diagnostics_service,
        workflow_lifecycle_service.clone(),
    ));
    let readiness = Arc::new(ShotReadinessService::new(
        resolver.clone(),
        comfy_preflight_service,
        workflow_lifecycle_service.clone(),
        structure_repository,
    ));
    let shot_batch_service = Arc::new(ShotBatchService::new(
        shot_repository,
        shot_batch_repository,
        Arc::new(SqliteTaskRepository::new(pool.clone())),
        asset_repository,
        definition_repository.clone(),
        project_repository.clone(),
        clock.clone(),
    ));
    let preparation = Arc::new(ProductionPreparationService::new(
        shot_batch_service,
        production_queue_repository_as_shot_batch(&queue_repository),
        readiness.clone(),
        definition_repository,
        project_repository,
        clock,
    ));

    source_calls.store(0, Ordering::SeqCst);
    comfy.reset();
    comfy.set_recipe_specific_incompatibility(recipe_specific_incompatibility);
    Harness {
        _directory: directory,
        pool,
        preparation,
        queue_repository,
        resolver,
        readiness,
        source_calls,
        comfy,
        lifecycle: workflow_lifecycle_service,
        admission,
        queue: queue_service,
        workflow_version_id,
        recipe_id,
        recipe_ids_by_version,
    }
}

fn production_queue_repository_as_shot_batch(
    repository: &Arc<SqliteProductionQueueRepository>,
) -> Arc<dyn ShotBatchRepository> {
    repository.clone()
}

async fn create_runtime_batch(harness: &Harness, item_count: usize) -> String {
    let recipe_id = harness.recipe_id.clone();
    create_runtime_batch_for_recipe(harness, &recipe_id, item_count).await
}

async fn create_runtime_batch_for_recipe(
    harness: &Harness,
    recipe_id: &str,
    item_count: usize,
) -> String {
    let items = (0..item_count)
        .map(|index| CreateProductionBatchItem {
            workflow_version_id: harness.workflow_version_id.clone(),
            recipe_id: recipe_id.to_owned(),
            values: BTreeMap::from([(
                "prompt".to_owned(),
                GenerationInputValue::Text(format!("DEV-078 prompt {index}")),
            )]),
        })
        .collect();
    harness
        .queue
        .create(CreateProductionBatchRequest {
            project_id: PROJECT_ID.to_owned(),
            name: format!("DEV-078 runtime batch {item_count}"),
            continue_on_failure: false,
            items,
        })
        .await
        .expect("DEV-078 runtime batch should be created without starting")
        .batch
        .id
        .as_str()
        .to_owned()
}

fn recipe_id_for_version(harness: &Harness, recipe_version: &str) -> String {
    harness
        .recipe_ids_by_version
        .get(recipe_version)
        .cloned()
        .expect("fixture recipe version should be registered")
}

fn runtime_failure_code(error: ProductionStartAdmissionError) -> &'static str {
    match error {
        ProductionStartAdmissionError::Runtime(failure) => failure.code,
        ProductionStartAdmissionError::Queue(error) => {
            panic!("expected runtime admission failure, got queue error: {error}")
        }
    }
}

#[tokio::test]
async fn dev078_b1_valid_runtime_starts_and_enters_existing_dispatch() {
    let harness = harness().await;
    let batch_id = create_runtime_batch(&harness, 1).await;
    harness.comfy.hold_submission();

    harness
        .admission
        .start(PROJECT_ID, &batch_id)
        .await
        .expect("a valid runtime should be admitted");
    harness.comfy.wait_for_submission().await;

    let detail = harness
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("started batch should be readable");
    assert_eq!(detail.batch.status, ProductionBatchStatus::Running);
    assert_eq!(
        detail.items[0].status,
        ProductionBatchItemStatus::Dispatched
    );
    assert_eq!(count(&harness.pool, "tasks").await, 1);
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 1);

    harness.comfy.release_submission();
}

#[tokio::test]
async fn dev078_b2_runtime_block_keeps_batch_pending_without_side_effects() {
    let harness = harness().await;
    let batch_id = create_runtime_batch(&harness, 1).await;
    harness.comfy.set_missing_nodes(true);

    let error = harness
        .admission
        .start(PROJECT_ID, &batch_id)
        .await
        .expect_err("a missing runtime node must block start");
    assert_eq!(runtime_failure_code(error), RUNTIME_ADMISSION_MISSING_NODES);

    let detail = harness
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("blocked batch should remain readable");
    assert_eq!(detail.batch.status, ProductionBatchStatus::Ready);
    assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Pending);
    assert_eq!(count(&harness.pool, "tasks").await, 0);
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dev078_p1_exact_recipe_inspection_is_isolated() {
    let harness = shared_recipe_harness().await;
    let recipe_a = recipe_id_for_version(&harness, SHARED_RECIPE_A_VERSION);
    let recipe_b = recipe_id_for_version(&harness, SHARED_RECIPE_B_VERSION);

    let inspection_a = harness
        .lifecycle
        .inspect_recipe_runtime(&harness.workflow_version_id, &recipe_a)
        .await
        .expect("Recipe A exact inspection should succeed");
    assert_eq!(inspection_a.recipe_id, recipe_a);
    assert_eq!(
        inspection_a.capability, "READY",
        "Recipe A capability issues: {:?}",
        inspection_a.capability_issues
    );

    let inspection_b = harness
        .lifecycle
        .inspect_recipe_runtime(&harness.workflow_version_id, &recipe_b)
        .await
        .expect("Recipe B exact inspection should return its runtime facts");
    assert_eq!(inspection_b.recipe_id, recipe_b);
    assert_eq!(inspection_b.capability, "INCOMPATIBLE_INPUT_VALUES");

    let inspection_a_again = harness
        .lifecycle
        .inspect_recipe_runtime(&harness.workflow_version_id, &recipe_a)
        .await
        .expect("Recipe A exact inspection should remain available");
    assert_eq!(inspection_a_again.recipe_id, recipe_a);
    assert_eq!(inspection_a_again.capability, "READY");

    let workspace = harness
        .lifecycle
        .list_workspace()
        .await
        .expect("fast workspace list should remain readable");
    let version_view = workspace
        .items
        .iter()
        .find(|view| view.workflow_version_id.as_deref() == Some(&harness.workflow_version_id))
        .expect("shared workflow version should remain in the workspace");
    assert_eq!(
        version_view.capability, "NOT_CHECKED",
        "exact inspection must not write version-level capability cache"
    );
}

#[tokio::test]
async fn dev078_p1_shared_workflow_blocks_only_incompatible_recipe() {
    let harness = shared_recipe_harness().await;
    let recipe_b = recipe_id_for_version(&harness, SHARED_RECIPE_B_VERSION);
    let batch_id = create_runtime_batch_for_recipe(&harness, &recipe_b, 1).await;

    let error = harness
        .admission
        .start(PROJECT_ID, &batch_id)
        .await
        .expect_err("Recipe B must be blocked by its own incompatible capability");
    match error {
        ProductionStartAdmissionError::Runtime(failure) => {
            assert_eq!(failure.code, RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE);
            assert_eq!(failure.workflow_version_id, harness.workflow_version_id);
            assert_eq!(failure.recipe_id, recipe_b);
        }
        ProductionStartAdmissionError::Queue(error) => {
            panic!("expected Recipe B runtime admission failure, got queue error: {error}");
        }
    }

    let detail = harness
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("blocked exact-recipe batch should remain readable");
    assert_eq!(detail.batch.status, ProductionBatchStatus::Ready);
    assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Pending);
    assert_eq!(count(&harness.pool, "tasks").await, 0);
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dev078_p1_shared_workflow_allows_ready_recipe() {
    let harness = shared_recipe_harness().await;
    let recipe_a = recipe_id_for_version(&harness, SHARED_RECIPE_A_VERSION);
    let batch_id = create_runtime_batch_for_recipe(&harness, &recipe_a, 1).await;
    harness.comfy.hold_submission();

    harness
        .admission
        .start(PROJECT_ID, &batch_id)
        .await
        .expect("Recipe A must be admitted when Recipe B is incompatible");
    harness.comfy.wait_for_submission().await;

    let detail = harness
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("admitted exact-recipe batch should be readable");
    assert_eq!(detail.batch.status, ProductionBatchStatus::Running);
    assert_eq!(
        detail.items[0].status,
        ProductionBatchItemStatus::Dispatched
    );
    assert_eq!(count(&harness.pool, "tasks").await, 1);
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 1);

    harness.comfy.release_submission();
}

#[tokio::test]
async fn dev078_b3_resume_checks_only_the_current_pending_workflow() {
    let harness = harness().await;
    let batch_id = create_runtime_batch(&harness, 2).await;
    let workflow_id =
        sqlx::query_scalar::<_, String>("SELECT workflow_id FROM workflow_versions WHERE id = ?")
            .bind(&harness.workflow_version_id)
            .fetch_one(&harness.pool)
            .await
            .expect("fixture workflow should be readable");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES (?, ?, '0.9.0', ?, 'dev078-old-workflow-sha', ?)",
    )
    .bind("missing-old-workflow")
    .bind(workflow_id)
    .bind(WORKFLOW_JSON)
    .bind(CREATED_AT)
    .execute(&harness.pool)
    .await
    .expect("old workflow version should be a valid terminal fixture");
    sqlx::query(
        "UPDATE production_batch_items
         SET status = 'SUCCEEDED', workflow_version_id = 'missing-old-workflow'
         WHERE batch_id = ? AND ordinal = 0",
    )
    .bind(&batch_id)
    .execute(&harness.pool)
    .await
    .expect("old terminal item should be editable for the fixture");
    harness.comfy.hold_submission();

    harness
        .admission
        .start(PROJECT_ID, &batch_id)
        .await
        .expect("a valid pending workflow should resume despite an old unavailable item");
    harness.comfy.wait_for_submission().await;

    let detail = harness
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("resumed batch should be readable");
    assert_eq!(detail.batch.status, ProductionBatchStatus::Running);
    assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Succeeded);
    assert_eq!(
        detail.items[1].status,
        ProductionBatchItemStatus::Dispatched
    );
    assert_eq!(count(&harness.pool, "tasks").await, 1);

    harness.comfy.release_submission();
}

#[tokio::test]
async fn dev078_b4_existing_busy_admission_wins_before_runtime_checks() {
    let harness = harness().await;
    let running_batch_id = create_runtime_batch(&harness, 1).await;
    let blocked_batch_id = create_runtime_batch(&harness, 1).await;
    harness.comfy.hold_submission();

    harness
        .admission
        .start(PROJECT_ID, &running_batch_id)
        .await
        .expect("the first batch should start");
    harness.comfy.wait_for_submission().await;
    let health_calls_before = harness.comfy.health_calls.load(Ordering::SeqCst);

    let error = harness
        .admission
        .start(PROJECT_ID, &blocked_batch_id)
        .await
        .expect_err("a second active batch must be rejected as busy");
    assert!(matches!(
        error,
        ProductionStartAdmissionError::Queue(ProductionQueueError::Busy(_))
    ));
    assert_eq!(
        harness.comfy.health_calls.load(Ordering::SeqCst),
        health_calls_before,
        "busy admission should short-circuit before the runtime health check"
    );

    harness.comfy.release_submission();
}

#[tokio::test]
async fn dev078_b5_offline_runtime_keeps_batch_unchanged() {
    let harness = harness().await;
    let batch_id = create_runtime_batch(&harness, 1).await;
    harness.comfy.set_offline(true);

    let error = harness
        .admission
        .start(PROJECT_ID, &batch_id)
        .await
        .expect_err("an offline ComfyUI must block start");
    assert_eq!(
        runtime_failure_code(error),
        RUNTIME_ADMISSION_COMFY_UNAVAILABLE
    );

    let detail = harness
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("offline batch should remain readable");
    assert_eq!(detail.batch.status, ProductionBatchStatus::Ready);
    assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Pending);
    assert_eq!(count(&harness.pool, "tasks").await, 0);
}

#[tokio::test]
async fn dev078_b6_capability_refresh_failure_fails_closed() {
    let harness = harness().await;
    let batch_id = create_runtime_batch(&harness, 1).await;
    harness.comfy.set_capability_refresh_failure(true);

    let error = harness
        .admission
        .start(PROJECT_ID, &batch_id)
        .await
        .expect_err("capability refresh failure must block start");
    assert_eq!(
        runtime_failure_code(error),
        RUNTIME_ADMISSION_CAPABILITY_REFRESH_FAILED
    );

    let detail = harness
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("refresh-failed batch should remain readable");
    assert_eq!(detail.batch.status, ProductionBatchStatus::Ready);
    assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Pending);
    assert_eq!(count(&harness.pool, "tasks").await, 0);
}

#[tokio::test]
async fn dev078_existing_gate_excludes_configuration_changes_until_start_commits() {
    let harness = harness().await;
    let batch_id = create_runtime_batch(&harness, 1).await;
    harness.comfy.hold_health_check();

    let admission = harness.admission.clone();
    let start_task = tokio::spawn(async move { admission.start(PROJECT_ID, &batch_id).await });
    harness.comfy.wait_for_health_check().await;

    let (acquired_tx, mut acquired_rx) = tokio::sync::oneshot::channel();
    let queue = harness.queue.clone();
    let configuration_task = tokio::spawn(async move {
        let guard = queue.acquire_runtime_configuration_admission().await;
        acquired_tx
            .send(())
            .expect("configuration waiter should still be observed");
        guard
    });
    tokio::task::yield_now().await;
    assert!(
        !configuration_task.is_finished(),
        "configuration changes must wait while runtime admission holds the existing gate"
    );
    assert!(
        matches!(
            acquired_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ),
        "configuration changes must not acquire the gate before start commits"
    );

    harness.comfy.release_health_check();
    start_task
        .await
        .expect("start task should join")
        .expect("valid runtime should eventually commit");
    let _configuration_guard = configuration_task
        .await
        .expect("configuration waiter should join");
    assert!(acquired_rx.await.is_ok());
}

async fn seed_hierarchy(pool: &SqlitePool, workflow_version_id: &str, recipe_id: &str) {
    sqlx::query(
        "INSERT INTO assets
         (id, project_id, type, category, name, original_name, storage_path, sha256,
          mime_type, width, height, file_size, metadata_json, created_at, updated_at)
         VALUES (?, ?, 'image', 'source_image', 'Selected frame', 'selected.png',
                 'selected.png', 'sha-selected-v1', 'image/png', 512, 512, 1, '{}', ?, ?)",
    )
    .bind(SELECTED_ASSET_ID)
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("selected image fixture should insert");

    sqlx::query(
        "INSERT INTO production_series
         (id, project_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, 0, 'DEV-052 Series', 'Series description', ?, ?)",
    )
    .bind(SERIES_ID)
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("series fixture should insert");
    sqlx::query(
        "INSERT INTO production_episodes
         (id, series_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, 0, 'DEV-052 Episode', 'Episode description', ?, ?)",
    )
    .bind(EPISODE_ID)
    .bind(SERIES_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("episode fixture should insert");
    sqlx::query(
        "INSERT INTO production_scenes
         (id, episode_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, 0, 'DEV-052 Scene', 'A prepared scene', ?, ?)",
    )
    .bind(SCENE_ID)
    .bind(EPISODE_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("scene fixture should insert");

    insert_shot(
        pool,
        READY_SHOT_ID,
        1,
        None,
        true,
        workflow_version_id,
        recipe_id,
    )
    .await;
    insert_shot(
        pool,
        INCOMPLETE_SHOT_ID,
        2,
        None,
        false,
        workflow_version_id,
        recipe_id,
    )
    .await;
    insert_shot(
        pool,
        I2V_SHOT_ID,
        3,
        Some(SELECTED_ASSET_ID),
        true,
        workflow_version_id,
        recipe_id,
    )
    .await;
    sqlx::query(
        "INSERT INTO shot_stage_configs
         (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
         VALUES (?, 'video', ?, ?, '{}', ?)",
    )
    .bind(I2V_SHOT_ID)
    .bind(workflow_version_id)
    .bind(recipe_id)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("I2V stage config should insert");
}

async fn insert_shot(
    pool: &SqlitePool,
    shot_id: &str,
    ordinal: i64,
    selected_image_asset_id: Option<&str>,
    configured: bool,
    workflow_version_id: &str,
    recipe_id: &str,
) {
    sqlx::query(
        "INSERT INTO shots
         (id, project_id, ordinal, name, prompt_text, selected_image_asset_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'A stable DEV-052 shot prompt', ?, ?, ?)",
    )
    .bind(shot_id)
    .bind(PROJECT_ID)
    .bind(ordinal)
    .bind(shot_id)
    .bind(selected_image_asset_id)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("shot fixture should insert");
    sqlx::query(
        "INSERT INTO shot_scene_assignments
         (shot_id, scene_id, ordinal, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(shot_id)
    .bind(SCENE_ID)
    .bind(ordinal)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("shot scene assignment should insert");
    if configured {
        sqlx::query(
            "INSERT INTO shot_stage_configs
             (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
             VALUES (?, 'image', ?, ?, '{}', ?)",
        )
        .bind(shot_id)
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("shot stage config should insert");
    }
}

async fn seed_bulk_shots(
    pool: &SqlitePool,
    workflow_version_id: &str,
    recipe_id: &str,
) -> Vec<String> {
    let mut transaction = pool
        .begin()
        .await
        .expect("bulk fixture transaction should begin");
    let mut ids = Vec::with_capacity(500);
    for index in 0..500 {
        let shot_id = format!("shot_dev052_bulk_{index:03}");
        ids.push(shot_id.clone());
        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'bulk planning prompt', ?, ?)",
        )
        .bind(&shot_id)
        .bind(PROJECT_ID)
        .bind(1000_i64 + i64::from(index))
        .bind(&shot_id)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("bulk shot should insert");
        sqlx::query(
            "INSERT INTO shot_stage_configs
             (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
             VALUES (?, 'image', ?, ?, '{}', ?)",
        )
        .bind(&shot_id)
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("bulk stage config should insert");
    }
    transaction
        .commit()
        .await
        .expect("bulk fixture transaction should commit");
    ids
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .expect("table count should succeed")
}

fn i2v_recipe() -> ai_studio_lib::domain::Recipe {
    ai_studio_lib::compiler::RecipeParser::parse(
        r#"schema_version: 1
id: rcp_i2v_runtime
name: Runtime I2V
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
  image:
    type: image
    label: Keyframe
    required: true
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: image
    target:
      node: "10"
      input: image
outputs:
  - id: generated_video
    type: video
    node: "9"
    required: true
"#,
    )
    .expect("I2V fixture recipe should parse")
}

fn ref2va_recipe() -> ai_studio_lib::domain::Recipe {
    ai_studio_lib::compiler::RecipeParser::parse(
        r#"schema_version: 1
id: rcp_ref2va_runtime
name: Runtime REF2VA
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
  references:
    type: images
    label: References
    required: true
    min_items: 2
    max_items: 3
bindings: []
outputs:
  - id: generated_video
    type: video
    node: "9"
    required: true
"#,
    )
    .expect("REF2VA fixture recipe should parse")
}

fn context_with_ordered_refs(mut context: ResolvedShotContext) -> ResolvedShotContext {
    context.reference_assets = vec![
        ResolvedReferenceAsset {
            asset_id: "ast_ref_b".to_owned(),
            sha256: "sha-b".to_owned(),
            role: BindingRole::ShotReference,
            ordinal: 0,
            source_reference_set_id: "legacy:shot".to_owned(),
            source_profile_id: None,
            source_scope: ContextSourceScope::Legacy,
        },
        ResolvedReferenceAsset {
            asset_id: "ast_ref_a".to_owned(),
            sha256: "sha-a".to_owned(),
            role: BindingRole::ShotReference,
            ordinal: 1,
            source_reference_set_id: "legacy:shot".to_owned(),
            source_profile_id: None,
            source_scope: ContextSourceScope::Legacy,
        },
    ];
    context
}

#[tokio::test]
async fn runtime_preflight_admission_is_generation_free_and_idempotent() {
    let harness = harness().await;
    let shot_ids = vec![READY_SHOT_ID.to_owned()];
    let before = (
        count(&harness.pool, "production_batches").await,
        count(&harness.pool, "production_batch_items").await,
        count(&harness.pool, "shot_generation_links").await,
        count(&harness.pool, "production_preparation_snapshots").await,
        count(&harness.pool, "tasks").await,
    );

    let plans = harness
        .preparation
        .plan_many(PROJECT_ID, &shot_ids, ShotStage::Image)
        .await
        .expect("preflight plan should use real resolver and SQLite");
    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].readiness.status,
        ai_studio_lib::domain::ShotReadinessStatus::Ready
    );
    assert_eq!(
        (
            count(&harness.pool, "production_batches").await,
            count(&harness.pool, "production_batch_items").await,
            count(&harness.pool, "shot_generation_links").await,
            count(&harness.pool, "production_preparation_snapshots").await,
            count(&harness.pool, "tasks").await,
        ),
        before,
        "preflight must be read-only"
    );

    let admitted = harness
        .preparation
        .admit(PROJECT_ID, &shot_ids, ShotStage::Image, false)
        .await
        .expect("ready shot should be admitted");
    assert_eq!(admitted.created_count, 1);
    assert_eq!(admitted.already_prepared_count, 0);
    assert_eq!(count(&harness.pool, "production_batches").await, 1);
    assert_eq!(count(&harness.pool, "production_batch_items").await, 1);
    assert_eq!(count(&harness.pool, "shot_generation_links").await, 1);
    assert_eq!(
        count(&harness.pool, "production_preparation_snapshots").await,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM production_batches LIMIT 1")
            .fetch_one(&harness.pool)
            .await
            .expect("prepared batch status should be readable"),
        "READY"
    );
    assert_eq!(count(&harness.pool, "tasks").await, 0);
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 0);

    let item_id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM production_batch_items ORDER BY created_at ASC, id ASC LIMIT 1",
    )
    .fetch_one(&harness.pool)
    .await
    .expect("prepared item should exist");
    let snapshot_before = harness
        .preparation
        .preparation_snapshot(PROJECT_ID, &item_id)
        .await
        .expect("snapshot read should succeed")
        .expect("prepared snapshot should exist");
    let snapshot_json_before = serde_json::to_value(&snapshot_before.snapshot).unwrap();
    let batch_values_json = sqlx::query_scalar::<_, String>(
        "SELECT values_json FROM production_batch_items WHERE id = ?",
    )
    .bind(&item_id)
    .fetch_one(&harness.pool)
    .await
    .expect("prepared batch values should be readable");
    let batch_values: Value = serde_json::from_str(&batch_values_json)
        .expect("prepared batch values should be valid JSON");
    assert_eq!(
        batch_values, snapshot_before.snapshot.frozen_generation_values,
        "BatchItem values_json must equal frozen snapshot values"
    );

    let repeated = harness
        .preparation
        .admit(PROJECT_ID, &shot_ids, ShotStage::Image, false)
        .await
        .expect("matching admission should be idempotent");
    assert_eq!(repeated.created_count, 0);
    assert_eq!(repeated.already_prepared_count, 1);
    assert_eq!(count(&harness.pool, "production_batches").await, 1);
    assert_eq!(
        count(&harness.pool, "production_preparation_snapshots").await,
        1
    );
    assert_eq!(count(&harness.pool, "tasks").await, 0);
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 0);
    let snapshot_after = harness
        .preparation
        .preparation_snapshot(PROJECT_ID, &item_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(snapshot_after.snapshot).unwrap(),
        snapshot_json_before
    );
}

#[tokio::test]
async fn runtime_changed_context_creates_new_snapshot_and_rollback_is_atomic() {
    let harness = harness().await;
    let shot_ids = vec![READY_SHOT_ID.to_owned()];
    harness
        .preparation
        .admit(PROJECT_ID, &shot_ids, ShotStage::Image, false)
        .await
        .expect("initial admission should succeed");
    let old_item_id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM production_batch_items ORDER BY created_at ASC, id ASC LIMIT 1",
    )
    .fetch_one(&harness.pool)
    .await
    .unwrap();
    let old_snapshot = harness
        .preparation
        .preparation_snapshot(PROJECT_ID, &old_item_id)
        .await
        .unwrap()
        .unwrap();
    let old_snapshot_json = serde_json::to_value(&old_snapshot.snapshot).unwrap();

    sqlx::query("UPDATE shots SET prompt_text = 'changed DEV-052 context' WHERE id = ?")
        .bind(READY_SHOT_ID)
        .execute(&harness.pool)
        .await
        .expect("context update should succeed");
    let changed = harness
        .preparation
        .admit(PROJECT_ID, &shot_ids, ShotStage::Image, false)
        .await
        .expect("changed context should create a new preparation");
    assert_eq!(changed.created_count, 1);
    assert_eq!(count(&harness.pool, "production_batches").await, 2);
    assert_eq!(
        count(&harness.pool, "production_preparation_snapshots").await,
        2
    );
    assert_eq!(
        count_distinct(
            &harness.pool,
            "context_hash",
            "production_preparation_snapshots"
        )
        .await,
        2
    );
    let old_snapshot_after = harness
        .preparation
        .preparation_snapshot(PROJECT_ID, &old_item_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(old_snapshot_after.snapshot).unwrap(),
        old_snapshot_json
    );

    let now = Utc::now();
    let batch = ProductionBatch {
        id: ai_studio_lib::domain::ProductionBatchId::new(),
        project_id: PROJECT_ID.to_owned(),
        name: "DEV-052 rollback fixture".to_owned(),
        status: ProductionBatchStatus::Ready,
        continue_on_failure: false,
        archived_at: None,
        created_at: now,
        updated_at: now,
    };
    let item = ProductionBatchItem {
        id: ProductionBatchItemId::new(),
        batch_id: batch.id.clone(),
        ordinal: 0,
        workflow_version_id: harness.workflow_version_id.clone(),
        recipe_id: harness.recipe_id.clone(),
        values_json: json!({"prompt": "rollback"}),
        status: ProductionBatchItemStatus::Pending,
        task_id: None,
        retry_of_item_id: None,
        error_code: None,
        error_message: None,
        created_at: now,
        updated_at: now,
    };
    let binding = ShotBatchBinding {
        shot_id: READY_SHOT_ID.to_owned(),
        stage: ShotStage::Image,
        production_batch_item_id: item.id.as_str().to_owned(),
    };
    let duplicate_snapshot = PreparationSnapshotRecord {
        id: old_snapshot.id,
        project_id: PROJECT_ID.to_owned(),
        shot_id: READY_SHOT_ID.to_owned(),
        stage: ShotStage::Image,
        context_hash: old_snapshot.context_hash,
        production_batch_id: batch.id.as_str().to_owned(),
        production_batch_item_id: item.id.as_str().to_owned(),
        snapshot: old_snapshot.snapshot,
        created_at: now,
    };
    let rollback_result = harness
        .queue_repository
        .insert_prepared_batch_with_bindings(
            &batch,
            std::slice::from_ref(&item),
            std::slice::from_ref(&binding),
            std::slice::from_ref(&duplicate_snapshot),
        )
        .await;
    assert!(
        rollback_result.is_err(),
        "duplicate snapshot id must fail late"
    );
    assert_eq!(count(&harness.pool, "production_batches").await, 2);
    assert_eq!(count(&harness.pool, "production_batch_items").await, 2);
    assert_eq!(count(&harness.pool, "shot_generation_links").await, 2);
    assert_eq!(
        count(&harness.pool, "production_preparation_snapshots").await,
        2
    );
}

#[tokio::test]
async fn runtime_i2v_selected_identity_and_ref2va_order_are_frozen() {
    let harness = harness().await;
    let context = harness
        .resolver
        .resolve_draft(PROJECT_ID, I2V_SHOT_ID, ShotStage::Video)
        .await
        .expect("real resolver should load selected I2V asset");
    assert_eq!(
        context.stage_input,
        ResolvedStageInput {
            selected_image_asset_id: Some(SELECTED_ASSET_ID.to_owned()),
            selected_image_sha256: Some("sha-selected-v1".to_owned()),
        }
    );
    let readiness = harness
        .readiness
        .preflight(PROJECT_ID, I2V_SHOT_ID, ShotStage::Video)
        .await
        .expect("I2V readiness should be evaluable without GPU execution");
    let frozen = PreparationSnapshotV1::from_context(
        &context,
        &readiness,
        json!({"image": SELECTED_ASSET_ID, "prompt": "motion"}),
        ComfyCapabilityEvidence::default(),
        Utc::now(),
    );
    assert_eq!(
        frozen.stage_input.selected_image_asset_id.as_deref(),
        Some(SELECTED_ASSET_ID)
    );
    assert_eq!(
        frozen.stage_input.selected_image_sha256.as_deref(),
        Some("sha-selected-v1")
    );

    let i2v_values =
        ShotBatchService::prepare_values_from_context(ShotStage::Video, &context, &i2v_recipe())
            .expect("I2V values should use the selected image from resolved context");
    assert_eq!(
        i2v_values.get("image"),
        Some(&GenerationInputValue::ImageAsset(
            AssetId::parse(SELECTED_ASSET_ID.to_owned()).unwrap()
        ))
    );

    let ref_context = context_with_ordered_refs(context);
    let ref_values = ShotBatchService::prepare_values_from_context(
        ShotStage::Video,
        &ref_context,
        &ref2va_recipe(),
    )
    .expect("REF2VA values should preserve resolved reference order");
    assert_eq!(
        ref_values.get("references"),
        Some(&GenerationInputValue::ImageAssets(vec![
            AssetId::parse("ast_ref_b".to_owned()).unwrap(),
            AssetId::parse("ast_ref_a".to_owned()).unwrap(),
        ]))
    );
}

#[tokio::test]
async fn runtime_non_ready_partial_and_101_limit_never_write_before_validation() {
    let harness = harness().await;
    let mixed = vec![READY_SHOT_ID.to_owned(), INCOMPLETE_SHOT_ID.to_owned()];
    let rejected = harness
        .preparation
        .admit(PROJECT_ID, &mixed, ShotStage::Image, false)
        .await
        .expect_err("non-partial admission must reject incomplete shots");
    assert!(rejected.to_string().contains("PREPARATION_NOT_READY"));
    assert_eq!(count(&harness.pool, "production_batches").await, 0);
    assert_eq!(count(&harness.pool, "production_batch_items").await, 0);
    assert_eq!(count(&harness.pool, "shot_generation_links").await, 0);
    assert_eq!(
        count(&harness.pool, "production_preparation_snapshots").await,
        0
    );

    let partial = harness
        .preparation
        .admit(PROJECT_ID, &mixed, ShotStage::Image, true)
        .await
        .expect("allow_partial should keep the ready subset");
    assert_eq!(partial.created_count, 1);
    assert_eq!(partial.skipped_incomplete, 1);
    assert_eq!(count(&harness.pool, "production_batches").await, 1);

    let too_many = (0..101)
        .map(|index| format!("shot_dev052_limit_{index:03}"))
        .collect::<Vec<_>>();
    let limit_error = harness
        .preparation
        .admit(PROJECT_ID, &too_many, ShotStage::Image, true)
        .await
        .expect_err("101-shot admission must fail before any write");
    assert!(limit_error.to_string().contains("100"));
    assert_eq!(count(&harness.pool, "production_batches").await, 1);
    assert_eq!(
        count(&harness.pool, "production_preparation_snapshots").await,
        1
    );
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn runtime_500_shot_plan_uses_one_bounded_preflight_path() {
    let harness = harness().await;
    let shot_ids = seed_bulk_shots(
        &harness.pool,
        &harness.workflow_version_id,
        &harness.recipe_id,
    )
    .await;
    harness.source_calls.store(0, Ordering::SeqCst);
    harness.comfy.reset();

    let plans = harness
        .preparation
        .plan_many(PROJECT_ID, &shot_ids, ShotStage::Image)
        .await
        .expect("500-shot planning should complete through batch resolver");
    assert_eq!(plans.len(), 500);
    assert_eq!(harness.source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(harness.comfy.health_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        harness.comfy.object_info_calls.load(Ordering::SeqCst),
        2,
        "one Comfy preflight refresh plus one workspace capability check"
    );
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 0);
    assert_eq!(count(&harness.pool, "production_batches").await, 0);
    assert_eq!(
        count(&harness.pool, "production_preparation_snapshots").await,
        0
    );
}

#[tokio::test]
async fn runtime_database_is_fresh_migrated_through_029() {
    let harness = harness().await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&harness.pool)
            .await
            .unwrap(),
        29
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(&harness.pool)
            .await
            .unwrap(),
        1
    );
}

async fn count_distinct(pool: &SqlitePool, column: &str, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(DISTINCT {column}) FROM {table}"))
        .fetch_one(pool)
        .await
        .expect("distinct count should succeed")
}
