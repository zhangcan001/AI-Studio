//! DEV-055 Agent C: real ComfyUI release-gate coverage.
//!
//! This test is deliberately ignored.  When explicitly enabled it discovers
//! the endpoint from the existing AI Studio settings file, performs the
//! read-only ComfyUI preflight, and then exercises the normal production
//! queue path against an isolated AI Studio data root.  It never edits the
//! user's database or workflow library and never sends a queue-control
//! request.

use ai_studio_lib::application::ports::SettingsStore;
use ai_studio_lib::application::{
    comfy_service::{ComfyRuntime, ComfyService},
    generation_service::GenerationService,
    ports::{
        AssetRepository, AssetStore, AvailableGenerationDefinition, Clock, ComfyAdapter,
        ComfyAdapterFactory, ComfyConnectionConfig, GenerationDefinition,
        GenerationDefinitionRepository, GenerationSnapshotRepository, NoopTaskUpdateSink,
        ProductionQueueRepository, ProjectRecord, ProjectRepository, ShotBatchRepository,
        TaskRepository, WorkflowLibrarySource, WorkflowRunRepository, WorkflowRuntimeRepository,
        WorkflowRuntimeStateRepository,
    },
    production_queue_service::ProductionQueueService,
    shot_batch_service::{CreateShotBatchRequest, ShotBatchService},
    shot_service::{ShotService, ShotStageConfigRequest, ShotUpdateRequest},
    task_query_service::TaskQueryService,
    task_recovery_service::TaskRecoveryService,
    workflow_library_service::WorkflowLibraryService,
    workflow_onboarding_service::{
        CapabilityCheckView, CapabilityState, WorkflowOnboardingService,
    },
};
use ai_studio_lib::compiler::RecipeParser;
use ai_studio_lib::domain::{
    Asset, AssetType, ProductionBatchId, ProductionBatchItemStatus, Recipe, ShotStage, Task,
    TaskEventType, TaskId, TaskStatus,
};
use ai_studio_lib::infrastructure::{
    comfy::ComfyHttpAdapterFactory,
    database::{
        initialize, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
        SqliteGenerationSnapshotRepository, SqliteProductionQueueRepository,
        SqliteProjectRepository, SqlitePromptLibraryRepository, SqliteShotRepository,
        SqliteTaskRepository, SqliteWorkflowLibraryRepository, SqliteWorkflowRunRepository,
        SqliteWorkflowRuntimeRepository, SqliteWorkflowRuntimeStateRepository,
    },
    filesystem::{
        AppDataDirs, FileSystemAssetStore, FileSystemWorkflowLibrarySource,
        FileSystemWorkflowPackageStore,
    },
    settings::JsonSettingsStore,
    time::SystemClock,
};
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

const PROJECT_ID: &str = "prj_05500000-0055-4055-8055-000000000055";
const KERA2_WORKFLOW_ID: &str = "wfl_kera2_t2i_local_v2";
const H3_I2V_WORKFLOW_ID: &str = "wfl_minimax_h3_fl2va_i2v_quality";
const H3_REF2VA_WORKFLOW_ID: &str = "wfl_minimax_h3_reference_video_quality";
const IMAGE_OUTPUT_ID: &str = "generated_image";
const VIDEO_OUTPUT_ID: &str = "generated_video";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real ComfyUI release gate; requires AI_STUDIO_LIVE_COMFY=1"]
async fn dev055_real_comfyui_release_gate() {
    let started_at = Utc::now();
    eprintln!("AGENT_C STARTED_AT={}", started_at.to_rfc3339());
    require_live_gate();

    let (settings_dir, settings_source) = discover_settings_directory();
    let settings_store = JsonSettingsStore::new(settings_dir.clone());
    let loaded = settings_store.load().await;
    if let Some(warning) = loaded.warning.as_deref() {
        fail_env(format!(
            "SETTINGS_NOT_READY source={} warning={warning}",
            settings_source.display()
        ));
    }
    let endpoint = loaded.settings.comfy.endpoint.trim().to_owned();
    let config = ComfyConnectionConfig::from_endpoint(&endpoint)
        .unwrap_or_else(|error| fail_env(format!("SETTINGS_ENDPOINT_INVALID: {error}")));
    eprintln!(
        "SETTINGS_ENDPOINT=PASS source={} endpoint={}",
        settings_source.display(),
        config.endpoint()
    );

    let adapter: Arc<dyn ComfyAdapter> = ComfyHttpAdapterFactory
        .create(config.clone())
        .unwrap_or_else(|error| fail_env(format!("COMFY_ADAPTER_NOT_READY: {error}")));

    let system_stats = adapter.get_system_stats().await.unwrap_or_else(|error| {
        fail_env(format!(
            "COMFY_RUNTIME_NOT_AVAILABLE endpoint={} /system_stats={error}; local_installation={}",
            config.endpoint(),
            local_comfy_installation_evidence()
        ))
    });
    let object_info = adapter.get_object_info().await.unwrap_or_else(|error| {
        fail_env(format!(
            "COMFY_RUNTIME_NOT_AVAILABLE endpoint={} /object_info={error}; local_installation={}",
            config.endpoint(),
            local_comfy_installation_evidence()
        ))
    });
    let object_info_node_count = object_info
        .as_object()
        .map(|object| object.len())
        .unwrap_or_else(|| {
            fail_env(format!(
                "ENV_NOT_READY: COMFY_PROTOCOL_ERROR /object_info is not an object at {}",
                config.endpoint()
            ))
        });
    if object_info_node_count == 0 {
        fail_env(format!(
            "ENV_NOT_READY: COMFY_PROTOCOL_ERROR /object_info is empty at {}",
            config.endpoint()
        ));
    }
    eprintln!(
        "COMFY_PREFLIGHT=PASS endpoint={} comfyui_version={:?} python_version={:?} object_info_nodes={}",
        config.endpoint(),
        system_stats.comfyui_version,
        system_stats.python_version,
        object_info_node_count
    );

    // Exercise the existing runtime/status service using the same adapter and
    // configuration that will be handed to GenerationService.
    let runtime = Arc::new(ComfyRuntime::new(adapter.clone(), config.clone()));
    let comfy_service = Arc::new(ComfyService::from_runtime(runtime));
    comfy_service
        .get_status()
        .await
        .unwrap_or_else(|error| fail_env(format!("COMFY_STATUS_NOT_READY: {error}")));
    let capability_summary = comfy_service
        .refresh_capabilities()
        .await
        .unwrap_or_else(|error| fail_env(format!("COMFY_CAPABILITY_CACHE_NOT_READY: {error}")));
    eprintln!(
        "COMFY_RUNTIME=PASS endpoint={} cached_node_count={}",
        config.endpoint(),
        capability_summary.node_count
    );

    let workflow_library_root = discover_workflow_library_root().unwrap_or_else(|| {
        fail_env(format!(
            "WORKFLOW_RUNTIME_PACKAGE_NOT_AVAILABLE; no existing workflow library was found; local_installation={}",
            local_comfy_installation_evidence()
        ))
    });

    // The only data root touched by the test is this temporary, isolated root.
    // The workflow library source above is read-only and is never used as a
    // write destination.
    let isolated_directory = tempdir().expect("DEV-055 isolated tempdir should exist");
    let isolated_dirs = AppDataDirs::initialize(isolated_directory.path().join("AIStudioData"))
        .expect("DEV-055 isolated AI Studio data root should initialize");
    let pool = initialize(&isolated_dirs.database)
        .await
        .expect("DEV-055 isolated database should initialize");

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let source: Arc<dyn WorkflowLibrarySource> = Arc::new(FileSystemWorkflowLibrarySource::new(
        workflow_library_root.clone(),
    ));
    let workflow_library_repository = Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone()));
    let workflow_library_service = Arc::new(WorkflowLibraryService::new(
        source.clone(),
        workflow_library_repository,
        clock.clone(),
    ));
    let sync = workflow_library_service
        .sync()
        .await
        .unwrap_or_else(|error| fail_env(format!("WORKFLOW_LIBRARY_NOT_READY: {error}")));
    eprintln!(
        "WORKFLOW_LIBRARY=PASS source={} packages_found={} valid={} invalid={}",
        workflow_library_root.display(),
        sync.packages_found,
        sync.valid,
        sync.invalid
    );

    let definition_repository_impl =
        Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let definition_repository: Arc<dyn GenerationDefinitionRepository> =
        definition_repository_impl.clone();
    let available_definitions = definition_repository
        .list_available()
        .await
        .expect("DEV-055 available generation definitions should be readable");
    let image_available = required_definition(&available_definitions, KERA2_WORKFLOW_ID);
    let i2v_available = required_definition(&available_definitions, H3_I2V_WORKFLOW_ID);
    let image_definition = load_definition(
        &definition_repository,
        &image_available.workflow_version_id,
        &image_available.recipe_id,
        "KREA2",
    )
    .await;
    let i2v_definition = load_definition(
        &definition_repository,
        &i2v_available.workflow_version_id,
        &i2v_available.recipe_id,
        "MINIMAX_H3_I2V",
    )
    .await;
    let image_recipe = parse_recipe(&image_definition, "KREA2");
    let i2v_recipe = parse_recipe(&i2v_definition, "MINIMAX_H3_I2V");

    let workflow_run_repository: Arc<dyn WorkflowRunRepository> =
        Arc::new(SqliteWorkflowRunRepository::new(pool.clone()));
    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let runtime_state_repository: Arc<dyn WorkflowRuntimeStateRepository> =
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone()));
    let package_store: Arc<dyn ai_studio_lib::application::ports::WorkflowPackageStore> =
        Arc::new(FileSystemWorkflowPackageStore::new(
            isolated_dirs.workflow_library.clone(),
            isolated_dirs.workflow_staging.clone(),
        ));
    let onboarding_service = Arc::new(
        WorkflowOnboardingService::new(
            source.clone(),
            adapter.clone(),
            workflow_library_service.clone(),
            workflow_run_repository,
            package_store,
            clock.clone(),
        )
        .with_runtime_state(runtime_repository, runtime_state_repository),
    );

    let image_capability = capability_for(
        &onboarding_service,
        &image_definition,
        &image_recipe,
        &object_info,
        "KREA2",
    );
    require_capability_ready("KREA2", &image_capability);
    let i2v_capability = capability_for(
        &onboarding_service,
        &i2v_definition,
        &i2v_recipe,
        &object_info,
        "MINIMAX_H3_I2V",
    );
    require_capability_ready("MINIMAX_H3_I2V", &i2v_capability);

    let ref2va_definition = available_definitions
        .iter()
        .find(|definition| definition.workflow_id == H3_REF2VA_WORKFLOW_ID)
        .cloned();
    let ref2va_definition = match ref2va_definition {
        Some(available) => Some(
            load_definition(
                &definition_repository,
                &available.workflow_version_id,
                &available.recipe_id,
                "MINIMAX_H3_REF2VA",
            )
            .await,
        ),
        None => {
            eprintln!(
                "REF2VA=ENV_NOT_READY reason=runtime package {} is not available; execution skipped",
                H3_REF2VA_WORKFLOW_ID
            );
            None
        }
    };
    let ref2va_recipe = ref2va_definition
        .as_ref()
        .map(|definition| parse_recipe(definition, "MINIMAX_H3_REF2VA"));
    let ref2va_ready = match (&ref2va_definition, &ref2va_recipe) {
        (Some(definition), Some(recipe)) => {
            let capability = capability_for(
                &onboarding_service,
                definition,
                recipe,
                &object_info,
                "MINIMAX_H3_REF2VA",
            );
            if capability.state == CapabilityState::Ready {
                eprintln!("REF2VA=CAPABILITY_READY state=READY");
                true
            } else {
                eprintln!(
                    "REF2VA=ENV_NOT_READY state={:?} issues={}; execution skipped",
                    capability.state,
                    capability_issues(&capability)
                );
                false
            }
        }
        _ => false,
    };

    let project_repository_impl = Arc::new(SqliteProjectRepository::new(pool.clone()));
    let project_repository: Arc<dyn ProjectRepository> = project_repository_impl.clone();
    let project_root = isolated_dirs.projects.join("dev055-live-project");
    fs::create_dir_all(&project_root).expect("DEV-055 isolated project root should exist");
    let now = Utc::now();
    project_repository
        .insert(&ProjectRecord {
            id: PROJECT_ID.to_owned(),
            name: "DEV-055 Live ComfyUI Release Gate".to_owned(),
            description: Some("isolated real ComfyUI release-gate project".to_owned()),
            root_path: project_root,
            created_at: now,
            updated_at: now,
        })
        .await
        .expect("DEV-055 isolated project should be persisted through ProjectRepository");

    let task_repository_impl = Arc::new(SqliteTaskRepository::new(pool.clone()));
    let task_repository: Arc<dyn TaskRepository> = task_repository_impl.clone();
    let snapshot_repository_impl = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> =
        snapshot_repository_impl.clone();
    let asset_repository_impl = Arc::new(SqliteAssetRepository::new(pool.clone()));
    let asset_repository: Arc<dyn AssetRepository> = asset_repository_impl.clone();
    let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
    let shot_repository = Arc::new(SqliteShotRepository::new(pool.clone()));
    let prompt_repository = Arc::new(SqlitePromptLibraryRepository::new(pool.clone()));
    let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let production_queue_repository: Arc<dyn ProductionQueueRepository> = queue_repository.clone();
    let shot_batch_repository: Arc<dyn ShotBatchRepository> = queue_repository.clone();

    let generation_service = Arc::new(
        GenerationService::new(
            task_repository.clone(),
            snapshot_repository.clone(),
            definition_repository.clone(),
            adapter.clone(),
            project_repository.clone(),
            asset_store.clone(),
            asset_repository.clone(),
            clock.clone(),
        )
        .with_workflow_compatibility_service(onboarding_service),
    );
    let task_recovery_service = Arc::new(TaskRecoveryService::new(
        task_repository.clone(),
        snapshot_repository.clone(),
        asset_repository.clone(),
        adapter.clone(),
        project_repository.clone(),
        asset_store.clone(),
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));
    let queue_service = Arc::new(ProductionQueueService::new(
        production_queue_repository,
        task_repository.clone(),
        definition_repository.clone(),
        generation_service.clone(),
        shot_batch_repository.clone(),
        task_recovery_service,
        clock.clone(),
    ));
    let task_query_service = Arc::new(TaskQueryService::new(
        task_repository.clone(),
        asset_repository.clone(),
        definition_repository.clone(),
    ));
    let shot_service = Arc::new(
        ShotService::new(
            shot_repository.clone(),
            task_repository.clone(),
            asset_repository.clone(),
            definition_repository.clone(),
            prompt_repository,
            task_query_service,
            generation_service,
            shot_batch_repository.clone(),
            clock.clone(),
        )
        .with_stage_prompt_repository(shot_repository.clone())
        .with_generation_snapshot_repository(snapshot_repository.clone()),
    );
    let shot_batch_service = Arc::new(
        ShotBatchService::new(
            shot_repository.clone(),
            shot_batch_repository,
            task_repository.clone(),
            asset_repository.clone(),
            definition_repository.clone(),
            project_repository.clone(),
            clock.clone(),
        )
        .with_stage_prompt_repository(shot_repository.clone()),
    );

    let shot_count = if ref2va_ready { 3 } else { 1 };
    let mut shot_ids = Vec::with_capacity(shot_count);
    for index in 0..shot_count {
        let shot = shot_service
            .create(PROJECT_ID)
            .await
            .expect("DEV-055 shot should be created through ShotService");
        let shot = shot_service
            .update(ShotUpdateRequest {
                project_id: PROJECT_ID.to_owned(),
                shot_id: shot.id,
                name: format!("DEV-055 Live Shot {}", index + 1),
                prompt_text: format!(
                    "DEV-055 release-gate image for shot {}; preserve a clear subject",
                    index + 1
                ),
                prompt_entry_id: None,
                prompt_version_id: None,
            })
            .await
            .expect("DEV-055 shot prompt should be updated through ShotService");
        shot_service
            .set_stage_config(ShotStageConfigRequest {
                project_id: PROJECT_ID.to_owned(),
                shot_id: shot.id.clone(),
                stage: ShotStage::Image,
                workflow_version_id: image_available.workflow_version_id.clone(),
                recipe_id: image_available.recipe_id.clone(),
                values: BTreeMap::new(),
            })
            .await
            .expect("DEV-055 Krea2 image stage should be configured through ShotService");
        shot_ids.push(shot.id);
    }

    let image_batch = shot_batch_service
        .create(CreateShotBatchRequest {
            project_id: PROJECT_ID.to_owned(),
            stage: ShotStage::Image,
            shot_ids: shot_ids.clone(),
        })
        .await
        .expect("DEV-055 Krea2 image batch should be created through ShotBatchService");
    queue_service
        .start_for_test(PROJECT_ID, image_batch.batch.id.as_str())
        .await
        .expect("DEV-055 Krea2 image batch should start through ProductionQueueService");
    let timeout_seconds = live_timeout_seconds();
    let image_tasks = wait_for_batch(
        &queue_repository,
        &task_repository_impl,
        &image_batch.batch.id,
        timeout_seconds,
        "KREA2",
    )
    .await;
    assert_eq!(image_tasks.len(), shot_ids.len());

    let mut image_assets = Vec::with_capacity(image_tasks.len());
    for (index, task) in image_tasks.iter().enumerate() {
        let image_asset = verify_task_output(
            adapter.as_ref(),
            &*task_repository,
            &*snapshot_repository,
            &*asset_repository,
            PROJECT_ID,
            task,
            &image_definition,
            IMAGE_OUTPUT_ID,
            AssetType::Image,
        )
        .await;
        shot_service
            .select_result(
                PROJECT_ID,
                &shot_ids[index],
                ShotStage::Image,
                image_asset.id.as_str(),
                true,
            )
            .await
            .expect("DEV-055 image output should be selectable through ShotService");
        image_assets.push(image_asset);
    }
    let selected_image = &image_assets[0];
    let selected_shot = shot_service
        .get(PROJECT_ID, &shot_ids[0])
        .await
        .expect("DEV-055 selected image shot should be readable");
    assert_eq!(
        selected_shot.selected_image_asset_id.as_deref(),
        Some(selected_image.id.as_str())
    );
    eprintln!(
        "MANUAL_IMAGE_SELECTION=PASS selectedImageAssetId={} sha256={}",
        selected_image.id, selected_image.sha256
    );

    shot_service
        .set_stage_config(ShotStageConfigRequest {
            project_id: PROJECT_ID.to_owned(),
            shot_id: shot_ids[0].clone(),
            stage: ShotStage::Video,
            workflow_version_id: i2v_available.workflow_version_id.clone(),
            recipe_id: i2v_available.recipe_id.clone(),
            values: BTreeMap::new(),
        })
        .await
        .expect("DEV-055 H3 I2V stage should be configured through ShotService");
    let i2v_batch = shot_batch_service
        .create(CreateShotBatchRequest {
            project_id: PROJECT_ID.to_owned(),
            stage: ShotStage::Video,
            shot_ids: vec![shot_ids[0].clone()],
        })
        .await
        .expect("DEV-055 H3 I2V batch should be created through ShotBatchService");
    let stage_input = i2v_batch.items[0]
        .values_json
        .get("first_frame")
        .and_then(|value| value.get("assetId"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("ENV_NOT_READY: I2V batch is missing first_frame.assetId"));
    assert_eq!(stage_input, selected_image.id.as_str());
    queue_service
        .start_for_test(PROJECT_ID, i2v_batch.batch.id.as_str())
        .await
        .expect("DEV-055 H3 I2V batch should start through ProductionQueueService");
    let i2v_tasks = wait_for_batch(
        &queue_repository,
        &task_repository_impl,
        &i2v_batch.batch.id,
        timeout_seconds,
        "MINIMAX_H3_I2V",
    )
    .await;
    assert_eq!(i2v_tasks.len(), 1);
    let i2v_task = &i2v_tasks[0];
    let i2v_asset = verify_task_output(
        adapter.as_ref(),
        &*task_repository,
        &*snapshot_repository,
        &*asset_repository,
        PROJECT_ID,
        i2v_task,
        &i2v_definition,
        VIDEO_OUTPUT_ID,
        AssetType::Video,
    )
    .await;
    let i2v_snapshot = snapshot_repository_impl
        .find_by_task_id(&i2v_task.id)
        .await
        .expect("DEV-055 I2V snapshot should be readable")
        .expect("DEV-055 I2V snapshot should exist");
    assert_eq!(
        i2v_snapshot.user_inputs_json["first_frame"]["assetId"],
        Value::String(selected_image.id.as_str().to_owned())
    );
    assert_eq!(
        i2v_snapshot.resolved_inputs_json["first_frame"]["assetId"],
        Value::String(selected_image.id.as_str().to_owned())
    );
    assert_eq!(
        i2v_snapshot.resolved_inputs_json["first_frame"]["sha256"],
        Value::String(selected_image.sha256.clone())
    );
    eprintln!(
        "I2V stageInput.assetId={} stageInput.sha256={} CONSISTENT=PASS",
        stage_input, selected_image.sha256
    );
    shot_service
        .select_result(
            PROJECT_ID,
            &shot_ids[0],
            ShotStage::Video,
            i2v_asset.id.as_str(),
            true,
        )
        .await
        .expect("DEV-055 I2V output should be selectable through ShotService");

    if ref2va_ready {
        let ref_definition = ref2va_definition
            .as_ref()
            .expect("READY REF2VA must have a loaded definition");
        let ref_recipe = ref2va_recipe
            .as_ref()
            .expect("READY REF2VA must have a parsed recipe");
        let reference_shot_id = &shot_ids[2];
        shot_service
            .set_stage_config(ShotStageConfigRequest {
                project_id: PROJECT_ID.to_owned(),
                shot_id: reference_shot_id.clone(),
                stage: ShotStage::Video,
                workflow_version_id: ref_definition.workflow_version_id.clone(),
                recipe_id: ref_definition.recipe_id.clone(),
                values: BTreeMap::new(),
            })
            .await
            .expect("DEV-055 REF2VA stage should be configured through ShotService");
        let reference_ids = vec![
            image_assets[0].id.as_str().to_owned(),
            image_assets[1].id.as_str().to_owned(),
        ];
        shot_service
            .replace_references(
                PROJECT_ID,
                reference_shot_id,
                ShotStage::Video,
                reference_ids.clone(),
            )
            .await
            .expect("DEV-055 REF2VA references should be bound through ShotService");
        let ref_batch = shot_batch_service
            .create(CreateShotBatchRequest {
                project_id: PROJECT_ID.to_owned(),
                stage: ShotStage::Video,
                shot_ids: vec![reference_shot_id.clone()],
            })
            .await
            .expect("DEV-055 REF2VA batch should be created through ShotBatchService");
        assert_eq!(
            ref_batch.items[0].values_json["reference_images"]["assetIds"],
            serde_json::json!(reference_ids)
        );
        queue_service
            .start_for_test(PROJECT_ID, ref_batch.batch.id.as_str())
            .await
            .expect("DEV-055 REF2VA batch should start through ProductionQueueService");
        let ref_tasks = wait_for_batch(
            &queue_repository,
            &task_repository_impl,
            &ref_batch.batch.id,
            timeout_seconds,
            "MINIMAX_H3_REF2VA",
        )
        .await;
        assert_eq!(ref_tasks.len(), 1);
        let ref_asset = verify_task_output(
            adapter.as_ref(),
            &*task_repository,
            &*snapshot_repository,
            &*asset_repository,
            PROJECT_ID,
            &ref_tasks[0],
            ref_definition,
            VIDEO_OUTPUT_ID,
            AssetType::Video,
        )
        .await;
        let ref_snapshot = snapshot_repository_impl
            .find_by_task_id(&ref_tasks[0].id)
            .await
            .expect("DEV-055 REF2VA snapshot should be readable")
            .expect("DEV-055 REF2VA snapshot should exist");
        assert_eq!(
            ref_snapshot.user_inputs_json["reference_images"]["assetIds"],
            serde_json::json!(reference_ids)
        );
        let resolved_references = ref_snapshot.resolved_inputs_json["reference_images"]
            .as_array()
            .expect("DEV-055 REF2VA resolved references should be an array");
        assert_eq!(resolved_references.len(), reference_ids.len());
        for (resolved, expected) in resolved_references.iter().zip(image_assets.iter()) {
            assert_eq!(
                resolved["assetId"],
                Value::String(expected.id.as_str().to_owned())
            );
            assert_eq!(resolved["sha256"], Value::String(expected.sha256.clone()));
        }
        shot_service
            .select_result(
                PROJECT_ID,
                reference_shot_id,
                ShotStage::Video,
                ref_asset.id.as_str(),
                true,
            )
            .await
            .expect("DEV-055 REF2VA output should be selectable through ShotService");
        eprintln!("REF2VA=PASS capability=READY");
        let _ = ref_recipe;
    }

    eprintln!("RELEASE_GATE=PASS");
    eprintln!("AGENT_C FINISHED_AT={}", Utc::now().to_rfc3339());
}

fn require_live_gate() {
    if env::var("AI_STUDIO_LIVE_COMFY").ok().as_deref() != Some("1") {
        panic!(
            "ENV_NOT_READY: AI_STUDIO_LIVE_COMFY=1 is required; the ignored real ComfyUI release gate was not executed"
        );
    }
}

fn fail_env(message: String) -> ! {
    let message = format!("ENV_NOT_READY: {message}");
    eprintln!("{message}");
    panic!("{message}");
}

fn discover_settings_directory() -> (PathBuf, PathBuf) {
    let mut candidates = Vec::new();
    if let Some(file) = absolute_env_path("AI_STUDIO_LIVE_SETTINGS_FILE") {
        if let Some(parent) = file.parent() {
            candidates.push(parent.to_path_buf());
        }
    }
    for variable in ["AI_STUDIO_LIVE_DATA_ROOT", "AI_STUDIO_DATA_ROOT"] {
        if let Some(root) = absolute_env_path(variable) {
            candidates.push(root.join("config"));
        }
    }
    if let Some(root) = default_ai_studio_data_root() {
        candidates.push(root.join("config"));
    }
    candidates.dedup();
    let directory = candidates
        .iter()
        .find(|candidate| candidate.join("settings.json").is_file())
        .cloned()
        .or_else(|| candidates.first().cloned())
        .unwrap_or_else(|| PathBuf::from("config"));
    let source = directory.join("settings.json");
    (directory, source)
}

fn discover_workflow_library_root() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = absolute_env_path("AI_STUDIO_LIVE_WORKFLOW_LIBRARY") {
        candidates.push(path);
    }
    for variable in ["AI_STUDIO_LIVE_DATA_ROOT", "AI_STUDIO_DATA_ROOT"] {
        if let Some(root) = absolute_env_path(variable) {
            candidates.push(root.join("workflow_library"));
        }
    }
    if let Some(root) = default_ai_studio_data_root() {
        candidates.push(root.join("workflow_library"));
    }
    candidates.into_iter().find(|candidate| candidate.is_dir())
}

fn default_ai_studio_data_root() -> Option<PathBuf> {
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        return Some(
            PathBuf::from(local_app_data)
                .join("AIStudio")
                .join("AIStudioData"),
        );
    }
    if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME") {
        return Some(PathBuf::from(xdg_data_home).join("AIStudioData"));
    }
    env::var_os("USERPROFILE").map(|profile| PathBuf::from(profile).join("AIStudioData"))
}

fn absolute_env_path(variable: &str) -> Option<PathBuf> {
    env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn local_comfy_installation_evidence() -> String {
    let mut roots = Vec::new();
    if let Some(root) = absolute_env_path("AI_STUDIO_LIVE_COMFY_ROOT") {
        roots.push(root);
    }
    roots.extend([
        PathBuf::from(r"D:\ComfyUI-WorkFisher-V2"),
        PathBuf::from(r"C:\ComfyUI-WorkFisher-V2"),
        PathBuf::from(r"C:\ComfyUI"),
    ]);
    if let Some(profile) = env::var_os("USERPROFILE") {
        roots.push(PathBuf::from(profile).join("ComfyUI"));
    }
    roots.dedup();

    let mut evidence = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        let entry = |relative: &str| root.join(relative).is_file();
        evidence.push(format!(
            "{}[main.py={},python.exe={},run_nvidia_gpu.bat={},run_cpu.bat={}]",
            root.display(),
            entry("main.py"),
            entry("python/python.exe"),
            entry("run_nvidia_gpu.bat"),
            entry("run_cpu.bat")
        ));
    }
    if evidence.is_empty() {
        "no known existing ComfyUI installation/start entrypoint found; automatic start disabled"
            .to_owned()
    } else {
        format!(
            "{}; automatic start disabled; no queue mutation performed",
            evidence.join(";")
        )
    }
}

fn required_definition(
    definitions: &[AvailableGenerationDefinition],
    workflow_id: &str,
) -> AvailableGenerationDefinition {
    definitions
        .iter()
        .find(|definition| definition.workflow_id == workflow_id)
        .cloned()
        .unwrap_or_else(|| {
            fail_env(format!(
                "required runtime package {} is not registered in the discovered workflow library",
                workflow_id
            ))
        })
}

async fn load_definition(
    repository: &Arc<dyn GenerationDefinitionRepository>,
    workflow_version_id: &str,
    recipe_id: &str,
    label: &str,
) -> GenerationDefinition {
    repository
        .find(workflow_version_id, recipe_id)
        .await
        .expect("DEV-055 generation definition lookup should work")
        .unwrap_or_else(|| {
            fail_env(format!(
                "{label} workflow definition disappeared after workflow library sync"
            ))
        })
}

fn parse_recipe(definition: &GenerationDefinition, label: &str) -> Recipe {
    RecipeParser::parse(&definition.recipe_yaml)
        .unwrap_or_else(|error| fail_env(format!("{label} recipe is invalid: {error}")))
}

fn capability_for(
    onboarding_service: &WorkflowOnboardingService,
    definition: &GenerationDefinition,
    recipe: &Recipe,
    object_info: &Value,
    label: &str,
) -> CapabilityCheckView {
    onboarding_service
        .check_runtime_workflow_with_recipe_and_object_info(
            &definition.workflow_json.to_string(),
            recipe,
            object_info,
        )
        .unwrap_or_else(|error| fail_env(format!("{label} capability check failed: {error}")))
}

fn require_capability_ready(label: &str, capability: &CapabilityCheckView) {
    if capability.state != CapabilityState::Ready {
        fail_env(format!(
            "{label} capability is {:?}; issues={}",
            capability.state,
            capability_issues(capability)
        ));
    }
    eprintln!("CAPABILITY {}=READY", label);
}

fn capability_issues(capability: &CapabilityCheckView) -> String {
    if capability.issues.is_empty() {
        return "none".to_owned();
    }
    capability
        .issues
        .iter()
        .map(|issue| format!("{}:{}", issue.code, issue.message))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn live_timeout_seconds() -> u64 {
    env::var("AI_STUDIO_LIVE_COMFY_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(900)
        .max(30)
}

async fn wait_for_batch(
    queue_repository: &SqliteProductionQueueRepository,
    task_repository: &SqliteTaskRepository,
    batch_id: &ProductionBatchId,
    timeout_seconds: u64,
    label: &str,
) -> Vec<Task> {
    for _ in 0..timeout_seconds {
        let detail = queue_repository
            .find_detail(PROJECT_ID, batch_id)
            .await
            .expect("DEV-055 production batch should remain readable");
        let detail = detail.expect("DEV-055 production batch should exist");
        let mut tasks = Vec::with_capacity(detail.items.len());
        let mut all_succeeded = detail.batch.status.as_str() == "COMPLETED";
        for item in &detail.items {
            if item.status == ProductionBatchItemStatus::Failed
                || item.status == ProductionBatchItemStatus::Cancelled
            {
                panic!(
                    "LIVE_COMFY_FAILURE label={} item_status={} code={:?} message={:?}",
                    label,
                    item.status.as_str(),
                    item.error_code,
                    item.error_message
                );
            }
            if item.status != ProductionBatchItemStatus::Succeeded {
                all_succeeded = false;
            }
            let Some(task_id) = item.task_id.as_deref() else {
                all_succeeded = false;
                continue;
            };
            let task_id = TaskId::parse(task_id.to_owned())
                .expect("DEV-055 production task id should have a valid domain shape");
            let task = task_repository
                .find_by_id(&task_id)
                .await
                .expect("DEV-055 production task should remain readable");
            let Some(task) = task else {
                all_succeeded = false;
                continue;
            };
            if task.status == TaskStatus::Failed || task.status == TaskStatus::Cancelled {
                let code = task
                    .error
                    .as_ref()
                    .map(|error| error.code.as_str())
                    .unwrap_or("TASK_FAILED");
                let message = task
                    .error
                    .as_ref()
                    .map(|error| error.message.as_str())
                    .unwrap_or("task entered a terminal failure state");
                if matches!(
                    code,
                    "COMFY_OFFLINE"
                        | "COMFY_TIMEOUT"
                        | "COMFY_STREAM_DISCONNECTED"
                        | "COMFY_INPUT_UPLOAD_FAILED"
                        | "COMFY_IMAGE_UPLOAD_FAILED"
                ) {
                    fail_env(format!(
                        "COMFY_RUNTIME_NOT_AVAILABLE during {label}: {code}: {message}; local_installation={}",
                        local_comfy_installation_evidence()
                    ));
                }
                panic!("LIVE_COMFY_FAILURE label={label} task_error={code}: {message}");
            }
            if task.status != TaskStatus::Succeeded {
                all_succeeded = false;
            }
            tasks.push(task);
        }
        if all_succeeded && tasks.len() == detail.items.len() {
            return tasks;
        }
        sleep(Duration::from_secs(1)).await;
    }
    panic!(
        "LIVE_COMFY_TIMEOUT label={} batch_id={} timeout_seconds={}; runtime left untouched",
        label,
        batch_id.as_str(),
        timeout_seconds
    );
}

async fn verify_task_output(
    adapter: &dyn ComfyAdapter,
    task_repository: &dyn TaskRepository,
    snapshot_repository: &dyn GenerationSnapshotRepository,
    asset_repository: &dyn AssetRepository,
    project_id: &str,
    task: &Task,
    definition: &GenerationDefinition,
    expected_output_id: &str,
    expected_asset_type: AssetType,
) -> Asset {
    assert_eq!(task.project_id, project_id);
    assert_eq!(task.status, TaskStatus::Succeeded);
    assert_eq!(task.workflow_version_id, definition.workflow_version_id);
    assert_eq!(task.recipe_id, definition.recipe_id);
    let prompt_id = task
        .prompt_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("LIVE_COMFY_FAILURE: succeeded task has no prompt id"));
    let provenance = task
        .runtime_provenance
        .as_ref()
        .unwrap_or_else(|| panic!("LIVE_COMFY_FAILURE: task has no runtime provenance"));
    assert_eq!(provenance.workflow_id, definition.workflow_id);
    assert_eq!(
        provenance.workflow_version_id,
        definition.workflow_version_id
    );
    assert_eq!(provenance.recipe_id, definition.recipe_id);
    assert!(!provenance.app_version.trim().is_empty());
    assert!(!provenance.build_commit.trim().is_empty());
    assert!(task
        .telemetry
        .generation_execution_id
        .as_deref()
        .is_some_and(|value| value.starts_with("gen_") && value.len() > 4));
    assert!(task
        .telemetry
        .compiled_workflow_sha256
        .as_deref()
        .is_some_and(|value| value.len() == 64));
    assert!(task.telemetry.submitted_at.is_some());
    assert!(task.telemetry.execution_started_at.is_some());
    assert!(task.telemetry.execution_finished_at.is_some());
    assert!(task.telemetry.collection_finished_at.is_some());

    let events = task_repository
        .list_events(&task.id)
        .await
        .expect("DEV-055 task events should be readable");
    let submission_event = events
        .iter()
        .find(|event| event.event_type == TaskEventType::TaskSubmissionPrepared)
        .unwrap_or_else(|| panic!("LIVE_COMFY_FAILURE: task has no submission identity event"));
    let submission = submission_event
        .payload
        .as_ref()
        .expect("DEV-055 submission identity event should have a payload");
    assert_eq!(submission["promptId"], Value::String(prompt_id.to_owned()));
    assert!(submission["clientId"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert_eq!(
        submission["generationExecutionId"],
        Value::String(
            task.telemetry
                .generation_execution_id
                .clone()
                .expect("generation execution id should be present"),
        )
    );

    let history = adapter
        .get_history(prompt_id)
        .await
        .unwrap_or_else(|error| {
            panic!("LIVE_COMFY_FAILURE: history for prompt {prompt_id}: {error}")
        });
    assert_eq!(history.prompt_id, prompt_id);
    assert!(history
        .outputs
        .values()
        .any(|output| !output.images.is_empty() || !output.saved_results.is_empty()));

    let snapshot = snapshot_repository
        .find_by_task_id(&task.id)
        .await
        .expect("DEV-055 generation snapshot lookup should work")
        .unwrap_or_else(|| panic!("LIVE_COMFY_FAILURE: task {} has no snapshot", task.id));
    assert_eq!(snapshot.task_id, task.id);
    assert!(snapshot.workflow_json.is_object());
    assert!(!snapshot.recipe_yaml.trim().is_empty());
    assert!(snapshot.user_inputs_json.is_object());
    assert!(snapshot.resolved_inputs_json.is_object());

    let mapped_assets = asset_repository
        .list_mapped_assets(&task.id)
        .await
        .expect("DEV-055 task output mappings should be readable");
    let (mapping, asset) = mapped_assets
        .into_iter()
        .find(|(mapping, asset)| {
            mapping.output_id == expected_output_id && asset.asset_type == expected_asset_type
        })
        .unwrap_or_else(|| {
            panic!(
                "LIVE_COMFY_FAILURE: task {} has no {} {:?} output mapping",
                task.id, expected_output_id, expected_asset_type
            )
        });
    assert_eq!(mapping.task_id, task.id);
    assert_eq!(asset.project_id, project_id);
    assert_eq!(asset.source_task_id.as_ref(), Some(&task.id));
    assert_eq!(asset.asset_type, expected_asset_type);
    assert_eq!(
        asset.metadata_json["outputId"],
        Value::String(expected_output_id.to_owned())
    );
    let file_path = PathBuf::from(&asset.storage_path);
    assert!(
        file_path.is_file(),
        "asset file should exist: {}",
        file_path.display()
    );
    let bytes = fs::read(&file_path).expect("DEV-055 asset file should be readable");
    assert!(!bytes.is_empty());
    assert_eq!(asset.file_size, bytes.len() as u64);
    assert_eq!(asset.sha256, sha256_bytes(&bytes));
    assert!(asset.width > 0, "asset width should be recorded");
    assert!(asset.height > 0, "asset height should be recorded");

    let thumbnail_path = asset
        .thumbnail_path
        .as_deref()
        .unwrap_or_else(|| panic!("LIVE_COMFY_FAILURE: asset {} has no thumbnail", asset.id));
    assert!(
        Path::new(thumbnail_path).is_file(),
        "thumbnail should exist: {thumbnail_path}"
    );
    let thumbnail_dimensions = image::image_dimensions(thumbnail_path)
        .unwrap_or_else(|error| panic!("LIVE_COMFY_FAILURE: thumbnail is unreadable: {error}"));
    assert!(thumbnail_dimensions.0 > 0 && thumbnail_dimensions.1 > 0);
    if expected_asset_type == AssetType::Image {
        let dimensions = image::image_dimensions(&file_path)
            .expect("DEV-055 generated image file should have readable dimensions");
        assert_eq!((asset.width, asset.height), dimensions);
    }
    eprintln!(
        "ARTIFACT=PASS taskId={} snapshotId={} assetId={} file={} sha256={} dimensions={}x{} thumbnail={} promptId={} generationExecutionId={}",
        task.id,
        snapshot.id,
        asset.id,
        file_path.display(),
        asset.sha256,
        asset.width,
        asset.height,
        thumbnail_path,
        prompt_id,
        task.telemetry
            .generation_execution_id
            .as_deref()
            .expect("generation execution id should be present")
    );
    asset
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
