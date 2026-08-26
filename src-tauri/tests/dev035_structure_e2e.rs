//! DEV-035 structural contract tests.
//!
//! This slice intentionally stays below the generation boundary: it uses a
//! temporary SQLite database only, so it never starts ComfyUI, submits a
//! prompt, or needs a GPU.  The fixture exercises the persisted production
//! structure contract independently from the production execution pipeline.

use ai_studio_lib::initialize;
use sqlx::{Row, SqlitePool};
use tempfile::tempdir;

const CREATED_AT: &str = "2026-08-18T00:00:00Z";

#[derive(Debug, PartialEq, Eq)]
struct SceneTree {
    id: String,
    ordinal: i64,
    shot_ids: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct EpisodeTree {
    id: String,
    ordinal: i64,
    scenes: Vec<SceneTree>,
}

#[derive(Debug, PartialEq, Eq)]
struct SeriesTree {
    id: String,
    ordinal: i64,
    episodes: Vec<EpisodeTree>,
}

#[derive(Debug, PartialEq, Eq)]
struct StructureTree {
    series: Vec<SeriesTree>,
    unassigned_shot_ids: Vec<String>,
}

async fn insert_project(pool: &SqlitePool, project_id: &str, name: &str) {
    sqlx::query(
        "INSERT INTO projects
         (id, name, description, root_path, created_at, updated_at)
         VALUES (?, ?, '', ?, ?, ?)",
    )
    .bind(project_id)
    .bind(name)
    .bind(format!("C:/{project_id}"))
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV035 project fixture should insert");
}

async fn insert_shots(pool: &SqlitePool, project_id: &str, prefix: &str, count: usize) {
    for ordinal in 0..count {
        let shot_id = format!("{prefix}-shot-{:03}", ordinal + 1);
        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, prompt_entry_id,
              prompt_version_id, selected_image_asset_id, selected_video_asset_id,
              created_at, updated_at)
             VALUES (?, ?, ?, ?, '', NULL, NULL, NULL, NULL, ?, ?)",
        )
        .bind(shot_id)
        .bind(project_id)
        .bind(ordinal as i64)
        .bind(format!("{prefix} Shot {}", ordinal + 1))
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("DEV035 shot fixture should insert");
    }
}

async fn insert_series(pool: &SqlitePool, project_id: &str, id: &str, ordinal: i64, name: &str) {
    sqlx::query(
        "INSERT INTO production_series
         (id, project_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, ?, ?, '', ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(ordinal)
    .bind(name)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV035 series fixture should insert");
}

async fn insert_episode(pool: &SqlitePool, series_id: &str, id: &str, ordinal: i64, name: &str) {
    sqlx::query(
        "INSERT INTO production_episodes
         (id, series_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, ?, ?, '', ?, ?)",
    )
    .bind(id)
    .bind(series_id)
    .bind(ordinal)
    .bind(name)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV035 episode fixture should insert");
}

async fn insert_scene(pool: &SqlitePool, episode_id: &str, id: &str, ordinal: i64, name: &str) {
    sqlx::query(
        "INSERT INTO production_scenes
         (id, episode_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, ?, ?, '', ?, ?)",
    )
    .bind(id)
    .bind(episode_id)
    .bind(ordinal)
    .bind(name)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("DEV035 scene fixture should insert");
}

async fn insert_assignments(pool: &SqlitePool, scene_id: &str, shot_ids: &[String]) {
    let mut transaction = pool
        .begin()
        .await
        .expect("DEV035 assignment transaction should begin");
    for (ordinal, shot_id) in shot_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO shot_scene_assignments
             (shot_id, scene_id, ordinal, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(shot_id)
        .bind(scene_id)
        .bind(ordinal as i64)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("DEV035 assignment should insert");
    }
    transaction
        .commit()
        .await
        .expect("DEV035 assignment transaction should commit");
}

async fn assign_shots(
    pool: &SqlitePool,
    project_id: &str,
    scene_id: &str,
    shot_ids: &[String],
) -> Result<(), String> {
    let scene_project = sqlx::query_scalar::<_, String>(
        "SELECT s.project_id
         FROM production_scenes ps
         JOIN production_episodes pe ON pe.id = ps.episode_id
         JOIN production_series s ON s.id = pe.series_id
         WHERE ps.id = ?",
    )
    .bind(scene_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "scene not found".to_owned())?;
    if scene_project != project_id {
        return Err("PRODUCTION_STRUCTURE_PROJECT_MISMATCH".to_owned());
    }

    let mut transaction = pool.begin().await.map_err(|error| error.to_string())?;
    for shot_id in shot_ids {
        let shot_project =
            sqlx::query_scalar::<_, String>("SELECT project_id FROM shots WHERE id = ?")
                .bind(shot_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("shot {shot_id} not found"))?;
        if shot_project != project_id {
            return Err("PRODUCTION_STRUCTURE_PROJECT_MISMATCH".to_owned());
        }
    }

    let max_ordinal = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT MAX(ordinal) FROM shot_scene_assignments WHERE scene_id = ?",
    )
    .bind(scene_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| error.to_string())?
    .unwrap_or(-1);
    for (offset, shot_id) in shot_ids.iter().enumerate() {
        sqlx::query("DELETE FROM shot_scene_assignments WHERE shot_id = ?")
            .bind(shot_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?;
        sqlx::query(
            "INSERT INTO shot_scene_assignments
             (shot_id, scene_id, ordinal, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(shot_id)
        .bind(scene_id)
        .bind(max_ordinal + 1 + offset as i64)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .map_err(|error| error.to_string())?;
    }
    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

async fn reorder_scene_shots(pool: &SqlitePool, scene_id: &str, ordered_shot_ids: &[String]) {
    let mut transaction = pool
        .begin()
        .await
        .expect("DEV035 reorder transaction should begin");
    sqlx::query("UPDATE shot_scene_assignments SET ordinal = ordinal + 10000 WHERE scene_id = ?")
        .bind(scene_id)
        .execute(&mut *transaction)
        .await
        .expect("DEV035 reorder should reserve ordinals");
    for (ordinal, shot_id) in ordered_shot_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE shot_scene_assignments
             SET ordinal = ?, updated_at = ?
             WHERE scene_id = ? AND shot_id = ?",
        )
        .bind(ordinal as i64)
        .bind(CREATED_AT)
        .bind(scene_id)
        .bind(shot_id)
        .execute(&mut *transaction)
        .await
        .expect("DEV035 reorder should update every assignment");
    }
    transaction
        .commit()
        .await
        .expect("DEV035 reorder transaction should commit");
}

async fn load_tree(pool: &SqlitePool, project_id: &str) -> StructureTree {
    let series_rows = sqlx::query(
        "SELECT id, ordinal FROM production_series WHERE project_id = ? ORDER BY ordinal",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .expect("DEV035 series set query should succeed");
    let episode_rows = sqlx::query(
        "SELECT pe.id, pe.series_id, pe.ordinal
         FROM production_episodes pe
         JOIN production_series ps ON ps.id = pe.series_id
         WHERE ps.project_id = ?
         ORDER BY pe.series_id, pe.ordinal",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .expect("DEV035 episode set query should succeed");
    let scene_rows = sqlx::query(
        "SELECT ps.id, ps.episode_id, ps.ordinal
         FROM production_scenes ps
         JOIN production_episodes pe ON pe.id = ps.episode_id
         JOIN production_series series ON series.id = pe.series_id
         WHERE series.project_id = ?
         ORDER BY ps.episode_id, ps.ordinal",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .expect("DEV035 scene set query should succeed");
    let assignment_rows = sqlx::query(
        "SELECT a.scene_id, a.shot_id, a.ordinal
         FROM shot_scene_assignments a
         JOIN production_scenes ps ON ps.id = a.scene_id
         JOIN production_episodes pe ON pe.id = ps.episode_id
         JOIN production_series series ON series.id = pe.series_id
         WHERE series.project_id = ?
         ORDER BY a.scene_id, a.ordinal",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .expect("DEV035 assignment set query should succeed");
    let unassigned_shot_ids = sqlx::query_scalar::<_, String>(
        "SELECT shots.id
         FROM shots
         LEFT JOIN shot_scene_assignments a ON a.shot_id = shots.id
         WHERE shots.project_id = ? AND a.shot_id IS NULL
         ORDER BY shots.ordinal",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .expect("DEV035 unassigned set query should succeed");

    let mut series = series_rows
        .into_iter()
        .map(|row| SeriesTree {
            id: row.get("id"),
            ordinal: row.get("ordinal"),
            episodes: Vec::new(),
        })
        .collect::<Vec<_>>();
    for row in episode_rows {
        let series_id: String = row.get("series_id");
        let series_item = series
            .iter_mut()
            .find(|item| item.id == series_id)
            .expect("episode should belong to a loaded series");
        series_item.episodes.push(EpisodeTree {
            id: row.get("id"),
            ordinal: row.get("ordinal"),
            scenes: Vec::new(),
        });
    }
    for row in scene_rows {
        let episode_id: String = row.get("episode_id");
        let episode_item = series
            .iter_mut()
            .flat_map(|item| item.episodes.iter_mut())
            .find(|item| item.id == episode_id)
            .expect("scene should belong to a loaded episode");
        episode_item.scenes.push(SceneTree {
            id: row.get("id"),
            ordinal: row.get("ordinal"),
            shot_ids: Vec::new(),
        });
    }
    for row in assignment_rows {
        let scene_id: String = row.get("scene_id");
        let scene_item = series
            .iter_mut()
            .flat_map(|item| item.episodes.iter_mut())
            .flat_map(|item| item.scenes.iter_mut())
            .find(|item| item.id == scene_id)
            .expect("assignment should belong to a loaded scene");
        scene_item.shot_ids.push(row.get("shot_id"));
    }

    StructureTree {
        series,
        unassigned_shot_ids,
    }
}

async fn production_table_count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table' AND name IN
         ('production_series', 'production_episodes', 'production_scenes',
          'shot_scene_assignments')",
    )
    .fetch_one(pool)
    .await
    .expect("DEV035 production table count should succeed")
}

async fn drop_021_and_mark_pending(pool: &SqlitePool) {
    sqlx::query("DROP TABLE shot_scene_assignments")
        .execute(pool)
        .await
        .expect("DEV035 assignment table should drop for compatibility test");
    sqlx::query("DROP TABLE production_scenes")
        .execute(pool)
        .await
        .expect("DEV035 scene table should drop for compatibility test");
    sqlx::query("DROP TABLE production_episodes")
        .execute(pool)
        .await
        .expect("DEV035 episode table should drop for compatibility test");
    sqlx::query("DROP TABLE production_series")
        .execute(pool)
        .await
        .expect("DEV035 series table should drop for compatibility test");
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 21")
        .execute(pool)
        .await
        .expect("DEV035 migration 021 row should be removable in isolated fixture");
}

#[tokio::test]
async fn dev035_migration_020_to_021_fresh_and_reopen_are_compatible() {
    let directory = tempdir().expect("DEV035 temporary directory should exist");
    let database_path = directory.path().join("dev035-migration.db");
    let pool = initialize(&database_path)
        .await
        .expect("DEV035 fresh migration should succeed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("DEV035 migration version should be readable"),
        24
    );
    assert_eq!(production_table_count(&pool).await, 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_series
             UNION ALL SELECT COUNT(*) FROM production_episodes
             UNION ALL SELECT COUNT(*) FROM production_scenes
             UNION ALL SELECT COUNT(*) FROM shot_scene_assignments",
        )
        .fetch_all(&pool)
        .await
        .expect("DEV035 migration should not manufacture structure rows"),
        vec![0, 0, 0, 0]
    );
    pool.close().await;

    let reopened = initialize(&database_path)
        .await
        .expect("DEV035 reopen migration should succeed");
    assert_eq!(production_table_count(&reopened).await, 4);
    drop_021_and_mark_pending(&reopened).await;
    reopened.close().await;

    let upgraded = initialize(&database_path)
        .await
        .expect("DEV035 020 to 021 upgrade should succeed");
    assert_eq!(production_table_count(&upgraded).await, 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&upgraded)
            .await
            .expect("DEV035 upgraded migration version should be readable"),
        24
    );
}

#[tokio::test]
async fn dev035_ai_drama_structure_e2e_preserves_shots_and_order() {
    let directory = tempdir().expect("DEV035 temporary directory should exist");
    let pool = initialize(&directory.path().join("dev035-ai-drama.db"))
        .await
        .expect("DEV035 database should initialize");
    insert_project(&pool, "project-ai-drama", "AI Drama").await;
    insert_project(&pool, "project-b", "Project B").await;
    insert_shots(&pool, "project-ai-drama", "drama", 12).await;
    insert_shots(&pool, "project-b", "other", 1).await;

    insert_series(&pool, "project-ai-drama", "series-s1", 0, "S1").await;
    insert_series(&pool, "project-ai-drama", "series-s2", 1, "S2").await;
    insert_episode(&pool, "series-s1", "episode-e1", 0, "E1").await;
    insert_episode(&pool, "series-s1", "episode-e2", 1, "E2").await;
    insert_scene(&pool, "episode-e1", "scene-a", 0, "Scene A").await;
    insert_scene(&pool, "episode-e1", "scene-b", 1, "Scene B").await;
    insert_scene(&pool, "episode-e1", "scene-c", 2, "Scene C").await;

    let shot = |ordinal: usize| format!("drama-shot-{ordinal:03}");
    insert_assignments(&pool, "scene-a", &[shot(1), shot(2), shot(3)]).await;
    insert_assignments(&pool, "scene-b", &[shot(4), shot(5)]).await;
    insert_assignments(&pool, "scene-c", &[shot(6), shot(7), shot(8)]).await;

    let initial = load_tree(&pool, "project-ai-drama").await;
    assert_eq!(
        initial
            .series
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["series-s1", "series-s2"]
    );
    assert_eq!(
        initial.series[0]
            .episodes
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["episode-e1", "episode-e2"]
    );
    assert_eq!(
        initial.series[0].episodes[0]
            .scenes
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["scene-a", "scene-b", "scene-c"]
    );
    assert_eq!(initial.series[0].episodes[0].scenes[0].shot_ids.len(), 3);
    assert_eq!(initial.unassigned_shot_ids.len(), 4);

    assign_shots(&pool, "project-ai-drama", "scene-b", &[shot(3)])
        .await
        .expect("DEV035 moving a shot within a project should succeed");
    reorder_scene_shots(&pool, "scene-b", &[shot(3), shot(4), shot(5)]).await;
    let after_move = load_tree(&pool, "project-ai-drama").await;
    assert_eq!(
        after_move.series[0].episodes[0].scenes[0].shot_ids,
        vec![shot(1), shot(2)]
    );
    assert_eq!(
        after_move.series[0].episodes[0].scenes[1].shot_ids,
        vec![shot(3), shot(4), shot(5)]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT ordinal FROM shots WHERE id = ?")
            .bind(shot(3))
            .fetch_one(&pool)
            .await
            .expect("DEV035 global shot ordinal should remain readable"),
        2
    );

    let cross_project_error = assign_shots(
        &pool,
        "project-ai-drama",
        "scene-a",
        &["other-shot-001".to_owned()],
    )
    .await
    .expect_err("DEV035 cross-project assignment must fail closed");
    assert_eq!(cross_project_error, "PRODUCTION_STRUCTURE_PROJECT_MISMATCH");

    sqlx::query("DELETE FROM production_scenes WHERE id = 'scene-b'")
        .execute(&pool)
        .await
        .expect("DEV035 scene delete should succeed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE project_id = ?")
            .bind("project-ai-drama")
            .fetch_one(&pool)
            .await
            .expect("DEV035 shots should remain after scene delete"),
        12
    );
    assert_eq!(
        load_tree(&pool, "project-ai-drama")
            .await
            .unassigned_shot_ids
            .len(),
        7
    );

    sqlx::query("DELETE FROM production_episodes WHERE id = 'episode-e1'")
        .execute(&pool)
        .await
        .expect("DEV035 episode delete should succeed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE project_id = ?")
            .bind("project-ai-drama")
            .fetch_one(&pool)
            .await
            .expect("DEV035 shots should remain after episode delete"),
        12
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shot_scene_assignments")
            .fetch_one(&pool)
            .await
            .expect("DEV035 episode cascade should remove assignments"),
        0
    );

    sqlx::query("DELETE FROM production_series WHERE id = 'series-s1'")
        .execute(&pool)
        .await
        .expect("DEV035 series delete should succeed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE project_id = ?")
            .bind("project-ai-drama")
            .fetch_one(&pool)
            .await
            .expect("DEV035 shots should remain after series delete"),
        12
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_series")
            .fetch_one(&pool)
            .await
            .expect("DEV035 remaining series should be readable"),
        1
    );

    let shot_columns = sqlx::query_scalar::<_, String>(
        "SELECT name FROM pragma_table_info('shots') WHERE name = 'scene_id'",
    )
    .fetch_all(&pool)
    .await
    .expect("DEV035 shot schema should be inspectable");
    assert!(
        shot_columns.is_empty(),
        "DEV035 must not alter the Shot schema"
    );
}

#[tokio::test]
async fn dev035_five_hundred_shot_fifty_scene_structure_is_set_based_sane() {
    let directory = tempdir().expect("DEV035 temporary directory should exist");
    let pool = initialize(&directory.path().join("dev035-500-shots.db"))
        .await
        .expect("DEV035 database should initialize");
    insert_project(&pool, "project-bulk", "Bulk Structure").await;
    insert_shots(&pool, "project-bulk", "bulk", 500).await;
    insert_series(&pool, "project-bulk", "bulk-series", 0, "Bulk Series").await;
    insert_episode(&pool, "bulk-series", "bulk-episode", 0, "Bulk Episode").await;
    for scene_ordinal in 0..50 {
        insert_scene(
            &pool,
            "bulk-episode",
            &format!("bulk-scene-{scene_ordinal:02}"),
            scene_ordinal,
            &format!("Scene {scene_ordinal}"),
        )
        .await;
    }

    let mut transaction = pool
        .begin()
        .await
        .expect("DEV035 bulk assignment transaction should begin");
    for scene_ordinal in 0..50 {
        for local_ordinal in 0..10 {
            let shot_ordinal = scene_ordinal * 10 + local_ordinal + 1;
            sqlx::query(
                "INSERT INTO shot_scene_assignments
                 (shot_id, scene_id, ordinal, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(format!("bulk-shot-{shot_ordinal:03}"))
            .bind(format!("bulk-scene-{scene_ordinal:02}"))
            .bind(local_ordinal as i64)
            .bind(CREATED_AT)
            .bind(CREATED_AT)
            .execute(&mut *transaction)
            .await
            .expect("DEV035 bulk assignment should insert");
        }
    }
    transaction
        .commit()
        .await
        .expect("DEV035 bulk assignment transaction should commit");

    let tree = load_tree(&pool, "project-bulk").await;
    assert_eq!(tree.series.len(), 1);
    assert_eq!(tree.series[0].episodes[0].scenes.len(), 50);
    assert_eq!(
        tree.series[0].episodes[0]
            .scenes
            .iter()
            .map(|scene| scene.shot_ids.len())
            .sum::<usize>(),
        500
    );
    assert!(tree.unassigned_shot_ids.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shot_scene_assignments")
            .fetch_one(&pool)
            .await
            .expect("DEV035 bulk assignment count should be readable"),
        500
    );
}
