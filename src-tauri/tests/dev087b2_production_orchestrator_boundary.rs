use ai_studio_lib::application::ports::{
    ProductionOrchestratorRepository, ProductionRunRecord, ProductionStageItemDraft,
    ProductionStageRecord,
};
use ai_studio_lib::infrastructure::database::{initialize, SqliteProductionOrchestratorRepository};
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;
use tempfile::tempdir;

const PROJECT_ID: &str = "prj_boundary";
const NOW: &str = "2026-01-02T00:00:00Z";

fn run(id: &str) -> ProductionRunRecord {
    ProductionRunRecord {
        id: id.to_owned(),
        project_id: PROJECT_ID.to_owned(),
        name: "boundary test".to_owned(),
        status: "READY".to_owned(),
        current_stage_ordinal: 0,
        template_id: None,
        created_at: NOW.to_owned(),
        updated_at: NOW.to_owned(),
        started_at: None,
        finished_at: None,
    }
}

fn stage(run_id: &str, id: &str, ordinal: i64, status: &str) -> ProductionStageRecord {
    ProductionStageRecord {
        id: id.to_owned(),
        run_id: run_id.to_owned(),
        ordinal,
        stage_type: match ordinal {
            0 => "KREA2_IMAGE_GENERATION",
            1 => "ASSET_SELECTION",
            _ => "H3_VIDEO_GENERATION",
        }
        .to_owned(),
        status: status.to_owned(),
        workflow_version_id: None,
        recipe_id: None,
        production_batch_id: None,
        frozen_config_json: "{}".to_owned(),
        prompt: None,
        created_at: NOW.to_owned(),
        updated_at: NOW.to_owned(),
    }
}

fn stages(run_id: &str) -> Vec<ProductionStageRecord> {
    vec![
        stage(run_id, "prst_boundary_0", 0, "READY"),
        stage(run_id, "prst_boundary_1", 1, "PENDING"),
        stage(run_id, "prst_boundary_2", 2, "PENDING"),
    ]
}

fn selection_item(batch_item_id: Option<&str>, ordinal: i64) -> ProductionStageItemDraft {
    ProductionStageItemDraft {
        ordinal,
        status: "SUCCEEDED".to_owned(),
        production_batch_item_id: batch_item_id.map(ToOwned::to_owned),
        task_id: None,
        asset_id: None,
        source_asset_id: None,
        reference_index: Some(ordinal),
        attempt: 1,
        parent_stage_item_id: None,
        frozen_values_json: json!({"referenceIndex": ordinal}).to_string(),
        error_code: None,
        error_message: None,
    }
}

async fn pool() -> (tempfile::TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory should be created");
    let pool = initialize(&directory.path().join("boundary.db"))
        .await
        .expect("database should initialize");
    sqlx::query(
        "INSERT INTO projects (id, name, root_path, created_at, updated_at)
         VALUES (?, 'Boundary project', 'C:/boundary', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("project should exist");
    (directory, pool)
}

async fn count(pool: &SqlitePool, table: &str, where_clause: &str, value: &str) -> i64 {
    let query = format!("SELECT COUNT(*) FROM {table} WHERE {where_clause}");
    sqlx::query_scalar(&query)
        .bind(value)
        .fetch_one(pool)
        .await
        .expect("count query should succeed")
}

#[tokio::test]
async fn create_run_rolls_back_run_stages_and_items_together() {
    let (_directory, pool) = pool().await;
    let repository = SqliteProductionOrchestratorRepository::new(pool.clone());
    let mut broken_stages = stages("prun_boundary_create");
    broken_stages[1].run_id = "missing-run".to_owned();

    assert!(repository
        .create_run_atomic(&run("prun_boundary_create"), &broken_stages)
        .await
        .is_err());
    assert_eq!(
        count(&pool, "production_runs", "id = ?", "prun_boundary_create").await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "production_stages",
            "run_id = ?",
            "prun_boundary_create"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "production_stage_items",
            "stage_id = ?",
            "prst_boundary_1"
        )
        .await,
        0
    );
}

#[tokio::test]
async fn selection_rollback_preserves_the_previous_selection() {
    let (_directory, pool) = pool().await;
    let repository = SqliteProductionOrchestratorRepository::new(pool.clone());
    repository
        .create_run_atomic(
            &run("prun_boundary_selection"),
            &stages("prun_boundary_selection"),
        )
        .await
        .expect("run should be created");
    repository
        .save_selection_atomic(
            "prun_boundary_selection",
            "prst_boundary_1",
            &[selection_item(None, 0)],
            NOW,
        )
        .await
        .expect("initial selection should be saved");

    let failed = repository
        .save_selection_atomic(
            "prun_boundary_selection",
            "prst_boundary_1",
            &[selection_item(Some("missing-batch-item"), 0)],
            NOW,
        )
        .await;
    assert!(failed.is_err());
    assert_eq!(
        count(
            &pool,
            "production_stage_items",
            "stage_id = ?",
            "prst_boundary_1"
        )
        .await,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_stages WHERE id = 'prst_boundary_1'",
        )
        .fetch_one(&pool)
        .await
        .expect("stage should remain readable"),
        "SUCCEEDED"
    );
}

#[tokio::test]
async fn retry_rollback_keeps_stage_and_parent_items_unchanged() {
    let (_directory, pool) = pool().await;
    let repository = SqliteProductionOrchestratorRepository::new(pool.clone());
    repository
        .create_run_atomic(&run("prun_boundary_retry"), &stages("prun_boundary_retry"))
        .await
        .expect("run should be created");
    repository
        .save_selection_atomic(
            "prun_boundary_retry",
            "prst_boundary_1",
            &[selection_item(None, 0)],
            NOW,
        )
        .await
        .expect("parent item should be saved");

    let failed = repository
        .prepare_retry_atomic(
            "prst_boundary_1",
            &[ProductionStageItemDraft {
                production_batch_item_id: Some("missing-batch-item".to_owned()),
                attempt: 2,
                parent_stage_item_id: None,
                ..selection_item(None, 1)
            }],
            NOW,
        )
        .await;
    assert!(failed.is_err());
    assert_eq!(
        count(
            &pool,
            "production_stage_items",
            "stage_id = ?",
            "prst_boundary_1"
        )
        .await,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_stages WHERE id = 'prst_boundary_1'",
        )
        .fetch_one(&pool)
        .await
        .expect("stage should remain readable"),
        "SUCCEEDED"
    );
}

#[tokio::test]
async fn cancel_rollback_keeps_stage_item_and_run_statuses_together() {
    let (_directory, pool) = pool().await;
    let repository = Arc::new(SqliteProductionOrchestratorRepository::new(pool.clone()));
    repository
        .create_run_atomic(
            &run("prun_boundary_cancel"),
            &stages("prun_boundary_cancel"),
        )
        .await
        .expect("run should be created");
    repository
        .save_selection_atomic(
            "prun_boundary_cancel",
            "prst_boundary_1",
            &[selection_item(None, 0)],
            NOW,
        )
        .await
        .expect("item should be saved");
    sqlx::query(
        "CREATE TRIGGER abort_cancel_run BEFORE UPDATE OF status ON production_runs
         BEGIN SELECT RAISE(ABORT, 'forced cancel rollback'); END",
    )
    .execute(&pool)
    .await
    .expect("rollback trigger should be installed");

    assert!(repository
        .cancel_run_atomic("prun_boundary_cancel", NOW)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_runs WHERE id = 'prun_boundary_cancel'"
        )
        .fetch_one(&pool)
        .await
        .expect("run should remain readable"),
        "READY"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_stages WHERE id = 'prst_boundary_1'"
        )
        .fetch_one(&pool)
        .await
        .expect("stage should remain readable"),
        "SUCCEEDED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_stage_items WHERE stage_id = 'prst_boundary_1'"
        )
        .fetch_one(&pool)
        .await
        .expect("item should remain readable"),
        "SUCCEEDED"
    );
}
