use super::*;
use ai_studio_lib::application::{
    artifact_service::ArtifactService, asset_query_service::AssetQueryService,
    task_history_service::TaskHistoryService,
};
use ai_studio_lib::domain::Task;

struct DetailAuthorities {
    history: TaskHistoryService,
    assets: AssetQueryService,
    artifacts: ArtifactService,
    shots: ShotService,
}
impl DetailAuthorities {
    fn projection(&self) -> RunDetailServices<'_> {
        RunDetailServices {
            history: &self.history,
            assets: &self.assets,
            artifacts: &self.artifacts,
            shots: &self.shots,
        }
    }
}
fn details(pool: &SqlitePool, queue: Arc<ProductionQueueService>) -> DetailAuthorities {
    let tasks = Arc::new(SqliteTaskRepository::new(pool.clone()));
    let assets = Arc::new(SqliteAssetRepository::new(pool.clone()));
    let definitions = Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    DetailAuthorities {
        history: TaskHistoryService::new(
            Arc::new(SqliteTaskHistoryRepository::new(pool.clone())),
            Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone())),
            definitions.clone(),
            assets.clone(),
        ),
        assets: AssetQueryService::new(
            assets.clone(),
            Arc::new(FileSystemAssetStore),
            Arc::new(SqliteProjectRepository::new(pool.clone())),
        ),
        artifacts: ArtifactService::new(
            Arc::new(SqliteArtifactRepository::new(pool.clone())),
            Arc::new(SqliteProjectRepository::new(pool.clone())),
            tasks.clone(),
            queue,
            Arc::new(SystemClock),
        ),
        shots: ShotService::new(
            Arc::new(SqliteShotRepository::new(pool.clone())),
            tasks.clone(),
            assets.clone(),
            definitions.clone(),
            Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
            Arc::new(TaskQueryService::new(tasks, assets, definitions)),
            Arc::new(SqliteProductionQueueRepository::new(pool.clone())),
            Arc::new(SystemClock),
        ),
    }
}

async fn completed(queue: &Arc<ProductionQueueService>, pool: &SqlitePool) -> RunRef {
    let detail = batch(queue, 1).await;
    queue
        .start_for_test(PROJECT_ID, detail.batch.id.as_str())
        .await
        .unwrap();
    wait_for_batch_status(
        pool,
        detail.batch.id.as_str(),
        ProductionBatchStatus::Completed,
    )
    .await;
    reference(&detail)
}

#[tokio::test]
async fn phase4_target_3_statuses_are_live_projections_without_parent_sync() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("status.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let services = build_services(
        &pool,
        Arc::new(ControlledComfy::new(ComfyBehavior::Success)),
        dir.path(),
    );
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let detail = batch(&services.queue, 2).await;
    let locator = reference(&detail);
    for (batch_status, first, second, expected) in [
        ("READY", "PENDING", "PENDING", "QUEUED"),
        ("RUNNING", "DISPATCHING", "PENDING", "RUNNING"),
        ("PAUSED", "PENDING", "PENDING", "PAUSED"),
        ("COMPLETED", "FAILED", "FAILED", "FAILED"),
        ("COMPLETED", "SUCCEEDED", "FAILED", "PARTIAL"),
        ("COMPLETED", "SUCCEEDED", "SUCCEEDED", "SUCCEEDED"),
        ("COMPLETED", "CANCELLED", "CANCELLED", "CANCELLED"),
    ] {
        sqlx::query("UPDATE production_batches SET status=? WHERE id=?")
            .bind(batch_status)
            .bind(&locator.id)
            .execute(&pool)
            .await
            .unwrap();
        for (item, status) in detail.items.iter().zip([first, second]) {
            sqlx::query("UPDATE production_batch_items SET status=? WHERE id=?")
                .bind(status)
                .bind(item.id.as_str())
                .execute(&pool)
                .await
                .unwrap();
        }
        assert_eq!(
            facade
                .get(PROJECT_ID, locator.clone())
                .await
                .unwrap()
                .status,
            expected
        );
    }
}

#[tokio::test]
async fn phase4_target_4_detail_preserves_exact_historical_input_and_scope() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("detail.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let services = build_services(
        &pool,
        Arc::new(ControlledComfy::new(ComfyBehavior::Success)),
        dir.path(),
    );
    let locator = completed(&services.queue, &pool).await;
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let authorities = details(&pool, services.queue.clone());
    let pending = batch(&services.queue, 1).await;
    let shot = authorities.shots.create(PROJECT_ID).await.unwrap();
    sqlx::query("INSERT INTO shot_generation_links (id,shot_id,stage,production_batch_item_id,created_at) VALUES ('sgl_pending',?,'video',?,?)").bind(&shot.id).bind(pending.items[0].id.as_str()).bind(CREATED_AT).execute(&pool).await.unwrap();
    let queued = facade
        .get_detail(PROJECT_ID, reference(&pending), &authorities.projection())
        .await
        .unwrap()
        .detail
        .unwrap();
    assert_eq!(queued.sources[0].id, shot.id);
    assert_eq!(queued.inputs[0].task_id, None);
    assert_eq!(
        queued.inputs[0].item_id.as_deref(),
        Some(pending.items[0].id.as_str())
    );
    assert!(queued.inputs[0].selection_ref.is_some());
    let run = facade
        .get_detail(PROJECT_ID, locator.clone(), &authorities.projection())
        .await
        .unwrap();
    let input = &run.detail.unwrap().inputs[0];
    let pair = ExactGeneratorSelection::decode(input.selection_ref.as_deref().unwrap()).unwrap();
    assert_eq!(pair.workflow_version_id, WORKFLOW_VERSION_ID);
    assert_eq!(pair.recipe_id, RECIPE_ID);
    assert_eq!(
        input.values["prompt"],
        ai_studio_lib::application::task_history_service::DraftValueView::String {
            value: "fixture 0".into()
        }
    );
    assert_eq!(run.progress.succeeded, 1);
    let task = services
        .queue
        .get(PROJECT_ID, &locator.id)
        .await
        .unwrap()
        .items[0]
        .task_id
        .clone()
        .unwrap();
    let old = facade
        .get_detail(
            PROJECT_ID,
            RunRef {
                source: RunSource::Task,
                id: task.clone(),
            },
            &authorities.projection(),
        )
        .await
        .unwrap();
    assert_eq!(old.detail.unwrap().inputs.len(), 1);
    sqlx::query("DELETE FROM generation_snapshots WHERE task_id = ?")
        .bind(&task)
        .execute(&pool)
        .await
        .unwrap();
    let unavailable = facade
        .get_detail(
            PROJECT_ID,
            RunRef {
                source: RunSource::Task,
                id: task,
            },
            &authorities.projection(),
        )
        .await
        .unwrap()
        .detail
        .unwrap();
    let input = &unavailable.inputs[0];
    assert!(
        input.selection_ref.is_some(),
        "unavailable historical inputs must still allow exact-generator edit"
    );
    assert!(input.values.is_empty());
    assert!(input.reuse_unavailable_reason.is_some());
    assert!(facade
        .get_detail(
            "prj_11111111-1111-4111-8111-111111111111",
            locator,
            &authorities.projection()
        )
        .await
        .is_err());
}

#[tokio::test]
async fn phase4_target_10_results_project_asset_review_selection_as_independent_facts() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("results.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let services = build_services(
        &pool,
        Arc::new(ControlledComfy::new(ComfyBehavior::Success)),
        dir.path(),
    );
    let locator = completed(&services.queue, &pool).await;
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let authorities = details(&pool, services.queue.clone());
    let results = facade
        .results_get(PROJECT_ID, locator.clone(), &authorities.projection())
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert!(result.asset_exists);
    assert_eq!(result.review_state.as_deref(), Some("PENDING"));
    assert!(result.selected_shot_ids.is_empty());
    assert_eq!(
        facade
            .get(PROJECT_ID, locator.clone())
            .await
            .unwrap()
            .status,
        "SUCCEEDED"
    );
    let shot = authorities.shots.create(PROJECT_ID).await.unwrap();
    authorities
        .shots
        .select_result(
            PROJECT_ID,
            &shot.id,
            ai_studio_lib::domain::ShotStage::Video,
            &result.asset_id,
            false,
        )
        .await
        .unwrap();
    let selected = facade
        .results_get(PROJECT_ID, locator, &authorities.projection())
        .await
        .unwrap();
    assert_eq!(selected[0].selected_shot_ids, vec![shot.id]);
    assert_eq!(selected[0].review_state.as_deref(), Some("PENDING"));
}

#[tokio::test]
async fn phase4_target_11_review_delegates_occ_without_task_shot_or_asset_mutation() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("review.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let services = build_services(
        &pool,
        Arc::new(ControlledComfy::new(ComfyBehavior::Success)),
        dir.path(),
    );
    let locator = completed(&services.queue, &pool).await;
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let authorities = details(&pool, services.queue.clone());
    let result = facade
        .results_get(PROJECT_ID, locator.clone(), &authorities.projection())
        .await
        .unwrap()
        .remove(0);
    let shot = authorities.shots.create(PROJECT_ID).await.unwrap();
    authorities
        .shots
        .select_result(
            PROJECT_ID,
            &shot.id,
            ai_studio_lib::domain::ShotStage::Video,
            &result.asset_id,
            false,
        )
        .await
        .unwrap();
    let before = sqlx::query_as::<_, (String, String)>("SELECT id,status FROM tasks ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    let request = RunResultReviewRequest {
        run_ref: locator.clone(),
        asset_id: result.asset_id.clone(),
        decision: "APPROVED".into(),
        comment: "checked".into(),
        expected_revision: 0,
    };
    facade
        .result_review(PROJECT_ID, request.clone(), &authorities.projection())
        .await
        .unwrap();
    let mut reject = request.clone();
    reject.decision = "REJECTED".into();
    assert_eq!(
        facade
            .result_review(PROJECT_ID, reject.clone(), &authorities.projection())
            .await
            .unwrap_err()
            .code,
        "RUN_REVIEW_CONFLICT"
    );
    reject.expected_revision = 1;
    assert_eq!(
        facade
            .result_review(PROJECT_ID, reject.clone(), &authorities.projection())
            .await
            .unwrap_err()
            .code,
        "RUN_REVIEW_INVALID"
    );
    let second_locator = completed(&services.queue, &pool).await;
    let second_result = facade
        .results_get(
            PROJECT_ID,
            second_locator.clone(),
            &authorities.projection(),
        )
        .await
        .unwrap()
        .remove(0);
    reject.run_ref = second_locator;
    reject.asset_id = second_result.asset_id;
    reject.expected_revision = 0;
    facade
        .result_review(PROJECT_ID, reject, &authorities.projection())
        .await
        .unwrap();
    let after = sqlx::query_as::<_, (String, String)>("SELECT id,status FROM tasks ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(before.iter().all(|row| after.contains(row)));

    assert_eq!(
        authorities
            .shots
            .get(PROJECT_ID, &shot.id)
            .await
            .unwrap()
            .selected_video_asset_id
            .as_deref(),
        Some(result.asset_id.as_str())
    );
    assert!(authorities
        .assets
        .get(PROJECT_ID, &result.asset_id)
        .await
        .is_ok());
    let mut foreign = request;
    foreign.run_ref = standalone(&pool).await;
    assert_eq!(
        facade
            .result_review(PROJECT_ID, foreign, &authorities.projection())
            .await
            .unwrap_err()
            .code,
        "RUN_RESULT_NOT_FOUND"
    );
}

async fn batch(
    queue: &ProductionQueueService,
    size: usize,
) -> ai_studio_lib::domain::ProductionBatchDetail {
    queue
        .create(CreateProductionBatchRequest {
            project_id: PROJECT_ID.into(),
            name: "Unified Runs fixture".into(),
            continue_on_failure: true,
            items: (0..size)
                .map(|i| CreateProductionBatchItem {
                    workflow_version_id: WORKFLOW_VERSION_ID.into(),
                    recipe_id: RECIPE_ID.into(),
                    values: BTreeMap::from([(
                        "prompt".into(),
                        GenerationInputValue::Text(format!("fixture {i}")),
                    )]),
                })
                .collect(),
        })
        .await
        .unwrap()
}

async fn standalone(pool: &SqlitePool) -> RunRef {
    let task = Task::new(
        PROJECT_ID,
        WORKFLOW_ID,
        WORKFLOW_VERSION_ID,
        RECIPE_ID,
        Utc::now(),
    );
    SqliteTaskRepository::new(pool.clone())
        .create(&task, &task.created_event())
        .await
        .unwrap();
    RunRef {
        source: RunSource::Task,
        id: task.id.as_str().into(),
    }
}

fn reference(detail: &ai_studio_lib::domain::ProductionBatchDetail) -> RunRef {
    RunRef {
        source: RunSource::QueueBatch,
        id: detail.batch.id.as_str().into(),
    }
}

#[tokio::test]
async fn phase4_target_1_list_scope_order_and_standalone() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("runs.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let services = build_services(
        &pool,
        Arc::new(ControlledComfy::new(ComfyBehavior::Success)),
        dir.path(),
    );
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let first = batch(&services.queue, 1).await;
    let old = standalone(&pool).await;
    let before = count(&pool, "tasks").await;
    let page = facade
        .list(PROJECT_ID, RunListFilter::All, None)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(page.items.iter().any(|item| item.run_ref == old));
    assert!(page
        .items
        .iter()
        .any(|item| item.run_ref == reference(&first)));
    assert!(page.items.iter().all(|item| item.project_id == PROJECT_ID));
    let again = facade
        .list(PROJECT_ID, RunListFilter::All, None)
        .await
        .unwrap();
    assert_eq!(
        page.items
            .iter()
            .map(|item| &item.run_ref)
            .collect::<Vec<_>>(),
        again
            .items
            .iter()
            .map(|item| &item.run_ref)
            .collect::<Vec<_>>()
    );
    assert!(facade
        .list(
            "prj_11111111-1111-4111-8111-111111111111",
            RunListFilter::All,
            None
        )
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(count(&pool, "tasks").await, before);
    assert_eq!(
        facade
            .list(PROJECT_ID, RunListFilter::All, Some("invalid"))
            .await
            .unwrap_err()
            .code,
        "RUN_CURSOR_UNSUPPORTED"
    );
    assert!(page.next_cursor.is_none());
}

#[tokio::test]
async fn phase4_target_2_persisted_parent_dedupe_read_only() {
    use ai_studio_lib::application::ports::ProductionOrchestratorRepository;
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("runs.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let services = build_services(
        &pool,
        Arc::new(ControlledComfy::new(ComfyBehavior::Success)),
        dir.path(),
    );
    let (facade, production) = run_facade(&pool, services.queue.clone());
    let detail = batch(&services.queue, 1).await;
    let child = standalone(&pool).await;
    let orphan = standalone(&pool).await;
    sqlx::query("UPDATE production_batch_items SET task_id=? WHERE id=?")
        .bind(&child.id)
        .bind(detail.items[0].id.as_str())
        .execute(&pool)
        .await
        .unwrap();
    let run = production
        .create(ProductionRunCreateRequest {
            project_id: PROJECT_ID.into(),
            name: "Parent".into(),
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
    let repo = SqliteProductionOrchestratorRepository::new(pool.clone());
    let stage = repo.load_stage(&run.id, 1).await.unwrap().unwrap();
    repo.attach_image_batch_atomic(
        &run.id,
        &stage.id,
        detail.batch.id.as_str(),
        &[],
        CREATED_AT,
    )
    .await
    .unwrap();
    let before = sqlx::query_as::<_, (String, String)>(
        "SELECT status, updated_at FROM production_runs WHERE id=?",
    )
    .bind(&run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let page = facade
        .list(PROJECT_ID, RunListFilter::All, None)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(page
        .items
        .iter()
        .any(|item| item.run_ref.source == RunSource::ProductionRun));
    assert!(page.items.iter().any(|item| item.run_ref == orphan));
    assert!(!page
        .items
        .iter()
        .any(|item| item.run_ref == child || item.run_ref == reference(&detail)));
    assert_eq!(
        facade
            .get(PROJECT_ID, reference(&detail))
            .await
            .unwrap()
            .preferred_parent
            .unwrap()
            .id,
        run.id
    );
    let after = sqlx::query_as::<_, (String, String)>(
        "SELECT status, updated_at FROM production_runs WHERE id=?",
    )
    .bind(&run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn phase4_target_5_start_revalidates_and_task_denial_has_no_side_effect() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("runs.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let services = build_services(
        &pool,
        Arc::new(ControlledComfy::new(ComfyBehavior::Success)),
        dir.path(),
    );
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let old = standalone(&pool).await;
    let n = count(&pool, "tasks").await;
    let error = facade
        .start(PROJECT_ID, old, |_, _| async {
            panic!("denied tasks never invoke admission")
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "RUN_NOT_STARTABLE");
    assert_eq!(count(&pool, "tasks").await, n);
    let detail = batch(&services.queue, 1).await;
    let locator = reference(&detail);
    let error = facade
        .start(PROJECT_ID, locator.clone(), |_, _| async {
            Err(
                ai_studio_lib::application::product::error::ProductError::new(
                    "RUN_START_BLOCKED",
                    "fixture",
                    None,
                ),
            )
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "RUN_START_BLOCKED");
    assert_eq!(count(&pool, "tasks").await, n);
    let queue = services.queue.clone();
    facade
        .start(PROJECT_ID, locator, move |p, b| async move {
            queue
                .start_for_test(&p, &b)
                .await
                .map_err(ai_studio_lib::application::product::error::ProductError::internal)
        })
        .await
        .unwrap();
    wait_for_batch_status(
        &pool,
        detail.batch.id.as_str(),
        ProductionBatchStatus::Completed,
    )
    .await;
}

#[tokio::test]
async fn phase4_target_6_pause_resume_preserves_locator() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("runs.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let services = build_services(
        &pool,
        Arc::new(ControlledComfy::new(ComfyBehavior::Success)),
        dir.path(),
    );
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let detail = batch(&services.queue, 1).await;
    let locator = reference(&detail);
    let paused = facade.pause(PROJECT_ID, locator.clone()).await.unwrap();
    assert_eq!(paused.status, "PAUSED");
    assert!(paused.available_actions.contains(&"START".into()));
    assert_eq!(
        facade
            .list(PROJECT_ID, RunListFilter::Active, None)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let queue = services.queue.clone();
    facade
        .start(PROJECT_ID, locator.clone(), move |p, b| async move {
            queue
                .start_for_test(&p, &b)
                .await
                .map_err(ai_studio_lib::application::product::error::ProductError::internal)
        })
        .await
        .unwrap();
    wait_for_batch_status(
        &pool,
        detail.batch.id.as_str(),
        ProductionBatchStatus::Completed,
    )
    .await;
    assert_eq!(
        facade.get(PROJECT_ID, locator).await.unwrap().status,
        "SUCCEEDED"
    );
    let task = standalone(&pool).await;
    assert_eq!(
        facade.pause(PROJECT_ID, task).await.unwrap_err().code,
        "RUN_PAUSE_UNSUPPORTED"
    );
}

#[tokio::test]
async fn phase4_target_7_pending_cancel_preserves_success_and_history() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("runs.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let mut adapter = ControlledComfy::new(ComfyBehavior::Success);
    adapter.fail_second = true;
    let services = build_services(&pool, Arc::new(adapter), dir.path());
    let (facade, _) = run_facade(&pool, services.queue.clone());
    let detail = batch(&services.queue, 3).await;
    services
        .queue
        .start_for_test(PROJECT_ID, detail.batch.id.as_str())
        .await
        .unwrap();
    wait_for_batch_status(
        &pool,
        detail.batch.id.as_str(),
        ProductionBatchStatus::Paused,
    )
    .await;
    let assets_before = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id,source_task_id,sha256 FROM assets ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!assets_before.is_empty());
    let task_count = count(&pool, "tasks").await;
    let before = services
        .queue
        .get(PROJECT_ID, detail.batch.id.as_str())
        .await
        .unwrap();
    let cancel = TaskCancellationService::new(
        Arc::new(SqliteTaskRepository::new(pool.clone())),
        TaskExecutionRegistry::default(),
        Arc::new(SystemClock),
        Arc::new(NoopTaskUpdateSink),
    );
    let projected = facade
        .cancel(PROJECT_ID, reference(&detail), &cancel)
        .await
        .unwrap();
    assert_eq!(projected.status, "PARTIAL");
    let after = services
        .queue
        .get(PROJECT_ID, detail.batch.id.as_str())
        .await
        .unwrap();
    assert_eq!(before.items[0], after.items[0]);
    assert_eq!(after.items.len(), 3);
    assert_eq!(after.items[2].status, ProductionBatchItemStatus::Cancelled);
    assert_eq!(count(&pool, "tasks").await, task_count);
    let assets_after = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id,source_task_id,sha256 FROM assets ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(assets_before, assets_after);
    assert_eq!(before.items[1], after.items[1]);
    assert_eq!(
        facade
            .cancel(PROJECT_ID, reference(&detail), &cancel)
            .await
            .unwrap_err()
            .code,
        "RUN_CANCEL_UNSUPPORTED"
    );
}
