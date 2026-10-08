//! Phase2 media authority regressions; reuses owned real Registry/SQLite fixture.
use super::*;
use ai_studio_lib::application::{
    ports::{
        ShotRecord, ShotRepository, ShotStageConfigRecord, ShotVideoInputAsset, ShotVideoInputScope,
    },
    shot_video_input_service::ShotVideoInputService,
};
use ai_studio_lib::domain::{Asset, AssetId};
use ai_studio_lib::infrastructure::database::repositories::SqliteShotVideoInputRepository;

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
        let request = ShotGenerationRequest {
            project_id: PROJECT_ID.into(),
            shot_id: "shot_phase2".into(),
            stage: ShotStage::Video,
            values: values(),
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
