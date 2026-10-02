use super::*;

pub(super) fn creation_context_services(
    pool: &SqlitePool,
) -> (
    ProductCreationFacade,
    ai_studio_lib::application::asset_query_service::AssetQueryService,
) {
    let definitions = Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let tasks = Arc::new(SqliteTaskRepository::new(pool.clone()));
    let assets = Arc::new(SqliteAssetRepository::new(pool.clone()));
    let shots = Arc::new(
        ShotService::new(
            Arc::new(SqliteShotRepository::new(pool.clone())),
            tasks.clone(),
            assets.clone(),
            definitions.clone(),
            Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
            Arc::new(TaskQueryService::new(
                tasks,
                assets.clone(),
                definitions.clone(),
            )),
            Arc::new(SqliteProductionQueueRepository::new(pool.clone())),
            Arc::new(SystemClock),
        )
        .with_stage_prompt_repository(Arc::new(SqliteShotRepository::new(pool.clone()))),
    );
    let facade = ProductCreationFacade::new(
        Arc::new(GenerationCatalogService::new(definitions)),
        bindings(pool),
        shots,
        Arc::new(WorkflowRegistryService::new(
            Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone())),
            Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone())),
            Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone())),
            Arc::new(SystemClock),
        )),
    );
    let query = ai_studio_lib::application::asset_query_service::AssetQueryService::new(
        assets,
        Arc::new(FileSystemAssetStore),
        Arc::new(SqliteProjectRepository::new(pool.clone())),
    );
    (facade, query)
}

#[tokio::test]
async fn product_creation_context_crud_preserves_stage_prompts_and_project_scope() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("create.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let (facade, assets) = creation_context_services(&pool);
    let center = command_center(&pool);
    let empty = facade
        .get(&center, &assets, PROJECT_ID, None, "image")
        .await
        .unwrap();
    assert!(empty.shots.is_empty());
    assert!(empty.selected_shot.is_none());
    assert!(facade.create_shot("prj_absent").await.is_err());
    let first = facade.create_shot(PROJECT_ID).await.unwrap();
    let second = facade.create_shot(PROJECT_ID).await.unwrap();
    let bulk = SqliteShotRepository::new(pool.clone());
    use ai_studio_lib::application::ports::{ShotBulkRepository, ShotStagePromptRecord};
    use ai_studio_lib::domain::ShotStage;
    bulk.update_stage_prompts_atomic(
        PROJECT_ID,
        &[
            ShotStagePromptRecord {
                shot_id: first.id.clone(),
                stage: ShotStage::Image,
                prompt_text: "image draft".into(),
                prompt_entry_id: None,
                prompt_version_id: None,
                updated_at: Utc::now(),
            },
            ShotStagePromptRecord {
                shot_id: first.id.clone(),
                stage: ShotStage::Video,
                prompt_text: "video draft".into(),
                prompt_entry_id: None,
                prompt_version_id: None,
                updated_at: Utc::now(),
            },
        ],
    )
    .await
    .unwrap();
    facade
        .update_shot(
            PROJECT_ID,
            CreationShotUpdate {
                shot_id: first.id.clone(),
                name: "重新命名".into(),
            },
        )
        .await
        .unwrap();
    let image = facade
        .get(&center, &assets, PROJECT_ID, Some(&first.id), "image")
        .await
        .unwrap();
    let video = facade
        .get(&center, &assets, PROJECT_ID, Some(&first.id), "video")
        .await
        .unwrap();
    assert_eq!(image.selected_shot.unwrap().prompt, "image draft");
    assert_eq!(video.selected_shot.unwrap().prompt, "video draft");
    assert_eq!(image.shots.len(), 2);
    assert!(facade
        .get(&center, &assets, PROJECT_ID, None, "audio")
        .await
        .is_err());
    assert!(facade
        .get(&center, &assets, PROJECT_ID, Some("sht_absent"), "image")
        .await
        .is_err());
    SqliteProjectRepository::new(pool.clone())
        .ensure_default_project(
            "prj_11111111-1111-4111-8111-111111111111",
            "Other",
            &dir.path().join("other"),
            Utc::now(),
        )
        .await
        .unwrap();
    assert_eq!(
        facade
            .get(
                &center,
                &assets,
                "prj_11111111-1111-4111-8111-111111111111",
                Some(&first.id),
                "image"
            )
            .await
            .unwrap_err()
            .code,
        "PROJECT_SCOPE_VIOLATION"
    );
    assert!(facade
        .update_shot(
            "prj_11111111-1111-4111-8111-111111111111",
            CreationShotUpdate {
                shot_id: first.id.clone(),
                name: "wrong".into()
            }
        )
        .await
        .is_err());
    assert!(facade
        .delete_shot("prj_11111111-1111-4111-8111-111111111111", &first.id)
        .await
        .is_err());
    assert!(facade
        .references_set(
            "prj_11111111-1111-4111-8111-111111111111",
            &first.id,
            "image",
            vec![]
        )
        .await
        .is_err());
    facade.delete_shot(PROJECT_ID, &first.id).await.unwrap();
    let remaining = facade
        .get(&center, &assets, PROJECT_ID, None, "image")
        .await
        .unwrap();
    assert_eq!(remaining.shots.len(), 1);
    assert_eq!(remaining.shots[0].id, second.id);
    assert!(remaining.selected_shot.is_none());
    assert_eq!(count(&pool, "tasks").await, 0);
    assert_eq!(count(&pool, "batches").await, 0);
}

#[tokio::test]
async fn product_creation_context_references_are_ordered_and_selection_requires_linked_output() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("references.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let (facade, query) = creation_context_services(&pool);
    let shot = facade.create_shot(PROJECT_ID).await.unwrap();
    use ai_studio_lib::domain::{Asset, AssetId};
    let references = (0..3)
        .map(|index| {
            Asset::new_source_image(
                AssetId::new(),
                PROJECT_ID,
                format!("ref {index}"),
                format!("ref{index}.png"),
                format!("ref{index}.png"),
                format!("sha{index}"),
                "image/png",
                16,
                16,
                100,
                serde_json::json!({}),
                Utc::now(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    SqliteAssetRepository::new(pool.clone())
        .insert_many(&references)
        .await
        .unwrap();
    let ids: Vec<_> = references
        .iter()
        .rev()
        .map(|asset| asset.id.as_str().to_owned())
        .collect();
    let response = facade
        .references_set(PROJECT_ID, &shot.id, "image", ids.clone())
        .await
        .unwrap();
    assert_eq!(response.reference_asset_ids, ids);
    assert!(facade
        .references_set(
            PROJECT_ID,
            &shot.id,
            "image",
            vec![ids[0].clone(), ids[0].clone()]
        )
        .await
        .is_err());
    assert!(facade
        .select_result(&query, PROJECT_ID, &shot.id, "image", &ids[0])
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
    let context = facade
        .get(
            &command_center(&pool),
            &query,
            PROJECT_ID,
            Some(&shot.id),
            "image",
        )
        .await
        .unwrap();
    assert!(context.candidates.is_empty());
    assert_eq!(context.selected_shot.unwrap().reference_asset_ids, ids);
    let json = serde_json::to_value(context.shots).unwrap();
    assert!(json[0].get("workflowVersionId").is_none());
    assert!(json[0].get("recipeId").is_none());
}

#[tokio::test]
async fn product_creation_context_select_second_preserves_candidates_and_task_history() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("selection.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let (facade, query) = creation_context_services(&pool);
    let shot = facade.create_shot(PROJECT_ID).await.unwrap();
    use ai_studio_lib::application::ports::ShotRepository;
    use ai_studio_lib::domain::{Asset, AssetId, ShotStage, Task};
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
    SqliteShotRepository::new(pool.clone())
        .link_generation(
            PROJECT_ID,
            &shot.id,
            ShotStage::Image,
            task.id.as_str(),
            None,
            Utc::now(),
        )
        .await
        .unwrap();
    let candidates = (0..3)
        .map(|index| {
            Asset::new_generated_image(
                AssetId::new(),
                PROJECT_ID,
                format!("candidate {index}"),
                format!("{index}.png"),
                format!("{index}.png"),
                format!("sha{index}"),
                "image/png",
                16,
                16,
                100,
                task.id.clone(),
                serde_json::json!({}),
                Utc::now(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    SqliteAssetRepository::new(pool.clone())
        .insert_many(&candidates)
        .await
        .unwrap();
    let events_before = SqliteTaskRepository::new(pool.clone())
        .list_events(&task.id)
        .await
        .unwrap();
    let selected_id = candidates[1].id.as_str();
    let response = facade
        .select_result(&query, PROJECT_ID, &shot.id, "image", selected_id)
        .await
        .unwrap();
    assert_eq!(response.selected_result_id.as_deref(), Some(selected_id));
    let context = facade
        .get(
            &command_center(&pool),
            &query,
            PROJECT_ID,
            Some(&shot.id),
            "image",
        )
        .await
        .unwrap();
    assert_eq!(context.candidates.len(), 3);
    assert_eq!(
        context
            .candidates
            .iter()
            .filter(|asset| asset.selected)
            .count(),
        1
    );
    assert!(context
        .candidates
        .iter()
        .any(|asset| asset.id == selected_id && asset.selected));
    assert!(context
        .candidates
        .iter()
        .any(|asset| asset.id == candidates[0].id.as_str()));
    assert!(context
        .candidates
        .iter()
        .any(|asset| asset.id == candidates[2].id.as_str()));
    assert_eq!(
        context.selected_shot.unwrap().recent_run.unwrap().id,
        task.id.as_str()
    );
    assert!(facade
        .get(
            &command_center(&pool),
            &query,
            PROJECT_ID,
            Some(&shot.id),
            "video"
        )
        .await
        .unwrap()
        .candidates
        .is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        SqliteTaskRepository::new(pool.clone())
            .list_events(&task.id)
            .await
            .unwrap()
            .len(),
        events_before.len()
    );
    assert_eq!(
        SqliteTaskRepository::new(pool.clone())
            .find_by_id(&task.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        task.status
    );
}
