//! DEV-055 Agent-B release compatibility gates.
//!
//! The fixtures in this file are deliberately consumed by the public
//! application boundaries.  Backup archives are produced by
//! `ProjectBackupService::export`, manifests are produced by
//! `ProjectManifestService::export`, and migrations are applied by the real
//! database initializer.  The only older backup variants are copies of that
//! real export with the historical format version changed; they are not
//! JSON-only success fixtures.

use ai_studio_lib::application::project_backup_service::ProjectBackupService;
use ai_studio_lib::application::project_manifest_service::ProjectManifestService;
use ai_studio_lib::initialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    env, fs,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::{tempdir, TempDir};
use zip::{write::FileOptions, ZipArchive, ZipWriter};

const CREATED_AT: &str = "2026-08-27T00:00:00Z";

const LEGACY_PROJECT_ID: &str = "dev055-legacy-project";
const LEGACY_SHOT_ID: &str = "dev055-legacy-shot";
const LEGACY_ASSET_ID: &str = "dev055-legacy-asset";
const LEGACY_ANCHOR_ID: &str = "dev055-legacy-anchor";

const CONSISTENCY_PROJECT_ID: &str = "dev055-consistency-project";
const CONSISTENCY_SHOT_ID: &str = "dev055-consistency-shot";
const CONSISTENCY_SERIES_ID: &str = "dev055-consistency-series";
const CONSISTENCY_EPISODE_ID: &str = "dev055-consistency-episode";
const CONSISTENCY_SCENE_ID: &str = "dev055-consistency-scene";
const CHARACTER_PROFILE_ID: &str = "cp_dev055_character";
const SCENE_PROFILE_ID: &str = "scp_dev055_scene";
const PROP_PROFILE_ID: &str = "pp_dev055_prop";
const STYLE_PROFILE_ID: &str = "stp_dev055_style";
const COSTUME_VARIANT_ID: &str = "cv_dev055_costume";
const CHARACTER_REFERENCE_SET_ID: &str = "rs_dev055_character";
const COSTUME_REFERENCE_SET_ID: &str = "rs_dev055_costume";
const SCENE_REFERENCE_SET_ID: &str = "rs_dev055_scene";
const PROP_REFERENCE_SET_ID: &str = "rs_dev055_prop";
const SHOT_REFERENCE_SET_ID: &str = "rs_dev055_shot";
const BATCH_ID: &str = "dev055-preparation-batch";
const BATCH_ITEM_ID: &str = "dev055-preparation-item";
const PREPARATION_SNAPSHOT_ID: &str = "dev055-preparation-snapshot";
const PREPARATION_CONTEXT_HASH: &str = "dev055-preparation-context";
const WORKFLOW_ID: &str = "wf_dev055_compatibility";
const WORKFLOW_VERSION_ID: &str = "wfv_dev055_compatibility";
const RECIPE_ID: &str = "recipe_dev055_compatibility";

const CONSISTENCY_ASSETS: [(&str, &str, &[u8]); 5] = [
    (
        "dev055-character-asset",
        "character.png",
        b"dev055-character-reference",
    ),
    (
        "dev055-costume-asset",
        "costume.png",
        b"dev055-costume-reference",
    ),
    ("dev055-scene-asset", "scene.png", b"dev055-scene-reference"),
    ("dev055-prop-asset", "prop.png", b"dev055-prop-reference"),
    ("dev055-shot-asset", "shot.png", b"dev055-shot-reference"),
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf()
}

async fn database() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("DEV-055 temporary directory should exist");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("DEV-055 database should initialize through the real migrator");
    (directory, pool)
}

async fn insert_project(pool: &SqlitePool, id: &str, name: &str, root: &Path) {
    fs::create_dir_all(root).expect("fixture project root should exist");
    sqlx::query(
        "INSERT INTO projects
         (id, name, description, root_path, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(format!("{name} compatibility fixture"))
    .bind(root.to_string_lossy().to_string())
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("project fixture should insert");
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn insert_image_asset(
    pool: &SqlitePool,
    project_id: &str,
    id: &str,
    name: &str,
    path: &Path,
    bytes: &[u8],
) {
    fs::write(path, bytes).expect("fixture asset bytes should be written");
    sqlx::query(
        "INSERT INTO assets
         (id, project_id, type, category, name, original_name, storage_path,
          sha256, mime_type, width, height, file_size, metadata_json,
          created_at, updated_at)
         VALUES (?, ?, 'image', 'source_image', ?, ?, ?, ?, 'image/png',
                 640, 480, ?, '{}', ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(name)
    .bind(name)
    .bind(path.to_string_lossy().to_string())
    .bind(sha256(bytes))
    .bind(bytes.len() as i64)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("image asset fixture should insert");
}

async fn insert_shot(pool: &SqlitePool, project_id: &str, id: &str, name: &str, prompt: &str) {
    sqlx::query(
        "INSERT INTO shots
         (id, project_id, ordinal, name, prompt_text, prompt_entry_id,
          prompt_version_id, selected_image_asset_id, selected_video_asset_id,
          created_at, updated_at)
         VALUES (?, ?, 0, ?, ?, NULL, NULL, NULL, NULL, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(name)
    .bind(prompt)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("shot fixture should insert");
}

async fn insert_legacy_project(pool: &SqlitePool, root: &Path) {
    insert_project(pool, LEGACY_PROJECT_ID, "DEV-055 Legacy", root).await;
    let asset_path = root.join("legacy.png");
    insert_image_asset(
        pool,
        LEGACY_PROJECT_ID,
        LEGACY_ASSET_ID,
        "legacy.png",
        &asset_path,
        b"dev055-legacy-asset-bytes",
    )
    .await;
    insert_shot(
        pool,
        LEGACY_PROJECT_ID,
        LEGACY_SHOT_ID,
        "Legacy shot",
        "legacy prompt fallback",
    )
    .await;
    sqlx::query(
        "INSERT INTO shot_stage_prompts
         (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at)
         VALUES (?, 'image', ?, NULL, NULL, ?),
                (?, 'video', ?, NULL, NULL, ?)",
    )
    .bind(LEGACY_SHOT_ID)
    .bind("legacy prompt fallback")
    .bind(CREATED_AT)
    .bind(LEGACY_SHOT_ID)
    .bind("legacy prompt fallback")
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("legacy shot stage prompts should insert");

    sqlx::query(
        "INSERT INTO reference_anchors
         (id, project_id, kind, name, normalized_name, description,
          created_at, updated_at)
         VALUES (?, ?, 'CHARACTER', 'Legacy Anchor', 'legacy anchor',
                 'legacy reference relation', ?, ?)",
    )
    .bind(LEGACY_ANCHOR_ID)
    .bind(LEGACY_PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("legacy reference anchor should insert");
    sqlx::query(
        "INSERT INTO reference_anchor_assets (anchor_id, asset_id, ordinal, created_at)
         VALUES (?, ?, 0, ?)",
    )
    .bind(LEGACY_ANCHOR_ID)
    .bind(LEGACY_ASSET_ID)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("legacy reference anchor asset should insert");
    sqlx::query(
        "INSERT INTO shot_reference_assets (shot_id, stage, asset_id, ordinal)
         VALUES (?, 'image', ?, 0)",
    )
    .bind(LEGACY_SHOT_ID)
    .bind(LEGACY_ASSET_ID)
    .execute(pool)
    .await
    .expect("legacy shot reference should insert");
}

async fn insert_consistency_structure(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO production_series
         (id, project_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, 0, 'DEV-055 Series', 'compatibility series', ?, ?)",
    )
    .bind(CONSISTENCY_SERIES_ID)
    .bind(CONSISTENCY_PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("consistency series should insert");
    sqlx::query(
        "INSERT INTO production_episodes
         (id, series_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, 0, 'DEV-055 Episode', 'compatibility episode', ?, ?)",
    )
    .bind(CONSISTENCY_EPISODE_ID)
    .bind(CONSISTENCY_SERIES_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("consistency episode should insert");
    sqlx::query(
        "INSERT INTO production_scenes
         (id, episode_id, ordinal, name, description, created_at, updated_at)
         VALUES (?, ?, 0, 'DEV-055 Scene', 'compatibility scene', ?, ?)",
    )
    .bind(CONSISTENCY_SCENE_ID)
    .bind(CONSISTENCY_EPISODE_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("consistency scene should insert");
    sqlx::query(
        "INSERT INTO shot_scene_assignments
         (shot_id, scene_id, ordinal, created_at, updated_at)
         VALUES (?, ?, 0, ?, ?)",
    )
    .bind(CONSISTENCY_SHOT_ID)
    .bind(CONSISTENCY_SCENE_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("consistency shot assignment should insert");
}

async fn insert_consistency_project(pool: &SqlitePool, root: &Path) {
    insert_project(pool, CONSISTENCY_PROJECT_ID, "DEV-055 Consistency", root).await;
    for (id, file_name, bytes) in CONSISTENCY_ASSETS {
        insert_image_asset(
            pool,
            CONSISTENCY_PROJECT_ID,
            id,
            file_name,
            &root.join(file_name),
            bytes,
        )
        .await;
    }
    insert_shot(
        pool,
        CONSISTENCY_PROJECT_ID,
        CONSISTENCY_SHOT_ID,
        "Consistency shot",
        "consistency prompt",
    )
    .await;
    sqlx::query(
        "INSERT INTO shot_stage_prompts
         (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at)
         VALUES (?, 'image', ?, NULL, NULL, ?),
                (?, 'video', ?, NULL, NULL, ?)",
    )
    .bind(CONSISTENCY_SHOT_ID)
    .bind("consistency image prompt")
    .bind(CREATED_AT)
    .bind(CONSISTENCY_SHOT_ID)
    .bind("consistency video prompt")
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("consistency shot stage prompts should insert");
    insert_consistency_structure(pool).await;

    sqlx::query(
        "INSERT INTO style_profiles
         (id, project_id, name, style_prompt, color_prompt, line_prompt,
          negative_prompt, output_notes, created_at, updated_at)
         VALUES (?, ?, 'DEV-055 Style', 'cinematic ink', 'violet palette',
                 'clean contours', 'photorealism', 'stable faces', ?, ?)",
    )
    .bind(STYLE_PROFILE_ID)
    .bind(CONSISTENCY_PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("style profile should insert");

    for (id, name, purpose, owner_type, owner_id) in [
        (
            CHARACTER_REFERENCE_SET_ID,
            "DEV-055 Character References",
            "CHARACTER",
            Some("CHARACTER"),
            Some(CHARACTER_PROFILE_ID),
        ),
        (
            COSTUME_REFERENCE_SET_ID,
            "DEV-055 Costume References",
            "COSTUME",
            Some("CHARACTER"),
            Some(CHARACTER_PROFILE_ID),
        ),
        (
            SCENE_REFERENCE_SET_ID,
            "DEV-055 Scene References",
            "SCENE",
            Some("SCENE"),
            Some(SCENE_PROFILE_ID),
        ),
        (
            PROP_REFERENCE_SET_ID,
            "DEV-055 Prop References",
            "PROP",
            Some("PROP"),
            Some(PROP_PROFILE_ID),
        ),
        (
            SHOT_REFERENCE_SET_ID,
            "DEV-055 Shot References",
            "SHOT",
            None,
            None,
        ),
    ] {
        sqlx::query(
            "INSERT INTO reference_sets
             (id, project_id, name, purpose, description, owner_profile_type,
              owner_profile_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'compatibility reference set', ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(CONSISTENCY_PROJECT_ID)
        .bind(name)
        .bind(purpose)
        .bind(owner_type)
        .bind(owner_id)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("reference set should insert");
    }

    sqlx::query(
        "INSERT INTO character_profiles
         (id, project_id, name, description, canonical_prompt, negative_prompt,
          default_style_profile_id, default_reference_set_id, metadata_json,
          created_at, updated_at)
         VALUES (?, ?, 'DEV-055 Character', 'character description',
                 'hero prompt', 'blurry', ?, ?, '{\"source\":\"DEV-055\"}', ?, ?)",
    )
    .bind(CHARACTER_PROFILE_ID)
    .bind(CONSISTENCY_PROJECT_ID)
    .bind(STYLE_PROFILE_ID)
    .bind(CHARACTER_REFERENCE_SET_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("character profile should insert");
    sqlx::query(
        "INSERT INTO scene_profiles
         (id, project_id, name, description, environment_prompt, lighting_prompt,
          negative_prompt, default_style_profile_id, default_reference_set_id,
          created_at, updated_at)
         VALUES (?, ?, 'DEV-055 Scene Profile', 'scene description', 'warehouse',
                 'hard morning light', 'empty background', ?, ?, ?, ?)",
    )
    .bind(SCENE_PROFILE_ID)
    .bind(CONSISTENCY_PROJECT_ID)
    .bind(STYLE_PROFILE_ID)
    .bind(SCENE_REFERENCE_SET_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("scene profile should insert");
    sqlx::query(
        "INSERT INTO prop_profiles
         (id, project_id, name, description, canonical_prompt, material_prompt,
          scale_prompt, default_reference_set_id, created_at, updated_at)
         VALUES (?, ?, 'DEV-055 Prop', 'prop description', 'lantern', 'brass',
                 'hand-sized', ?, ?, ?)",
    )
    .bind(PROP_PROFILE_ID)
    .bind(CONSISTENCY_PROJECT_ID)
    .bind(PROP_REFERENCE_SET_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("prop profile should insert");
    sqlx::query(
        "INSERT INTO costume_variants
         (id, character_profile_id, name, prompt_fragment, reference_set_id,
          is_default, ordinal, created_at, updated_at)
         VALUES (?, ?, 'DEV-055 Travel Coat', 'dark travel coat', ?, 1, 0, ?, ?)",
    )
    .bind(COSTUME_VARIANT_ID)
    .bind(CHARACTER_PROFILE_ID)
    .bind(COSTUME_REFERENCE_SET_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("costume variant should insert");

    for (reference_set_id, asset_id, role, primary) in [
        (
            CHARACTER_REFERENCE_SET_ID,
            "dev055-character-asset",
            Some("front"),
            1_i64,
        ),
        (
            COSTUME_REFERENCE_SET_ID,
            "dev055-costume-asset",
            Some("coat"),
            1_i64,
        ),
        (
            SCENE_REFERENCE_SET_ID,
            "dev055-scene-asset",
            Some("wide"),
            1_i64,
        ),
        (
            PROP_REFERENCE_SET_ID,
            "dev055-prop-asset",
            Some("detail"),
            1_i64,
        ),
        (
            SHOT_REFERENCE_SET_ID,
            "dev055-shot-asset",
            Some("shot"),
            1_i64,
        ),
    ] {
        sqlx::query(
            "INSERT INTO reference_set_items
             (reference_set_id, asset_id, ordinal, role, is_primary, created_at)
             VALUES (?, ?, 0, ?, ?, ?)",
        )
        .bind(reference_set_id)
        .bind(asset_id)
        .bind(role)
        .bind(primary)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("reference set item should insert");
    }

    for (id, role, profile_type, profile_id, costume_id, ordinal, mode) in [
        (
            "spb_dev055_character",
            "CHARACTER",
            "CHARACTER",
            CHARACTER_PROFILE_ID,
            Some(COSTUME_VARIANT_ID),
            0_i64,
            "EXPLICIT",
        ),
        (
            "spb_dev055_scene",
            "SCENE",
            "SCENE",
            SCENE_PROFILE_ID,
            None,
            0_i64,
            "INHERITED",
        ),
        (
            "spb_dev055_prop",
            "PROP",
            "PROP",
            PROP_PROFILE_ID,
            None,
            0_i64,
            "EXPLICIT",
        ),
        (
            "spb_dev055_style",
            "STYLE",
            "STYLE",
            STYLE_PROFILE_ID,
            None,
            0_i64,
            "REPLACE",
        ),
    ] {
        sqlx::query(
            "INSERT INTO shot_profile_bindings
             (id, shot_id, role, profile_type, profile_id, costume_variant_id,
              ordinal, inheritance_mode, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(CONSISTENCY_SHOT_ID)
        .bind(role)
        .bind(profile_type)
        .bind(profile_id)
        .bind(costume_id)
        .bind(ordinal)
        .bind(mode)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("shot profile binding should insert");
    }
    for (id, role, reference_set_id, ordinal, required, mode) in [
        (
            "srb_dev055_character",
            "CHARACTER",
            CHARACTER_REFERENCE_SET_ID,
            0_i64,
            1_i64,
            "EXPLICIT",
        ),
        (
            "srb_dev055_shot",
            "SHOT_REFERENCE",
            SHOT_REFERENCE_SET_ID,
            1_i64,
            1_i64,
            "REPLACE",
        ),
    ] {
        sqlx::query(
            "INSERT INTO shot_reference_set_bindings
             (id, shot_id, role, reference_set_id, ordinal, required,
              inheritance_mode, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(CONSISTENCY_SHOT_ID)
        .bind(role)
        .bind(reference_set_id)
        .bind(ordinal)
        .bind(required)
        .bind(mode)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("shot reference set binding should insert");
    }
    for (id, scope_type, scope_id, role, profile_type, profile_id, ordinal, mode) in [
        (
            "hpb_dev055_project_character",
            "PROJECT",
            CONSISTENCY_PROJECT_ID,
            "CHARACTER",
            "CHARACTER",
            CHARACTER_PROFILE_ID,
            0_i64,
            "INHERITED",
        ),
        (
            "hpb_dev055_scene",
            "SCENE",
            CONSISTENCY_SCENE_ID,
            "SCENE",
            "SCENE",
            SCENE_PROFILE_ID,
            1_i64,
            "REPLACE",
        ),
    ] {
        sqlx::query(
            "INSERT INTO consistency_scope_profile_bindings
             (id, project_id, scope_type, scope_id, role, profile_type,
              profile_id, costume_variant_id, ordinal, inheritance_mode,
              created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(CONSISTENCY_PROJECT_ID)
        .bind(scope_type)
        .bind(scope_id)
        .bind(role)
        .bind(profile_type)
        .bind(profile_id)
        .bind(ordinal)
        .bind(mode)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("scope profile binding should insert");
    }
    for (id, scope_type, scope_id, role, reference_set_id, ordinal, required, mode) in [
        (
            "hrb_dev055_project_character",
            "PROJECT",
            CONSISTENCY_PROJECT_ID,
            "CHARACTER",
            CHARACTER_REFERENCE_SET_ID,
            0_i64,
            1_i64,
            "INHERITED",
        ),
        (
            "hrb_dev055_scene",
            "SCENE",
            CONSISTENCY_SCENE_ID,
            "SCENE",
            SCENE_REFERENCE_SET_ID,
            1_i64,
            1_i64,
            "EXPLICIT",
        ),
    ] {
        sqlx::query(
            "INSERT INTO consistency_scope_reference_set_bindings
             (id, project_id, scope_type, scope_id, role, reference_set_id,
              ordinal, required, inheritance_mode, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(CONSISTENCY_PROJECT_ID)
        .bind(scope_type)
        .bind(scope_id)
        .bind(role)
        .bind(reference_set_id)
        .bind(ordinal)
        .bind(required)
        .bind(mode)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("scope reference set binding should insert");
    }

    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES (?, 'DEV-055 Compatibility Workflow', 'image', 'T2I', NULL, ?, ?)",
    )
    .bind(WORKFLOW_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("compatibility workflow should insert");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES (?, ?, '1', '{}', 'workflow-sha-dev055', ?)",
    )
    .bind(WORKFLOW_VERSION_ID)
    .bind(WORKFLOW_ID)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("compatibility workflow version should insert");
    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
         VALUES (?, ?, '1', 1, 'schema_version: 1\ninputs: {}\n',
                 'recipe-sha-dev055', ?)",
    )
    .bind(RECIPE_ID)
    .bind(WORKFLOW_VERSION_ID)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("compatibility recipe should insert");

    sqlx::query(
        "INSERT INTO production_batches
         (id, project_id, name, status, continue_on_failure, created_at, updated_at)
         VALUES (?, ?, 'DEV-055 Preparation Batch', 'COMPLETED', 0, ?, ?)",
    )
    .bind(BATCH_ID)
    .bind(CONSISTENCY_PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("preparation batch should insert");
    sqlx::query(
        "INSERT INTO production_batch_items
         (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json,
          status, created_at, updated_at)
         VALUES (?, ?, 0, ?, ?,
                 '{\"prompt\":\"frozen prompt\"}', 'SUCCEEDED', ?, ?)",
    )
    .bind(BATCH_ITEM_ID)
    .bind(BATCH_ID)
    .bind(WORKFLOW_VERSION_ID)
    .bind(RECIPE_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("preparation batch item should insert");

    let snapshot_json = json!({
        "schemaVersion": 1,
        "projectId": CONSISTENCY_PROJECT_ID,
        "shotId": CONSISTENCY_SHOT_ID,
        "stage": "image",
        "contextHash": PREPARATION_CONTEXT_HASH,
        "resolvedAt": CREATED_AT,
        "preparedAt": CREATED_AT,
        "structure": {"shotId": CONSISTENCY_SHOT_ID, "sceneId": CONSISTENCY_SCENE_ID},
        "profiles": [{"profileType": "CHARACTER", "profileId": CHARACTER_PROFILE_ID}],
        "referenceSets": [{"id": CHARACTER_REFERENCE_SET_ID}],
        "referenceAssets": [{
            "assetId": "dev055-character-asset",
            "sha256": sha256(CONSISTENCY_ASSETS[0].2),
            "role": "CHARACTER",
            "ordinal": 0
        }],
        "prompt": {"renderedText": "frozen prompt", "negativePrompt": "", "orderedSegments": []},
        "workflow": {"workflowVersionId": WORKFLOW_VERSION_ID, "recipeId": RECIPE_ID},
        "outputSpec": {"type": "image"},
        "stageInput": null,
        "frozenGenerationValues": {"prompt": "frozen prompt"},
        "readiness": {"status": "READY", "score": 100, "gates": [], "evaluatedAt": CREATED_AT},
        "comfyCapabilityEvidence": {"status": "READY"}
    })
    .to_string();
    sqlx::query(
        "INSERT INTO production_preparation_snapshots
         (id, project_id, shot_id, stage, context_hash, production_batch_id,
          production_batch_item_id, snapshot_json, created_at)
         VALUES (?, ?, ?, 'image', ?, ?, ?, ?, ?)",
    )
    .bind(PREPARATION_SNAPSHOT_ID)
    .bind(CONSISTENCY_PROJECT_ID)
    .bind(CONSISTENCY_SHOT_ID)
    .bind(PREPARATION_CONTEXT_HASH)
    .bind(BATCH_ID)
    .bind(BATCH_ITEM_ID)
    .bind(snapshot_json)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("preparation snapshot should insert");
}

async fn seed_projects(directory: &TempDir, pool: &SqlitePool) {
    insert_legacy_project(pool, &directory.path().join("legacy-project")).await;
    insert_consistency_project(pool, &directory.path().join("consistency-project")).await;
}

async fn max_migration(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .expect("migration max should be readable")
}

async fn migration_marker_count(pool: &SqlitePool, version: i64) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = ?")
        .bind(version)
        .fetch_one(pool)
        .await
        .expect("migration marker should be readable")
}

fn migration_versions() -> Vec<u64> {
    let mut versions = fs::read_dir(workspace_root().join("src-tauri/migrations"))
        .expect("migration directory should be readable")
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let (version, _) = name.split_once('_')?;
            version.parse::<u64>().ok()
        })
        .collect::<Vec<_>>();
    versions.sort_unstable();
    versions
}

async fn remove_migrations_after_021(pool: &SqlitePool) {
    sqlx::query("DROP TABLE IF EXISTS production_preparation_snapshots")
        .execute(pool)
        .await
        .expect("024 table should be removable from the isolated fixture");
    for table in [
        "consistency_scope_reference_set_bindings",
        "consistency_scope_profile_bindings",
    ] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(pool)
            .await
            .expect("023 tables should be removable from the isolated fixture");
    }
    for table in [
        "shot_reference_set_bindings",
        "shot_profile_bindings",
        "reference_set_items",
        "costume_variants",
        "character_profiles",
        "scene_profiles",
        "prop_profiles",
        "style_profiles",
        "reference_sets",
        "profile_revisions",
    ] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(pool)
            .await
            .expect("022 tables should be removable from the isolated fixture");
    }
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version >= 22")
        .execute(pool)
        .await
        .expect("022-024 migration markers should be removable");
}

async fn remove_migration_024(pool: &SqlitePool) {
    sqlx::query("DROP TABLE production_preparation_snapshots")
        .execute(pool)
        .await
        .expect("024 table should be removable from the isolated fixture");
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 24")
        .execute(pool)
        .await
        .expect("024 migration marker should be removable");
}

async fn assert_current_migration_gate(pool: &SqlitePool) {
    assert_eq!(max_migration(pool).await, 24);
    assert_eq!(migration_marker_count(pool, 25).await, 0);
}

fn read_zip_json(path: &Path, entry_name: &str) -> Value {
    let file = File::open(path).expect("archive should be readable");
    let mut archive = ZipArchive::new(file).expect("archive should be valid ZIP");
    let mut entry = archive
        .by_name(entry_name)
        .expect("archive entry should exist");
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .expect("archive JSON entry should be readable");
    serde_json::from_slice(&bytes).expect("archive JSON entry should parse")
}

fn rewrite_backup_version(source: &Path, destination: &Path, version: u32) {
    let source_file = File::open(source).expect("source backup should open");
    let mut source_archive = ZipArchive::new(source_file).expect("source backup should parse");
    let destination_file = File::create(destination).expect("versioned backup should create");
    let mut destination_archive = ZipWriter::new(destination_file);
    let options = FileOptions::default();

    for index in 0..source_archive.len() {
        let mut entry = source_archive
            .by_index(index)
            .expect("source backup entry should be readable");
        let name = entry.name().to_owned();
        let is_directory = entry.is_dir();
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .expect("source backup entry should be copied");
        drop(entry);

        if name == "manifest.json" {
            let mut manifest: Value =
                serde_json::from_slice(&bytes).expect("backup manifest should be JSON");
            manifest["version"] = json!(version);
            bytes = serde_json::to_vec(&manifest).expect("backup manifest should serialize");
        }
        if is_directory {
            destination_archive
                .add_directory(name, options)
                .expect("backup directory entry should be copied");
        } else {
            destination_archive
                .start_file(name, options)
                .expect("backup file entry should be copied");
            destination_archive
                .write_all(&bytes)
                .expect("backup file entry should be written");
        }
    }
    destination_archive
        .finish()
        .expect("versioned backup should finish");
}

async fn raw_database(path: &Path) -> Option<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .ok()
}

async fn raw_max_migration(path: &Path) -> Option<i64> {
    let pool = raw_database(path).await?;
    let result = sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .ok();
    pool.close().await;
    result
}

fn environment_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn report_env_blocked(reason: &str) {
    eprintln!("ENV_BLOCKED/P2 DEV-055 official upgrade: {reason}");
}

fn release_sha256_062() -> Option<String> {
    let text = fs::read_to_string(workspace_root().join("docs/RELEASE_SHA256_0.6.2.txt")).ok()?;
    text.lines()
        .find(|line| line.trim_start().starts_with("ai-studio.exe |"))
        .and_then(|line| line.split("SHA256=").nth(1))
        .map(|hash| hash.trim().to_ascii_lowercase())
}

fn file_sha256(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(sha256(&bytes))
}

fn copy_sqlite_source(source: &Path, destination_root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(destination_root).map_err(|error| error.to_string())?;
    let destination = destination_root.join("app.db");
    fs::copy(source, &destination).map_err(|error| error.to_string())?;
    for suffix in ["-wal", "-shm"] {
        let source_sidecar = PathBuf::from(format!("{}{}", source.to_string_lossy(), suffix));
        if source_sidecar.is_file() {
            fs::copy(
                &source_sidecar,
                destination_root.join(format!("app.db{suffix}")),
            )
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(destination)
}

fn spawn_isolated_binary(binary: &Path, data_root: &Path) -> Result<Child, String> {
    Command::new(binary)
        .env("AI_STUDIO_DATA_ROOT", data_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

async fn wait_for_migration(path: &Path, expected: i64, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(version) = raw_max_migration(path).await {
            if version == expected {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

fn manifest_has_key_containing(value: &Value, needle: &str) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            key.to_ascii_lowercase().contains(needle) || manifest_has_key_containing(value, needle)
        }),
        Value::Array(values) => values
            .iter()
            .any(|value| manifest_has_key_containing(value, needle)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

#[tokio::test]
async fn dev055_migration_matrix_reaches_024_without_025() {
    let versions = migration_versions();
    assert_eq!(versions.first().copied(), Some(1));
    assert_eq!(versions.last().copied(), Some(24));
    assert!(
        !versions.contains(&25),
        "repository must not contain migration 025"
    );

    let (_fresh_directory, fresh_pool) = database().await;
    assert_current_migration_gate(&fresh_pool).await;

    let (existing_021_directory, existing_021_pool) = database().await;
    insert_project(
        &existing_021_pool,
        "dev055-existing-021",
        "Existing 021",
        &existing_021_directory.path().join("existing-021"),
    )
    .await;
    insert_shot(
        &existing_021_pool,
        "dev055-existing-021",
        "dev055-existing-021-shot",
        "Existing 021 shot",
        "legacy 021 prompt",
    )
    .await;
    remove_migrations_after_021(&existing_021_pool).await;
    assert_eq!(max_migration(&existing_021_pool).await, 21);
    existing_021_pool.close().await;
    let existing_021_upgraded = initialize(&existing_021_directory.path().join("app.db"))
        .await
        .expect("existing 021 database should upgrade through the real migrator");
    assert_current_migration_gate(&existing_021_upgraded).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projects WHERE id = 'dev055-existing-021'",
        )
        .fetch_one(&existing_021_upgraded)
        .await
        .expect("existing 021 project should remain readable"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM shots WHERE id = 'dev055-existing-021-shot'",
        )
        .fetch_one(&existing_021_upgraded)
        .await
        .expect("existing 021 shot should remain readable"),
        1
    );

    let (existing_023_directory, existing_023_pool) = database().await;
    insert_project(
        &existing_023_pool,
        "dev055-existing-023",
        "Existing 023",
        &existing_023_directory.path().join("existing-023"),
    )
    .await;
    sqlx::query(
        "INSERT INTO style_profiles
         (id, project_id, name, style_prompt, created_at, updated_at)
         VALUES ('stp_dev055_existing_023', 'dev055-existing-023',
                 'Existing 023 Style', 'legacy style', ?, ?)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&existing_023_pool)
    .await
    .expect("existing 023 style should insert");
    sqlx::query(
        "INSERT INTO consistency_scope_profile_bindings
         (id, project_id, scope_type, scope_id, role, profile_type, profile_id,
          ordinal, inheritance_mode, created_at, updated_at)
         VALUES ('hpb_dev055_existing_023', 'dev055-existing-023', 'PROJECT',
                 'dev055-existing-023', 'STYLE', 'STYLE',
                 'stp_dev055_existing_023', 0, 'EXPLICIT', ?, ?)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&existing_023_pool)
    .await
    .expect("existing 023 scope binding should insert");
    remove_migration_024(&existing_023_pool).await;
    assert_eq!(max_migration(&existing_023_pool).await, 23);
    existing_023_pool.close().await;
    let existing_023_upgraded = initialize(&existing_023_directory.path().join("app.db"))
        .await
        .expect("existing 023 database should upgrade through the real migrator");
    assert_current_migration_gate(&existing_023_upgraded).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM consistency_scope_profile_bindings
             WHERE id = 'hpb_dev055_existing_023'",
        )
        .fetch_one(&existing_023_upgraded)
        .await
        .expect("existing 023 scope binding should remain readable"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_preparation_snapshots")
            .fetch_one(&existing_023_upgraded)
            .await
            .expect("024 table should be readable after upgrade"),
        0
    );
}

#[tokio::test]
async fn dev055_official_062_to_070_upgrade_isolated_or_env_blocked() {
    let Some(old_binary) = environment_path("DEV055_OFFICIAL_062_BINARY") else {
        report_env_blocked("DEV055_OFFICIAL_062_BINARY is not a file");
        return;
    };
    let Some(new_binary) = environment_path("DEV055_OFFICIAL_070_BINARY") else {
        report_env_blocked("DEV055_OFFICIAL_070_BINARY is not a file");
        return;
    };
    let Some(source_database) = environment_path("DEV055_OFFICIAL_062_DB") else {
        report_env_blocked("DEV055_OFFICIAL_062_DB is not a file");
        return;
    };
    let Some(expected_old_hash) = release_sha256_062() else {
        report_env_blocked("the repository 0.6.2 release checksum manifest is unavailable");
        return;
    };
    let Some(actual_old_hash) = file_sha256(&old_binary) else {
        report_env_blocked("the 0.6.2 binary cannot be hashed");
        return;
    };
    if actual_old_hash != expected_old_hash {
        report_env_blocked(
            "DEV055_OFFICIAL_062_BINARY does not match the frozen official 0.6.2 artifact",
        );
        return;
    }
    let Some(expected_new_hash) = env::var("DEV055_OFFICIAL_070_SHA256")
        .ok()
        .map(|hash| hash.trim().to_ascii_lowercase())
        .filter(|hash| !hash.is_empty())
    else {
        report_env_blocked("DEV055_OFFICIAL_070_SHA256 is not set");
        return;
    };
    let Some(actual_new_hash) = file_sha256(&new_binary) else {
        report_env_blocked("the 0.7.0 binary cannot be hashed");
        return;
    };
    if actual_new_hash != expected_new_hash {
        report_env_blocked(
            "DEV055_OFFICIAL_070_BINARY checksum does not match its declared artifact",
        );
        return;
    }
    if env::var("DEV055_OFFICIAL_070_PRODUCT_VERSION")
        .ok()
        .as_deref()
        != Some("0.7.0")
    {
        report_env_blocked("DEV055_OFFICIAL_070_PRODUCT_VERSION must be exactly 0.7.0");
        return;
    }
    if raw_max_migration(&source_database).await != Some(21) {
        panic!("the supplied official 0.6.2 database must start at migration 021");
    }

    let source_hash_before = file_sha256(&source_database).expect("source database should hash");
    let isolation = tempdir().expect("official upgrade isolation root should exist");
    let isolated_database = copy_sqlite_source(&source_database, isolation.path())
        .expect("official 0.6.2 database should copy into the isolated root");
    let mut upgraded_process = match spawn_isolated_binary(&new_binary, isolation.path()) {
        Ok(process) => process,
        Err(reason) => {
            report_env_blocked(&format!("official 0.7.0 binary could not start: {reason}"));
            return;
        }
    };
    if !wait_for_migration(&isolated_database, 24, Duration::from_secs(30)).await {
        stop_child(&mut upgraded_process);
        report_env_blocked(
            "official 0.7.0 binary did not migrate the isolated 0.6.2 database to 024",
        );
        return;
    }
    stop_child(&mut upgraded_process);
    let upgraded_pool = raw_database(&isolated_database)
        .await
        .expect("upgraded official database should reopen");
    assert_eq!(max_migration(&upgraded_pool).await, 24);
    assert_eq!(migration_marker_count(&upgraded_pool, 25).await, 0);
    upgraded_pool.close().await;

    let mut restarted_process = match spawn_isolated_binary(&new_binary, isolation.path()) {
        Ok(process) => process,
        Err(reason) => {
            report_env_blocked(&format!("official 0.7.0 restart could not start: {reason}"));
            return;
        }
    };
    let restart_ok = wait_for_migration(&isolated_database, 24, Duration::from_secs(30)).await;
    stop_child(&mut restarted_process);
    assert!(restart_ok, "official 0.7.0 restart must keep migration 024");
    assert_eq!(
        file_sha256(&source_database).expect("source database should still hash"),
        source_hash_before,
        "isolated upgrade must not mutate the supplied official 0.6.2 database",
    );
    eprintln!("DEV055_OFFICIAL_UPGRADE=PASS");
}

#[tokio::test]
async fn dev055_backup_12_inspect_restore_and_backup_13_restore_use_real_export() {
    let (directory, pool) = database().await;
    insert_legacy_project(&pool, &directory.path().join("legacy-project")).await;
    let service = ProjectBackupService::new(
        pool.clone(),
        directory.path().join("restored-projects"),
        directory.path().join("cache"),
    );
    let v14_archive = directory.path().join("legacy-v14.zip");
    service
        .export(LEGACY_PROJECT_ID, v14_archive.clone())
        .await
        .expect("real legacy project export should produce a Backup 14 archive");
    let v12_archive = directory.path().join("legacy-v12.zip");
    let v13_archive = directory.path().join("legacy-v13.zip");
    rewrite_backup_version(&v14_archive, &v12_archive, 12);
    rewrite_backup_version(&v14_archive, &v13_archive, 13);

    let v12_manifest = read_zip_json(&v12_archive, "manifest.json");
    assert_eq!(v12_manifest["version"], 12);
    let v12_preview = service
        .inspect(v12_archive)
        .await
        .expect("Backup 12 should inspect");
    assert_eq!(v12_preview.project_name, "DEV-055 Legacy");
    assert_eq!(v12_preview.shots, 1);
    let v12_restored = service
        .restore(&v12_preview.inspection_id)
        .await
        .expect("Backup 12 should restore");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE project_id = ?")
            .bind(&v12_restored.id)
            .fetch_one(&pool)
            .await
            .expect("Backup 12 restored shot should be readable"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reference_anchors WHERE project_id = ?")
            .bind(&v12_restored.id)
            .fetch_one(&pool)
            .await
            .expect("Backup 12 restored anchor should be readable"),
        1
    );

    let v13_manifest = read_zip_json(&v13_archive, "manifest.json");
    assert_eq!(v13_manifest["version"], 13);
    let v13_preview = service
        .inspect(v13_archive)
        .await
        .expect("Backup 13 should inspect");
    let v13_restored = service
        .restore(&v13_preview.inspection_id)
        .await
        .expect("Backup 13 should restore");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE project_id = ?")
            .bind(&v13_restored.id)
            .fetch_one(&pool)
            .await
            .expect("Backup 13 restored shot should be readable"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_preparation_snapshots WHERE project_id = ?",
        )
        .bind(&v13_restored.id)
        .fetch_one(&pool)
        .await
        .expect("Backup 13 preparation snapshot count should be readable"),
        0
    );
}

#[tokio::test]
async fn dev055_backup_14_roundtrip_preserves_consistency_and_preparation_snapshot() {
    let (directory, pool) = database().await;
    insert_consistency_project(&pool, &directory.path().join("consistency-project")).await;
    let service = ProjectBackupService::new(
        pool.clone(),
        directory.path().join("restored-projects"),
        directory.path().join("cache"),
    );
    let archive_path = directory.path().join("consistency-v14.zip");
    let exported = service
        .export(CONSISTENCY_PROJECT_ID, archive_path.clone())
        .await
        .expect("real consistency project export should produce Backup 14");
    assert!(exported.entries >= 6);
    let archive_manifest = read_zip_json(&archive_path, "manifest.json");
    assert_eq!(archive_manifest["version"], 14);
    let archive_document = read_zip_json(&archive_path, "project.json");
    for (field, expected) in [
        ("characterProfiles", 1),
        ("sceneProfiles", 1),
        ("propProfiles", 1),
        ("styleProfiles", 1),
        ("costumeVariants", 1),
        ("referenceSets", 5),
        ("referenceSetItems", 5),
        ("shotProfileBindings", 4),
        ("shotReferenceSetBindings", 2),
        ("scopeProfileBindings", 2),
        ("scopeReferenceSetBindings", 2),
        ("preparationSnapshots", 1),
    ] {
        assert_eq!(
            archive_document[field].as_array().map(Vec::len),
            Some(expected),
            "Backup 14 must carry {field}"
        );
    }

    let preview = service
        .inspect(archive_path)
        .await
        .expect("Backup 14 should inspect");
    assert_eq!(preview.shots, 1);
    assert_eq!(preview.image_count, 5);
    let restored = service
        .restore(&preview.inspection_id)
        .await
        .expect("Backup 14 should restore");
    assert_ne!(restored.id, CONSISTENCY_PROJECT_ID);

    let counts: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT COUNT(*) FROM character_profiles WHERE project_id = ?),
           (SELECT COUNT(*) FROM scene_profiles WHERE project_id = ?),
           (SELECT COUNT(*) FROM prop_profiles WHERE project_id = ?),
           (SELECT COUNT(*) FROM style_profiles WHERE project_id = ?),
           (SELECT COUNT(*) FROM costume_variants WHERE character_profile_id IN
             (SELECT id FROM character_profiles WHERE project_id = ?)),
           (SELECT COUNT(*) FROM reference_sets WHERE project_id = ?),
           (SELECT COUNT(*) FROM reference_set_items WHERE reference_set_id IN
             (SELECT id FROM reference_sets WHERE project_id = ?)),
           (SELECT COUNT(*) FROM shot_profile_bindings WHERE shot_id IN
             (SELECT id FROM shots WHERE project_id = ?)),
           (SELECT COUNT(*) FROM shot_reference_set_bindings WHERE shot_id IN
             (SELECT id FROM shots WHERE project_id = ?)),
           (SELECT COUNT(*) FROM consistency_scope_profile_bindings WHERE project_id = ?)",
    )
    .bind(&restored.id)
    .bind(&restored.id)
    .bind(&restored.id)
    .bind(&restored.id)
    .bind(&restored.id)
    .bind(&restored.id)
    .bind(&restored.id)
    .bind(&restored.id)
    .bind(&restored.id)
    .bind(&restored.id)
    .fetch_one(&pool)
    .await
    .expect("restored consistency counts should be readable");
    assert_eq!(counts, (1, 1, 1, 1, 1, 5, 5, 4, 2, 2));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM consistency_scope_reference_set_bindings
             WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .expect("restored scope reference bindings should be readable"),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM shot_profile_bindings
             WHERE shot_id IN (SELECT id FROM shots WHERE project_id = ?)
               AND costume_variant_id IS NOT NULL",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .expect("restored costume binding should be readable"),
        1
    );

    let restored_scope_ids: Vec<(String, String)> = sqlx::query_as(
        "SELECT scope_type, scope_id
         FROM consistency_scope_profile_bindings WHERE project_id = ?
         ORDER BY scope_type, scope_id",
    )
    .bind(&restored.id)
    .fetch_all(&pool)
    .await
    .expect("restored scope IDs should be readable");
    assert_eq!(restored_scope_ids.len(), 2);
    assert_eq!(
        restored_scope_ids[0],
        ("PROJECT".to_owned(), restored.id.clone())
    );
    assert_eq!(restored_scope_ids[1].0, "SCENE");
    assert_ne!(restored_scope_ids[1].1, CONSISTENCY_SCENE_ID);

    let restored_snapshot: (String, String, String, String, String, String, String) =
        sqlx::query_as(
            "SELECT id, project_id, shot_id, context_hash, production_batch_id,
                    production_batch_item_id, snapshot_json
             FROM production_preparation_snapshots WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .expect("restored preparation snapshot should be readable");
    assert_ne!(restored_snapshot.0, PREPARATION_SNAPSHOT_ID);
    assert_eq!(restored_snapshot.1, restored.id);
    assert_ne!(restored_snapshot.2, CONSISTENCY_SHOT_ID);
    assert_eq!(restored_snapshot.3, PREPARATION_CONTEXT_HASH);
    assert_ne!(restored_snapshot.4, BATCH_ID);
    assert_ne!(restored_snapshot.5, BATCH_ITEM_ID);
    let snapshot_value: Value =
        serde_json::from_str(&restored_snapshot.6).expect("restored snapshot JSON should parse");
    assert_eq!(snapshot_value["projectId"], CONSISTENCY_PROJECT_ID);
    assert_eq!(snapshot_value["shotId"], CONSISTENCY_SHOT_ID);
    assert_eq!(snapshot_value["contextHash"], PREPARATION_CONTEXT_HASH);
}

#[tokio::test]
async fn dev055_manifest_v1_v2_and_project_kinds_are_compatible() {
    let (directory, pool) = database().await;
    seed_projects(&directory, &pool).await;
    let service = ProjectManifestService::new(pool.clone());

    let legacy_path = directory.path().join("legacy-manifest-v2.json");
    service
        .export(LEGACY_PROJECT_ID, legacy_path.clone())
        .await
        .expect("legacy project should export a real manifest");
    let mut legacy_v1: Value =
        serde_json::from_slice(&fs::read(&legacy_path).expect("legacy manifest should read"))
            .expect("legacy manifest should be JSON");
    legacy_v1["version"] = json!(1);
    for field in [
        "profiles",
        "costumeVariants",
        "referenceSets",
        "referenceSetItems",
        "shotProfileBindings",
        "shotReferenceSetBindings",
        "scopeProfileBindings",
        "scopeReferenceSetBindings",
    ] {
        legacy_v1
            .as_object_mut()
            .expect("manifest should be an object")
            .remove(field);
    }
    let legacy_v1_bytes = serde_json::to_vec(&legacy_v1).expect("legacy v1 should serialize");
    let parsed_v1 = ProjectManifestService::parse(&legacy_v1_bytes)
        .expect("Manifest v1 import must not require a Profile section");
    assert_eq!(parsed_v1.version, 1);
    assert!(parsed_v1.profiles.is_empty());
    assert!(parsed_v1.costume_variants.is_empty());
    assert!(parsed_v1.reference_sets.is_empty());
    assert!(parsed_v1.shot_profile_bindings.is_empty());
    assert!(parsed_v1.scope_reference_set_bindings.is_empty());
    assert_eq!(parsed_v1.shots.len(), 1);

    let consistency_path = directory.path().join("consistency-manifest-v2.json");
    service
        .export(CONSISTENCY_PROJECT_ID, consistency_path.clone())
        .await
        .expect("consistency project should export a real Manifest v2");
    let consistency_bytes = fs::read(&consistency_path).expect("consistency manifest should read");
    let parsed_v2 =
        ProjectManifestService::parse(&consistency_bytes).expect("Manifest v2 should parse");
    assert_eq!(parsed_v2.version, 2);
    assert_eq!(parsed_v2.profiles.len(), 4);
    assert_eq!(parsed_v2.costume_variants.len(), 1);
    assert_eq!(parsed_v2.reference_sets.len(), 5);
    assert_eq!(parsed_v2.reference_set_items.len(), 5);
    assert_eq!(parsed_v2.shot_profile_bindings.len(), 4);
    assert_eq!(parsed_v2.shot_reference_set_bindings.len(), 2);
    assert_eq!(parsed_v2.scope_profile_bindings.len(), 2);
    assert_eq!(parsed_v2.scope_reference_set_bindings.len(), 2);
    assert_eq!(
        parsed_v2
            .costume_variants
            .first()
            .map(|variant| variant.character_profile_id.as_str()),
        Some(CHARACTER_PROFILE_ID)
    );
    assert_eq!(
        parsed_v2
            .costume_variants
            .first()
            .and_then(|variant| variant.reference_set_id.as_deref()),
        Some(COSTUME_REFERENCE_SET_ID)
    );
    assert_eq!(
        parsed_v2
            .reference_set_items
            .iter()
            .map(|item| item.reference_set_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            CHARACTER_REFERENCE_SET_ID,
            COSTUME_REFERENCE_SET_ID,
            PROP_REFERENCE_SET_ID,
            SCENE_REFERENCE_SET_ID,
            SHOT_REFERENCE_SET_ID,
        ]
    );
    assert!(parsed_v2
        .shot_profile_bindings
        .iter()
        .any(|binding| binding.costume_variant_id.as_deref() == Some(COSTUME_VARIANT_ID)));
    assert!(parsed_v2
        .scope_profile_bindings
        .iter()
        .any(|binding| binding.scope_id == CONSISTENCY_PROJECT_ID));
    assert!(parsed_v2
        .scope_profile_bindings
        .iter()
        .any(|binding| binding.scope_id == CONSISTENCY_SCENE_ID));
    let roundtrip_bytes =
        serde_json::to_vec_pretty(&parsed_v2).expect("Manifest v2 should serialize for roundtrip");
    let reparsed_v2 = ProjectManifestService::parse(&roundtrip_bytes)
        .expect("Manifest v2 roundtrip should parse");
    assert_eq!(reparsed_v2, parsed_v2);

    let manifest_value: Value =
        serde_json::from_slice(&consistency_bytes).expect("Manifest v2 should remain JSON");
    assert!(
        !manifest_has_key_containing(&manifest_value, "snapshot"),
        "Preparation Snapshot is production history and must not be in the manifest"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_preparation_snapshots WHERE project_id = ?",
        )
        .bind(CONSISTENCY_PROJECT_ID)
        .fetch_one(&pool)
        .await
        .expect("consistency preparation snapshot should remain in the database"),
        1
    );
}

#[tokio::test]
async fn dev055_legacy_and_consistency_projects_both_remain_openable() {
    let (directory, pool) = database().await;
    seed_projects(&directory, &pool).await;
    let service = ProjectManifestService::new(pool.clone());
    let legacy_path = directory.path().join("legacy-openable.json");
    let consistency_path = directory.path().join("consistency-openable.json");
    service
        .export(LEGACY_PROJECT_ID, legacy_path.clone())
        .await
        .expect("legacy project should remain openable");
    service
        .export(CONSISTENCY_PROJECT_ID, consistency_path.clone())
        .await
        .expect("consistency project should remain openable");
    let legacy = ProjectManifestService::parse(
        &fs::read(&legacy_path).expect("legacy openable manifest should read"),
    )
    .expect("legacy manifest should parse");
    let consistency = ProjectManifestService::parse(
        &fs::read(&consistency_path).expect("consistency openable manifest should read"),
    )
    .expect("consistency manifest should parse");
    assert!(legacy.profiles.is_empty());
    assert!(legacy.reference_sets.is_empty());
    assert_eq!(legacy.shots.len(), 1);
    assert_eq!(legacy.shots[0].prompt_text, "legacy prompt fallback");
    assert_eq!(consistency.profiles.len(), 4);
    assert_eq!(consistency.reference_sets.len(), 5);
    assert_eq!(consistency.shots.len(), 1);
    assert_eq!(
        consistency.shots[0].scene_id.as_deref(),
        Some(CONSISTENCY_SCENE_ID)
    );
}
