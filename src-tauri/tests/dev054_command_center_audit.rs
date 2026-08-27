//! DEV-054 command-center and audit read-path contract tests.
//!
//! These tests use the real SQLite schema and public application services. The
//! read models must remain set-based, historical snapshots must remain the
//! source of audit truth, and neither read path may mutate production state.

use ai_studio_lib::application::{
    production_audit_service::ProductionAuditService,
    project_command_center_service::ProjectCommandCenterService,
};
use ai_studio_lib::infrastructure::database::initialize;
use serde_json::json;
use sqlx::SqlitePool;
use tempfile::{tempdir, TempDir};

const LEGACY_PROJECT: &str = "prj_default";
const CONSISTENCY_PROJECT: &str = "prj_550e8400-e29b-41d4-a716-446655440000";
const NOW: &str = "2026-08-27T00:00:00Z";

async fn fixture() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary database directory should exist");
    let pool = initialize(&directory.path().join("dev054.db"))
        .await
        .expect("database should migrate");

    for (id, name) in [
        (LEGACY_PROJECT, "Legacy project"),
        (CONSISTENCY_PROJECT, "Consistency project"),
    ] {
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(directory.path().join(id).to_string_lossy().to_string())
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("project should insert");
    }

    sqlx::query(
        "INSERT INTO workflows (id, name, category, mode, created_at, updated_at)
         VALUES ('wf-054', 'DEV-054 workflow', 'image', 'T2I', ?, ?)",
    )
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("workflow should insert");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES ('wv-054', 'wf-054', '1', '{}', 'workflow-sha', ?)",
    )
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("workflow version should insert");
    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
         VALUES ('recipe-054', 'wv-054', '1', 1, 'inputs: {}', 'recipe-sha', ?)",
    )
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("recipe should insert");

    sqlx::query(
        "INSERT INTO character_profiles
         (id, project_id, name, description, canonical_prompt, negative_prompt,
          metadata_json, created_at, updated_at)
         VALUES ('cp-054', ?, 'Hero', 'Current profile', 'current prompt', '', '{}', ?, ?)",
    )
    .bind(CONSISTENCY_PROJECT)
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("character profile should insert");
    sqlx::query(
        "INSERT INTO reference_sets
         (id, project_id, name, purpose, description, created_at, updated_at)
         VALUES ('rs-054', ?, 'Hero refs', 'CHARACTER', '', ?, ?)",
    )
    .bind(CONSISTENCY_PROJECT)
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("reference set should insert");
    sqlx::query(
        "INSERT INTO shots
         (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
         VALUES ('shot-054', ?, 0, 'Shot 054', 'legacy prompt', ?, ?)",
    )
    .bind(CONSISTENCY_PROJECT)
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("shot should insert");
    sqlx::query(
        "INSERT INTO shot_profile_bindings
         (id, shot_id, role, profile_type, profile_id, ordinal, inheritance_mode, created_at, updated_at)
         VALUES ('spb-054', 'shot-054', 'CHARACTER', 'CHARACTER', 'cp-054', 0, 'EXPLICIT', ?, ?)",
    )
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("shot profile binding should insert");
    sqlx::query(
        "INSERT INTO shot_reference_set_bindings
         (id, shot_id, role, reference_set_id, ordinal, required, inheritance_mode, created_at, updated_at)
         VALUES ('srb-054', 'shot-054', 'CHARACTER', 'rs-054', 0, 1, 'EXPLICIT', ?, ?)",
    )
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("shot reference binding should insert");
    sqlx::query(
        "INSERT INTO consistency_scope_profile_bindings
         (id, project_id, scope_type, scope_id, role, profile_type, profile_id, ordinal,
          inheritance_mode, created_at, updated_at)
         VALUES ('hpb-054', ?, 'PROJECT', ?, 'CHARACTER', 'CHARACTER', 'cp-054', 0, 'EXPLICIT', ?, ?)",
    )
    .bind(CONSISTENCY_PROJECT)
    .bind(CONSISTENCY_PROJECT)
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("scope profile binding should insert");
    sqlx::query(
        "INSERT INTO consistency_scope_reference_set_bindings
         (id, project_id, scope_type, scope_id, role, reference_set_id, ordinal, required,
          inheritance_mode, created_at, updated_at)
         VALUES ('hrb-054', ?, 'PROJECT', ?, 'CHARACTER', 'rs-054', 0, 1, 'EXPLICIT', ?, ?)",
    )
    .bind(CONSISTENCY_PROJECT)
    .bind(CONSISTENCY_PROJECT)
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("scope reference binding should insert");

    sqlx::query(
        "INSERT INTO tasks
         (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at, finished_at)
         VALUES ('task-054', ?, 'wf-054', 'wv-054', 'recipe-054', 'SUCCEEDED', ?, ?)",
    )
    .bind(CONSISTENCY_PROJECT)
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("task should insert");
    sqlx::query(
        "INSERT INTO production_batches
         (id, project_id, name, status, continue_on_failure, created_at, updated_at)
         VALUES ('batch-054', ?, 'Prepared batch', 'COMPLETED', 0, ?, ?)",
    )
    .bind(CONSISTENCY_PROJECT)
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("batch should insert");
    for (id, ordinal, status, task_id) in [
        ("item-054-image", 0_i64, "PENDING", Some("task-054")),
        ("item-054-video", 1_i64, "SUCCEEDED", None),
    ] {
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
              task_id, created_at, updated_at)
             VALUES (?, 'batch-054', ?, 'wv-054', 'recipe-054', '{}', ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(ordinal)
        .bind(status)
        .bind(task_id)
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("batch item should insert");
    }
    sqlx::query(
        "INSERT INTO shot_generation_links
         (id, shot_id, stage, task_id, production_batch_item_id, created_at)
         VALUES ('link-054', 'shot-054', 'image', 'task-054', 'item-054-image', ?)",
    )
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("shot generation link should insert");

    for (id, item_id, stage, hash, prompt) in [
        (
            "prep-054-image",
            "item-054-image",
            "image",
            "context-image-054",
            "historical image prompt",
        ),
        (
            "prep-054-video",
            "item-054-video",
            "video",
            "context-video-054",
            "historical video prompt",
        ),
    ] {
        let snapshot = json!({
            "schemaVersion": 1,
            "projectId": CONSISTENCY_PROJECT,
            "shotId": "shot-054",
            "stage": stage,
            "contextHash": hash,
            "prompt": {"renderedText": prompt, "negativePrompt": "historical negative"},
            "workflow": {"workflowVersionId": "wv-054", "recipeId": "recipe-054"},
            "referenceSets": [{"referenceSetId": "rs-054"}],
            "referenceAssets": [{"sha256": "historical-asset-sha"}]
        });
        sqlx::query(
            "INSERT INTO production_preparation_snapshots
             (id, project_id, shot_id, stage, context_hash, production_batch_id,
              production_batch_item_id, snapshot_json, created_at)
             VALUES (?, ?, 'shot-054', ?, ?, 'batch-054', ?, ?, ?)",
        )
        .bind(id)
        .bind(CONSISTENCY_PROJECT)
        .bind(stage)
        .bind(hash)
        .bind(item_id)
        .bind(snapshot.to_string())
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("preparation snapshot should insert");
    }

    (directory, pool)
}

#[tokio::test]
async fn command_center_distinguishes_legacy_and_consistency_projects() {
    let (_directory, pool) = fixture().await;
    let service = ProjectCommandCenterService::new(pool.clone());

    let legacy = service
        .get(LEGACY_PROJECT)
        .await
        .expect("legacy command center should load");
    assert!(!legacy.consistency.consistency_in_use);
    assert_eq!(legacy.consistency.character_profiles, 0);
    assert_eq!(legacy.preparation.snapshot_count, 0);
    assert!(!legacy.structure.blocked);

    let consistent = service
        .get(CONSISTENCY_PROJECT)
        .await
        .expect("consistency command center should load");
    assert!(consistent.consistency.consistency_in_use);
    assert_eq!(consistent.consistency.character_profiles, 1);
    assert_eq!(consistent.consistency.reference_sets, 1);
    assert_eq!(consistent.consistency.shot_profile_bindings, 1);
    assert_eq!(consistent.consistency.shot_reference_set_bindings, 1);
    assert_eq!(consistent.consistency.scope_profile_bindings, 1);
    assert_eq!(consistent.consistency.scope_reference_set_bindings, 1);
    assert_eq!(consistent.preparation.snapshot_count, 2);
    assert_eq!(consistent.preparation.prepared_image_items, 1);
    assert_eq!(consistent.preparation.prepared_video_items, 1);
    assert_eq!(consistent.preparation.active_prepared_items, 1);
    assert_eq!(
        consistent.preparation.latest_prepared_at.as_deref(),
        Some(NOW)
    );
    assert!(consistent
        .quick_actions
        .iter()
        .any(|action| action.id == "preparation" && action.destination == "shots"));
}

#[tokio::test]
async fn command_center_and_audit_reads_do_not_mutate_queue_state() {
    let (_directory, pool) = fixture().await;
    let before: (i64, i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT COUNT(*) FROM production_batches),
           (SELECT COUNT(*) FROM production_batch_items),
           (SELECT COUNT(*) FROM production_preparation_snapshots)",
    )
    .fetch_one(&pool)
    .await
    .expect("before counts should load");

    let command_center = ProjectCommandCenterService::new(pool.clone());
    let _ = command_center
        .get(CONSISTENCY_PROJECT)
        .await
        .expect("command center should load");
    let audit = ProductionAuditService::new(pool.clone());
    let _ = audit
        .project_summary(CONSISTENCY_PROJECT)
        .await
        .expect("audit summary should load");
    let _ = audit
        .recent_activity(CONSISTENCY_PROJECT, Some(200))
        .await
        .expect("audit activity should load");

    let after: (i64, i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT COUNT(*) FROM production_batches),
           (SELECT COUNT(*) FROM production_batch_items),
           (SELECT COUNT(*) FROM production_preparation_snapshots)",
    )
    .fetch_one(&pool)
    .await
    .expect("after counts should load");
    assert_eq!(after, before);
}

#[tokio::test]
async fn audit_exposes_preparation_lineage_activity_and_lazy_historical_detail() {
    let (_directory, pool) = fixture().await;
    let service = ProductionAuditService::new(pool.clone());

    let activity = service
        .recent_activity(CONSISTENCY_PROJECT, Some(200))
        .await
        .expect("activity should load");
    let preparation = activity
        .iter()
        .find(|entry| entry.kind == "PREPARATION_CREATED")
        .expect("preparation activity should be derived");
    assert_eq!(preparation.batch_id.as_deref(), Some("batch-054"));
    assert_eq!(preparation.item_id.as_deref(), Some("item-054-image"));
    assert_eq!(preparation.shot_id.as_deref(), Some("shot-054"));
    assert_eq!(preparation.snapshot_id.as_deref(), Some("prep-054-image"));
    assert!(preparation.detail.contains("image"));
    assert!(preparation.detail.contains("context-"));

    for (root_type, root_id) in [
        ("BATCH", "batch-054"),
        ("SHOT", "shot-054"),
        ("TASK", "task-054"),
    ] {
        let lineage = service
            .lineage(CONSISTENCY_PROJECT, root_type, root_id)
            .await
            .expect("lineage should load");
        let snapshot = lineage
            .nodes
            .iter()
            .find(|node| node.entity_type == "PREPARATION_SNAPSHOT")
            .expect("preparation snapshot should be in lineage");
        assert_eq!(snapshot.context_hash.as_deref(), Some("context-image-054"));
        assert_eq!(snapshot.snapshot_schema_version, Some(1));
        assert_eq!(snapshot.batch_id.as_deref(), Some("batch-054"));
        assert_eq!(snapshot.item_id.as_deref(), Some("item-054-image"));
        assert_eq!(snapshot.shot_id.as_deref(), Some("shot-054"));
    }

    let detail = service
        .snapshot_detail(CONSISTENCY_PROJECT, "item-054-image")
        .await
        .expect("snapshot detail should load")
        .expect("snapshot detail should exist");
    assert_eq!(detail.context_hash, "context-image-054");
    assert_eq!(detail.prompt, "historical image prompt");
    assert_eq!(detail.workflow_version_id.as_deref(), Some("wv-054"));
    assert_eq!(detail.recipe_id.as_deref(), Some("recipe-054"));
    assert_eq!(detail.reference_set_ids, vec!["rs-054"]);
    assert_eq!(detail.asset_checksums, vec!["historical-asset-sha"]);

    sqlx::query(
        "UPDATE character_profiles SET canonical_prompt = 'new current prompt' WHERE id = 'cp-054'",
    )
    .execute(&pool)
    .await
    .expect("current profile should update");
    let historical_again = service
        .snapshot_detail(CONSISTENCY_PROJECT, "item-054-image")
        .await
        .expect("historical detail should reload")
        .expect("historical detail should remain");
    assert_eq!(historical_again.prompt, "historical image prompt");

    let legacy_lineage = service
        .lineage(LEGACY_PROJECT, "BATCH", "missing-batch")
        .await;
    assert!(legacy_lineage.is_err());
    assert!(service
        .snapshot_detail(LEGACY_PROJECT, "missing-item")
        .await
        .expect("legacy snapshot lookup should be read-only")
        .is_none());
}
