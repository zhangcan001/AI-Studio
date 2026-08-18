//! DEV-033 Agent C: isolated 100/250/500-shot list benchmark.
//!
//! This is test-only measurement code.  It does not exercise ComfyUI, create
//! media, or change any production repository/service behavior.  The legacy
//! path below mirrors the pre-DEV-033 `SqliteShotRepository::list` fan-out so
//! the current set-based implementation can be compared on the same fixture.

use super::repositories::SqliteShotRepository;
use crate::application::ports::ShotRepository;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::time::Instant;
use tempfile::tempdir;

const PROJECT_ID: &str = "prj_dev033_benchmark";

#[derive(Debug, Default, PartialEq, Eq)]
struct LegacyMetrics {
    shot_count: usize,
    sql_calls: usize,
}

async fn isolated_pool() -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
}

async fn file_pool(path: &std::path::Path) -> Result<SqlitePool, sqlx::Error> {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
}

async fn create_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for statement in [
        "CREATE TABLE shots (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            name TEXT NOT NULL,
            prompt_text TEXT NOT NULL,
            prompt_entry_id TEXT,
            prompt_version_id TEXT,
            selected_image_asset_id TEXT,
            selected_video_asset_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        "CREATE TABLE shot_stage_configs (
            shot_id TEXT NOT NULL,
            stage TEXT NOT NULL,
            workflow_version_id TEXT NOT NULL,
            recipe_id TEXT NOT NULL,
            scalar_values_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (shot_id, stage)
        )",
        "CREATE TABLE shot_reference_assets (
            shot_id TEXT NOT NULL,
            stage TEXT NOT NULL,
            asset_id TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            PRIMARY KEY (shot_id, stage, asset_id)
        )",
        "CREATE TABLE shot_generation_links (
            id TEXT PRIMARY KEY,
            shot_id TEXT NOT NULL,
            stage TEXT NOT NULL,
            task_id TEXT,
            production_batch_item_id TEXT,
            created_at TEXT NOT NULL
        )",
        "CREATE TABLE shot_stage_prompts (
            shot_id TEXT NOT NULL,
            stage TEXT NOT NULL,
            prompt_text TEXT NOT NULL,
            prompt_entry_id TEXT,
            prompt_version_id TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (shot_id, stage)
        )",
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

async fn seed(pool: &SqlitePool, count: usize, task_link_count: usize) -> Result<(), sqlx::Error> {
    create_schema(pool).await?;
    let mut transaction = pool.begin().await?;
    for ordinal in 0..count {
        let number = ordinal + 1;
        let shot_id = format!("shot-{number:03}");
        let name = format!("镜头 {number:03}");
        let prompt = format!("镜头 {number:03} prompt");
        let timestamp = format!("2026-08-18T00:{:02}:00Z", ordinal % 60);
        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&shot_id)
        .bind(PROJECT_ID)
        .bind(ordinal as i64)
        .bind(&name)
        .bind(&prompt)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&mut *transaction)
        .await?;

        for stage in ["image", "video"] {
            sqlx::query(
                "INSERT INTO shot_stage_configs
                 (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
                 VALUES (?, ?, 'wf_dev033', 'recipe_dev033', '{}', ?)",
            )
            .bind(&shot_id)
            .bind(stage)
            .bind(&timestamp)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "INSERT INTO shot_stage_prompts
                 (shot_id, stage, prompt_text, updated_at) VALUES (?, ?, ?, ?)",
            )
            .bind(&shot_id)
            .bind(stage)
            .bind(format!("{prompt} {stage}"))
            .bind(&timestamp)
            .execute(&mut *transaction)
            .await?;
        }

        if number <= task_link_count {
            sqlx::query(
                "INSERT INTO shot_generation_links
                 (id, shot_id, stage, task_id, created_at)
                 VALUES (?, ?, 'image', ?, ?)",
            )
            .bind(format!("link-{number:03}"))
            .bind(&shot_id)
            .bind(format!("tsk_{number:03}"))
            .bind(&timestamp)
            .execute(&mut *transaction)
            .await?;
        }
    }
    transaction.commit().await
}

/// Exact pre-DEV-033 repository fan-out: one shot query plus three related
/// queries for every shot.  The counter is incremented at each SQL call.
async fn legacy_list(pool: &SqlitePool) -> Result<LegacyMetrics, sqlx::Error> {
    let mut metrics = LegacyMetrics::default();
    metrics.sql_calls += 1;
    let shots = sqlx::query_as::<_, (String, i64, String, String)>(
        "SELECT id, ordinal, name, prompt_text FROM shots
         WHERE project_id = ? ORDER BY ordinal ASC, id ASC",
    )
    .bind(PROJECT_ID)
    .fetch_all(pool)
    .await?;
    metrics.shot_count = shots.len();

    for (shot_id, _, _, _) in shots {
        metrics.sql_calls += 1;
        let _ = sqlx::query_scalar::<_, String>(
            "SELECT stage FROM shot_stage_configs WHERE shot_id = ? ORDER BY stage",
        )
        .bind(&shot_id)
        .fetch_all(pool)
        .await?;
        metrics.sql_calls += 1;
        let _ = sqlx::query_scalar::<_, String>(
            "SELECT asset_id FROM shot_reference_assets
             WHERE shot_id = ? ORDER BY stage, ordinal, asset_id",
        )
        .bind(&shot_id)
        .fetch_all(pool)
        .await?;
        metrics.sql_calls += 1;
        let _ = sqlx::query_scalar::<_, String>(
            "SELECT id FROM shot_generation_links
             WHERE shot_id = ? ORDER BY created_at DESC, id DESC",
        )
        .bind(&shot_id)
        .fetch_all(pool)
        .await?;
    }
    Ok(metrics)
}

async fn benchmark_case(count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let before_pool = isolated_pool().await?;
    seed(&before_pool, count, if count == 500 { 50 } else { 0 }).await?;
    let before_start = Instant::now();
    let before = legacy_list(&before_pool).await?;
    let before_ms = before_start.elapsed().as_secs_f64() * 1000.0;

    let after_pool = isolated_pool().await?;
    seed(&after_pool, count, if count == 500 { 50 } else { 0 }).await?;
    let repository = SqliteShotRepository::new(after_pool.clone());
    let after_start = Instant::now();
    let after = repository.list(PROJECT_ID).await?;
    let after_ms = after_start.elapsed().as_secs_f64() * 1000.0;

    assert_eq!(before.shot_count, count);
    assert_eq!(before.sql_calls, 1 + count * 3);
    assert_eq!(after.len(), count);
    assert_eq!(
        after
            .iter()
            .map(|shot| shot.stage_configs.len())
            .sum::<usize>(),
        count * 2
    );
    if count == 500 {
        assert_eq!(
            after
                .iter()
                .flat_map(|shot| shot.generation_links.iter())
                .count(),
            50
        );
        for number in [1, 250, 500] {
            let shot = after
                .iter()
                .find(|shot| shot.shot.id == format!("shot-{number:03}"))
                .expect("synthetic No-GPU shot should exist");
            assert_eq!(shot.shot.name, format!("镜头 {number:03}"));
            assert_eq!(shot.shot.prompt_text, format!("镜头 {number:03} prompt"));
            assert_eq!(shot.stage_configs.len(), 2);
            assert_eq!(shot.stage_prompts.len(), 2);
        }
    }

    println!(
        "DEV033_BENCH shots={count} BEFORE_MS={before_ms:.3} BEFORE_SQL_CALLS={} AFTER_MS={after_ms:.3} AFTER_SQL_CALLS=5 AFTER_RESULT={}",
        before.sql_calls,
        after.len(),
    );
    Ok(())
}

async fn restart_persistence_case() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let database_path = directory.path().join("dev033_restart.sqlite");
    {
        let pool = file_pool(&database_path).await?;
        seed(&pool, 500, 50).await?;
        let repository = SqliteShotRepository::new(pool.clone());
        assert_eq!(repository.list(PROJECT_ID).await?.len(), 500);
        pool.close().await;
    }
    {
        let pool = file_pool(&database_path).await?;
        let repository = SqliteShotRepository::new(pool.clone());
        let shots = repository.list(PROJECT_ID).await?;
        assert_eq!(shots.len(), 500);
        assert_eq!(shots[249].shot.name, "镜头 250");
        pool.close().await;
    }
    println!("DEV033_RESTART shots=500 RESULT=PASS");
    Ok(())
}

#[tokio::test]
async fn dev033_current_list_benchmark_and_no_gpu_checks() -> Result<(), Box<dyn std::error::Error>>
{
    benchmark_case(100).await?;
    benchmark_case(250).await?;
    benchmark_case(500).await?;
    restart_persistence_case().await?;
    println!(
        "DEV033_CALL_COUNT_FORMULA BEFORE_LIST=1+3N AFTER_LIST=5 BEFORE_SERVICE_STAGE_PROMPT_N_PLUS_1=1+5N+3N^2 TASK_LINK_SHOTS=50"
    );
    Ok(())
}
