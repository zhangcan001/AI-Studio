//! DEV-101 external-agent production handoff gates.

use ai_studio_lib::{
    application::{
        external_production_handoff_service::ExternalProductionHandoffService,
        ports::{
            ExternalProductionHandoffAssetReference, ExternalProductionHandoffEntityMapping,
            ExternalProductionHandoffEpisode, ExternalProductionHandoffImportPlan,
            ExternalProductionHandoffRecord, ExternalProductionHandoffRepository,
            ExternalProductionHandoffScene, ExternalProductionHandoffSceneAssignment,
            ExternalProductionHandoffSeries, ExternalProductionHandoffShot,
        },
        project_backup_service::ProjectBackupService,
    },
    infrastructure::{
        database::{
            initialize, SqliteAssetRepository, SqliteExternalProductionHandoffRepository,
            SqliteGenerationDefinitionRepository, SqliteProjectRepository,
        },
        time::SystemClock,
    },
};
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};

const PROJECT_A: &str = "prj_550e8400-e29b-41d4-a716-446655440000";
const PROJECT_B: &str = "prj_550e8400-e29b-41d4-a716-446655440001";
const CREATED_AT: &str = "2026-09-12T00:00:00Z";

struct Harness {
    _directory: TempDir,
    pool: SqlitePool,
    service: ExternalProductionHandoffService,
    handoff_repository: Arc<SqliteExternalProductionHandoffRepository>,
}

async fn harness() -> Harness {
    let directory = tempdir().unwrap();
    let pool = initialize(&directory.path().join("app.db")).await.unwrap();
    for project_id in [PROJECT_A, PROJECT_B] {
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES (?, ?, NULL, ?, ?, ?)",
        )
        .bind(project_id)
        .bind(project_id)
        .bind(
            directory
                .path()
                .join(project_id)
                .to_string_lossy()
                .to_string(),
        )
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&pool)
        .await
        .unwrap();
    }
    let handoff_repository = Arc::new(SqliteExternalProductionHandoffRepository::new(pool.clone()));
    let service = ExternalProductionHandoffService::new(
        handoff_repository.clone(),
        Arc::new(SqliteProjectRepository::new(pool.clone())),
        Arc::new(SqliteAssetRepository::new(pool.clone())),
        Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone())),
        Arc::new(SystemClock),
    );
    Harness {
        _directory: directory,
        pool,
        service,
        handoff_repository,
    }
}

fn document(
    project_id: &str,
    revision: Option<&str>,
    asset_id: Option<&str>,
    stage: Option<Value>,
) -> String {
    let mut shot = json!({
        "externalId": "shot-1",
        "name": "Gate opens",
        "ordinal": 1,
        "description": "The gate opens in the rain.",
        "imagePrompt": "cinematic rain",
        "videoPrompt": "camera pushes forward"
    });
    if let Some(asset_id) = asset_id {
        shot["assetRefs"] = json!([{ "assetId": asset_id }]);
    }
    if let Some(stage) = stage {
        shot["stages"] = stage;
    }
    json!({
        "schemaVersion": 1,
        "projectId": project_id,
        "source": { "agent": "dev101-agent", "revision": revision },
        "series": [{
            "externalId": "series-1",
            "name": "Opening",
            "description": "Opening",
            "ordinal": 1,
            "episodes": [{
                "externalId": "episode-1",
                "name": "Arrival",
                "description": "Arrival",
                "ordinal": 1,
                "scenes": [{
                    "externalId": "scene-1",
                    "name": "Gate",
                    "description": "Gate",
                    "ordinal": 1,
                    "shots": [shot]
                }]
            }]
        }]
    })
    .to_string()
}

async fn count(pool: &SqlitePool, table: &str, project_id: &str) -> i64 {
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM {table} WHERE project_id = ?"
    ))
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn total_count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_asset(pool: &SqlitePool, id: &str, project_id: &str) {
    sqlx::query(
        "INSERT INTO assets
         (id, project_id, type, category, name, original_name, storage_path,
          thumbnail_path, sha256, mime_type, width, height, duration_ms, file_size,
          source_task_id, metadata_json, created_at, updated_at)
         VALUES (?, ?, 'image', 'source_image', 'Reference', 'reference.png',
                 'reference.png', NULL, 'sha', 'image/png', 1, 1, NULL, 1,
                 NULL, '{}', ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_workflow(pool: &SqlitePool, archived: bool) {
    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at,
          source_kind, library_state, removed_at)
         VALUES ('wf_dev101', 'DEV101', 'test', 'test', 'wv_dev101', ?, ?,
                 'USER', 'ACTIVE', NULL)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES ('wv_dev101', 'wf_dev101', '1', '{}', 'workflow-sha', ?)",
    )
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml,
          recipe_sha256, created_at)
         VALUES ('recipe_dev101', 'wv_dev101', '1', 1, 'recipe', 'recipe-sha', ?)",
    )
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO workflow_recipe_runtime_states
         (workflow_version_id, recipe_id, archived, archived_at, updated_at)
         VALUES ('wv_dev101', 'recipe_dev101', ?, ?, ?)",
    )
    .bind(archived as i64)
    .bind(archived.then_some(CREATED_AT))
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn valid_preview_confirm_replay_and_side_effect_safety() {
    let harness = harness().await;
    let content = document(PROJECT_A, Some("revision-1"), None, None);
    let before = (
        count(&harness.pool, "production_series", PROJECT_A).await,
        total_count(&harness.pool, "production_episodes").await,
        total_count(&harness.pool, "production_scenes").await,
        count(&harness.pool, "shots", PROJECT_A).await,
        count(&harness.pool, "tasks", PROJECT_A).await,
        count(&harness.pool, "production_batches", PROJECT_A).await,
    );
    let preview = harness.service.preview(PROJECT_A, &content).await.unwrap();
    assert!(preview.errors.is_empty());
    assert_eq!(
        (
            preview.series_count,
            preview.episode_count,
            preview.scene_count,
            preview.shot_count
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(
        before.0,
        count(&harness.pool, "production_series", PROJECT_A).await
    );
    assert_eq!(before.3, count(&harness.pool, "shots", PROJECT_A).await);

    let first = harness
        .service
        .confirm(PROJECT_A, &content, &preview.document_sha256)
        .await
        .unwrap();
    assert!(!first.replayed);
    assert_eq!(first.mappings.len(), 4);
    assert_eq!(
        count(&harness.pool, "external_production_handoffs", PROJECT_A).await,
        1
    );
    assert_eq!(count(&harness.pool, "shots", PROJECT_A).await, 1);
    assert_eq!(count(&harness.pool, "tasks", PROJECT_A).await, before.4);
    assert_eq!(
        count(&harness.pool, "production_batches", PROJECT_A).await,
        before.5
    );

    let replay_preview = harness.service.preview(PROJECT_A, &content).await.unwrap();
    assert_eq!(replay_preview.replay.status, "ALREADY_IMPORTED");
    let replay = harness
        .service
        .confirm(PROJECT_A, &content, &preview.document_sha256)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.handoff_id, first.handoff_id);
    assert_eq!(
        count(&harness.pool, "external_production_handoffs", PROJECT_A).await,
        1
    );
    assert_eq!(count(&harness.pool, "shots", PROJECT_A).await, 1);

    let backup = ProjectBackupService::new(
        harness.pool.clone(),
        harness._directory.path().join("restored-projects"),
        harness._directory.path().join("backup-cache"),
    );
    let archive = harness._directory.path().join("handoff-v18.zip");
    let exported = backup.export(PROJECT_A, archive).await.unwrap();
    assert!(exported.entries >= 6);
    let inspection = backup
        .inspect(harness._directory.path().join("handoff-v18.zip"))
        .await
        .unwrap();
    let restored = backup.restore(&inspection.inspection_id).await.unwrap();
    let restored_handoff_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM external_production_handoffs WHERE project_id = ?",
    )
    .bind(&restored.id)
    .fetch_one(&harness.pool)
    .await
    .unwrap();
    assert_eq!(restored_handoff_count, 1);
    let restored_mapping_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM external_production_handoff_entities e
         INNER JOIN external_production_handoffs h ON h.id = e.handoff_id
         WHERE h.project_id = ?",
    )
    .bind(&restored.id)
    .fetch_one(&harness.pool)
    .await
    .unwrap();
    assert_eq!(restored_mapping_count, 4);
}

#[tokio::test]
async fn stale_preview_revision_conflict_project_and_unknown_field_are_rejected() {
    let harness = harness().await;
    let original = document(PROJECT_A, Some("revision-2"), None, None);
    let preview = harness.service.preview(PROJECT_A, &original).await.unwrap();
    let stale_hash = "0".repeat(64);
    let stale = harness
        .service
        .confirm(PROJECT_A, &original, &stale_hash)
        .await
        .unwrap_err();
    assert_eq!(stale.code(), "HANDOFF_PREVIEW_STALE");
    harness
        .service
        .confirm(PROJECT_A, &original, &preview.document_sha256)
        .await
        .unwrap();

    let changed =
        document(PROJECT_A, Some("revision-2"), None, None).replace("Gate opens", "Gate closes");
    let conflict_preview = harness.service.preview(PROJECT_A, &changed).await.unwrap();
    assert!(conflict_preview
        .errors
        .iter()
        .any(|issue| issue.code == "HANDOFF_SOURCE_REVISION_CONFLICT"));
    let conflict = harness
        .service
        .confirm(PROJECT_A, &changed, &conflict_preview.document_sha256)
        .await
        .unwrap_err();
    assert_eq!(conflict.code(), "HANDOFF_SOURCE_REVISION_CONFLICT");

    let wrong_project = document(PROJECT_B, Some("revision-3"), None, None);
    let mismatch = harness
        .service
        .preview(PROJECT_A, &wrong_project)
        .await
        .unwrap();
    assert!(mismatch
        .errors
        .iter()
        .any(|issue| issue.code == "HANDOFF_PROJECT_MISMATCH"));
    let mut unknown_value: Value = serde_json::from_str(&original).unwrap();
    unknown_value["unexpectedMagic"] = json!(true);
    let unknown = unknown_value.to_string();
    assert_eq!(
        harness
            .service
            .preview(PROJECT_A, &unknown)
            .await
            .unwrap_err()
            .code(),
        "HANDOFF_UNKNOWN_FIELD"
    );
}

#[tokio::test]
async fn asset_and_archived_recipe_validation_are_project_scoped() {
    let harness = harness().await;
    insert_asset(&harness.pool, "ast_dev101_b", PROJECT_B).await;
    let wrong_asset = document(
        PROJECT_A,
        Some("revision-asset"),
        Some("ast_dev101_b"),
        None,
    );
    let preview = harness
        .service
        .preview(PROJECT_A, &wrong_asset)
        .await
        .unwrap();
    assert!(preview
        .errors
        .iter()
        .any(|issue| issue.code == "HANDOFF_ASSET_PROJECT_MISMATCH"));

    insert_workflow(&harness.pool, true).await;
    let archived = document(
        PROJECT_A,
        Some("revision-archived"),
        None,
        Some(json!({
            "image": { "workflowVersionId": "wv_dev101", "recipeId": "recipe_dev101" }
        })),
    );
    let preview = harness.service.preview(PROJECT_A, &archived).await.unwrap();
    assert!(preview
        .errors
        .iter()
        .any(|issue| issue.code == "HANDOFF_RECIPE_UNAVAILABLE"));
}

#[tokio::test]
async fn over_limit_handoff_is_rejected_before_any_write() {
    let harness = harness().await;
    let shots = (1..=501)
        .map(|ordinal| json!({ "externalId": format!("shot-{ordinal}"), "name": format!("Shot {ordinal}"), "ordinal": ordinal, "description": "" }))
        .collect::<Vec<_>>();
    let content = json!({
        "schemaVersion": 1,
        "projectId": PROJECT_A,
        "source": { "agent": "dev101-agent", "revision": "revision-limit" },
        "series": [{ "externalId": "series-1", "name": "Series", "description": "", "ordinal": 1,
            "episodes": [{ "externalId": "episode-1", "name": "Episode", "description": "", "ordinal": 1,
                "scenes": [{ "externalId": "scene-1", "name": "Scene", "description": "", "ordinal": 1, "shots": shots }] }] }]
    }).to_string();
    let preview = harness.service.preview(PROJECT_A, &content).await.unwrap();
    assert!(preview
        .errors
        .iter()
        .any(|issue| issue.code == "HANDOFF_TOO_LARGE"));
    assert_eq!(
        count(&harness.pool, "external_production_handoffs", PROJECT_A).await,
        0
    );
    assert_eq!(count(&harness.pool, "shots", PROJECT_A).await, 0);
}

#[tokio::test]
async fn repository_rolls_back_after_a_mid_transaction_constraint_failure() {
    let harness = harness().await;
    insert_asset(&harness.pool, "ast_atomic", PROJECT_A).await;
    let now = Utc::now();
    let plan = ExternalProductionHandoffImportPlan {
        handoff: ExternalProductionHandoffRecord {
            id: "hnd_atomic".to_owned(),
            project_id: PROJECT_A.to_owned(),
            schema_version: 1,
            source_agent: "atomic-test".to_owned(),
            source_revision: None,
            document_sha256: "b".repeat(64),
            imported_at: now,
        },
        series: vec![ExternalProductionHandoffSeries {
            id: "ser_atomic".to_owned(),
            project_id: PROJECT_A.to_owned(),
            ordinal: 0,
            name: "Series".to_owned(),
            description: String::new(),
            created_at: now,
            updated_at: now,
        }],
        episodes: vec![ExternalProductionHandoffEpisode {
            id: "epi_atomic".to_owned(),
            series_id: "ser_atomic".to_owned(),
            ordinal: 0,
            name: "Episode".to_owned(),
            description: String::new(),
            created_at: now,
            updated_at: now,
        }],
        scenes: vec![ExternalProductionHandoffScene {
            id: "scn_atomic".to_owned(),
            episode_id: "epi_atomic".to_owned(),
            ordinal: 0,
            name: "Scene".to_owned(),
            description: String::new(),
            created_at: now,
            updated_at: now,
        }],
        shots: vec![ExternalProductionHandoffShot {
            id: "sht_atomic".to_owned(),
            project_id: PROJECT_A.to_owned(),
            ordinal: 0,
            name: "Shot".to_owned(),
            description: String::new(),
            created_at: now,
            updated_at: now,
        }],
        prompts: Vec::new(),
        stage_configs: Vec::new(),
        asset_references: vec![
            ExternalProductionHandoffAssetReference {
                shot_id: "sht_atomic".to_owned(),
                stage: "image".to_owned(),
                asset_id: "ast_atomic".to_owned(),
                ordinal: 0,
            },
            ExternalProductionHandoffAssetReference {
                shot_id: "sht_atomic".to_owned(),
                stage: "image".to_owned(),
                asset_id: "ast_atomic".to_owned(),
                ordinal: 0,
            },
        ],
        assignments: vec![ExternalProductionHandoffSceneAssignment {
            shot_id: "sht_atomic".to_owned(),
            scene_id: "scn_atomic".to_owned(),
            ordinal: 0,
            created_at: now,
            updated_at: now,
        }],
        mappings: vec![
            ExternalProductionHandoffEntityMapping {
                handoff_id: "hnd_atomic".to_owned(),
                entity_kind: "series".to_owned(),
                external_id: "series".to_owned(),
                formal_entity_id: "ser_atomic".to_owned(),
            },
            ExternalProductionHandoffEntityMapping {
                handoff_id: "hnd_atomic".to_owned(),
                entity_kind: "episode".to_owned(),
                external_id: "episode".to_owned(),
                formal_entity_id: "epi_atomic".to_owned(),
            },
            ExternalProductionHandoffEntityMapping {
                handoff_id: "hnd_atomic".to_owned(),
                entity_kind: "scene".to_owned(),
                external_id: "scene".to_owned(),
                formal_entity_id: "scn_atomic".to_owned(),
            },
            ExternalProductionHandoffEntityMapping {
                handoff_id: "hnd_atomic".to_owned(),
                entity_kind: "shot".to_owned(),
                external_id: "shot".to_owned(),
                formal_entity_id: "sht_atomic".to_owned(),
            },
        ],
    };
    assert!(harness
        .handoff_repository
        .import_atomic(&plan)
        .await
        .is_err());
    for table in [
        "external_production_handoffs",
        "production_series",
        "production_episodes",
        "production_scenes",
        "shots",
        "shot_stage_prompts",
        "shot_stage_configs",
        "shot_reference_assets",
        "shot_scene_assignments",
        "external_production_handoff_entities",
    ] {
        assert_eq!(
            total_count(&harness.pool, table).await,
            0,
            "{table} must roll back"
        );
    }
}
