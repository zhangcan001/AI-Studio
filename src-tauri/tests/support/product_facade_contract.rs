use super::*;
use ai_studio_lib::application::{
    generation_catalog_service::GenerationCatalogService,
    generation_input_preparer::GenerationInputValue,
    product::{creation_facade::*, project_facade::*, run_facade::*, selection_ref::*},
    production_audit_service::ProductionAuditService,
    production_orchestrator_service::{ProductionOrchestratorService, ProductionRunCreateRequest},
    production_queue_service::{CreateProductionBatchItem, CreateProductionBatchRequest},
    project_command_center_service::ProjectCommandCenterService,
    project_workflow_binding_service::{
        ProjectWorkflowBindingRemoveRequest, ProjectWorkflowBindingService,
    },
    shot_service::ShotService,
    task_cancellation_service::TaskCancellationService,
    task_execution_registry::TaskExecutionRegistry,
    task_query_service::TaskQueryService,
    workflow_registry_service::WorkflowRegistryService,
};
use ai_studio_lib::infrastructure::database::*;
use std::collections::BTreeMap;

fn command_center(pool: &SqlitePool) -> Arc<ProjectCommandCenterService> {
    Arc::new(ProjectCommandCenterService::new(
        Arc::new(SqliteProjectCommandCenterRepository::new(pool.clone())),
        Arc::new(ProductionAuditService::new(Arc::new(
            SqliteProductionAuditRepository::new(pool.clone()),
        ))),
    ))
}

fn bindings(pool: &SqlitePool) -> Arc<ProjectWorkflowBindingService> {
    Arc::new(ProjectWorkflowBindingService::new(
        Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone())),
        Arc::new(SqliteProjectRepository::new(pool.clone())),
        Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone())),
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone())),
        Arc::new(SystemClock),
    ))
}

fn run_facade(
    pool: &SqlitePool,
    queue: Arc<ProductionQueueService>,
) -> (ProductRunFacade, Arc<ProductionOrchestratorService>) {
    let task_repo = Arc::new(SqliteTaskRepository::new(pool.clone()));
    let definitions = Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let production = Arc::new(ProductionOrchestratorService::new(
        Arc::new(SqliteProductionOrchestratorRepository::new(pool.clone())),
        definitions.clone(),
        queue.clone(),
        Arc::new(TaskCancellationService::new(
            task_repo.clone(),
            TaskExecutionRegistry::default(),
            Arc::new(SystemClock),
            Arc::new(NoopTaskUpdateSink),
        )),
        Arc::new(SystemClock),
    ));
    let facade = ProductRunFacade::new(
        queue,
        production.clone(),
        Arc::new(TaskQueryService::new(
            task_repo,
            Arc::new(SqliteAssetRepository::new(pool.clone())),
            definitions,
        )),
    );
    (facade, production)
}

fn selection() -> String {
    ExactGeneratorSelection {
        workflow_version_id: WORKFLOW_VERSION_ID.into(),
        recipe_id: RECIPE_ID.into(),
    }
    .encode()
    .unwrap()
}
fn binding_request(instance: Option<String>, revision: Option<i64>) -> GeneratorBindingSetRequest {
    GeneratorBindingSetRequest {
        stage: "VIDEO".into(),
        mode: "DEFAULT".into(),
        selection_ref: selection(),
        expected_binding_instance_id: instance,
        expected_revision: revision,
    }
}

#[tokio::test]
async fn product_contract_overview_and_occ_preserve_authority() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("product.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let center = command_center(&pool);
    let legacy = center.get(PROJECT_ID).await.unwrap();
    let overview = ProductProjectFacade::new(center)
        .get_overview(PROJECT_ID)
        .await
        .unwrap();
    assert_eq!(overview.project, legacy.project);
    assert_eq!(overview.blocking_state, legacy.audit.health);
    assert_eq!(overview.progress.total, legacy.shots.total);
    assert_eq!(overview.next_action, legacy.recommended_action);
    assert_eq!(overview.runtime_readiness, legacy.readiness);
    assert_eq!(
        overview.active_runs.running_batches,
        legacy.queue.running_queues
    );
    assert_eq!(
        overview.recent_results.total,
        legacy.tasks_assets.asset_count
    );
    assert_eq!(overview.blocking_issues.len(), legacy.issues.len());
    let authority = bindings(&pool);
    let (first, second) = tokio::join!(
        ProductProjectFacade::set_generator_binding(
            &authority,
            PROJECT_ID,
            binding_request(None, None)
        ),
        ProductProjectFacade::set_generator_binding(
            &authority,
            PROJECT_ID,
            binding_request(None, None)
        )
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let initial = first.or(second).unwrap().remove(0);
    let pair = ExactGeneratorSelection::decode(&initial.selection_ref).unwrap();
    assert_eq!(pair.workflow_version_id, WORKFLOW_VERSION_ID);
    assert_eq!(pair.recipe_id, RECIPE_ID);
    let saved = ProductProjectFacade::set_generator_binding(
        &authority,
        PROJECT_ID,
        binding_request(
            Some(initial.binding_instance_id.clone()),
            Some(initial.revision),
        ),
    )
    .await
    .unwrap()
    .remove(0);
    let conflict = ProductProjectFacade::set_generator_binding(
        &authority,
        PROJECT_ID,
        binding_request(
            Some(initial.binding_instance_id.clone()),
            Some(initial.revision),
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(conflict.code, "GENERATOR_BINDING_CONFLICT");
    assert_eq!(
        conflict.details.current_binding.unwrap().revision,
        saved.revision
    );
    authority
        .remove(
            PROJECT_ID,
            ProjectWorkflowBindingRemoveRequest {
                stage: "VIDEO".into(),
                mode: "DEFAULT".into(),
                expected_binding_instance_id: Some(saved.binding_instance_id),
                expected_revision: Some(saved.revision),
            },
        )
        .await
        .unwrap();
    let recreated = ProductProjectFacade::set_generator_binding(
        &authority,
        PROJECT_ID,
        binding_request(None, None),
    )
    .await
    .unwrap()
    .remove(0);
    assert_ne!(recreated.binding_instance_id, initial.binding_instance_id);
    assert_eq!(
        ProductProjectFacade::set_generator_binding(
            &authority,
            PROJECT_ID,
            binding_request(Some(initial.binding_instance_id), Some(initial.revision))
        )
        .await
        .unwrap_err()
        .code,
        "GENERATOR_BINDING_CONFLICT"
    );
    assert_eq!(
        authority
            .get(PROJECT_ID)
            .await
            .unwrap()
            .video_default
            .unwrap()
            .binding_instance_id,
        recreated.binding_instance_id
    );
}

#[tokio::test]
async fn product_contract_catalog_retirement_unavailable_and_no_raw_identity() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("product.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    for (id, category, yaml) in [
        (
            "wfl_kera2_t2i_local_v2",
            "image",
            RECIPE_YAML.replace("type: video", "type: image"),
        ),
        (
            "wfl_minimax_h3_fl2va_i2v_quality",
            "video",
            RECIPE_YAML.to_owned(),
        ),
        (
            "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
            "video",
            RECIPE_YAML.to_owned(),
        ),
    ] {
        let version = format!("{id}-v1");
        let recipe = format!("{id}-r1");
        sqlx::query("INSERT INTO workflows(id,name,category,mode,created_at,updated_at) VALUES(?,?,?,'fl2va',?,?)").bind(id).bind(id).bind(category).bind(CREATED_AT).bind(CREATED_AT).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO workflow_versions(id,workflow_id,version,api_workflow_json,workflow_sha256,created_at) VALUES(?,?,'1',?,'fixture',?)").bind(&version).bind(id).bind(WORKFLOW_JSON).bind(CREATED_AT).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO recipes(id,workflow_version_id,version,schema_version,recipe_yaml,recipe_sha256,created_at) VALUES(?,?,'1',1,?,'fixture',?)").bind(recipe).bind(&version).bind(yaml).bind(CREATED_AT).execute(&pool).await.unwrap();
        sqlx::query("UPDATE workflows SET current_version_id=? WHERE id=?")
            .bind(version)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let authority = bindings(&pool);
    ProductProjectFacade::set_generator_binding(
        &authority,
        PROJECT_ID,
        binding_request(None, None),
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO workflow_runtime_states(workflow_version_id,enabled,updated_at) VALUES(?,0,?)",
    )
    .bind(WORKFLOW_VERSION_ID)
    .bind(CREATED_AT)
    .execute(&pool)
    .await
    .unwrap();
    let definitions = Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let tasks = Arc::new(SqliteTaskRepository::new(pool.clone()));
    let assets = Arc::new(SqliteAssetRepository::new(pool.clone()));
    let shots = Arc::new(ShotService::new(
        Arc::new(SqliteShotRepository::new(pool.clone())),
        tasks.clone(),
        assets.clone(),
        definitions.clone(),
        Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
        Arc::new(TaskQueryService::new(tasks, assets, definitions.clone())),
        Arc::new(SqliteProductionQueueRepository::new(pool.clone())),
        Arc::new(SystemClock),
    ));
    let registry = Arc::new(WorkflowRegistryService::new(
        Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone())),
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone())),
        Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone())),
        Arc::new(SystemClock),
    ));
    let facade = ProductCreationFacade::new(
        Arc::new(GenerationCatalogService::new(definitions)),
        authority,
        shots,
        registry,
    );
    let image = facade
        .generators_list(PROJECT_ID, None, "image")
        .await
        .unwrap();
    assert!(image.iter().any(|o| o.name == "wfl_kera2_t2i_local_v2"));
    let video = facade
        .generators_list(PROJECT_ID, None, "video")
        .await
        .unwrap();
    assert!(video
        .iter()
        .any(|o| o.name == "wfl_minimax_h3_fl2va_i2v_quality"));
    assert!(!video.iter().any(|o| o.name.contains("8step_fast")));
    assert!(video
        .iter()
        .any(|o| !o.availability && o.availability_reason.is_some()));
    let wire = serde_json::to_value(&video).unwrap();
    for option in wire.as_array().unwrap() {
        for key in [
            "workflowVersionId",
            "recipeId",
            "nodeId",
            "packagePath",
            "packageHash",
            "workflowHash",
        ] {
            assert!(option.get(key).is_none());
        }
    }
}

#[tokio::test]
async fn product_contract_partial_retry_idempotent_preserves_history_and_scope() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("product.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let mut adapter = ControlledComfy::new(ComfyBehavior::Success);
    adapter.fail_second = true;
    let comfy = Arc::new(adapter);
    let services = build_services(&pool, comfy.clone(), &dir.path().join("package"));
    let initial = services
        .queue
        .create(CreateProductionBatchRequest {
            project_id: PROJECT_ID.into(),
            name: "Product recovery".into(),
            continue_on_failure: true,
            items: (0..3)
                .map(|i| CreateProductionBatchItem {
                    workflow_version_id: WORKFLOW_VERSION_ID.into(),
                    recipe_id: RECIPE_ID.into(),
                    values: BTreeMap::from([(
                        "prompt".into(),
                        GenerationInputValue::Text(format!("input {i}")),
                    )]),
                })
                .collect(),
        })
        .await
        .unwrap();
    let batch = initial.batch.id.as_str();
    let ids: Vec<String> = initial
        .items
        .iter()
        .map(|item| item.id.as_str().to_owned())
        .collect();
    services
        .queue
        .start_for_test(PROJECT_ID, batch)
        .await
        .unwrap();
    wait_for_item_status(&pool, &ids[1], ProductionBatchItemStatus::Failed).await;
    wait_for_batch_status(&pool, batch, ProductionBatchStatus::Paused).await;
    let (facade, production) = run_facade(&pool, services.queue.clone());
    let locator = RunRef {
        source: RunSource::QueueBatch,
        id: batch.into(),
    };
    assert_eq!(
        facade
            .get(PROJECT_ID, locator.clone())
            .await
            .unwrap()
            .status,
        "PAUSED"
    );
    services
        .queue
        .start_for_test(PROJECT_ID, batch)
        .await
        .unwrap();
    wait_for_item_status(&pool, &ids[2], ProductionBatchItemStatus::Succeeded).await;
    wait_for_batch_status(&pool, batch, ProductionBatchStatus::Completed).await;
    let before = services.queue.get(PROJECT_ID, batch).await.unwrap();
    let assets_before = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, source_task_id, sha256 FROM assets ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let projected = facade.get(PROJECT_ID, locator.clone()).await.unwrap();
    assert_eq!(projected.status, "PARTIAL");
    assert_eq!(projected.progress.succeeded, 2);
    assert_eq!(projected.progress.failed, 1);
    assert!(facade.get("prj_other", locator.clone()).await.is_err());
    let task_ref = RunRef {
        source: RunSource::Task,
        id: before.items[0].task_id.clone().unwrap(),
    };
    assert_eq!(
        facade
            .get(PROJECT_ID, task_ref.clone())
            .await
            .unwrap()
            .preferred_parent,
        Some(locator.clone())
    );
    assert!(facade.get("prj_other", task_ref).await.is_err());
    let retry = RunRetryRequest {
        run_ref: locator.clone(),
        selected_item_ids: vec![ids[1].clone()],
    };
    facade.retry(PROJECT_ID, retry.clone()).await.unwrap();
    facade.retry(PROJECT_ID, retry).await.unwrap();
    let prepared = services.queue.get(PROJECT_ID, batch).await.unwrap();
    assert_eq!(prepared.items.len(), 4);
    assert_eq!(count(&pool, "tasks").await, 3);
    let child = prepared.items.last().unwrap().id.as_str();
    services
        .queue
        .start_for_test(PROJECT_ID, batch)
        .await
        .unwrap();
    wait_for_item_status(&pool, child, ProductionBatchItemStatus::Succeeded).await;
    wait_for_batch_status(&pool, batch, ProductionBatchStatus::Completed).await;
    let after = services.queue.get(PROJECT_ID, batch).await.unwrap();
    for i in 0..3 {
        assert_eq!(after.items[i], before.items[i]);
    }
    assert_ne!(after.items[3].task_id, before.items[1].task_id);
    let assets_after = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, source_task_id, sha256 FROM assets ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(assets_before.len(), 2);
    assert_eq!(assets_after.len(), 3);
    for asset in assets_before {
        assert!(assets_after.contains(&asset));
    }
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        facade.get(PROJECT_ID, locator).await.unwrap().status,
        "SUCCEEDED"
    );
    let run = production
        .create(ProductionRunCreateRequest {
            project_id: PROJECT_ID.into(),
            name: "Production projection".into(),
            krea2_workflow_version_id: WORKFLOW_VERSION_ID.into(),
            krea2_recipe_id: RECIPE_ID.into(),
            krea2_preset_id: None,
            krea2_values: BTreeMap::new(),
            image_count: 1,
            h3_workflow_version_id: None,
            h3_recipe_id: None,
            h3_profile: None,
            h3_values: BTreeMap::new(),
            template_id: None,
        })
        .await
        .unwrap();
    let reference = RunRef {
        source: RunSource::ProductionRun,
        id: run.id.clone(),
    };
    assert_eq!(
        facade
            .get(PROJECT_ID, reference.clone())
            .await
            .unwrap()
            .status,
        "QUEUED"
    );
    assert!(facade.get("prj_other", reference).await.is_err());
    use ai_studio_lib::application::ports::ProductionOrchestratorRepository;
    let repository = SqliteProductionOrchestratorRepository::new(pool.clone());
    let stage = repository.load_stage(&run.id, 1).await.unwrap().unwrap();
    repository
        .attach_image_batch_atomic(&run.id, &stage.id, batch, &[], CREATED_AT)
        .await
        .unwrap();
    assert_eq!(
        facade
            .get(
                PROJECT_ID,
                RunRef {
                    source: RunSource::QueueBatch,
                    id: batch.into()
                }
            )
            .await
            .unwrap()
            .preferred_parent,
        Some(RunRef {
            source: RunSource::ProductionRun,
            id: run.id
        })
    );
}

#[tokio::test]
async fn product_contract_standalone_input_error_denies_retry_without_side_effects() {
    use ai_studio_lib::domain::{Task, TaskError, TaskStatus};
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("product.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let comfy = Arc::new(ControlledComfy::new(ComfyBehavior::Success));
    let services = build_services(&pool, comfy.clone(), dir.path());
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let repository = SqliteTaskRepository::new(pool.clone());
    let mut task = Task::new(
        PROJECT_ID,
        WORKFLOW_ID,
        WORKFLOW_VERSION_ID,
        RECIPE_ID,
        Utc::now(),
    );
    repository
        .create(&task, &task.created_event())
        .await
        .unwrap();
    let event = task
        .fail(
            TaskError {
                code: "INPUT_OUT_OF_RANGE".into(),
                message: "fixture width is invalid".into(),
                raw: None,
            },
            Utc::now(),
        )
        .unwrap();
    repository
        .persist_transition(&task, &event, TaskStatus::Created)
        .await
        .unwrap();
    let reference = RunRef {
        source: RunSource::Task,
        id: task.id.as_str().into(),
    };
    let run = facade.get(PROJECT_ID, reference.clone()).await.unwrap();
    assert_eq!(run.status, "FAILED");
    assert!(run.preferred_parent.is_none());
    let error = facade
        .retry(
            PROJECT_ID,
            RunRetryRequest {
                run_ref: reference,
                selected_item_ids: vec![],
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "EDIT_INPUT_REQUIRED");
    assert_eq!(error.details.action, Some("EDIT_INPUT"));
    assert_eq!(
        error.details.action_location.unwrap().project_id,
        PROJECT_ID
    );
    assert!(!error.details.retryable);
    assert_eq!(count(&pool, "tasks").await, 1);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
}
