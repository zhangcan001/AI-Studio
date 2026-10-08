//! Real Registry/SQLite/package bytes and production admission seams. No live ComfyUI.
use super::*;
use ai_studio_lib::application::{
    builtin_runtime_packages,
    generation_catalog_service::GenerationCatalogService,
    generation_input_preparer::{GenerationInputPreparer, GenerationInputValue},
    generation_service::{CreateGenerationRequest, NewGenerationAdmission},
    minimax_video_product_policy::AUTHORIZED_PACKAGES,
    ports::{
        WorkflowLibraryRepository, WorkflowPackageRecord, WorkflowRuntimeRepository,
        WorkflowRuntimeStateRepository,
    },
    production_queue_service::{CreateProductionBatchItem, CreateProductionBatchRequest},
    shot_batch_service::{CreateShotBatchRequest, ShotBatchService},
    workflow_benchmark_service::{
        WorkflowBenchmarkCandidateRequest, WorkflowBenchmarkCreateRequest, WorkflowBenchmarkService,
    },
    workflow_manifest::WorkflowManifest,
    workflow_registry_service::WorkflowRegistryService,
};
use ai_studio_lib::compiler::{RecipeParser, WorkflowCompiler};
use ai_studio_lib::domain::{CompileRequest, InputDefinition, SeedValue, WorkflowDocument};
use ai_studio_lib::infrastructure::{
    database::{
        SqlitePresetRepository, SqliteProjectWorkflowBindingRepository, SqliteShotRepository,
        SqliteWorkflowBenchmarkRepository, SqliteWorkflowLibraryRepository,
        SqliteWorkflowRecipeRuntimeStateRepository, SqliteWorkflowRegistryRepository,
        SqliteWorkflowRuntimeArtifactRepository, SqliteWorkflowRuntimeRepository,
        SqliteWorkflowRuntimeStateRepository,
    },
    filesystem::FileSystemWorkflowPackageStore,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

struct Fixture {
    _owned: tempfile::TempDir,
    root: PathBuf,
    pool: SqlitePool,
    registry: Arc<WorkflowRegistryService>,
    comfy: Arc<ControlledComfy>,
    services: Services,
    pairs: Vec<(String, String)>,
}

async fn fixture() -> Fixture {
    let owned = tempdir().unwrap();
    let root = owned.path().join("library");
    let pool = initialize(&owned.path().join("owned.sqlite"))
        .await
        .unwrap();
    let project = owned.path().join("project");
    fs::create_dir_all(&project).unwrap();
    seed_database(&pool, &project).await;
    builtin_runtime_packages::ensure_installed(&root).unwrap();
    assert!(!root.join("kera2_t2i_local_v2_1_1_1_90894e9e").exists());
    // An existing Krea2 package is historical user data, not a new default.
    let krea = "kera2_t2i_local_v2_1_1_1_90894e9e";
    let old = "minimax_h3_fl2va_t2v_quality_2_1_0";
    for name in [krea, old] {
        fs::create_dir_all(root.join(name)).unwrap();
        for file in ["manifest.yaml", "recipe.yaml", "workflow_api.json"] {
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("runtime_packages")
                    .join(name)
                    .join(file),
                root.join(name).join(file),
            )
            .unwrap();
        }
    }
    let library = SqliteWorkflowLibraryRepository::new(pool.clone());
    for name in AUTHORIZED_PACKAGES.into_iter().chain([krea, old]) {
        let manifest = WorkflowManifest::parse(
            &fs::read_to_string(root.join(name).join("manifest.yaml")).unwrap(),
        )
        .unwrap();
        let recipe = fs::read_to_string(root.join(name).join("recipe.yaml")).unwrap();
        let graph = fs::read_to_string(root.join(name).join("workflow_api.json")).unwrap();
        library
            .register_package(&WorkflowPackageRecord {
                workflow_id: manifest.id,
                source_kind: "PRODUCT".into(),
                package_name: name.into(),
                package_source_path: Some(root.join(name).to_string_lossy().into()),
                name: manifest.name,
                category: manifest.category,
                mode: manifest.mode,
                workflow_version: manifest.workflow_version,
                workflow_json: serde_json::from_str(&graph).unwrap(),
                workflow_sha256: format!("{:x}", Sha256::digest(graph.as_bytes())),
                source_workflow_json: None,
                recognition_metadata_json: None,
                recipe_version: manifest.recipe_version,
                recipe_schema_version: 1,
                recipe_sha256: format!("{:x}", Sha256::digest(recipe.as_bytes())),
                recipe_yaml: recipe,
                created_at: Utc::now(),
            })
            .await
            .unwrap();
    }
    let runtime = Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let versions = runtime.list_versions().await.unwrap();
    let pairs = AUTHORIZED_PACKAGES
        .iter()
        .map(|name| {
            let v = versions
                .iter()
                .find(|v| v.package_name.as_deref() == Some(name))
                .unwrap();
            (
                v.workflow_version_id.clone(),
                v.recipes[0].recipe_id.clone(),
            )
        })
        .collect();
    let registry = Arc::new(
        WorkflowRegistryService::new(
            runtime,
            Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone())),
            Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone())),
            Arc::new(SystemClock),
        )
        .with_registry_repository(Arc::new(SqliteWorkflowRegistryRepository::new(
            pool.clone(),
        )))
        .with_recipe_runtime_state_repository(Arc::new(
            SqliteWorkflowRecipeRuntimeStateRepository::new(pool.clone()),
        ))
        .with_runtime_artifact_repository(Arc::new(SqliteWorkflowRuntimeArtifactRepository::new(
            pool.clone(),
        )))
        .with_package_store(Arc::new(FileSystemWorkflowPackageStore::new(
            root.clone(),
            owned.path().join("staging"),
        ))),
    );
    let comfy = Arc::new(ControlledComfy::new(ComfyBehavior::Success));
    let services =
        build_services_with_admission(&pool, comfy.clone(), &root, None, Some(registry.clone()));
    Fixture {
        _owned: owned,
        root,
        pool,
        registry,
        comfy,
        services,
        pairs,
    }
}

fn values() -> BTreeMap<String, GenerationInputValue> {
    BTreeMap::from([
        (
            "prompt".into(),
            GenerationInputValue::Text("An owned video fixture".into()),
        ),
        ("duration_seconds".into(), GenerationInputValue::Integer(5)),
        ("width".into(), GenerationInputValue::Integer(1344)),
        ("height".into(), GenerationInputValue::Integer(768)),
        (
            "seed".into(),
            GenerationInputValue::Seed(SeedValue::Fixed(42)),
        ),
    ])
}
fn generation_request(version: &str, recipe: &str) -> CreateGenerationRequest {
    CreateGenerationRequest {
        project_id: PROJECT_ID.into(),
        workflow_version_id: version.into(),
        recipe_id: recipe.into(),
        model_version_id: None,
        prompt_version_id: None,
        tool_instance_id: None,
        tool_version_id: None,
        values: values(),
        reference_manifest: None,
        submission_idempotency_key: None,
        submission_attempt: None,
        parent_task_id: None,
    }
}
fn batch_request(version: &str, recipe: &str) -> CreateProductionBatchRequest {
    CreateProductionBatchRequest {
        project_id: PROJECT_ID.into(),
        name: "owned policy fixture".into(),
        continue_on_failure: false,
        items: vec![CreateProductionBatchItem {
            workflow_version_id: version.into(),
            recipe_id: recipe.into(),
            values: values(),
        }],
    }
}
async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn all_four_exact_products_admit_and_compile_without_changing_recipe_bytes() {
    let f = fixture().await;
    let source = SourceAssetImportService::new(
        Arc::new(SqliteProjectRepository::new(f.pool.clone())),
        Arc::new(FileSystemAssetStore::new()),
        Arc::new(SqliteAssetRepository::new(f.pool.clone())),
        Arc::new(SystemClock),
    );
    let image = source
        .import_bytes(PROJECT_ID, "owned.png", &png_bytes([70, 80, 90, 255]))
        .await
        .unwrap();
    assert!(image.source_task_id.is_none());
    for (name, (version, recipe_id)) in AUTHORIZED_PACKAGES.iter().zip(&f.pairs) {
        assert!(
            f.registry
                .is_available_for_new_generation(version, recipe_id)
                .await
                .unwrap(),
            "{name}"
        );
        let definition = SqliteGenerationDefinitionRepository::new(f.pool.clone())
            .find(version, recipe_id)
            .await
            .unwrap()
            .unwrap();
        let recipe = RecipeParser::parse(&definition.recipe_yaml).unwrap();
        let mut request = batch_request(version, recipe_id);
        for (key, input) in &recipe.inputs {
            match input {
                InputDefinition::Image { .. } => {
                    request.items[0].values.insert(
                        key.clone(),
                        GenerationInputValue::ImageAsset(image.id.clone()),
                    );
                }
                InputDefinition::Images { .. } => {
                    request.items[0].values.insert(
                        key.clone(),
                        GenerationInputValue::ImageAssets(vec![image.id.clone()]),
                    );
                }
                _ => {}
            }
        }
        let compile = CompileRequest::new(GenerationInputPreparer::preflight_values(
            &request.items[0].values,
        ));
        ai_studio_lib::application::product::h3_resolution::compile_checked(
            &WorkflowCompiler,
            &WorkflowDocument::parse(definition.workflow_json).unwrap(),
            &recipe,
            &compile,
        )
        .unwrap();
        let batch = f.services.queue.create(request).await.unwrap();
        assert_eq!(batch.items[0].workflow_version_id, *version);
        assert_eq!(batch.items[0].recipe_id, *recipe_id);
    }
    let catalog = GenerationCatalogService::new(Arc::new(
        SqliteGenerationDefinitionRepository::new(f.pool.clone()),
    ))
    .with_new_generation_admission(f.registry.clone());
    assert_eq!(catalog.list().await.unwrap().len(), 4);
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn krea_other_images_and_fake_minimax_advanced_requests_create_no_task_or_batch() {
    let f = fixture().await;
    let versions = SqliteWorkflowRuntimeRepository::new(f.pool.clone())
        .list_versions()
        .await
        .unwrap();
    let krea = versions
        .iter()
        .find(|v| v.workflow_id == "wfl_kera2_t2i_local_v2")
        .unwrap();
    sqlx::query("UPDATE workflows SET name='MiniMax H3 video' WHERE id=?")
        .bind(WORKFLOW_ID)
        .execute(&f.pool)
        .await
        .unwrap();
    for (version, recipe) in [
        (WORKFLOW_VERSION_ID, RECIPE_ID),
        (
            krea.workflow_version_id.as_str(),
            krea.recipes[0].recipe_id.as_str(),
        ),
        (f.pairs[0].0.as_str(), f.pairs[1].1.as_str()),
    ] {
        assert!(!f
            .registry
            .is_available_for_new_generation(version, recipe)
            .await
            .unwrap());
        assert!(f
            .services
            .queue
            .create(batch_request(version, recipe))
            .await
            .is_err());
        assert!(f
            .services
            .generation
            .start_generation(generation_request(version, recipe))
            .await
            .is_err());
    }
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(count(&f.pool, "production_batches").await, 0);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 0);
    assert!(SqliteGenerationDefinitionRepository::new(f.pool.clone())
        .find(&krea.workflow_version_id, &krea.recipes[0].recipe_id)
        .await
        .unwrap()
        .is_some());
    assert!(SqliteProjectRepository::new(f.pool.clone())
        .find_by_id(PROJECT_ID)
        .await
        .unwrap()
        .is_some());
    // A separate custom image recipe, even with a MiniMax display name, is denied.
    sqlx::query("UPDATE recipes SET recipe_yaml=replace(recipe_yaml,'type: video','type: image') WHERE id=?")
        .bind(RECIPE_ID).execute(&f.pool).await.unwrap();
    assert!(f
        .services
        .generation
        .start_generation(generation_request(WORKFLOW_VERSION_ID, RECIPE_ID))
        .await
        .is_err());
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn package_name_source_version_snapshot_lifecycle_and_all_live_bytes_are_verified() {
    let f = fixture().await;
    let (version, recipe) = &f.pairs[0];
    let admitted = || f.registry.is_available_for_new_generation(version, recipe);
    // Even byte-identical copies under a user-chosen package name are not products.
    let alias = "my_minimax_h3_fl2va_t2v_quality_2_2_0";
    fs::create_dir_all(f.root.join(alias)).unwrap();
    for file in ["manifest.yaml", "recipe.yaml", "workflow_api.json"] {
        fs::copy(
            f.root.join(AUTHORIZED_PACKAGES[0]).join(file),
            f.root.join(alias).join(file),
        )
        .unwrap();
    }
    sqlx::query("UPDATE workflow_runtime_artifacts SET package_name=? WHERE workflow_version_id=?")
        .bind(alias)
        .bind(version)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        f.registry.is_available(version, recipe).await.unwrap(),
        "generic live integrity remains valid"
    );
    assert!(
        !admitted().await.unwrap(),
        "custom package name is not product authorization"
    );
    sqlx::query("UPDATE workflow_runtime_artifacts SET package_name=? WHERE workflow_version_id=?")
        .bind(AUTHORIZED_PACKAGES[0])
        .bind(version)
        .execute(&f.pool)
        .await
        .unwrap();
    // An authentic older package has the same Workflow ID but a different exact pair/version.
    let old = SqliteWorkflowRuntimeRepository::new(f.pool.clone())
        .list_versions()
        .await
        .unwrap()
        .into_iter()
        .find(|v| v.package_name.as_deref() == Some("minimax_h3_fl2va_t2v_quality_2_1_0"))
        .unwrap();
    let current = SqliteWorkflowRuntimeRepository::new(f.pool.clone())
        .find_version(version)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old.workflow_id, current.workflow_id);
    assert!(f
        .registry
        .is_available(&old.workflow_version_id, &old.recipes[0].recipe_id)
        .await
        .unwrap());
    assert!(!f
        .registry
        .is_available_for_new_generation(&old.workflow_version_id, &old.recipes[0].recipe_id)
        .await
        .unwrap());
    for file in ["manifest.yaml", "recipe.yaml", "workflow_api.json"] {
        let path = f.root.join(AUTHORIZED_PACKAGES[0]).join(file);
        let original = fs::read(&path).unwrap();
        let mut tampered = original.clone();
        tampered.extend_from_slice(b"\n");
        fs::write(&path, &tampered).unwrap();
        assert!(!admitted().await.unwrap(), "tampered {file}");
        fs::write(path, original).unwrap();
        assert!(admitted().await.unwrap());
    }
    for (sql,restore) in [
        ("UPDATE workflow_versions SET version='2.9.9' WHERE id=?", "UPDATE workflow_versions SET version='2.2.0' WHERE id=?"),
        ("UPDATE workflow_runtime_artifacts SET source_kind='USER' WHERE workflow_version_id=?", "UPDATE workflow_runtime_artifacts SET source_kind='PRODUCT' WHERE workflow_version_id=?"),
        ("UPDATE workflows SET source_kind='USER' WHERE current_version_id=?", "UPDATE workflows SET source_kind='PRODUCT' WHERE current_version_id=?"),
    ] { sqlx::query(sql).bind(version).execute(&f.pool).await.unwrap(); assert!(!admitted().await.unwrap()); sqlx::query(restore).bind(version).execute(&f.pool).await.unwrap(); assert!(admitted().await.unwrap()); }
    let original: String =
        sqlx::query_scalar("SELECT api_workflow_json FROM workflow_versions WHERE id=?")
            .bind(version)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    sqlx::query("UPDATE workflow_versions SET api_workflow_json='{}' WHERE id=?")
        .bind(version)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(!admitted().await.unwrap());
    sqlx::query("UPDATE workflow_versions SET api_workflow_json=? WHERE id=?")
        .bind(original)
        .bind(version)
        .execute(&f.pool)
        .await
        .unwrap();
    let states = SqliteWorkflowRuntimeStateRepository::new(f.pool.clone());
    states
        .set_enabled(version, false, Utc::now())
        .await
        .unwrap();
    assert!(!admitted().await.unwrap());
    states.set_enabled(version, true, Utc::now()).await.unwrap();
    assert!(admitted().await.unwrap());
}

#[tokio::test]
async fn retired_pending_retry_partial_resume_and_restored_reexecution_are_denied_without_rewriting_items(
) {
    let f = fixture().await;
    let historical = build_services(&f.pool, f.comfy.clone(), &f.root);
    let old = historical
        .queue
        .create(batch_request(WORKFLOW_VERSION_ID, RECIPE_ID))
        .await
        .unwrap();
    let id = old.batch.id.as_str();
    let item = old.items[0].id.as_str();
    assert!(f
        .services
        .queue
        .inspect_start_admitted(PROJECT_ID, id)
        .await
        .is_err());
    assert!(
        f.services.queue.commit_start_admitted(&old).await.is_err(),
        "a historical detail is not a start authorization token"
    );
    assert!(f
        .services
        .queue
        .start_for_test(PROJECT_ID, id)
        .await
        .is_err());
    assert_eq!(
        f.services.queue.get(PROJECT_ID, id).await.unwrap().items,
        old.items
    );
    f.services.queue.archive(PROJECT_ID, id).await.unwrap();
    f.services.queue.restore(PROJECT_ID, id).await.unwrap();
    assert!(f
        .services
        .queue
        .start_for_test(PROJECT_ID, id)
        .await
        .is_err());
    // Startup recovery must not bypass manual start admission for historical pending work.
    sqlx::query("UPDATE production_batches SET status='RUNNING' WHERE id=?")
        .bind(id)
        .execute(&f.pool)
        .await
        .unwrap();
    f.services.queue.recover_and_resume().await.unwrap();
    wait_for_batch_status(&f.pool, id, ProductionBatchStatus::Paused).await;
    assert_eq!(
        f.services.queue.get(PROJECT_ID, id).await.unwrap().items,
        old.items
    );
    sqlx::query("UPDATE production_batch_items SET status='FAILED',error_code='COMFYUI_OFFLINE',error_message='owned old failure' WHERE id=?").bind(item).execute(&f.pool).await.unwrap();
    let before = f.services.queue.get(PROJECT_ID, id).await.unwrap();
    assert!(f
        .services
        .queue
        .retry_item(PROJECT_ID, id, item)
        .await
        .is_err());
    assert!(f
        .services
        .queue
        .requeue_item(PROJECT_ID, id, item)
        .await
        .is_err());
    assert!(f
        .services
        .queue
        .requeue_item_by_item(PROJECT_ID, item)
        .await
        .is_err());
    assert!(f
        .services
        .queue
        .partial_resume(PROJECT_ID, id, &[item.into()])
        .await
        .is_err());
    assert_eq!(
        f.services.queue.get(PROJECT_ID, id).await.unwrap().items,
        before.items
    );
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(count(&f.pool, "production_batch_items").await, 1);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn shot_and_preparation_direct_writes_benchmark_and_package_import_fail_before_side_effects()
{
    use ai_studio_lib::application::ports::{ShotRecord, ShotRepository, ShotStageConfigRecord};
    use ai_studio_lib::domain::{ProductionBatchId, ProductionBatchItemId, ShotStage};
    let f = fixture().await;
    let historical = build_services(&f.pool, f.comfy.clone(), &f.root);
    let old = historical
        .queue
        .create(batch_request(WORKFLOW_VERSION_ID, RECIPE_ID))
        .await
        .unwrap();
    let shots = Arc::new(SqliteShotRepository::new(f.pool.clone()));
    let source = SourceAssetImportService::new(
        Arc::new(SqliteProjectRepository::new(f.pool.clone())),
        Arc::new(FileSystemAssetStore::new()),
        Arc::new(SqliteAssetRepository::new(f.pool.clone())),
        Arc::new(SystemClock),
    );
    let imported = source
        .import_bytes(PROJECT_ID, "owned-source.png", &png_bytes([1, 2, 3, 255]))
        .await
        .unwrap();
    shots
        .insert(&ShotRecord {
            id: "sht_policy".into(),
            project_id: PROJECT_ID.into(),
            ordinal: 0,
            name: "Policy".into(),
            prompt_text: "Owned".into(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: Some(imported.id.as_str().into()),
            selected_video_asset_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .await
        .unwrap();
    shots
        .upsert_stage_config(
            PROJECT_ID,
            &ShotStageConfigRecord {
                shot_id: "sht_policy".into(),
                stage: ShotStage::Video,
                workflow_version_id: WORKFLOW_VERSION_ID.into(),
                recipe_id: RECIPE_ID.into(),
                scalar_values: json!({}),
                updated_at: Utc::now(),
            },
        )
        .await
        .unwrap();
    let batch_service = ShotBatchService::new(
        shots,
        Arc::new(SqliteProductionQueueRepository::new(f.pool.clone())),
        Arc::new(SqliteTaskRepository::new(f.pool.clone())),
        Arc::new(SqliteAssetRepository::new(f.pool.clone())),
        Arc::new(SqliteGenerationDefinitionRepository::new(f.pool.clone())),
        Arc::new(SqliteProjectRepository::new(f.pool.clone())),
        Arc::new(SystemClock),
    )
    .with_new_generation_admission(f.registry.clone());
    let error = batch_service
        .create(CreateShotBatchRequest {
            project_id: PROJECT_ID.into(),
            stage: ShotStage::Video,
            shot_ids: vec!["sht_policy".into()],
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("MINIMAX_VIDEO_PRODUCT_REQUIRED"));
    let mut batch = old.batch.clone();
    batch.id = ProductionBatchId::new();
    let mut item = old.items[0].clone();
    item.id = ProductionBatchItemId::new();
    item.batch_id = batch.id.clone();
    let error = batch_service
        .insert_prepared_batch_with_bindings(&batch, &[item], &[], &[])
        .await
        .unwrap_err();
    assert!(error.to_string().contains("MINIMAX_VIDEO_PRODUCT_REQUIRED"));
    let benchmark = WorkflowBenchmarkService::new(
        Arc::new(SqliteWorkflowBenchmarkRepository::new(f.pool.clone())),
        Arc::new(SqliteGenerationDefinitionRepository::new(f.pool.clone())),
        Arc::new(SqlitePresetRepository::new(f.pool.clone())),
        f.services.queue.clone(),
        Arc::new(SystemClock),
    );
    let error = benchmark
        .create(WorkflowBenchmarkCreateRequest {
            project_id: PROJECT_ID.into(),
            name: "Rejected".into(),
            media_type: "VIDEO".into(),
            base_values: values(),
            candidates: vec![
                WorkflowBenchmarkCandidateRequest {
                    workflow_version_id: WORKFLOW_VERSION_ID.into(),
                    recipe_id: RECIPE_ID.into(),
                    preset_id: None,
                    label: None,
                },
                WorkflowBenchmarkCandidateRequest {
                    workflow_version_id: f.pairs[0].0.clone(),
                    recipe_id: f.pairs[0].1.clone(),
                    preset_id: None,
                    label: None,
                },
            ],
            seed_mode: "FIXED".into(),
            fixed_seed: Some(42),
            repeat_count: 1,
            auto_start: false,
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("MINIMAX_VIDEO_PRODUCT_REQUIRED"));
    let package_root = f._owned.path().join("rejected-package");
    write_package(&package_root, &png_bytes([4, 5, 6, 255])).await;
    let (session, inspection) = f
        .services
        .package
        .inspect_session(PROJECT_ID, package_root)
        .await
        .unwrap();
    let assets_before = count(&f.pool, "assets").await;
    let error = f
        .services
        .package
        .create_batches(&session, &[inspection.items[0].id.clone()])
        .await
        .unwrap_err();
    assert!(error.to_string().contains("MINIMAX_VIDEO_PRODUCT_REQUIRED"));
    assert_eq!(
        count(&f.pool, "assets").await,
        assets_before,
        "rejected imports cannot create source Assets"
    );
    assert_eq!(count(&f.pool, "production_batches").await, 1);
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn i2v_requires_managed_first_frame_before_batch_or_task_but_t2v_does_not() {
    let f = fixture().await;
    let (version, recipe) = &f.pairs[1];
    assert!(f
        .services
        .queue
        .create(batch_request(version, recipe))
        .await
        .unwrap_err()
        .to_string()
        .contains("first_frame"));
    assert!(f
        .services
        .generation
        .start_generation(generation_request(version, recipe))
        .await
        .unwrap_err()
        .to_string()
        .contains("first_frame"));
    let mut invalid = batch_request(version, recipe);
    invalid.items[0].values.insert(
        "first_frame".into(),
        GenerationInputValue::Text("external.png".into()),
    );
    assert!(f.services.queue.create(invalid).await.is_err());
    f.services
        .queue
        .create(batch_request(&f.pairs[0].0, &f.pairs[0].1))
        .await
        .unwrap();
    assert_eq!(count(&f.pool, "tasks").await, 0);
    assert_eq!(f.comfy.submit_calls.load(Ordering::SeqCst), 0);
}
