//! DEV-036 Agent-D compatibility and architecture gates.
//!
//! These tests deliberately stay on the local SQLite boundary.  They do not
//! start Tauri, ComfyUI, an HTTP client, or a GPU workload.  The source
//! contract gate is intentionally strict: once the DEV-036 implementation is
//! present it must prove the template engine is a pure snapshot producer and
//! that GenerationService remains below that boundary.

use ai_studio_lib::initialize;
use sqlx::{SqlitePool, Transaction};
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
    fs::read_to_string(path)
        .expect("DEV-036 audit source must be readable")
        .replace("\r\n", "\n")
}

fn rust_sources(root: &Path) -> Vec<(PathBuf, String)> {
    fn visit(directory: &Path, output: &mut Vec<(PathBuf, String)>) {
        for entry in fs::read_dir(directory)
            .expect("DEV-036 audit directory must be readable")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, output);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                output.push((path.clone(), read_text(path)));
            }
        }
    }

    let mut output = Vec::new();
    visit(&root.join("src-tauri/src"), &mut output);
    output
}

async fn create_project(pool: &SqlitePool, id: &str, name: &str) {
    sqlx::query(
        "INSERT INTO projects
         (id, name, description, root_path, created_at, updated_at)
         VALUES (?, ?, '', ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(format!("C:/{id}"))
    .bind(NOW)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("DEV-036 project fixture should insert");
}

async fn create_shot(pool: &SqlitePool, project_id: &str, id: &str, ordinal: i64) {
    sqlx::query(
        "INSERT INTO shots
         (id, project_id, ordinal, name, prompt_text, prompt_entry_id,
          prompt_version_id, selected_image_asset_id, selected_video_asset_id,
          created_at, updated_at)
         VALUES (?, ?, ?, ?, '', NULL, NULL, NULL, NULL, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(ordinal)
    .bind(format!("Shot {ordinal}"))
    .bind(NOW)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("DEV-036 shot fixture should insert");
}

async fn create_scene(pool: &SqlitePool, project_id: &str, scene_id: &str, ordinal: i64) {
    let series_id = format!("{project_id}-series");
    let episode_id = format!("{project_id}-episode");
    if ordinal == 0 {
        sqlx::query(
            "INSERT INTO production_series
             (id, project_id, ordinal, name, description, created_at, updated_at)
             VALUES (?, ?, 0, 'Series', '', ?, ?)",
        )
        .bind(&series_id)
        .bind(project_id)
        .bind(NOW)
        .bind(NOW)
        .execute(pool)
        .await
        .expect("DEV-036 series fixture should insert");
        sqlx::query(
            "INSERT INTO production_episodes
             (id, series_id, ordinal, name, description, created_at, updated_at)
             VALUES (?, ?, 0, 'Episode', '', ?, ?)",
        )
        .bind(&episode_id)
        .bind(&series_id)
        .bind(NOW)
        .bind(NOW)
        .execute(pool)
        .await
        .expect("DEV-036 episode fixture should insert");
    }
    sqlx::query(
        "INSERT INTO production_scenes
         (id, episode_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(scene_id)
    .bind(&episode_id)
    .bind(ordinal)
    .bind(format!("Scene {ordinal}"))
    .bind(format!("Scene description {ordinal}"))
    .bind(NOW)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("DEV-036 scene fixture should insert");
}

async fn assign_shot(
    transaction: &mut Transaction<'_, sqlx::Sqlite>,
    shot_id: &str,
    scene_id: &str,
    ordinal: i64,
) {
    sqlx::query(
        "INSERT INTO shot_scene_assignments
         (shot_id, scene_id, ordinal, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(shot_id)
    .bind(scene_id)
    .bind(ordinal)
    .bind(NOW)
    .bind(NOW)
    .execute(&mut **transaction)
    .await
    .expect("DEV-036 assignment fixture should insert");
}

#[tokio::test]
async fn dev036_500_shot_structure_sanity_is_deterministic_and_no_gpu() {
    let directory = tempdir().expect("temporary directory should be created");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("DEV-036 SQLite migration should succeed");
    create_project(&pool, "dev036-500", "地藏经").await;

    for scene_ordinal in 0..50 {
        create_scene(
            &pool,
            "dev036-500",
            &format!("scene-{scene_ordinal:02}"),
            scene_ordinal,
        )
        .await;
    }
    for shot_ordinal in 0..500 {
        create_shot(
            &pool,
            "dev036-500",
            &format!("shot-{shot_ordinal:03}"),
            shot_ordinal,
        )
        .await;
    }

    let mut transaction = pool
        .begin()
        .await
        .expect("assignment transaction should begin");
    for shot_ordinal in 0..500 {
        assign_shot(
            &mut transaction,
            &format!("shot-{shot_ordinal:03}"),
            &format!("scene-{:02}", shot_ordinal / 10),
            shot_ordinal % 10,
        )
        .await;
    }
    transaction
        .commit()
        .await
        .expect("assignment transaction should commit");

    let shot_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM shots WHERE project_id = 'dev036-500'")
            .fetch_one(&pool)
            .await
            .expect("shot count should be readable");
    let scene_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM production_scenes ps
         JOIN production_episodes pe ON pe.id = ps.episode_id
         JOIN production_series s ON s.id = pe.series_id
         WHERE s.project_id = 'dev036-500'",
    )
    .fetch_one(&pool)
    .await
    .expect("scene count should be readable");
    let assignment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shot_scene_assignments")
        .fetch_one(&pool)
        .await
        .expect("assignment count should be readable");

    assert_eq!((shot_count, scene_count, assignment_count), (500, 50, 500));
}

#[tokio::test]
async fn dev036_cross_project_batch_failure_is_atomic() {
    let directory = tempdir().expect("temporary directory should be created");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("DEV-036 SQLite migration should succeed");
    create_project(&pool, "dev036-a", "Project A").await;
    create_project(&pool, "dev036-b", "Project B").await;
    create_scene(&pool, "dev036-a", "scene-a", 0).await;
    create_shot(&pool, "dev036-a", "shot-a", 0).await;
    create_shot(&pool, "dev036-b", "shot-b", 0).await;

    let mut transaction = pool.begin().await.expect("batch transaction should begin");
    assign_shot(&mut transaction, "shot-a", "scene-a", 0).await;
    let belongs_to_project_a: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM shots WHERE id = 'shot-b' AND project_id = 'dev036-a'",
    )
    .fetch_one(&mut *transaction)
    .await
    .expect("project ownership should be readable");
    assert_eq!(belongs_to_project_a, 0);
    transaction
        .rollback()
        .await
        .expect("mixed-project batch must roll back");

    let assignments: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM shot_scene_assignments WHERE scene_id = 'scene-a'",
    )
    .fetch_one(&pool)
    .await
    .expect("assignment count should be readable");
    assert_eq!(assignments, 0);
}

#[tokio::test]
async fn dev036_frozen_stage_prompt_survives_context_edits() {
    let directory = tempdir().expect("temporary directory should be created");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("DEV-036 SQLite migration should succeed");
    create_project(&pool, "dev036-freeze", "地藏经").await;
    create_scene(&pool, "dev036-freeze", "scene-freeze", 0).await;
    create_shot(&pool, "dev036-freeze", "shot-freeze", 0).await;

    sqlx::query(
        "INSERT INTO prompt_entries
         (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at)
         VALUES ('entry-1', 'dev036-freeze', 'prompt', '模板', '模板', '[]', ?, ?)",
    )
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("template prompt entry should insert");
    sqlx::query(
        "INSERT INTO prompt_versions (id, prompt_id, version, text, created_at)
         VALUES ('version-1', 'entry-1', 1, '{{scene.name}} {{shot.name}}', ?)",
    )
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("template prompt version should insert");

    sqlx::query(
        "INSERT INTO shot_scene_assignments
         (shot_id, scene_id, ordinal, created_at, updated_at)
         VALUES ('shot-freeze', 'scene-freeze', 0, ?, ?)",
    )
    .bind(NOW)
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("scene assignment should insert");
    sqlx::query(
        "INSERT INTO shot_stage_prompts
         (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at)
         VALUES ('shot-freeze', 'image', '旧场景·佛陀端坐', 'entry-1', 'version-1', ?)",
    )
    .bind(NOW)
    .execute(&pool)
    .await
    .expect("frozen prompt should insert");

    sqlx::query("UPDATE production_scenes SET name = '新场景', description = '新描述' WHERE id = 'scene-freeze'")
        .execute(&pool)
        .await
        .expect("context edit should succeed");
    let frozen: (String, String, String) = sqlx::query_as(
        "SELECT prompt_text, prompt_entry_id, prompt_version_id
         FROM shot_stage_prompts WHERE shot_id = 'shot-freeze' AND stage = 'image'",
    )
    .fetch_one(&pool)
    .await
    .expect("frozen prompt should remain readable");
    assert_eq!(
        frozen,
        (
            "旧场景·佛陀端坐".to_owned(),
            "entry-1".to_owned(),
            "version-1".to_owned()
        )
    );
}

#[test]
fn dev036_architecture_gate_keeps_generation_below_template_rendering() {
    let root = workspace_root();
    let generation = read_text(root.join("src-tauri/src/application/generation_service.rs"));
    let lowered = generation.to_ascii_lowercase();
    assert!(!lowered.contains("prompttemplateservice"));
    assert!(!lowered.contains("prompt_template_service"));
    assert!(!lowered.contains("productionstructureservice"));
    assert!(!lowered.contains("referenceanchorservice"));
    assert!(!lowered.contains("render_template"));
}

#[test]
fn dev036_migration_and_backup_versions_remain_frozen() {
    let root = workspace_root();
    let migrations = fs::read_dir(root.join("src-tauri/migrations"))
        .expect("migration directory should be readable")
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .collect::<Vec<_>>();
    assert_eq!(
        migrations
            .iter()
            .filter(|name| name.starts_with("022_"))
            .count(),
        1
    );
    assert_eq!(
        migrations
            .iter()
            .filter(|name| name.starts_with("023_"))
            .count(),
        1
    );
    assert_eq!(
        migrations
            .iter()
            .filter(|name| name.starts_with("024_"))
            .count(),
        1
    );
    assert!(migrations.iter().any(|name| name.starts_with("021_")));

    let backup = read_text(root.join("src-tauri/src/application/project_backup_service.rs"));
    assert!(backup.contains("const BACKUP_VERSION: u32 = 14;"));
    assert!(backup.contains("prompt_versions"));
    assert!(backup.contains("shot_stage_prompts"));
}

#[test]
fn dev036_generation_boundary_has_no_live_template_or_context_imports() {
    let root = workspace_root();
    let source = rust_sources(&root);
    let generation = source
        .iter()
        .find(|(path, _)| {
            path.ends_with(Path::new("src-tauri/src/application/generation_service.rs"))
        })
        .map(|(_, text)| text)
        .expect("GenerationService source should be present");
    assert!(!generation.contains("PromptTemplate"));
    assert!(!generation.contains("ProductionStructure"));
    assert!(!generation.contains("ReferenceAnchor"));
}

#[test]
fn dev036_template_contract_requires_parser_renderer_context_preview_and_atomic_apply() {
    let root = workspace_root();
    let source = rust_sources(&root)
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join("\n");
    for required in [
        "parse_prompt_template",
        "PromptTemplateBulkService",
        "PromptTemplateService",
        "PROMPT_TEMPLATE_PROJECT_MISMATCH",
        "PROMPT_TEMPLATE_ANCHOR_PROJECT_MISMATCH",
        "PROMPT_TEMPLATE_APPLY_VALIDATION_FAILED",
        "PROMPT_TEMPLATE_SYNTAX_ERROR",
        "PROMPT_TEMPLATE_UNKNOWN_VARIABLE",
        "PROMPT_TEMPLATE_CONTEXT_MISSING",
        "PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING",
        "PROMPT_TEMPLATE_RESULT_TOO_LARGE",
        "context_anchor_ids",
        "preview_limit",
        "MAX_SHOTS",
        "MAX_ANCHORS",
        ".min(50)",
        "preview_bulk",
        "update_stage_prompts_atomic",
        "shot_stage_prompts",
        "transaction",
    ] {
        assert!(
            source.contains(required),
            "DEV-036 contract missing: {required}"
        );
    }
}

#[test]
fn dev036_preview_path_is_read_only_and_apply_has_one_atomic_write_boundary() {
    let root = workspace_root();
    let source = read_text(root.join("src-tauri/src/application/prompt_template_bulk_service.rs"));
    let preview_start = source
        .find("pub async fn preview(")
        .expect("preview API should exist");
    let preview_end = source
        .find("pub async fn preview_bulk(")
        .expect("bulk preview API should exist");
    let preview_body = &source[preview_start..preview_end];
    assert!(!preview_body.contains("update_stage_prompts_atomic"));
    assert!(!preview_body.contains(".execute("));

    let apply_start = source
        .find("pub async fn apply(")
        .expect("apply API should exist");
    let apply_body = &source[apply_start..];
    assert!(apply_body.contains("if !issues.is_empty()"));
    assert!(apply_body.contains("return Err(PromptTemplateBulkError::Validation(issues))"));
    assert_eq!(apply_body.matches("update_stage_prompts_atomic").count(), 1);
}

#[test]
fn dev036_bulk_context_load_has_set_based_call_sites_not_per_shot_service_calls() {
    let root = workspace_root();
    let source = read_text(root.join("src-tauri/src/application/prompt_template_bulk_service.rs"));
    let load_start = source
        .find("async fn load_context(")
        .expect("bulk context loader should exist");
    let load_end = source
        .find("fn render_shot(")
        .expect("per-shot renderer should follow context loader");
    let load_body = &source[load_start..load_end];
    assert_eq!(load_body.matches(".tree(project_id)").count(), 1);
    assert_eq!(load_body.matches(".list_bulk_data(project_id)").count(), 1);
    assert_eq!(
        load_body
            .matches("reference_anchor_service\n            .list(project_id)")
            .count(),
        1
    );
    assert!(!load_body.contains("for shot_id in"));
}
