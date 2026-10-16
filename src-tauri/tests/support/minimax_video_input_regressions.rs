//! Phase2 media authority regressions; reuses owned real Registry/SQLite fixture.
#[path = "migration_head.rs"]
mod migration_head;
use super::*;
use ai_studio_lib::application::{
    comfy_preflight_service::ComfyPreflightService,
    comfy_service::{ComfyRuntime, ComfyService},
    diagnostics_service::DiagnosticsService,
    production_preparation_service::ProductionPreparationService,
    production_start_admission_service::ProductionStartAdmissionService,
    shot_context_resolver::ShotContextResolver,
    shot_readiness_service::ShotReadinessService,
    workflow_library_service::WorkflowLibraryService,
    workflow_lifecycle_service::WorkflowLifecycleService,
    workflow_onboarding_service::WorkflowOnboardingService,
};
use ai_studio_lib::application::{
    ports::{
        ShotRecord, ShotRepository, ShotStageConfigRecord, ShotVideoInputAsset, ShotVideoInputScope,
    },
    shot_video_input_service::ShotVideoInputService,
};
use ai_studio_lib::domain::{Asset, AssetId};
use ai_studio_lib::infrastructure::database::repositories::SqliteConsistencyScopeRepository;
use ai_studio_lib::infrastructure::database::repositories::SqliteShotVideoInputRepository;
use ai_studio_lib::infrastructure::{
    database::{
        SqliteConsistencyProfileRepository, SqliteDatabaseHealthProbe,
        SqliteProductionStructureRepository, SqliteReferenceSetRepository,
        SqliteShotConsistencyRepository, SqliteWorkflowRunRepository,
    },
    filesystem::FileSystemWorkflowLibrarySource,
    logging::LoggingStatus,
};

fn input(key: &str, ordinal: i64, id: &AssetId) -> ShotVideoInputAsset {
    ShotVideoInputAsset {
        input_key: key.into(),
        ordinal,
        asset_id: id.to_string(),
    }
}
fn scope(f: &Fixture, index: usize) -> ShotVideoInputScope {
    ShotVideoInputScope {
        project_id: PROJECT_ID.into(),
        shot_id: "shot_phase2".into(),
        workflow_version_id: f.pairs[index].0.clone(),
        recipe_id: f.pairs[index].1.clone(),
    }
}
async fn setup(f: &Fixture) -> (Arc<ShotVideoInputService>, Asset, Asset) {
    let shots = Arc::new(SqliteShotRepository::new(f.pool.clone()));
    let now = Utc::now();
    shots
        .insert(&ShotRecord {
            id: "shot_phase2".into(),
            project_id: PROJECT_ID.into(),
            ordinal: 0,
            name: "Phase2".into(),
            prompt_text: "move".into(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
    let assets = Arc::new(SqliteAssetRepository::new(f.pool.clone()));
    let projects = Arc::new(SqliteProjectRepository::new(f.pool.clone()));
    let store = Arc::new(FileSystemAssetStore::new());
    let importer = SourceAssetImportService::new(
        projects.clone(),
        store.clone(),
        assets.clone(),
        Arc::new(SystemClock),
    );
    let first = importer
        .import_bytes(PROJECT_ID, "first.png", &png_bytes([10, 20, 30, 255]))
        .await
        .unwrap();
    let last = importer
        .import_bytes(PROJECT_ID, "last.png", &png_bytes([40, 50, 60, 255]))
        .await
        .unwrap();
    let service = Arc::new(ShotVideoInputService::new(
        assets.clone(),
        Arc::new(SqliteShotVideoInputRepository::new(f.pool.clone())),
        shots,
        Arc::new(SqliteGenerationDefinitionRepository::new(f.pool.clone())),
        f.registry.clone(),
        Arc::new(GenerationInputPreparer::new(
            assets,
            store,
            projects,
            f.comfy.clone(),
        )),
        Arc::new(SystemClock),
    ));
    (service, first, last)
}
async fn configure(f: &Fixture, index: usize) {
    SqliteShotRepository::new(f.pool.clone()).upsert_stage_config(PROJECT_ID, &ShotStageConfigRecord { shot_id: "shot_phase2".into(), stage: ai_studio_lib::domain::ShotStage::Video,
        workflow_version_id: f.pairs[index].0.clone(), recipe_id: f.pairs[index].1.clone(), scalar_values: json!({"duration_seconds":{"type":"integer","value":5},"width":{"type":"integer","value":1344},"height":{"type":"integer","value":768},"seed":{"type":"seed_fixed","value":"42"}}), updated_at: Utc::now() }).await.unwrap();
}
fn batch_service(f: &Fixture, inputs: Arc<ShotVideoInputService>) -> ShotBatchService {
    ShotBatchService::new(
        Arc::new(SqliteShotRepository::new(f.pool.clone())),
        Arc::new(SqliteProductionQueueRepository::new(f.pool.clone())),
        Arc::new(SqliteTaskRepository::new(f.pool.clone())),
        Arc::new(SqliteAssetRepository::new(f.pool.clone())),
        Arc::new(SqliteGenerationDefinitionRepository::new(f.pool.clone())),
        Arc::new(SqliteProjectRepository::new(f.pool.clone())),
        Arc::new(SystemClock),
    )
    .with_video_inputs(inputs)
    .with_new_generation_admission(f.registry.clone())
}

fn shot_service(
    f: &Fixture,
    inputs: Arc<ShotVideoInputService>,
) -> ai_studio_lib::application::shot_service::ShotService {
    use ai_studio_lib::application::task_query_service::TaskQueryService;
    use ai_studio_lib::infrastructure::database::SqlitePromptLibraryRepository;
    let tasks = Arc::new(SqliteTaskRepository::new(f.pool.clone()));
    let assets = Arc::new(SqliteAssetRepository::new(f.pool.clone()));
    let definitions = Arc::new(SqliteGenerationDefinitionRepository::new(f.pool.clone()));
    ai_studio_lib::application::shot_service::ShotService::new(
        Arc::new(SqliteShotRepository::new(f.pool.clone())),
        tasks.clone(),
        assets.clone(),
        definitions.clone(),
        Arc::new(SqlitePromptLibraryRepository::new(f.pool.clone())),
        Arc::new(TaskQueryService::new(tasks, assets, definitions)),
        Arc::new(SqliteProductionQueueRepository::new(f.pool.clone())),
        Arc::new(SystemClock),
    )
    .with_video_inputs(inputs)
}

// Same formal composition as Native bootstrap. Only Comfy capabilities/events
// and media codec bytes are fixtures; SQLite, Registry, readiness and freeze are real.
async fn formal_preparation(
    f: &Fixture,
    inputs: Arc<ShotVideoInputService>,
) -> (
    Arc<ProductionPreparationService>,
    Arc<ProductionStartAdmissionService>,
) {
    fs::create_dir_all(f._owned.path().join("staging")).unwrap();
    let source = Arc::new(FileSystemWorkflowLibrarySource::new(f.root.clone()));
    let library = Arc::new(WorkflowLibraryService::new(
        source.clone(),
        Arc::new(SqliteWorkflowLibraryRepository::new(f.pool.clone())),
        Arc::new(SystemClock),
    ));
    let runtime = Arc::new(SqliteWorkflowRuntimeRepository::new(f.pool.clone()));
    let state = Arc::new(SqliteWorkflowRuntimeStateRepository::new(f.pool.clone()));
    let packages = Arc::new(FileSystemWorkflowPackageStore::new(
        f.root.clone(),
        f._owned.path().join("staging"),
    ));
    let onboarding = Arc::new(
        WorkflowOnboardingService::new(
            source.clone(),
            f.comfy.clone(),
            library.clone(),
            Arc::new(SqliteWorkflowRunRepository::new(f.pool.clone())),
            packages.clone(),
            Arc::new(SystemClock),
        )
        .with_runtime_state(runtime.clone(), state.clone()),
    );
    let lifecycle = Arc::new(
        WorkflowLifecycleService::new(
            source,
            library,
            onboarding,
            runtime,
            state,
            packages,
            Arc::new(SystemClock),
        )
        .with_recipe_runtime_state_repository(Arc::new(
            SqliteWorkflowRecipeRuntimeStateRepository::new(f.pool.clone()),
        ))
        .with_runtime_artifact_repository(Arc::new(
            SqliteWorkflowRuntimeArtifactRepository::new(f.pool.clone()),
        )),
    );
    lifecycle.recheck_all_capabilities().await.unwrap();
    let comfy = Arc::new(ComfyService::from_runtime(Arc::new(ComfyRuntime::new(
        f.comfy.clone(),
        Default::default(),
    ))));
    let diagnostics = Arc::new(DiagnosticsService::new(
        Arc::new(SqliteDatabaseHealthProbe::new(f.pool.clone())),
        Arc::new(SqliteTaskRepository::new(f.pool.clone())),
        comfy.clone(),
        lifecycle.clone(),
        f.services.queue.clone(),
        f._owned.path().join("logs"),
        LoggingStatus {
            available: false,
            retention_days: 7,
        },
    ));
    let structure = Arc::new(SqliteProductionStructureRepository::new(f.pool.clone()));
    let resolver = Arc::new(
        ShotContextResolver::new(
            Arc::new(SqliteProjectRepository::new(f.pool.clone())),
            structure.clone(),
            Arc::new(SqliteShotRepository::new(f.pool.clone())),
            Arc::new(SqliteConsistencyScopeRepository::new(f.pool.clone())),
            Arc::new(SqliteConsistencyProfileRepository::new(f.pool.clone())),
            Arc::new(SqliteReferenceSetRepository::new(f.pool.clone())),
            Arc::new(SqliteShotConsistencyRepository::new(f.pool.clone())),
            Arc::new(SqliteAssetRepository::new(f.pool.clone())),
            Arc::new(SystemClock),
        )
        .with_video_inputs(inputs.clone()),
    );
    let start = Arc::new(ProductionStartAdmissionService::new(
        f.services.queue.clone(),
        comfy.clone(),
        lifecycle.clone(),
    ));
    let readiness = Arc::new(ShotReadinessService::new(
        resolver,
        Arc::new(ComfyPreflightService::new(
            comfy,
            diagnostics,
            lifecycle.clone(),
        )),
        lifecycle,
        structure,
    ));
    (
        Arc::new(ProductionPreparationService::new(
            Arc::new(batch_service(f, inputs)),
            Arc::new(SqliteProductionQueueRepository::new(f.pool.clone())),
            readiness,
            Arc::new(SqliteGenerationDefinitionRepository::new(f.pool.clone())),
            Arc::new(SqliteProjectRepository::new(f.pool.clone())),
            Arc::new(SystemClock),
        )),
        start,
    )
}

async fn formal_video_shot(f: &Fixture, id: &str, index: usize) {
    let shots = SqliteShotRepository::new(f.pool.clone());
    let mut record = shots
        .find(PROJECT_ID, "shot_phase2")
        .await
        .unwrap()
        .unwrap()
        .shot;
    record.id = id.into();
    record.name = id.into();
    record.ordinal = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM shots WHERE project_id=?",
    )
    .bind(PROJECT_ID)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    shots.insert(&record).await.unwrap();
    shots.upsert_stage_config(PROJECT_ID, &ShotStageConfigRecord { shot_id: id.into(), stage: ai_studio_lib::domain::ShotStage::Video,
        workflow_version_id: f.pairs[index].0.clone(), recipe_id: f.pairs[index].1.clone(), scalar_values: json!({"duration_seconds":{"type":"integer","value":5},"width":{"type":"integer","value":1344},"height":{"type":"integer","value":768},"seed":{"type":"seed_fixed","value":"42"}}), updated_at: Utc::now() }).await.unwrap();
}

fn formal_scope(f: &Fixture, id: &str, index: usize) -> ShotVideoInputScope {
    ShotVideoInputScope {
        shot_id: id.into(),
        ..scope(f, index)
    }
}

async fn managed_media(
    f: &Fixture,
    template: &Asset,
    kind: ai_studio_lib::domain::AssetType,
) -> Asset {
    let mut asset = template.clone();
    asset.id = AssetId::new();
    asset.asset_type = kind;
    let name = if kind == ai_studio_lib::domain::AssetType::Video {
        "video"
    } else {
        "audio"
    };
    asset.category = format!("source_{name}");
    asset.storage_path = f
        ._owned
        .path()
        .join("project")
        .join(format!("{}.fixture", asset.id))
        .display()
        .to_string();
    let bytes = format!("owned NO_GPU {name}");
    fs::write(&asset.storage_path, &bytes).unwrap();
    asset.sha256 = format!("{:x}", Sha256::digest(bytes.as_bytes()));
    asset.file_size = bytes.len() as u64;
    asset.mime_type = if name == "video" {
        "video/mp4"
    } else {
        "audio/wav"
    }
    .into();
    asset.duration_ms = Some(2000);
    SqliteAssetRepository::new(f.pool.clone())
        .insert_external_source(&asset)
        .await
        .unwrap();
    asset
}

#[tokio::test]
async fn phase5b_formal_mixed_batch_freezes_four_modes_reuses_and_preserves_order() {
    use ai_studio_lib::domain::{AssetType, ShotStage};
    let f = fixture_with_capability(true).await;
    let (inputs, first, last) = setup(&f).await;
    let video = managed_media(&f, &first, AssetType::Video).await;
    let audio = managed_media(&f, &first, AssetType::Audio).await;
    let ids = ["t2v", "i2v", "first_last", "ref2va"];
    for (index, id) in ids.iter().enumerate() {
        formal_video_shot(&f, id, index).await;
    }
    inputs
        .save(
            &formal_scope(&f, ids[1], 1),
            None,
            &[input("first_frame", 0, &first.id)],
        )
        .await
        .unwrap();
    let fl = inputs
        .save(
            &formal_scope(&f, ids[2], 2),
            None,
            &[
                input("last_frame", 0, &last.id),
                input("first_frame", 0, &first.id),
            ],
        )
        .await
        .unwrap();
    inputs
        .save(
            &formal_scope(&f, ids[3], 3),
            None,
            &[
                input("reference_images", 1, &first.id),
                input("reference_images", 0, &last.id),
                input("reference_videos", 0, &video.id),
                input("reference_audios", 0, &audio.id),
            ],
        )
        .await
        .unwrap();
    let (preparation, start) = formal_preparation(&f, inputs.clone()).await;
    let order = ids
        .iter()
        .rev()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    let plan = preparation
        .plan_many(PROJECT_ID, &order, ShotStage::Video)
        .await
        .unwrap();
    assert!(
        plan.iter()
            .all(|p| p.readiness.status == ai_studio_lib::domain::ShotReadinessStatus::Ready),
        "{plan:?}"
    );
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(count(&f.pool, "production_batches").await, 0);
    let (a, b) = tokio::join!(
        preparation.admit(PROJECT_ID, &order, ShotStage::Video, false),
        preparation.admit(PROJECT_ID, &order, ShotStage::Video, false)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.created_count + b.created_count, 4);
    assert_eq!(a.already_prepared_count + b.already_prepared_count, 4);
    assert_eq!(count(&f.pool, "production_batches").await, 1);
    assert_eq!(count(&f.pool, "production_preparation_snapshots").await, 4);
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 0);
    let batch = a
        .created_batch_ids
        .first()
        .or_else(|| b.created_batch_ids.first())
        .unwrap();
    let detail = f.services.queue.get(PROJECT_ID, batch).await.unwrap();
    assert_eq!(detail.items.len(), 4);
    for (ordinal, item) in detail.items.iter().enumerate() {
        let index = 3 - ordinal;
        assert_eq!(item.ordinal, ordinal as u32);
        assert_eq!(item.workflow_version_id, f.pairs[index].0);
        assert_eq!(item.recipe_id, f.pairs[index].1);
        assert!(item.task_id.is_none());
        assert_eq!(item.status, ProductionBatchItemStatus::Pending);
        let snapshot = preparation
            .preparation_snapshot(PROJECT_ID, item.id.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.snapshot.frozen_generation_values, item.values_json);
    }
    assert!(detail.items[3].values_json.get("first_frame").is_none());
    assert_eq!(
        detail.items[2].values_json["first_frame"]["assetId"],
        first.id.as_str()
    );
    assert_eq!(
        detail.items[1].values_json["first_frame"]["assetId"],
        first.id.as_str()
    );
    assert_eq!(
        detail.items[1].values_json["last_frame"]["assetId"],
        last.id.as_str()
    );
    assert_eq!(
        detail.items[0].values_json["reference_images"]["assetIds"],
        json!([last.id.as_str(), first.id.as_str()])
    );
    assert_eq!(
        detail.items[0].values_json["reference_videos"]["assetIds"],
        json!([video.id.as_str()])
    );
    assert_eq!(
        detail.items[0].values_json["reference_audios"]["assetIds"],
        json!([audio.id.as_str()])
    );
    inputs
        .save(
            &formal_scope(&f, ids[2], 2),
            Some(&fl.token),
            &[
                input("first_frame", 0, &last.id),
                input("last_frame", 0, &first.id),
            ],
        )
        .await
        .unwrap();
    let changed = preparation
        .plan_many(PROJECT_ID, &order, ShotStage::Video)
        .await
        .unwrap();
    assert!(changed[1].stale_prepared_batch_ids.contains(batch));
    assert_eq!(
        f.services.queue.get(PROJECT_ID, batch).await.unwrap().items[1].values_json,
        detail.items[1].values_json
    );
    assert!(preparation
        .admit("foreign_project", &order, ShotStage::Video, true)
        .await
        .is_err());
    assert!(preparation
        .admit(
            PROJECT_ID,
            &[ids[0].into(), ids[0].into()],
            ShotStage::Video,
            true
        )
        .await
        .is_err());
    assert!(preparation
        .admit(
            PROJECT_ID,
            &(0..101).map(|i| format!("limit-{i}")).collect::<Vec<_>>(),
            ShotStage::Video,
            true
        )
        .await
        .is_err());
    assert_eq!(count(&f.pool, "production_batches").await, 1);
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        migration_head::latest_migration_version()
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&f.pool)
        .await
        .unwrap()
        .is_empty());
    // NO_GPU: explicit official admission executes all four frozen recipes
    // using only the controlled external adapter, never the user ComfyUI.
    start.start(PROJECT_ID, batch).await.unwrap();
    for item in &detail.items {
        wait_for_item_status(
            &f.pool,
            item.id.as_str(),
            ProductionBatchItemStatus::Succeeded,
        )
        .await;
    }
    wait_for_batch_status(&f.pool, batch, ProductionBatchStatus::Completed).await;
    let executed = f.services.queue.get(PROJECT_ID, batch).await.unwrap();
    assert_eq!(count(&f.pool, "tasks").await, 4);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 4);
    assert_eq!(f.comfy.max_concurrent.load(Ordering::SeqCst), 1);
    let tasks = SqliteTaskRepository::new(f.pool.clone());
    let mut task_ids = std::collections::HashSet::new();
    for (item, frozen) in executed.items.iter().zip(&detail.items) {
        assert_eq!(item.values_json, frozen.values_json);
        let task_id = item.task_id.as_ref().unwrap();
        assert!(task_ids.insert(task_id.clone()));
        let task = tasks
            .find_by_id(&ai_studio_lib::domain::TaskId::parse(task_id).unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.project_id, PROJECT_ID);
        assert_eq!(task.workflow_version_id, item.workflow_version_id);
        assert_eq!(task.recipe_id, item.recipe_id);
        assert_eq!(task.status, ai_studio_lib::domain::TaskStatus::Succeeded);
    }
    assert_eq!(count(&f.pool, "task_output_assets").await, 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM artifact_reviews WHERE decision='PENDING'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        4
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM shots WHERE selected_video_asset_id IS NOT NULL"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&f.pool)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn phase5b_formal_partial_is_explicit_and_fresh_media_denial_is_atomic() {
    use ai_studio_lib::domain::ShotStage;
    let f = fixture_with_capability(true).await;
    let (inputs, first, _) = setup(&f).await;
    formal_video_shot(&f, "ready-t2v", 0).await;
    formal_video_shot(&f, "missing-i2v", 1).await;
    let (preparation, _) = formal_preparation(&f, inputs.clone()).await;
    let ids = vec!["ready-t2v".into(), "missing-i2v".into()];
    assert!(preparation
        .admit(PROJECT_ID, &ids, ShotStage::Video, false)
        .await
        .is_err());
    assert_eq!(count(&f.pool, "production_batches").await, 0);
    assert_eq!(count(&f.pool, "shot_generation_links").await, 0);
    let partial = preparation
        .admit(PROJECT_ID, &ids, ShotStage::Video, true)
        .await
        .unwrap();
    assert_eq!(partial.created_count, 1);
    assert_eq!(partial.skipped_incomplete + partial.skipped_blocked, 1);
    inputs
        .save(
            &formal_scope(&f, "missing-i2v", 1),
            None,
            &[input("first_frame", 0, &first.id)],
        )
        .await
        .unwrap();
    preparation
        .plan_many(PROJECT_ID, &["missing-i2v".into()], ShotStage::Video)
        .await
        .unwrap();
    fs::write(&first.storage_path, b"tampered after fresh plan").unwrap();
    assert!(preparation
        .admit(PROJECT_ID, &["missing-i2v".into()], ShotStage::Video, false)
        .await
        .is_err());
    assert_eq!(count(&f.pool, "production_batches").await, 1);
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 0);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&f.pool)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn phase5b_scope_moved_after_plan_rejects_atomic_admission_in_same_project() {
    use ai_studio_lib::application::production_structure_service::{
        CreateEpisodeRequest, CreateSceneRequest, CreateSeriesRequest, ProductionStructureService,
    };
    use ai_studio_lib::domain::{production_preparation::PreparationScope, ShotStage};
    let f = fixture_with_capability(true).await;
    let (inputs, _, _) = setup(&f).await;
    formal_video_shot(&f, "move-t2v", 0).await;
    let structure = ProductionStructureService::new(
        Arc::new(SqliteProductionStructureRepository::new(f.pool.clone())),
        Arc::new(SystemClock),
    );
    let series = structure
        .create_series(CreateSeriesRequest {
            project_id: PROJECT_ID.into(),
            name: "series".into(),
            description: String::new(),
        })
        .await
        .unwrap();
    let episode = structure
        .create_episode(CreateEpisodeRequest {
            project_id: PROJECT_ID.into(),
            series_id: series.id.clone(),
            name: "episode".into(),
            description: String::new(),
        })
        .await
        .unwrap();
    let mut scenes = Vec::new();
    for name in ["before", "after"] {
        scenes.push(
            structure
                .create_scene(CreateSceneRequest {
                    project_id: PROJECT_ID.into(),
                    episode_id: episode.id.clone(),
                    name: name.into(),
                    description: String::new(),
                })
                .await
                .unwrap()
                .id,
        );
    }
    let ids = vec!["move-t2v".into()];
    structure
        .assign_shots(PROJECT_ID, &scenes[0], &ids)
        .await
        .unwrap();
    let (preparation, _) = formal_preparation(&f, inputs).await;
    preparation
        .plan_many(PROJECT_ID, &ids, ShotStage::Video)
        .await
        .unwrap();
    structure
        .assign_shots(PROJECT_ID, &scenes[1], &ids)
        .await
        .unwrap();
    for denied in [
        PreparationScope::Scene {
            id: scenes[0].clone(),
        },
        PreparationScope::Episode {
            id: "foreign-episode".into(),
        },
        PreparationScope::Series {
            id: "foreign-series".into(),
        },
        PreparationScope::Unassigned,
    ] {
        let error = preparation
            .admit_scoped(PROJECT_ID, &ids, ShotStage::Video, true, Some(&denied))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("PREPARATION_SCOPE_CHANGED"));
        assert_eq!(count(&f.pool, "production_batches").await, 0);
        assert_eq!(count(&f.pool, "shot_generation_links").await, 0);
    }
    let allowed = PreparationScope::Scene {
        id: scenes[1].clone(),
    };
    assert_eq!(
        preparation
            .admit_scoped(PROJECT_ID, &ids, ShotStage::Video, false, Some(&allowed))
            .await
            .unwrap()
            .created_count,
        1
    );
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&f.pool)
        .await
        .unwrap()
        .is_empty());
}

async fn scene_service(
    f: &Fixture,
    inputs: Arc<ShotVideoInputService>,
) -> (
    ai_studio_lib::application::scene_production_service::SceneProductionService,
    String,
) {
    use ai_studio_lib::application::production_structure_service::{
        CreateEpisodeRequest, CreateSceneRequest, CreateSeriesRequest, ProductionStructureService,
    };
    use ai_studio_lib::infrastructure::database::SqliteProductionStructureRepository;
    let structure = Arc::new(ProductionStructureService::new(
        Arc::new(SqliteProductionStructureRepository::new(f.pool.clone())),
        Arc::new(SystemClock),
    ));
    let series = structure
        .create_series(CreateSeriesRequest {
            project_id: PROJECT_ID.into(),
            name: "owned series".into(),
            description: String::new(),
        })
        .await
        .unwrap();
    let episode = structure
        .create_episode(CreateEpisodeRequest {
            project_id: PROJECT_ID.into(),
            series_id: series.id,
            name: "owned episode".into(),
            description: String::new(),
        })
        .await
        .unwrap();
    let scene = structure
        .create_scene(CreateSceneRequest {
            project_id: PROJECT_ID.into(),
            episode_id: episode.id,
            name: "owned scene".into(),
            description: String::new(),
        })
        .await
        .unwrap();
    structure
        .assign_shots(PROJECT_ID, &scene.id, &["shot_phase2".into()])
        .await
        .unwrap();
    (
        ai_studio_lib::application::scene_production_service::SceneProductionService::new(
            structure,
            Arc::new(batch_service(f, inputs)),
        ),
        scene.id,
    )
}

#[tokio::test]
async fn scene_preparation_supports_all_four_modes_without_image_result_selection_or_generation() {
    use ai_studio_lib::application::shot_service::ShotGenerationRequest;
    use ai_studio_lib::domain::ShotStage;
    for index in 0..4 {
        let f = fixture().await;
        let (inputs, first, last) = setup(&f).await;
        let bindings = match index {
            0 => Vec::new(),
            1 => vec![input("first_frame", 0, &first.id)],
            2 => vec![
                input("first_frame", 0, &first.id),
                input("last_frame", 0, &last.id),
            ],
            _ => vec![input("reference_images", 0, &first.id)],
        };
        inputs
            .save(&scope(&f, index), None, &bindings)
            .await
            .unwrap();
        assert_eq!(count(&f.pool, "shot_generation_links").await, 0); // Binding is not production.
        configure(&f, index).await;
        let shots = shot_service(&f, inputs.clone());
        // Prompt belongs to the Shot (`prompt_text`), not the scalar bag.
        let mut scalars = values();
        scalars.remove("prompt");
        let request = ShotGenerationRequest {
            project_id: PROJECT_ID.into(),
            shot_id: "shot_phase2".into(),
            stage: ShotStage::Video,
            values: scalars,
            retry_task_id: None,
            submission_idempotency_key: None,
        };
        let advanced = shots
            .prepare_generation_submission(request.clone())
            .await
            .unwrap();
        let mut normal_request = request;
        // A stale client field must never replace the backend-saved exact slots.
        normal_request.values.insert(
            "first_frame".into(),
            GenerationInputValue::ImageAsset(last.id.clone()),
        );
        let normal = shots
            .prepare_creation_submission(
                normal_request,
                f.pairs[index].0.clone(),
                f.pairs[index].1.clone(),
            )
            .await
            .unwrap();
        for prepared in [&advanced, &normal] {
            assert_eq!(
                prepared.values.get("prompt"),
                Some(&GenerationInputValue::Text("move".into())),
                "mode {index} must submit the Shot prompt"
            );
            f.services
                .generation
                .validate_new_product_inputs(
                    PROJECT_ID,
                    &prepared.workflow_version_id,
                    &prepared.recipe_id,
                    &prepared.values,
                )
                .await
                .unwrap();
            for binding in &bindings {
                let id = AssetId::parse(&binding.asset_id).unwrap();
                let expected = if binding.input_key == "reference_images" {
                    GenerationInputValue::ImageAssets(vec![id])
                } else {
                    GenerationInputValue::ImageAsset(id)
                };
                assert_eq!(prepared.values.get(&binding.input_key), Some(&expected));
            }
            if index == 0 || index == 3 {
                assert!(!prepared.values.contains_key("first_frame"));
            }
        }
        assert_eq!(count(&f.pool, "tasks").await, 0);
        assert_eq!(count(&f.pool, "production_batches").await, 0);
        assert_eq!(count(&f.pool, "shot_generation_links").await, 0);
        let (scenes, id) = scene_service(&f, inputs).await;
        let plan = scenes
            .plan(PROJECT_ID, &id, ShotStage::Video)
            .await
            .unwrap();
        assert_eq!(plan.eligible, 1, "mode {index}: {plan:?}");
        let prepared = scenes
            .prepare(PROJECT_ID, &id, ShotStage::Video, false)
            .await
            .unwrap();
        assert!(prepared.created);
        assert_eq!(prepared.created_count, 1);
        let item = &prepared.detail.as_ref().unwrap().items[0];
        assert_eq!(item.workflow_version_id, f.pairs[index].0);
        assert_eq!(item.recipe_id, f.pairs[index].1);
        for binding in bindings {
            let expected = if binding.input_key == "reference_images" {
                json!([binding.asset_id])
            } else {
                json!(binding.asset_id)
            };
            let key = if binding.input_key == "reference_images" {
                "assetIds"
            } else {
                "assetId"
            };
            assert_eq!(item.values_json[&binding.input_key][key], expected);
        }
        assert!(
            !scenes
                .prepare(PROJECT_ID, &id, ShotStage::Video, false)
                .await
                .unwrap()
                .created
        );
        assert_eq!(count(&f.pool, "production_batches").await, 1);
        assert_eq!(count(&f.pool, "tasks").await, 0);
        // Explicit production preparation uses the existing Queue binding:
        // one real video item link, no Task and no fabricated image result.
        assert_eq!(count(&f.pool, "shot_generation_links").await, 1);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM shot_generation_links WHERE stage='video' AND task_id IS NULL AND production_batch_item_id=?")
            .bind(item.id.as_str()).fetch_one(&f.pool).await.unwrap(),1);
        assert!(SqliteShotRepository::new(f.pool.clone())
            .find(PROJECT_ID, "shot_phase2")
            .await
            .unwrap()
            .unwrap()
            .shot
            .selected_image_asset_id
            .is_none());
    }
}

#[tokio::test]
async fn scene_preparation_rechecks_tampered_media_after_a_read_only_plan() {
    let f = fixture().await;
    let (inputs, first, _) = setup(&f).await;
    inputs
        .save(&scope(&f, 1), None, &[input("first_frame", 0, &first.id)])
        .await
        .unwrap();
    configure(&f, 1).await;
    let (scenes, id) = scene_service(&f, inputs).await;
    assert_eq!(
        scenes
            .plan(PROJECT_ID, &id, ai_studio_lib::domain::ShotStage::Video)
            .await
            .unwrap()
            .eligible,
        1
    );
    fs::write(&first.storage_path, b"changed after plan").unwrap();
    assert!(scenes
        .prepare(
            PROJECT_ID,
            &id,
            ai_studio_lib::domain::ShotStage::Video,
            false
        )
        .await
        .is_err());
    assert_eq!(count(&f.pool, "production_batches").await, 0);
    assert_eq!(count(&f.pool, "tasks").await, 0);
}

#[tokio::test]
async fn first_last_maps_independent_keys_and_recipe_switching_never_copies_inputs() {
    let f = fixture().await;
    let (service, first, last) = setup(&f).await;
    let fl = scope(&f, 2);
    let i2v = scope(&f, 1);
    let saved = service
        .save(
            &fl,
            None,
            &[
                input("last_frame", 0, &last.id),
                input("first_frame", 0, &first.id),
            ],
        )
        .await
        .unwrap();
    service
        .save(&i2v, None, &[input("first_frame", 0, &last.id)])
        .await
        .unwrap();
    assert_eq!(service.get(&fl).await.unwrap().unwrap(), saved);
    assert!(service.get(&scope(&f, 3)).await.unwrap().is_none());
    assert!(service
        .save(
            &i2v,
            Some(&service.get(&i2v).await.unwrap().unwrap().token),
            &[input("last_frame", 0, &last.id)]
        )
        .await
        .is_err());
    configure(&f, 2).await;
    let batch = batch_service(&f, service.clone())
        .create(CreateShotBatchRequest {
            project_id: PROJECT_ID.into(),
            stage: ai_studio_lib::domain::ShotStage::Video,
            shot_ids: vec!["shot_phase2".into()],
        })
        .await
        .unwrap();
    assert_eq!(
        batch.items[0].values_json["first_frame"]["assetId"],
        first.id.as_str()
    );
    assert_eq!(
        batch.items[0].values_json["last_frame"]["assetId"],
        last.id.as_str()
    );
    service
        .save(
            &fl,
            Some(&saved.token),
            &[
                input("first_frame", 0, &last.id),
                input("last_frame", 0, &first.id),
            ],
        )
        .await
        .unwrap();
    let frozen = f
        .services
        .queue
        .get(PROJECT_ID, batch.batch.id.as_str())
        .await
        .unwrap();
    assert_eq!(frozen.items[0].values_json, batch.items[0].values_json);
    assert!(service.save(&fl, Some(&saved.token), &[]).await.is_err());
    assert_eq!(count(&f.pool, "tasks").await, 0);
}

#[tokio::test]
async fn historical_image_and_tampered_external_bytes_fail_before_new_generation_writes() {
    let f = fixture().await;
    let (service, first, _) = setup(&f).await;
    let assets = SqliteAssetRepository::new(f.pool.clone());
    // Seed an owned historical source task; the production import/binding paths
    // must create no additional Task or GenerationLink. Respect Asset invariants.
    let historical_task = ai_studio_lib::domain::Task::new(
        PROJECT_ID,
        WORKFLOW_ID,
        WORKFLOW_VERSION_ID,
        RECIPE_ID,
        Utc::now(),
    );
    SqliteTaskRepository::new(f.pool.clone())
        .create(&historical_task, &historical_task.created_event())
        .await
        .unwrap();
    let mut legacy = first.clone();
    legacy.id = AssetId::new();
    legacy.category = "generated_image".into();
    legacy.source_task_id = Some(historical_task.id.clone());
    assets.insert_many(&[legacy.clone()]).await.unwrap();
    let mut unattested = first.clone();
    unattested.id = AssetId::new();
    assets.insert_many(&[unattested.clone()]).await.unwrap();
    let baseline = (
        count(&f.pool, "tasks").await,
        count(&f.pool, "production_batches").await,
        count(&f.pool, "shot_generation_links").await,
    );
    assert!(service
        .save(&scope(&f, 1), None, &[input("first_frame", 0, &legacy.id)])
        .await
        .is_err());
    assert!(service
        .save(
            &scope(&f, 1),
            None,
            &[input("first_frame", 0, &unattested.id)]
        )
        .await
        .is_err());
    for id in [&legacy.id, &unattested.id, &first.id] {
        if id == &first.id {
            fs::write(&first.storage_path, b"tampered").unwrap();
        }
        let mut request = batch_request(&f.pairs[1].0, &f.pairs[1].1);
        request.items[0].values.insert(
            "first_frame".into(),
            GenerationInputValue::ImageAsset(id.clone()),
        );
        assert!(f.services.queue.create(request).await.is_err());
        let mut request = generation_request(&f.pairs[1].0, &f.pairs[1].1);
        request.values.insert(
            "first_frame".into(),
            GenerationInputValue::ImageAsset(id.clone()),
        );
        assert!(f
            .services
            .generation
            .start_generation(request)
            .await
            .is_err());
    }
    assert_eq!(
        baseline,
        (
            count(&f.pool, "tasks").await,
            count(&f.pool, "production_batches").await,
            count(&f.pool, "shot_generation_links").await
        )
    );
    assert!(service.get(&scope(&f, 1)).await.unwrap().is_none());
}

#[tokio::test]
async fn mixed_reference_contract_accepts_video_only_and_rejects_audio_only_before_batch_write() {
    let f = fixture().await;
    let (service, first, _) = setup(&f).await;
    // Typed managed-media fixtures test mapping/integrity, not real codec import.
    let root = SqliteProjectRepository::new(f.pool.clone())
        .get_storage_root(PROJECT_ID)
        .await
        .unwrap()
        .unwrap();
    let mut refs = Vec::new();
    for (name, kind) in [
        ("video", ai_studio_lib::domain::AssetType::Video),
        ("audio", ai_studio_lib::domain::AssetType::Audio),
    ] {
        let mut asset = first.clone();
        asset.id = AssetId::new();
        asset.asset_type = kind;
        asset.category = format!("source_{name}");
        asset.storage_path = root.join(format!("{name}.fixture")).display().to_string();
        let bytes = format!("owned-{name}");
        fs::write(&asset.storage_path, &bytes).unwrap();
        asset.sha256 = format!("{:x}", Sha256::digest(bytes.as_bytes()));
        asset.file_size = bytes.len() as u64;
        asset.mime_type = if name == "video" {
            "video/mp4"
        } else {
            "audio/wav"
        }
        .into();
        asset.duration_ms = Some(2000);
        SqliteAssetRepository::new(f.pool.clone())
            .insert_external_source(&asset)
            .await
            .unwrap();
        refs.push(asset);
    }
    let ref_scope = scope(&f, 3);
    configure(&f, 3).await;
    let batches = batch_service(&f, service.clone());
    let video_only = service
        .save(
            &ref_scope,
            None,
            &[input("reference_videos", 0, &refs[0].id)],
        )
        .await
        .unwrap();
    let request = || CreateShotBatchRequest {
        project_id: PROJECT_ID.into(),
        stage: ai_studio_lib::domain::ShotStage::Video,
        shot_ids: vec!["shot_phase2".into()],
    };
    let batch = batches.create(request()).await.unwrap();
    assert!(batch.items[0].values_json.get("reference_images").is_none());
    assert_eq!(
        batch.items[0].values_json["reference_videos"]["assetIds"],
        json!([refs[0].id.as_str()])
    );
    // Owned historical generated video: legal reference media without an import
    // receipt; the binding/preparation operation must not create another task.
    let historical_task = ai_studio_lib::domain::Task::new(
        PROJECT_ID,
        WORKFLOW_ID,
        WORKFLOW_VERSION_ID,
        RECIPE_ID,
        Utc::now(),
    );
    SqliteTaskRepository::new(f.pool.clone())
        .create(&historical_task, &historical_task.created_event())
        .await
        .unwrap();
    let mut generated_video = refs[0].clone();
    generated_video.id = AssetId::new();
    generated_video.category = "generated_video".into();
    generated_video.source_task_id = Some(historical_task.id.clone());
    SqliteAssetRepository::new(f.pool.clone())
        .insert_many(&[generated_video.clone()])
        .await
        .unwrap();
    let mixed = service
        .save(
            &ref_scope,
            Some(&video_only.token),
            &[
                input("reference_images", 0, &first.id),
                input("reference_videos", 0, &generated_video.id),
                input("reference_audios", 0, &refs[1].id),
            ],
        )
        .await
        .unwrap();
    let batch = batches.create(request()).await.unwrap();
    assert_eq!(
        batch.items[0].values_json["reference_videos"]["assetIds"],
        json!([generated_video.id.as_str()])
    );
    assert_eq!(
        batch.items[0].values_json["reference_audios"]["assetIds"],
        json!([refs[1].id.as_str()])
    );
    let before = count(&f.pool, "production_batches").await;
    let audio_only = service
        .save(
            &ref_scope,
            Some(&mixed.token),
            &[input("reference_audios", 0, &refs[1].id)],
        )
        .await;
    assert!(
        matches!(
            audio_only,
            Err(ai_studio_lib::application::shot_video_input_service::ShotVideoInputError::Combination)
        ),
        "audio-only REF2VA is rejected when the binding is saved, before any batch write"
    );
    assert_eq!(count(&f.pool, "production_batches").await, before);
    assert_eq!(count(&f.pool, "tasks").await, 1); // Only the explicitly seeded historical task.
}

#[tokio::test]
async fn phase5c_four_mode_selection_review_occ_and_cold_sqlite_persistence_are_independent() {
    use ai_studio_lib::application::artifact_service::ArtifactService;
    use ai_studio_lib::application::ports::ArtifactRepository;
    use ai_studio_lib::domain::{ArtifactReviewDecision as Decision, AssetType, ShotStage};
    use ai_studio_lib::infrastructure::database::SqliteArtifactRepository;
    let f = fixture_with_capability(true).await;
    let (inputs, first, last) = setup(&f).await;
    let video = managed_media(&f, &first, AssetType::Video).await;
    let audio = managed_media(&f, &first, AssetType::Audio).await;
    let ids = ["c_t2v", "c_i2v", "c_fl", "c_ref"];
    for (i, id) in ids.iter().enumerate() {
        formal_video_shot(&f, id, i).await;
    }
    for (i, bindings) in [
        (1, vec![input("first_frame", 0, &first.id)]),
        (
            2,
            vec![
                input("first_frame", 0, &first.id),
                input("last_frame", 0, &last.id),
            ],
        ),
        (
            3,
            vec![
                input("reference_images", 0, &first.id),
                input("reference_videos", 0, &video.id),
                input("reference_audios", 0, &audio.id),
            ],
        ),
    ] {
        inputs
            .save(&formal_scope(&f, ids[i], i), None, &bindings)
            .await
            .unwrap();
    }
    let (prepare, start) = formal_preparation(&f, inputs.clone()).await;
    let order: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
    let admitted = prepare
        .admit(PROJECT_ID, &order, ShotStage::Video, false)
        .await
        .unwrap();
    let batch_id = &admitted.created_batch_ids[0];
    assert_eq!(count(&f.pool, "tasks").await, 0);
    start.start(PROJECT_ID, batch_id).await.unwrap();
    wait_for_batch_status(&f.pool, batch_id, ProductionBatchStatus::Completed).await;
    let executed = f.services.queue.get(PROJECT_ID, batch_id).await.unwrap();
    assert_eq!(count(&f.pool, "tasks").await, 4);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 4);
    let reviews = Arc::new(SqliteArtifactRepository::new(f.pool.clone()));
    let review_service = ArtifactService::new(
        reviews.clone(),
        Arc::new(SqliteProjectRepository::new(f.pool.clone())),
        Arc::new(SqliteTaskRepository::new(f.pool.clone())),
        f.services.queue.clone(),
        Arc::new(SystemClock),
    );
    let shots = shot_service(&f, inputs.clone());
    let mut outputs = Vec::new();
    for (i, item) in executed.items.iter().enumerate() {
        assert_eq!(&item.workflow_version_id, &f.pairs[i].0);
        assert_eq!(&item.recipe_id, &f.pairs[i].1);
        let task = item.task_id.as_ref().unwrap();
        let artifacts = review_service
            .task_artifacts(PROJECT_ID, &[task.clone()])
            .await
            .unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].review_status.as_deref(), Some("PENDING"));
        let asset_id = artifacts[0].id.clone();
        let before = shots.get(PROJECT_ID, ids[i]).await.unwrap();
        assert!(before.selected_video_asset_id.is_none());
        // Reading/previewing a registered result cannot select or approve it.
        review_service
            .task_artifacts(PROJECT_ID, &[task.clone()])
            .await
            .unwrap();
        assert!(shots
            .get(PROJECT_ID, ids[i])
            .await
            .unwrap()
            .selected_video_asset_id
            .is_none());
        if i != 3 {
            let chosen = shots
                .select_result(PROJECT_ID, ids[i], ShotStage::Video, &asset_id, true)
                .await
                .unwrap();
            assert_eq!(
                chosen.selected_video_asset_id.as_deref(),
                Some(asset_id.as_str())
            );
            assert_eq!(
                reviews
                    .find_review(PROJECT_ID, &AssetId::parse(asset_id.clone()).unwrap())
                    .await
                    .unwrap()
                    .unwrap()
                    .decision,
                Decision::Pending
            );
        }
        outputs.push(asset_id);
    }
    let approved = review_service
        .submit_review(
            PROJECT_ID,
            &outputs[0],
            Decision::Approved,
            "human approved".into(),
            0,
        )
        .await
        .unwrap();
    assert_eq!(approved.revision, 1);
    let rejected = review_service
        .submit_review(
            PROJECT_ID,
            &outputs[1],
            Decision::Rejected,
            "human rejection: frame drift".into(),
            0,
        )
        .await
        .unwrap();
    assert_eq!(rejected.revision, 1);
    assert!(review_service
        .submit_review(PROJECT_ID, &outputs[2], Decision::Rejected, "   ".into(), 0)
        .await
        .is_err());
    assert!(review_service
        .submit_review(
            PROJECT_ID,
            &outputs[0],
            Decision::Rejected,
            "conflicting reviewer".into(),
            0
        )
        .await
        .is_err());
    assert_eq!(
        review_service
            .submit_review(
                PROJECT_ID,
                &outputs[0],
                Decision::Approved,
                "human approved".into(),
                0
            )
            .await
            .unwrap()
            .revision,
        1
    );
    review_service
        .submit_review(
            PROJECT_ID,
            &outputs[3],
            Decision::Approved,
            "approve without selecting".into(),
            0,
        )
        .await
        .unwrap();
    assert!(shots
        .get(PROJECT_ID, ids[3])
        .await
        .unwrap()
        .selected_video_asset_id
        .is_none());
    assert_eq!(
        shots
            .get(PROJECT_ID, ids[1])
            .await
            .unwrap()
            .selected_video_asset_id
            .as_deref(),
        Some(outputs[1].as_str())
    );
    assert!(shots
        .select_result("foreign", ids[0], ShotStage::Video, &outputs[0], true)
        .await
        .is_err());
    assert!(shots
        .select_result(PROJECT_ID, ids[0], ShotStage::Video, &outputs[1], true)
        .await
        .is_err());
    assert!(shots
        .select_result(
            PROJECT_ID,
            ids[0],
            ShotStage::Video,
            first.id.as_str(),
            false
        )
        .await
        .is_err());
    assert!(shots
        .select_result(PROJECT_ID, ids[0], ShotStage::Video, "missing_asset", true)
        .await
        .is_err());
    assert!(review_service
        .submit_review(
            "foreign",
            &outputs[0],
            Decision::Approved,
            "foreign".into(),
            0
        )
        .await
        .is_err());
    assert!(review_service
        .submit_review(
            PROJECT_ID,
            "missing_asset",
            Decision::Approved,
            "missing".into(),
            0
        )
        .await
        .is_err());
    assert_eq!(count(&f.pool, "tasks").await, 4);
    assert_eq!(count(&f.pool, "production_batches").await, 1);
    assert_eq!(count(&f.pool, "shot_generation_links").await, 4);
    assert_eq!(count(&f.pool, "task_output_assets").await, 4);
    assert_eq!(f.comfy.max_concurrent.load(Ordering::SeqCst), 1);
    // Cold reopen the actual owned SQLite file, not an in-memory fake.
    f.pool.close().await;
    let cold = initialize(&f._owned.path().join("owned.sqlite"))
        .await
        .unwrap();
    let cold_shots = SqliteShotRepository::new(cold.clone());
    let cold_reviews = SqliteArtifactRepository::new(cold.clone());
    for (i, id) in ids.iter().enumerate() {
        let data = cold_shots.find(PROJECT_ID, id).await.unwrap().unwrap();
        assert_eq!(data.generation_links.len(), 1);
        assert_eq!(data.generation_links[0].task_id, executed.items[i].task_id);
        assert_eq!(
            data.generation_links[0].production_batch_item_id.as_deref(),
            Some(executed.items[i].id.as_str())
        );
        assert_eq!(
            data.shot.selected_video_asset_id.as_deref(),
            if i == 3 {
                None
            } else {
                Some(outputs[i].as_str())
            }
        );
        let r = cold_reviews
            .find_review(PROJECT_ID, &AssetId::parse(outputs[i].clone()).unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            r.decision,
            [
                Decision::Approved,
                Decision::Rejected,
                Decision::Pending,
                Decision::Approved
            ][i]
        );
        assert_eq!(r.revision, if i == 2 { 0 } else { 1 });
        if i == 1 {
            assert_eq!(r.comment, "human rejection: frame drift");
        }
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&cold)
            .await
            .unwrap(),
        migration_head::latest_migration_version()
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&cold)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(count(&cold, "tasks").await, 4);
    assert_eq!(count(&cold, "production_batches").await, 1);
}
