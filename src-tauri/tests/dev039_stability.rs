//! DEV-039 post-0.6 no-GPU stability and architecture gates.
//!
//! These checks stay on the local SQLite boundary.  They do not start Tauri,
//! ComfyUI, an HTTP server, an installer, or a GPU workload.  The repeated
//! reads are deliberately boring: they model the persistence work performed
//! by app startup, project-context reload, and the Command Center surface.

use ai_studio_lib::initialize;
use sqlx::SqlitePool;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::tempdir;

const NOW: &str = "2026-08-18T00:00:00Z";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf()
}

fn read_text(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).expect("DEV-039 audit source must be readable")
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
    .bind(NOW)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("DEV-039 project fixture should insert");
}

async fn insert_shots(pool: &SqlitePool, project_id: &str, count: usize) {
    let mut transaction = pool
        .begin()
        .await
        .expect("DEV-039 shot fixture transaction should begin");
    for ordinal in 0..count {
        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, prompt_entry_id,
              prompt_version_id, selected_image_asset_id, selected_video_asset_id,
              created_at, updated_at)
             VALUES (?, ?, ?, ?, '', NULL, NULL, NULL, NULL, ?, ?)",
        )
        .bind(format!("{project_id}-shot-{:03}", ordinal + 1))
        .bind(project_id)
        .bind(ordinal as i64)
        .bind(format!("Shot {}", ordinal + 1))
        .bind(NOW)
        .bind(NOW)
        .execute(&mut *transaction)
        .await
        .expect("DEV-039 shot fixture should insert");
    }
    transaction
        .commit()
        .await
        .expect("DEV-039 shot fixture transaction should commit");
}

#[tokio::test]
async fn dev039_fresh_project_open_close_is_stable_for_20_cycles_without_gpu() {
    let directory = tempdir().expect("DEV-039 temporary directory should exist");
    let database_path = directory.path().join("dev039-open-close.db");
    let pool = initialize(&database_path)
        .await
        .expect("DEV-039 database should initialize");
    insert_project(&pool, "dev039-project", "DEV-039").await;
    pool.close().await;

    for cycle in 0..20 {
        let pool = initialize(&database_path)
            .await
            .expect("DEV-039 database should reopen");
        let project_id =
            sqlx::query_scalar::<_, String>("SELECT id FROM projects WHERE id = 'dev039-project'")
                .fetch_one(&pool)
                .await
                .expect("DEV-039 project should survive reopen");
        assert_eq!(project_id, "dev039-project", "open/close cycle {cycle}");
        pool.close().await;
    }
}

#[tokio::test]
async fn dev039_500_shot_command_center_reload_is_stable_for_30_cycles_without_gpu() {
    let directory = tempdir().expect("DEV-039 temporary directory should exist");
    let pool = initialize(&directory.path().join("dev039-command-center.db"))
        .await
        .expect("DEV-039 database should initialize");
    insert_project(&pool, "dev039-large", "500 Shot Project").await;
    insert_shots(&pool, "dev039-large", 500).await;

    for cycle in 0..30 {
        let ids = sqlx::query_scalar::<_, String>(
            "SELECT id FROM shots WHERE project_id = ? ORDER BY ordinal ASC, id ASC",
        )
        .bind("dev039-large")
        .fetch_all(&pool)
        .await
        .expect("DEV-039 Command Center shot reload should succeed");
        let shot_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE project_id = ?")
                .bind("dev039-large")
                .fetch_one(&pool)
                .await
                .expect("DEV-039 shot count should be readable");

        assert_eq!(ids.len(), 500, "Command Center reload cycle {cycle}");
        assert_eq!(shot_count, 500, "Command Center count cycle {cycle}");
        assert_eq!(
            ids.first().map(String::as_str),
            Some("dev039-large-shot-001")
        );
        assert_eq!(
            ids.last().map(String::as_str),
            Some("dev039-large-shot-500")
        );
    }
}

#[tokio::test]
async fn dev039_workspace_switch_is_project_scoped_for_20_cycles_without_gpu() {
    let directory = tempdir().expect("DEV-039 temporary directory should exist");
    let pool = initialize(&directory.path().join("dev039-workspace-switch.db"))
        .await
        .expect("DEV-039 database should initialize");
    insert_project(&pool, "dev039-a", "Project A").await;
    insert_project(&pool, "dev039-b", "Project B").await;
    insert_shots(&pool, "dev039-a", 3).await;
    insert_shots(&pool, "dev039-b", 3).await;

    for cycle in 0..20 {
        let project_id = if cycle % 2 == 0 {
            "dev039-a"
        } else {
            "dev039-b"
        };
        let visible_project_ids = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT project_id FROM shots WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&pool)
        .await
        .expect("DEV-039 workspace switch should read project data");
        assert_eq!(
            visible_project_ids,
            vec![project_id],
            "workspace cycle {cycle}"
        );
    }
}

#[test]
fn dev039_architecture_has_one_queue_history_and_workflow_owner() {
    let source = read_text(workspace_root().join("src-tauri/src/lib.rs"));
    for constructor in [
        "ProductionQueueService::new",
        "TaskHistoryService::new",
        "WorkflowLifecycleService::new",
    ] {
        assert_eq!(
            source.matches(constructor).count(),
            1,
            "DEV-039 must keep one startup owner for {constructor}"
        );
    }
}

#[test]
fn dev039_architecture_keeps_bulk_reads_and_comfy_calls_behind_boundaries() {
    let root = workspace_root();
    let shot_service = read_text(root.join("src-tauri/src/application/shot_service.rs"));
    let list_start = shot_service
        .find("pub async fn list(")
        .expect("ShotService list boundary should exist");
    let get_start = shot_service[list_start..]
        .find("pub async fn get(")
        .map(|offset| list_start + offset)
        .expect("ShotService get boundary should follow list");
    let list_body = &shot_service[list_start..get_start];
    assert!(list_body.contains("self.repository.list(project_id).await?"));
    assert!(list_body.contains("self.views(data).await"));
    assert!(!list_body.contains("find_by_id"));

    let repository =
        read_text(root.join("src-tauri/src/infrastructure/database/repositories/shot.rs"));
    let bulk_start = repository
        .find("async fn load_related_many(")
        .expect("bulk shot hydration boundary should exist");
    let bulk_end = repository[bulk_start..]
        .find("#[async_trait]")
        .map(|offset| bulk_start + offset)
        .expect("bulk shot hydration should end before repository implementation");
    let bulk_body = &repository[bulk_start..bulk_end];
    assert_eq!(bulk_body.matches("fetch_all(&self.pool)").count(), 4);
    assert!(bulk_body.contains("JOIN shots"));
    assert!(!bulk_body.contains("load_related(shot"));

    let generation = read_text(root.join("src-tauri/src/application/generation_service.rs"));
    assert!(!generation.contains("reqwest::"));
    assert!(!generation.contains("ComfyHttpAdapter"));
    let commands = fs::read_dir(root.join("src-tauri/src/commands"))
        .expect("DEV-039 command directory should be readable")
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        })
        .map(|entry| read_text(entry.path()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!commands.contains("/prompt"));
    assert!(!commands.contains("reqwest::"));
}
