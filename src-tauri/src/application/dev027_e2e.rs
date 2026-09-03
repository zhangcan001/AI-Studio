//! DEV-027's deterministic, no-GPU production-pipeline acceptance test.
//!
//! The test deliberately uses the application services for import, stage
//! configuration, batch creation, ordered reference binding, and manual
//! result selection.  Only the queue/task/output boundary is simulated, so it
//! never submits a prompt to ComfyUI or depends on a GPU.

#[cfg(test)]
mod tests {
    use crate::application::generation_service::GenerationService;
    use crate::application::ports::{
        AssetRepository, ComfyAdapterFactory, ComfyConnectionConfig,
        GenerationDefinitionRepository, NoopTaskUpdateSink, ProductionQueueRepository,
        ProjectRecord, ProjectRepository, ShotBatchRepository, ShotBulkRepository, ShotRepository,
        TaskRepository,
    };
    use crate::application::production_queue_service::ProductionQueueService;
    use crate::application::shot_batch_service::{CreateShotBatchRequest, ShotBatchService};
    use crate::application::shot_bulk_service::{
        BulkStageConfigRequest, ShotBulkImportRequest, ShotBulkInputFormat, ShotBulkService,
    };
    use crate::application::shot_service::{ShotService, ShotUpdateRequest};
    use crate::application::task_query_service::TaskQueryService;
    use crate::application::task_recovery_service::TaskRecoveryService;
    use crate::domain::{
        Asset, AssetId, AssetType, ProductionBatchId, ProductionBatchItem,
        ProductionBatchItemStatus, ProductionBatchStatus, ShotStage, Task, TaskId, TaskStatus,
    };
    use crate::infrastructure::comfy::ComfyHttpAdapterFactory;
    use crate::infrastructure::database::{
        initialize,
        repositories::{
            test_support, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
            SqliteGenerationSnapshotRepository, SqliteProductionQueueRepository,
            SqliteProjectRepository, SqlitePromptLibraryRepository, SqliteShotRepository,
            SqliteTaskRepository,
        },
    };
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use crate::infrastructure::time::SystemClock;
    use chrono::Utc;
    use serde_json::{json, Value};
    use std::{collections::BTreeMap, env, path::PathBuf, sync::Arc};
    use tempfile::tempdir;
    use tokio::time::{sleep, Duration};
    use uuid::Uuid;

    const PROJECT_ID: &str = "prj_default";
    const WORKFLOW_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_i2i/workflow_api.json"
    ));
    const KERA2_RECIPE_YAML: &str = r#"
schema_version: 1
id: dev027_kera2
name: DEV027 Kera2 Image
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
    const I2V_RECIPE_YAML: &str = r#"
schema_version: 1
id: dev027_i2v
name: DEV027 I2V
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  first_frame:
    type: image
    label: First frame
    required: false
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: first_frame
    target:
      node: "10"
      input: image
outputs:
  - id: generated_video
    type: video
    node: "9"
    required: true
"#;
    const REF2VA_RECIPE_YAML: &str = r#"
schema_version: 1
id: dev027_ref2va
name: DEV027 REF2VA
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_images:
    type: images
    label: Reference Images
    required: true
    min_items: 3
    max_items: 3
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: reference_images
    target:
      node: "10"
      input: image
outputs:
  - id: generated_video
    type: video
    node: "9"
    required: true
"#;

    async fn insert_definition(
        pool: &sqlx::SqlitePool,
        workflow_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
        mode: &str,
        recipe_yaml: &str,
    ) {
        let now = "2026-08-17T00:00:00Z";
        sqlx::query(
            "INSERT INTO workflows
             (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(workflow_id)
        .bind(workflow_id)
        .bind(mode)
        .bind(mode)
        .bind(workflow_version_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("DEV027 workflow should insert");
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES (?, ?, '1', ?, 'dev027-sha', ?)",
        )
        .bind(workflow_version_id)
        .bind(workflow_id)
        .bind(WORKFLOW_JSON)
        .bind(now)
        .execute(pool)
        .await
        .expect("DEV027 workflow version should insert");
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES (?, ?, '1', 1, ?, 'dev027-sha', ?)",
        )
        .bind(recipe_id)
        .bind(workflow_version_id)
        .bind(recipe_yaml)
        .bind(now)
        .execute(pool)
        .await
        .expect("DEV027 recipe should insert");
    }

    async fn simulate_successful_item(
        pool: &sqlx::SqlitePool,
        queue: &SqliteProductionQueueRepository,
        task_repository: &SqliteTaskRepository,
        asset_repository: &SqliteAssetRepository,
        item: &ProductionBatchItem,
        stage: ShotStage,
        workflow_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> String {
        let now = Utc::now();
        assert!(queue
            .set_item_dispatching(&item.id, now)
            .await
            .expect("simulated item should enter dispatching"));

        let task = Task::new(PROJECT_ID, workflow_id, workflow_version_id, recipe_id, now);
        task_repository
            .create(&task, &task.created_event())
            .await
            .expect("simulated task should be created");
        sqlx::query(
            "UPDATE tasks SET status = 'SUCCEEDED', queued_at = ?, started_at = ?, finished_at = ? WHERE id = ?",
        )
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(task.id.as_str())
        .execute(pool)
        .await
        .expect("simulated task should finish");
        assert!(queue
            .bind_shot_item_task(item.id.as_str(), task.id.as_str(), now)
            .await
            .expect("simulated task should bind to Shot item"));

        let asset_id = AssetId::new();
        let asset = match stage {
            ShotStage::Image => Asset::new_generated_image(
                asset_id.clone(),
                PROJECT_ID,
                format!("DEV027 image {}", item.ordinal + 1),
                "dev027-image.png",
                format!("assets/{}/image.png", asset_id.as_str()),
                format!("dev027-image-{}", item.ordinal),
                "image/png",
                512,
                512,
                1,
                task.id.clone(),
                json!({"simulated": true, "stage": "image"}),
                now,
            )
            .expect("simulated image should be valid"),
            ShotStage::Video => Asset::new_generated_video(
                asset_id.clone(),
                PROJECT_ID,
                format!("DEV027 video {}", item.ordinal + 1),
                "dev027-video.mp4",
                format!("assets/{}/video.mp4", asset_id.as_str()),
                format!("dev027-video-{}", item.ordinal),
                "video/mp4",
                Some(512),
                Some(512),
                Some(1000),
                1,
                task.id.clone(),
                json!({"simulated": true, "stage": "video"}),
                now,
            )
            .expect("simulated video should be valid"),
        };
        asset_repository
            .insert_many(&[asset])
            .await
            .expect("simulated output should persist");
        assert!(queue
            .finish_item(
                &item.id,
                crate::domain::ProductionBatchItemStatus::Succeeded,
                None,
                None,
                now,
            )
            .await
            .expect("simulated item should finish"));
        asset_id.as_str().to_owned()
    }

    async fn wait_for_live_batch(
        queue_repository: &SqliteProductionQueueRepository,
        task_repository: &SqliteTaskRepository,
        project_id: &str,
        batch_id: &ProductionBatchId,
    ) -> Task {
        for _ in 0..900 {
            let detail = queue_repository
                .find_detail(project_id, batch_id)
                .await
                .expect("DEV027 live batch should remain readable")
                .expect("DEV027 live batch should exist");
            let item = detail
                .items
                .first()
                .expect("DEV027 live batch should contain one item");
            if matches!(
                item.status,
                ProductionBatchItemStatus::Failed | ProductionBatchItemStatus::Cancelled
            ) {
                panic!(
                    "DEV027 live batch item failed: status={} code={:?} message={:?}",
                    item.status.as_str(),
                    item.error_code,
                    item.error_message
                );
            }
            if let Some(task_id) = item.task_id.as_deref() {
                let task_id = TaskId::parse(task_id.to_owned())
                    .expect("DEV027 live batch task id should be valid");
                if let Some(task) = task_repository
                    .find_by_id(&task_id)
                    .await
                    .expect("DEV027 live task should remain readable")
                {
                    if task.status == TaskStatus::Failed || task.status == TaskStatus::Cancelled {
                        panic!(
                            "DEV027 live task failed: status={} error={:?}",
                            task.status.as_str(),
                            task.error
                        );
                    }
                    if task.status == TaskStatus::Succeeded
                        && item.status == ProductionBatchItemStatus::Succeeded
                        && detail.batch.status == ProductionBatchStatus::Completed
                    {
                        return task;
                    }
                }
            }
            sleep(Duration::from_secs(1)).await;
        }
        panic!(
            "DEV027 live batch did not complete within 900 seconds: {}",
            batch_id.as_str()
        );
    }

    #[tokio::test]
    async fn dev027_project_production_pipeline_six_shot_no_gpu_e2e() {
        let directory = tempdir().expect("DEV027 temporary directory should exist");
        let database_path = directory.path().join("dev027-e2e.db");
        let pool = initialize(&database_path)
            .await
            .expect("DEV027 database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET id = 'prj_default' WHERE id = 'project-1'")
            .execute(&pool)
            .await
            .expect("DEV027 test project should use production id format");

        insert_definition(
            &pool,
            "wfl_kera2_t2i_local_v2",
            "wfv-dev027-kera2",
            "rcp-dev027-kera2",
            "image",
            KERA2_RECIPE_YAML,
        )
        .await;
        insert_definition(
            &pool,
            "wfl_minimax_h3_fl2va_i2v_quality",
            "wfv-dev027-i2v",
            "rcp-dev027-i2v",
            "video",
            I2V_RECIPE_YAML,
        )
        .await;
        insert_definition(
            &pool,
            "wfl_minimax_h3_reference_video_quality",
            "wfv-dev027-ref2va",
            "rcp-dev027-ref2va",
            "video",
            REF2VA_RECIPE_YAML,
        )
        .await;

        let shot_repository = Arc::new(SqliteShotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let prompt_repository = Arc::new(SqlitePromptLibraryRepository::new(pool.clone()));
        let bulk_service = ShotBulkService::new(
            shot_repository.clone(),
            definition_repository.clone(),
            prompt_repository.clone(),
            Arc::new(SystemClock),
        );
        let imported = bulk_service
            .commit_import(&ShotBulkImportRequest {
                project_id: PROJECT_ID.to_owned(),
                format: ShotBulkInputFormat::Json,
                contents: json!({
                    "schemaVersion": 1,
                    "shots": (1..=6).map(|index| json!({
                        "name": format!("DEV027 Shot {index}"),
                        "description": format!("镜头 {index} 的叙事描述"),
                        "imagePrompt": format!("image prompt {index}"),
                        "videoPrompt": format!("video prompt {index}"),
                    })).collect::<Vec<Value>>(),
                })
                .to_string(),
            })
            .await
            .expect("DEV027 bulk import should commit atomically");
        assert_eq!(imported.created.len(), 6);
        let shot_ids = imported
            .created
            .iter()
            .map(|shot| shot.shot_id.clone())
            .collect::<Vec<_>>();

        bulk_service
            .set_stage_config(BulkStageConfigRequest {
                project_id: PROJECT_ID.to_owned(),
                stage: ShotStage::Image,
                shot_ids: shot_ids.clone(),
                workflow_version_id: "wfv-dev027-kera2".to_owned(),
                recipe_id: "rcp-dev027-kera2".to_owned(),
                values: BTreeMap::new(),
                prompt: None,
            })
            .await
            .expect("DEV027 image stage config should commit atomically");
        bulk_service
            .set_stage_config(BulkStageConfigRequest {
                project_id: PROJECT_ID.to_owned(),
                stage: ShotStage::Video,
                shot_ids: shot_ids[..3].to_vec(),
                workflow_version_id: "wfv-dev027-i2v".to_owned(),
                recipe_id: "rcp-dev027-i2v".to_owned(),
                values: BTreeMap::new(),
                prompt: None,
            })
            .await
            .expect("DEV027 I2V config should commit atomically");
        bulk_service
            .set_stage_config(BulkStageConfigRequest {
                project_id: PROJECT_ID.to_owned(),
                stage: ShotStage::Video,
                shot_ids: shot_ids[3..].to_vec(),
                workflow_version_id: "wfv-dev027-ref2va".to_owned(),
                recipe_id: "rcp-dev027-ref2va".to_owned(),
                values: BTreeMap::new(),
                prompt: None,
            })
            .await
            .expect("DEV027 REF2VA config should commit atomically");

        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let clock = Arc::new(SystemClock);
        let task_query_service = Arc::new(TaskQueryService::new(
            task_repository.clone(),
            asset_repository.clone(),
            definition_repository.clone(),
        ));
        let comfy_adapter = ComfyHttpAdapterFactory
            .create(ComfyConnectionConfig::default())
            .expect("DEV027 should construct the normal Comfy adapter without using it");
        let generation_service = Arc::new(GenerationService::new(
            task_repository.clone(),
            Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone())),
            definition_repository.clone(),
            comfy_adapter,
            project_repository.clone(),
            Arc::new(FileSystemAssetStore::new()),
            asset_repository.clone(),
            clock.clone(),
        ));
        let shot_service = ShotService::new(
            shot_repository.clone(),
            task_repository.clone(),
            asset_repository.clone(),
            definition_repository.clone(),
            prompt_repository.clone(),
            task_query_service,
            generation_service.clone(),
            queue_repository.clone(),
            clock.clone(),
        )
        .with_stage_prompt_repository(shot_repository.clone());
        let batch_service = ShotBatchService::new(
            shot_repository.clone(),
            queue_repository.clone(),
            task_repository.clone(),
            asset_repository.clone(),
            definition_repository.clone(),
            project_repository.clone(),
            clock.clone(),
        )
        .with_stage_prompt_repository(shot_repository.clone());

        shot_service
            .update(ShotUpdateRequest {
                project_id: PROJECT_ID.to_owned(),
                shot_id: shot_ids[0].clone(),
                name: "DEV027 Shot 1".to_owned(),
                prompt_text: "编辑后的镜头叙事，不应改写阶段 Prompt".to_owned(),
                prompt_entry_id: None,
                prompt_version_id: None,
            })
            .await
            .expect("editing the legacy narrative should succeed");
        let preserved = shot_repository
            .list_bulk_data(PROJECT_ID)
            .await
            .expect("stage prompts should remain readable")
            .into_iter()
            .find(|item| item.shot.id == shot_ids[0])
            .expect("edited Shot should remain present");
        assert_eq!(
            preserved
                .stage_prompts
                .iter()
                .find(|prompt| prompt.stage == ShotStage::Image)
                .map(|prompt| prompt.prompt_text.as_str()),
            Some("image prompt 1")
        );
        assert_eq!(
            preserved
                .stage_prompts
                .iter()
                .find(|prompt| prompt.stage == ShotStage::Video)
                .map(|prompt| prompt.prompt_text.as_str()),
            Some("video prompt 1")
        );

        let image_batch = batch_service
            .create(CreateShotBatchRequest {
                project_id: PROJECT_ID.to_owned(),
                stage: ShotStage::Image,
                shot_ids: shot_ids.clone(),
            })
            .await
            .expect("DEV027 image batch should be created");
        assert_eq!(image_batch.items.len(), 6);
        assert_eq!(
            image_batch.items[0].values_json["prompt"]["value"],
            json!("image prompt 1")
        );

        let mut image_asset_ids = Vec::with_capacity(6);
        for item in &image_batch.items {
            let asset_id = simulate_successful_item(
                &pool,
                &queue_repository,
                &task_repository,
                &asset_repository,
                item,
                ShotStage::Image,
                "wfl_kera2_t2i_local_v2",
                "wfv-dev027-kera2",
                "rcp-dev027-kera2",
            )
            .await;
            let shot_id = &shot_ids[item.ordinal as usize];
            shot_service
                .select_result(PROJECT_ID, shot_id, ShotStage::Image, &asset_id, true)
                .await
                .expect("DEV027 image result should be manually selectable");
            image_asset_ids.push(asset_id);
        }
        assert_eq!(image_asset_ids.len(), 6);

        for (index, shot_id) in shot_ids[3..].iter().enumerate() {
            let references = match index {
                0 => image_asset_ids[..3].to_vec(),
                1 => image_asset_ids[1..4].to_vec(),
                _ => image_asset_ids[3..6].to_vec(),
            };
            shot_service
                .replace_references(PROJECT_ID, shot_id, ShotStage::Video, references.clone())
                .await
                .expect("DEV027 REF2VA ordered references should persist");
            let stored = shot_repository
                .find(PROJECT_ID, shot_id)
                .await
                .expect("DEV027 REF2VA shot should load")
                .expect("DEV027 REF2VA shot should exist");
            let stored = stored
                .reference_assets
                .iter()
                .filter(|reference| reference.stage == ShotStage::Video)
                .map(|reference| (reference.ordinal, reference.asset_id.clone()))
                .collect::<Vec<_>>();
            assert_eq!(
                stored,
                references
                    .iter()
                    .enumerate()
                    .map(|(ordinal, asset_id)| (ordinal as i64, asset_id.clone()))
                    .collect::<Vec<_>>()
            );
        }

        let i2v_batch = batch_service
            .create(CreateShotBatchRequest {
                project_id: PROJECT_ID.to_owned(),
                stage: ShotStage::Video,
                shot_ids: shot_ids[..3].to_vec(),
            })
            .await
            .expect("DEV027 I2V batch should be created");
        assert_eq!(i2v_batch.items.len(), 3);
        assert_eq!(
            i2v_batch.items[0].values_json["prompt"]["value"],
            json!("video prompt 1")
        );
        assert!(i2v_batch.items[0].values_json["first_frame"]["assetId"]
            .as_str()
            .is_some());
        for item in &i2v_batch.items {
            let asset_id = simulate_successful_item(
                &pool,
                &queue_repository,
                &task_repository,
                &asset_repository,
                item,
                ShotStage::Video,
                "wfl_minimax_h3_fl2va_i2v_quality",
                "wfv-dev027-i2v",
                "rcp-dev027-i2v",
            )
            .await;
            shot_service
                .select_result(
                    PROJECT_ID,
                    &shot_ids[item.ordinal as usize],
                    ShotStage::Video,
                    &asset_id,
                    true,
                )
                .await
                .expect("DEV027 I2V result should be manually selectable");
        }

        let ref2va_batch = batch_service
            .create(CreateShotBatchRequest {
                project_id: PROJECT_ID.to_owned(),
                stage: ShotStage::Video,
                shot_ids: shot_ids[3..].to_vec(),
            })
            .await
            .expect("DEV027 REF2VA batch should be created");
        assert_eq!(ref2va_batch.items.len(), 3);
        assert_eq!(
            ref2va_batch.items[0].values_json["prompt"]["value"],
            json!("video prompt 4")
        );
        assert_eq!(
            ref2va_batch.items[0].values_json["reference_images"]["assetIds"],
            json!([image_asset_ids[0], image_asset_ids[1], image_asset_ids[2]])
        );
        for item in &ref2va_batch.items {
            let asset_id = simulate_successful_item(
                &pool,
                &queue_repository,
                &task_repository,
                &asset_repository,
                item,
                ShotStage::Video,
                "wfl_minimax_h3_reference_video_quality",
                "wfv-dev027-ref2va",
                "rcp-dev027-ref2va",
            )
            .await;
            shot_service
                .select_result(
                    PROJECT_ID,
                    &shot_ids[item.ordinal as usize + 3],
                    ShotStage::Video,
                    &asset_id,
                    true,
                )
                .await
                .expect("DEV027 REF2VA result should be manually selectable");
        }

        let completed = shot_service
            .list(PROJECT_ID)
            .await
            .expect("DEV027 shots should be readable after production");
        assert_eq!(completed.len(), 6);
        assert_eq!(
            completed
                .iter()
                .filter(|shot| shot.status == "COMPLETED")
                .count(),
            6
        );
        assert!(completed.iter().all(|shot| {
            shot.selected_image_asset_id.is_some() && shot.selected_video_asset_id.is_some()
        }));

        drop(shot_service);
        drop(batch_service);
        drop(bulk_service);
        drop(generation_service);
        drop(queue_repository);
        drop(task_repository);
        drop(asset_repository);
        drop(shot_repository);
        drop(definition_repository);
        drop(prompt_repository);
        drop(project_repository);
        pool.close().await;

        let restarted_pool = initialize(&database_path)
            .await
            .expect("DEV027 database should restart");
        let restarted_repository = SqliteShotRepository::new(restarted_pool.clone());
        let restarted = restarted_repository
            .list(PROJECT_ID)
            .await
            .expect("DEV027 shots should survive restart");
        assert_eq!(restarted.len(), 6);
        assert!(restarted.iter().all(|item| {
            item.shot.selected_image_asset_id.is_some()
                && item.shot.selected_video_asset_id.is_some()
        }));
        restarted_pool.close().await;
    }

    #[tokio::test]
    #[ignore = "requires the configured local ComfyUI runtime and mutates the supplied live database"]
    async fn dev027_live_representative_bulk_import_image_select_i2v() {
        assert_eq!(
            env::var("AI_STUDIO_LIVE_IN_PLACE").as_deref(),
            Ok("1"),
            "set AI_STUDIO_LIVE_IN_PLACE=1 to acknowledge the in-place live validation"
        );
        let database_path = PathBuf::from(
            env::var_os("AI_STUDIO_LIVE_DB_SOURCE")
                .expect("AI_STUDIO_LIVE_DB_SOURCE must point to the real AI Studio database"),
        );
        assert!(
            database_path.is_file(),
            "live database does not exist: {}",
            database_path.display()
        );

        let pool = initialize(&database_path)
            .await
            .expect("DEV027 live database should initialize and apply migration 019");
        let project_id = format!("prj_{}", Uuid::new_v4());
        let project_root = database_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("projects")
            .join(&project_id);
        std::fs::create_dir_all(&project_root).expect("DEV027 live project root should be created");

        let now = Utc::now();
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
        project_repository
            .insert(&ProjectRecord {
                id: project_id.clone(),
                name: "DEV027 Live Representative".to_owned(),
                description: Some(
                    "DEV027 backend live gate: bulk import -> image -> manual select -> I2V"
                        .to_owned(),
                ),
                root_path: project_root,
                created_at: now,
                updated_at: now,
            })
            .await
            .expect("DEV027 live project should be persisted");

        let shot_repository = Arc::new(SqliteShotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let definitions = definition_repository
            .list_available()
            .await
            .expect("DEV027 live definitions should be readable");
        let image_definition = definitions
            .iter()
            .find(|definition| definition.workflow_id == "wfl_kera2_t2i_local_v2")
            .expect("DEV027 live Krea2 definition should be available");
        let i2v_definition = definitions
            .iter()
            .find(|definition| definition.workflow_id == "wfl_minimax_h3_fl2va_i2v_quality")
            .expect("DEV027 live I2V definition should be available");

        let prompt_repository = Arc::new(SqlitePromptLibraryRepository::new(pool.clone()));
        let bulk_service = ShotBulkService::new(
            shot_repository.clone(),
            definition_repository.clone(),
            prompt_repository.clone(),
            Arc::new(SystemClock),
        );
        let imported = bulk_service
            .commit_import(&ShotBulkImportRequest {
                project_id: project_id.clone(),
                format: ShotBulkInputFormat::Json,
                contents: json!({
                    "schemaVersion": 1,
                    "shots": [{
                        "name": "DEV027 Live Shot 1",
                        "description": "单镜头真实生产验证",
                        "imagePrompt": "A cinematic still of a red kite flying over a quiet winter lake, soft morning light, detailed composition",
                        "videoPrompt": "The red kite glides slowly across the winter sky while the camera makes a gentle forward push, stable cinematic motion",
                    }],
                })
                .to_string(),
            })
            .await
            .expect("DEV027 live bulk import should commit");
        assert_eq!(imported.created.len(), 1);
        let shot_id = imported.created[0].shot_id.clone();

        bulk_service
            .set_stage_config(BulkStageConfigRequest {
                project_id: project_id.clone(),
                stage: ShotStage::Image,
                shot_ids: vec![shot_id.clone()],
                workflow_version_id: image_definition.workflow_version_id.clone(),
                recipe_id: image_definition.recipe_id.clone(),
                values: BTreeMap::new(),
                prompt: None,
            })
            .await
            .expect("DEV027 live image stage config should commit");
        bulk_service
            .set_stage_config(BulkStageConfigRequest {
                project_id: project_id.clone(),
                stage: ShotStage::Video,
                shot_ids: vec![shot_id.clone()],
                workflow_version_id: i2v_definition.workflow_version_id.clone(),
                recipe_id: i2v_definition.recipe_id.clone(),
                values: BTreeMap::new(),
                prompt: None,
            })
            .await
            .expect("DEV027 live I2V stage config should commit");

        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let clock = Arc::new(SystemClock);
        let task_query_service = Arc::new(TaskQueryService::new(
            task_repository.clone(),
            asset_repository.clone(),
            definition_repository.clone(),
        ));
        let asset_store = Arc::new(FileSystemAssetStore::new());
        let comfy_config = ComfyConnectionConfig::default();
        let comfy_endpoint = comfy_config.endpoint();
        let comfy_adapter = ComfyHttpAdapterFactory
            .create(comfy_config)
            .expect("DEV027 live Comfy adapter should construct");
        let health = comfy_adapter
            .health_check()
            .await
            .expect("DEV027 live ComfyUI health check should pass");
        eprintln!(
            "DEV027_LIVE_COMFY endpoint={} version={:?} python={:?} devices={:?}",
            comfy_endpoint,
            health.system.comfyui_version,
            health.system.python_version,
            health.system.devices
        );
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
            asset_store,
            clock.clone(),
            Arc::new(NoopTaskUpdateSink),
        ));
        let queue_service = Arc::new(ProductionQueueService::new(
            queue_repository.clone(),
            task_repository.clone(),
            definition_repository.clone(),
            generation_service.clone(),
            queue_repository.clone(),
            task_recovery_service,
            clock.clone(),
        ));
        let shot_service = ShotService::new(
            shot_repository.clone(),
            task_repository.clone(),
            asset_repository.clone(),
            definition_repository.clone(),
            prompt_repository.clone(),
            task_query_service,
            generation_service.clone(),
            queue_repository.clone(),
            clock.clone(),
        )
        .with_stage_prompt_repository(shot_repository.clone());
        let batch_service = ShotBatchService::new(
            shot_repository.clone(),
            queue_repository.clone(),
            task_repository.clone(),
            asset_repository.clone(),
            definition_repository.clone(),
            project_repository.clone(),
            clock.clone(),
        )
        .with_stage_prompt_repository(shot_repository.clone());

        let image_batch = batch_service
            .create(CreateShotBatchRequest {
                project_id: project_id.clone(),
                stage: ShotStage::Image,
                shot_ids: vec![shot_id.clone()],
            })
            .await
            .expect("DEV027 live image batch should be created");
        assert_eq!(image_batch.items.len(), 1);
        let image_batch_id = image_batch.batch.id.clone();
        queue_service
            .start_for_test(&project_id, image_batch_id.as_str())
            .await
            .expect("DEV027 live image batch should start through ProductionQueueService");
        let image_task = wait_for_live_batch(
            &queue_repository,
            &task_repository,
            &project_id,
            &image_batch_id,
        )
        .await;
        let image_asset = asset_repository
            .list_by_source_task(&image_task.id)
            .await
            .expect("DEV027 live image outputs should be readable")
            .into_iter()
            .find(|asset| asset.asset_type == AssetType::Image)
            .expect("DEV027 live image task should produce an image asset");
        let image_asset_id = image_asset.id.as_str().to_owned();
        shot_service
            .select_result(
                &project_id,
                &shot_id,
                ShotStage::Image,
                &image_asset_id,
                true,
            )
            .await
            .expect("DEV027 live image should be manually selectable");

        let video_batch = batch_service
            .create(CreateShotBatchRequest {
                project_id: project_id.clone(),
                stage: ShotStage::Video,
                shot_ids: vec![shot_id.clone()],
            })
            .await
            .expect("DEV027 live I2V batch should be created");
        assert_eq!(video_batch.items.len(), 1);
        assert_eq!(
            video_batch.items[0].values_json["prompt"]["value"],
            json!("The red kite glides slowly across the winter sky while the camera makes a gentle forward push, stable cinematic motion")
        );
        assert_eq!(
            video_batch.items[0].values_json["first_frame"]["assetId"],
            json!(image_asset_id)
        );
        let video_batch_id = video_batch.batch.id.clone();
        queue_service
            .start_for_test(&project_id, video_batch_id.as_str())
            .await
            .expect("DEV027 live I2V batch should start through ProductionQueueService");
        let video_task = wait_for_live_batch(
            &queue_repository,
            &task_repository,
            &project_id,
            &video_batch_id,
        )
        .await;
        let video_asset = asset_repository
            .list_by_source_task(&video_task.id)
            .await
            .expect("DEV027 live video outputs should be readable")
            .into_iter()
            .find(|asset| asset.asset_type == AssetType::Video)
            .expect("DEV027 live I2V task should produce a video asset");
        let video_asset_id = video_asset.id.as_str().to_owned();
        let completed = shot_service
            .select_result(
                &project_id,
                &shot_id,
                ShotStage::Video,
                &video_asset_id,
                true,
            )
            .await
            .expect("DEV027 live video should be manually selectable");
        assert_eq!(completed.status, "COMPLETED");
        assert_eq!(
            completed.selected_image_asset_id.as_deref(),
            Some(image_asset_id.as_str())
        );
        assert_eq!(
            completed.selected_video_asset_id.as_deref(),
            Some(video_asset_id.as_str())
        );

        eprintln!(
            "DEV027_LIVE_RESULT project={} shot={} image_batch={} image_task={} image_asset={} video_batch={} video_task={} video_asset={} image_workflow={}/{} image_recipe={} video_workflow={}/{} video_recipe={} endpoint={}",
            project_id,
            shot_id,
            image_batch_id.as_str(),
            image_task.id.as_str(),
            image_asset_id,
            video_batch_id.as_str(),
            video_task.id.as_str(),
            video_asset_id,
            image_definition.workflow_id,
            image_definition.workflow_version_id,
            image_definition.recipe_id,
            i2v_definition.workflow_id,
            i2v_definition.workflow_version_id,
            i2v_definition.recipe_id,
            comfy_endpoint
        );

        drop(shot_service);
        drop(batch_service);
        drop(queue_service);
        drop(generation_service);
        drop(queue_repository);
        drop(task_repository);
        drop(asset_repository);
        drop(shot_repository);
        drop(definition_repository);
        drop(prompt_repository);
        drop(project_repository);
        pool.close().await;
    }
}
