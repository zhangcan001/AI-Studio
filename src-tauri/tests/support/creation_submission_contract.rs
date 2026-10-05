use super::creation_context_contract::creation_context_services;
use super::*;
use ai_studio_lib::application::product::error::ProductError;
use ai_studio_lib::application::prompt_library_service::PromptLibraryService;
use ai_studio_lib::domain::{Asset, AssetId, TaskId};

fn request(
    shot: &str,
    values: BTreeMap<String, GenerationInputValue>,
    key: &str,
) -> CreationSubmission {
    CreationSubmission {
        prompt_id: None,
        prompt_version_id: None,
        project_id: PROJECT_ID.into(),
        shot_id: shot.into(),
        stage: "video".into(),
        selection_ref: selection(),
        values,
        submission_idempotency_key: key.into(),
    }
}

fn prompt_service(pool: &SqlitePool) -> PromptLibraryService {
    PromptLibraryService::new(
        Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
        Arc::new(SystemClock),
    )
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
    let prompts = prompt_service(&pool);
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
        .readiness_get(
            &services.queue,
            &prompts,
            request(&shot.id, values.clone(), "ready"),
        )
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
        .readiness_get(
            &services.queue,
            &prompts,
            request(&shot.id, wrong.clone(), "bad"),
        )
        .await;
    assert!(!bad.ready);
    assert_eq!(
        bad.field_errors[0].details.field.as_deref(),
        Some("reference_video")
    );
    let error = facade
        .generate(
            &services.queue,
            &prompts,
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
            .readiness_get(
                &services.queue,
                &prompts,
                request(&shot.id, wrong_type, "type")
            )
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
                &prompts,
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
    let prompts = prompt_service(&pool);
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
    let prompt = prompts
        .create(PROJECT_ID, "prompt", "Owned provenance", &[], "v1")
        .await
        .unwrap();
    let version = prompts
        .add_version(PROJECT_ID, &prompt.id, "frozen prompt")
        .await
        .unwrap();
    let with_provenance = |values, key| {
        let mut r = request(&shot.id, values, key);
        r.prompt_id = Some(prompt.id.clone());
        r.prompt_version_id = Some(version.id.clone());
        r
    };
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
            &prompts,
            with_provenance(values.clone(), "same-key"),
            failed_start
        ),
        facade.generate(
            &services.queue,
            &prompts,
            with_provenance(values, "same-key"),
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
                                                // Owned fixture: represent an existing running batch without starting execution.
    sqlx::query("UPDATE production_batches SET status = 'RUNNING' WHERE id = ?")
        .bind(&first.run_ref.id)
        .execute(&pool)
        .await
        .unwrap();
    let busy = facade
        .readiness_get(
            &services.queue,
            &prompts,
            request(
                &shot.id,
                BTreeMap::from([
                    (
                        "prompt".into(),
                        GenerationInputValue::Text("recheck".into()),
                    ),
                    (
                        "reference_video".into(),
                        GenerationInputValue::VideoAsset(asset.clone()),
                    ),
                ]),
                "readiness-only",
            ),
        )
        .await;
    assert!(!busy.ready);
    assert_eq!(busy.issues[0].code, "RUNTIME_BLOCKED");
    assert_eq!(busy.issues[0].details.action, Some("TRY_LATER"));
    assert_eq!(count(&pool, "batches").await, 1);
    assert_eq!(count(&pool, "tasks").await, 0);
    sqlx::query("UPDATE production_batches SET status = 'READY' WHERE id = ?")
        .bind(&first.run_ref.id)
        .execute(&pool)
        .await
        .unwrap();
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
    assert_eq!(
        snapshot.prompt_version_id.as_deref(),
        Some(version.id.as_str())
    );
    assert!(snapshot.model_version_id.is_none());
    assert_eq!(snapshot.user_inputs_json["prompt"], "frozen prompt");
    assert_eq!(
        snapshot.user_inputs_json["reference_video"]["assetId"],
        asset.as_str()
    );
    assert_eq!(count(&pool, "tasks").await, 1);
}

#[tokio::test]
async fn product_creation_submission_prompt_provenance_rejects_foreign_wrong_modified_and_partial_without_writes(
) {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("provenance.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    video_recipe(&pool).await;
    let services = build_services(&pool, schema_comfy(), &dir.path().join("packages"));
    let (facade, _) = creation_context_services(&pool);
    let prompts = prompt_service(&pool);
    let shot = facade.create_shot(PROJECT_ID).await.unwrap();
    let asset = source_video(&pool, dir.path(), PROJECT_ID).await;
    let a = prompts
        .create(PROJECT_ID, "prompt", "Owned A", &[], "fixture prompt")
        .await
        .unwrap();
    let b = prompts
        .create(PROJECT_ID, "prompt", "Owned B", &[], "fixture prompt")
        .await
        .unwrap();
    let other = "prj_11111111-1111-4111-8111-111111111111";
    SqliteProjectRepository::new(pool.clone())
        .ensure_default_project(other, "Other", &dir.path().join("other"), Utc::now())
        .await
        .unwrap();
    let foreign = prompts
        .create(other, "prompt", "Foreign", &[], "fixture prompt")
        .await
        .unwrap();
    let values = BTreeMap::from([
        (
            "prompt".into(),
            GenerationInputValue::Text("fixture prompt".into()),
        ),
        (
            "reference_video".into(),
            GenerationInputValue::VideoAsset(asset),
        ),
    ]);
    for (prompt_id, version_id, text) in [
        (
            Some(foreign.id.clone()),
            Some(foreign.versions[0].id.clone()),
            "fixture prompt",
        ),
        (
            Some(a.id.clone()),
            Some(b.versions[0].id.clone()),
            "fixture prompt",
        ),
        (
            Some(a.id.clone()),
            Some(a.versions[0].id.clone()),
            "changed prompt",
        ),
        (Some(a.id.clone()), None, "fixture prompt"),
        (None, Some(a.versions[0].id.clone()), "fixture prompt"),
    ] {
        let build = || {
            let mut r = request(&shot.id, values.clone(), "invalid-provenance");
            r.prompt_id = prompt_id.clone();
            r.prompt_version_id = version_id.clone();
            r.values
                .insert("prompt".into(), GenerationInputValue::Text(text.into()));
            r
        };
        let ready = facade
            .readiness_get(&services.queue, &prompts, build())
            .await;
        assert!(!ready.ready);
        assert_eq!(ready.issues[0].code, "INVALID_INPUT");
        assert_eq!(ready.issues[0].details.field.as_deref(), Some("prompt"));
        let e = facade
            .generate(&services.queue, &prompts, build(), |_, _| async {
                panic!("invalid provenance must not start")
            })
            .await
            .unwrap_err();
        assert_eq!(e.code, ready.issues[0].code);
        for table in ["tasks", "batches", "items"] {
            assert_eq!(count(&pool, table).await, 0);
        }
    }
    assert!(
        facade
            .readiness_get(
                &services.queue,
                &prompts,
                request(&shot.id, values, "manual")
            )
            .await
            .ready
    );
}

#[tokio::test]
async fn product_creation_submission_recent_prompt_choices_keep_exact_identity_and_twenty_bound() {
    let dir = tempdir().unwrap();
    let pool = initialize(&dir.path().join("recent.db")).await.unwrap();
    seed_database(&pool, dir.path()).await;
    let (facade, assets) = creation_context_services(&pool);
    let prompts = prompt_service(&pool);
    let old = prompts
        .create(PROJECT_ID, "prompt", "Old21", &[], "old")
        .await
        .unwrap();
    for n in 0..20 {
        prompts
            .create(PROJECT_ID, "prompt", &format!("Recent{n}"), &[], "new")
            .await
            .unwrap();
    }
    let mut context = facade
        .get(&command_center(&pool), &assets, PROJECT_ID, None, "video")
        .await
        .unwrap();
    facade
        .project_prompt_choices(&mut context, &prompts)
        .await
        .unwrap();
    assert_eq!(context.prompt_choices.len(), 20);
    assert!(context
        .prompt_choices
        .iter()
        .all(|choice| choice.prompt_id != old.id));
    for choice in context.prompt_choices {
        let entry = prompts.get(PROJECT_ID, &choice.prompt_id).await.unwrap();
        let latest = entry.versions.iter().max_by_key(|v| v.version).unwrap();
        assert_eq!(choice.prompt_version_id, latest.id);
        assert_eq!(choice.text, latest.text);
        let wire = serde_json::to_value(choice).unwrap();
        assert!(wire["promptId"].is_string());
        assert!(wire["promptVersionId"].is_string());
    }
}
