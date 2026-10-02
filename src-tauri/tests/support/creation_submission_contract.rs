use super::creation_context_contract::creation_context_services;
use super::*;
use ai_studio_lib::application::product::error::ProductError;
use ai_studio_lib::domain::{Asset, AssetId, TaskId};

fn request(
    shot: &str,
    values: BTreeMap<String, GenerationInputValue>,
    key: &str,
) -> CreationSubmission {
    CreationSubmission {
        project_id: PROJECT_ID.into(),
        shot_id: shot.into(),
        stage: "video".into(),
        selection_ref: selection(),
        values,
        submission_idempotency_key: key.into(),
    }
}

fn schema_comfy() -> Arc<ControlledComfy> {
    let mut comfy = ControlledComfy::new(ComfyBehavior::Offline);
    comfy.object_info = Some(json!({
        "CLIPTextEncode": {"input":{"required":{"text":["STRING",{}]}},"output":["CONDITIONING"]},
        "LoadVideo": {"input":{"required":{"video":["STRING",{}]}},"output":["VIDEO"]},
        "SaveVideo": {"input":{"required":{"video":["VIDEO",{}]}},"output":[],"output_node":true}
    }));
    Arc::new(comfy)
}

async fn video_recipe(pool: &SqlitePool) {
    let recipe = RECIPE_YAML
        .replace("first_frame", "reference_video")
        .replace("type: image", "type: video")
        .replace("label: First frame", "label: Reference video")
        .replace("input: image", "input: video");
    sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = ?")
        .bind(recipe)
        .bind(RECIPE_ID)
        .execute(pool)
        .await
        .unwrap();
    let workflow = json!({"6":{"class_type":"CLIPTextEncode","inputs":{"text":""}},
        "10":{"class_type":"LoadVideo","inputs":{"video":""}},"11":{"class_type":"SaveVideo","inputs":{"video":["10",0]}}});
    sqlx::query("UPDATE workflow_versions SET api_workflow_json = ? WHERE id = ?")
        .bind(workflow.to_string())
        .bind(WORKFLOW_VERSION_ID)
        .execute(pool)
        .await
        .unwrap();
}

async fn source_video(pool: &SqlitePool, root: &Path, project: &str) -> AssetId {
    let id = AssetId::new();
    let name = format!("{}.mp4", id.as_str());
    tokio::fs::write(root.join(&name), b"fixture-video-bytes")
        .await
        .unwrap();
    let asset = Asset::new_source_video(
        id.clone(),
        project,
        "reference",
        &name,
        &name,
        "fixture-sha",
        "video/mp4",
        Some(16),
        Some(16),
        Some(1000),
        19,
        json!({}),
        Utc::now(),
    )
    .unwrap();
    SqliteAssetRepository::new(pool.clone())
        .insert_many(&[asset])
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn product_creation_submission_shared_validation_is_read_only_and_scoped() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("submission.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    video_recipe(&pool).await;
    let services = build_services(&pool, schema_comfy(), &dir.path().join("packages"));
    let (facade, _) = creation_context_services(&pool);
    let shot = facade.create_shot(PROJECT_ID).await.unwrap();
    let asset = source_video(&pool, dir.path(), PROJECT_ID).await;
    let values = BTreeMap::from([
        (
            "prompt".into(),
            GenerationInputValue::Text("draft prompt".into()),
        ),
        (
            "reference_video".into(),
            GenerationInputValue::VideoAsset(asset.clone()),
        ),
    ]);
    let readiness = facade
        .readiness_get(&services.queue, request(&shot.id, values.clone(), "ready"))
        .await;
    assert!(readiness.ready, "{:?}", readiness.issues);
    for table in ["tasks", "batches", "items"] {
        assert_eq!(count(&pool, table).await, 0);
    }
    let links: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shot_generation_links")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(links, 0);
    let refs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shot_reference_assets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(refs, 0); // Video input never enters persistent image references.
    let assets_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(assets_before, 1);
    let mut wrong = values.clone();
    wrong.insert(
        "reference_video".into(),
        GenerationInputValue::ImageAsset(asset.clone()),
    );
    let bad = facade
        .readiness_get(&services.queue, request(&shot.id, wrong.clone(), "bad"))
        .await;
    assert!(!bad.ready);
    assert_eq!(
        bad.field_errors[0].details.field.as_deref(),
        Some("reference_video")
    );
    let error = facade
        .generate(
            &services.queue,
            request(&shot.id, wrong, "bad"),
            |_, _| async { panic!("invalid input must not reach start") },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, bad.issues[0].code);
    // Variant matches schema, but the actual asset type does not.
    let image = Asset::new_source_image(
        AssetId::new(),
        PROJECT_ID,
        "image",
        "x.png",
        "x.png",
        "sha",
        "image/png",
        16,
        16,
        100,
        json!({}),
        Utc::now(),
    )
    .unwrap();
    SqliteAssetRepository::new(pool.clone())
        .insert_many(&[image.clone()])
        .await
        .unwrap();
    let mut wrong_type = values.clone();
    wrong_type.insert(
        "reference_video".into(),
        GenerationInputValue::VideoAsset(image.id),
    );
    assert_eq!(
        facade
            .readiness_get(&services.queue, request(&shot.id, wrong_type, "type"))
            .await
            .issues[0]
            .code,
        "ASSET_TYPE_MISMATCH"
    );
    let other = "prj_11111111-1111-4111-8111-111111111111";
    SqliteProjectRepository::new(pool.clone())
        .ensure_default_project(other, "Other", &dir.path().join("other"), Utc::now())
        .await
        .unwrap();
    let foreign = source_video(&pool, dir.path(), other).await;
    let mut foreign_values = values;
    foreign_values.insert(
        "reference_video".into(),
        GenerationInputValue::VideoAsset(foreign),
    );
    assert_eq!(
        facade
            .readiness_get(
                &services.queue,
                request(&shot.id, foreign_values, "foreign")
            )
            .await
            .issues[0]
            .code,
        "ASSET_PROJECT_MISMATCH"
    );
    assert_eq!(count(&pool, "batches").await, 0);
    assert_eq!(count(&pool, "tasks").await, 0);
}

#[tokio::test]
async fn product_creation_submission_deduplicates_and_preserves_runref_after_start_failure() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("accepted.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    video_recipe(&pool).await;
    let services = build_services(&pool, schema_comfy(), &dir.path().join("packages"));
    let (facade, _) = creation_context_services(&pool);
    let shot = facade.create_shot(PROJECT_ID).await.unwrap();
    let asset = source_video(&pool, dir.path(), PROJECT_ID).await;
    let values = BTreeMap::from([
        (
            "prompt".into(),
            GenerationInputValue::Text("frozen prompt".into()),
        ),
        (
            "reference_video".into(),
            GenerationInputValue::VideoAsset(asset.clone()),
        ),
    ]);
    let failed_start = |_, _| async {
        Err(ProductError::new(
            "RUNTIME_BLOCKED",
            "fixture start blocked",
            Some("OPEN_RUN"),
        ))
    };
    let (first, second) = tokio::join!(
        facade.generate(
            &services.queue,
            request(&shot.id, values.clone(), "same-key"),
            failed_start
        ),
        facade.generate(
            &services.queue,
            request(&shot.id, values, "same-key"),
            failed_start
        )
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert!(first.accepted);
    assert_eq!(first.start_outcome, "FAILED_TO_START");
    assert!(first.start_issue.is_some());
    assert_eq!(first.run_ref, second.run_ref);
    assert_eq!(count(&pool, "batches").await, 1);
    assert_eq!(count(&pool, "items").await, 1);
    assert_eq!(count(&pool, "tasks").await, 0); // Accepted is not succeeded.
    services
        .queue
        .start_for_test(PROJECT_ID, &first.run_ref.id)
        .await
        .unwrap();
    let mut snapshot = None;
    for _ in 0..100 {
        let detail = services
            .queue
            .get(PROJECT_ID, &first.run_ref.id)
            .await
            .unwrap();
        if let Some(id) = detail.items[0].task_id.as_ref() {
            snapshot = SqliteGenerationSnapshotRepository::new(pool.clone())
                .find_by_task_id(&TaskId::parse(id.clone()).unwrap())
                .await
                .unwrap();
            if snapshot.is_some() {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let snapshot = snapshot.expect("existing queue creates immutable generation snapshot");
    assert_eq!(snapshot.user_inputs_json["prompt"], "frozen prompt");
    assert_eq!(
        snapshot.user_inputs_json["reference_video"]["assetId"],
        asset.as_str()
    );
    assert_eq!(count(&pool, "tasks").await, 1);
}
