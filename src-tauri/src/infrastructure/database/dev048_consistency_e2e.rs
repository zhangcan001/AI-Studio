//! DEV-048 persistence and compatibility tests.
//!
//! These tests exercise the SQLite/application boundary without starting a
//! runtime, submitting a generation request, or touching the filesystem
//! outside temporary SQLite databases.

use super::{initialize, repositories::test_support};
use crate::application::consistency_profile_service::{
    ConsistencyProfileService, CreateCharacterProfileRequest, CreateCostumeVariantRequest,
    CreatePropProfileRequest, CreateSceneProfileRequest, CreateStyleProfileRequest,
    UpdateCharacterProfileRequest, UpdateCostumeVariantRequest, UpdatePropProfileRequest,
    UpdateSceneProfileRequest, UpdateStyleProfileRequest,
};
use crate::application::ports::{
    AssetRepository, Clock, ConsistencyProfileRepository, ProjectRepository,
    ReferenceAnchorRepository, ReferenceSetRepository, ShotConsistencyRepository,
};
use crate::application::reference_set_service::{
    CreateReferenceSetRequest, ReferenceSetItemRequest, ReferenceSetService,
};
use crate::domain::consistency::{
    BindingRole, CharacterProfile, ConsistencyProfileRecord, CostumeVariant, InheritanceMode,
    ProfileRevision, ProfileRevisionStatus, ProfileType, PropProfile, ReferenceSet,
    ReferenceSetItem, ReferenceSetPurpose, SceneProfile, ShotProfileBinding,
    ShotReferenceSetBinding, StyleProfile,
};
use crate::domain::ReferenceAnchorId;
use chrono::{DateTime, TimeZone, Utc};
use std::{fs, path::PathBuf, sync::Arc};
use tempfile::{tempdir, TempDir};

const CREATED_AT: &str = "2026-08-26T00:00:00Z";
const PROJECT_ONE: &str = "project-1";
const PROJECT_TWO: &str = "project-2";
const SHOT_ID: &str = "shot-dev048";
const ANCHOR_ID: &str = "anc_dev048";
const STYLE_ID: &str = "stp-dev048-style";
const CHARACTER_ID: &str = "cp-dev048-character";
const SCENE_ID: &str = "scp-dev048-scene";
const PROP_ID: &str = "pp-dev048-prop";
const REFERENCE_SET_ID: &str = "rs-dev048-reference";
const COSTUME_ID: &str = "cv-dev048-costume";
const REVISION_ID: &str = "prv-dev048-revision";

struct FixedClock(DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 26, 0, 0, 0)
        .single()
        .expect("test timestamp should be valid")
}

async fn setup() -> (TempDir, sqlx::SqlitePool) {
    let directory = tempdir().expect("temporary directory should exist");
    let pool = initialize(&directory.path().join("dev048.db"))
        .await
        .expect("database should initialize");
    test_support::seed_task_dependencies(&pool).await;
    insert_project(&pool, PROJECT_TWO).await;
    (directory, pool)
}

async fn insert_project(pool: &sqlx::SqlitePool, project_id: &str) {
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES (?, ?, '', ?, ?, ?)",
    )
    .bind(project_id)
    .bind(project_id)
    .bind(format!("C:/{project_id}"))
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("project fixture should insert");
}

async fn insert_asset(pool: &sqlx::SqlitePool, id: &str, project_id: &str, media_type: &str) {
    let (category, mime_type) = match media_type {
        "image" => ("source_image", "image/png"),
        "video" => ("source_video", "video/mp4"),
        "audio" => ("source_audio", "audio/wav"),
        other => panic!("unsupported fixture media type: {other}"),
    };
    sqlx::query(
        "INSERT INTO assets
         (id, project_id, type, category, name, original_name, storage_path,
          sha256, mime_type, width, height, file_size, source_task_id, metadata_json,
          created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 2, 2, 1, NULL, '{}', ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(media_type)
    .bind(category)
    .bind(id)
    .bind(format!("{id}.asset"))
    .bind(format!("C:/{project_id}/{id}.asset"))
    .bind(format!("sha-{id}"))
    .bind(mime_type)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("asset fixture should insert");
}

async fn insert_shot(pool: &sqlx::SqlitePool, shot_id: &str, project_id: &str) {
    sqlx::query(
        "INSERT INTO shots
         (id, project_id, ordinal, name, prompt_text, prompt_entry_id,
          prompt_version_id, selected_image_asset_id, selected_video_asset_id,
          created_at, updated_at)
         VALUES (?, ?, 0, 'DEV-048 Shot', 'legacy prompt', NULL, NULL, NULL, NULL, ?, ?)",
    )
    .bind(shot_id)
    .bind(project_id)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("shot fixture should insert");
}

async fn insert_anchor(pool: &sqlx::SqlitePool, asset_ids: &[&str]) {
    sqlx::query(
        "INSERT INTO reference_anchors
         (id, project_id, kind, name, normalized_name, description, created_at, updated_at)
         VALUES (?, ?, 'CHARACTER', 'Legacy Anchor', 'legacy anchor', 'legacy description', ?, ?)",
    )
    .bind(ANCHOR_ID)
    .bind(PROJECT_ONE)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("anchor fixture should insert");
    for (ordinal, asset_id) in asset_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO reference_anchor_assets (anchor_id, asset_id, ordinal, created_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(ANCHOR_ID)
        .bind(asset_id)
        .bind(ordinal as i64)
        .bind(CREATED_AT)
        .execute(pool)
        .await
        .expect("anchor asset fixture should insert");
    }
}

async fn insert_legacy_sentinels(pool: &sqlx::SqlitePool) {
    insert_asset(pool, "ast_sentinel", PROJECT_ONE, "image").await;
    insert_shot(pool, "shot-sentinel", PROJECT_ONE).await;
    sqlx::query(
        "INSERT INTO prompt_entries
         (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at)
         VALUES ('prompt-sentinel', ?, 'prompt', 'Sentinel Prompt', 'sentinel prompt', '[]', ?, ?)",
    )
    .bind(PROJECT_ONE)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("prompt fixture should insert");
    sqlx::query(
        "INSERT INTO prompt_versions (id, prompt_id, version, text, created_at)
         VALUES ('prompt-version-sentinel', 'prompt-sentinel', 1, 'sentinel prompt', ?)",
    )
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("prompt version fixture should insert");
    insert_anchor(pool, &["ast_sentinel"]).await;
    sqlx::query(
        "INSERT INTO production_series
         (id, project_id, ordinal, name, description, created_at, updated_at)
         VALUES ('series-sentinel', ?, 0, 'Sentinel Series', '', ?, ?)",
    )
    .bind(PROJECT_ONE)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("series fixture should insert");
    sqlx::query(
        "INSERT INTO production_episodes
         (id, series_id, ordinal, name, description, created_at, updated_at)
         VALUES ('episode-sentinel', 'series-sentinel', 0, 'Sentinel Episode', '', ?, ?)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("episode fixture should insert");
    sqlx::query(
        "INSERT INTO production_scenes
         (id, episode_id, ordinal, name, description, created_at, updated_at)
         VALUES ('scene-sentinel', 'episode-sentinel', 0, 'Sentinel Scene', '', ?, ?)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("scene fixture should insert");
    sqlx::query(
        "INSERT INTO production_batches
         (id, project_id, name, status, continue_on_failure, created_at, updated_at)
         VALUES ('batch-sentinel', ?, 'Sentinel Batch', 'READY', 0, ?, ?)",
    )
    .bind(PROJECT_ONE)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("batch fixture should insert");
    sqlx::query(
        "INSERT INTO production_batch_items
         (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
          created_at, updated_at)
         VALUES ('batch-item-sentinel', 'batch-sentinel', 0, 'workflow-version-1',
                 'recipe-1', '{}', 'READY', ?, ?)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("batch item fixture should insert");
    sqlx::query(
        "INSERT INTO production_runs
         (id, project_id, name, status, current_stage_ordinal, created_at, updated_at)
         VALUES ('run-sentinel', ?, 'Sentinel Run', 'DRAFT', 0, ?, ?)",
    )
    .bind(PROJECT_ONE)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("production run fixture should insert");
}

async fn legacy_counts(pool: &sqlx::SqlitePool) -> Vec<i64> {
    let tables = [
        ("projects", "id = 'project-1'"),
        ("assets", "id = 'ast_sentinel'"),
        ("shots", "id = 'shot-sentinel'"),
        ("reference_anchors", "id = 'anc_dev048'"),
        ("prompt_entries", "id = 'prompt-sentinel'"),
        ("production_series", "id = 'series-sentinel'"),
        ("production_episodes", "id = 'episode-sentinel'"),
        ("production_scenes", "id = 'scene-sentinel'"),
        ("production_batches", "id = 'batch-sentinel'"),
        ("production_runs", "id = 'run-sentinel'"),
    ];
    let mut counts = Vec::with_capacity(tables.len());
    for (table, predicate) in tables {
        let query = format!("SELECT COUNT(*) FROM {table} WHERE {predicate}");
        counts.push(
            sqlx::query_scalar::<_, i64>(&query)
                .fetch_one(pool)
                .await
                .expect("legacy sentinel count should succeed"),
        );
    }
    counts
}

async fn remove_022_for_upgrade_fixture(pool: &sqlx::SqlitePool) {
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
        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(pool)
            .await
            .expect("022 table should be removable in isolated upgrade fixture");
    }
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 22")
        .execute(pool)
        .await
        .expect("022 migration marker should be removable in isolated fixture");
}

async fn remove_024_for_upgrade_fixture(pool: &sqlx::SqlitePool) {
    sqlx::query("DROP TABLE IF EXISTS script_import_drafts")
        .execute(pool)
        .await
        .expect("025 draft table should be removable in isolated upgrade fixture");
    sqlx::query("DROP TABLE IF EXISTS script_sources")
        .execute(pool)
        .await
        .expect("025 source table should be removable in isolated upgrade fixture");
    sqlx::query("DROP TABLE production_preparation_snapshots")
        .execute(pool)
        .await
        .expect("024 table should be removable in isolated fixture");
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version >= 24")
        .execute(pool)
        .await
        .expect("024 migration marker should be removable in isolated fixture");
}

fn profile_repository(pool: &sqlx::SqlitePool) -> Arc<dyn ConsistencyProfileRepository> {
    Arc::new(
        crate::infrastructure::database::repositories::SqliteConsistencyProfileRepository::new(
            pool.clone(),
        ),
    )
}

fn reference_set_repository(pool: &sqlx::SqlitePool) -> Arc<dyn ReferenceSetRepository> {
    Arc::new(
        crate::infrastructure::database::repositories::SqliteReferenceSetRepository::new(
            pool.clone(),
        ),
    )
}

fn services(pool: &sqlx::SqlitePool) -> (ConsistencyProfileService, ReferenceSetService) {
    let profile_repository = profile_repository(pool);
    let reference_repository = reference_set_repository(pool);
    let project_repository: Arc<dyn ProjectRepository> = Arc::new(
        crate::infrastructure::database::repositories::SqliteProjectRepository::new(pool.clone()),
    );
    let asset_repository: Arc<dyn AssetRepository> = Arc::new(
        crate::infrastructure::database::repositories::SqliteAssetRepository::new(pool.clone()),
    );
    let anchor_repository: Arc<dyn ReferenceAnchorRepository> = Arc::new(
        crate::infrastructure::database::repositories::SqliteReferenceAnchorRepository::new(
            pool.clone(),
        ),
    );
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(now()));
    let profiles = ConsistencyProfileService::new(
        profile_repository.clone(),
        reference_repository.clone(),
        project_repository.clone(),
        clock.clone(),
    );
    let reference_sets = ReferenceSetService::new(
        reference_repository,
        profile_repository,
        asset_repository,
        anchor_repository,
        project_repository,
        clock,
    );
    (profiles, reference_sets)
}

fn character_profile(id: &str, project_id: &str, name: &str) -> CharacterProfile {
    CharacterProfile {
        id: id.to_owned(),
        project_id: project_id.to_owned(),
        name: name.to_owned(),
        description: "character description".to_owned(),
        canonical_prompt: "character prompt".to_owned(),
        negative_prompt: "blurry".to_owned(),
        default_style_profile_id: None,
        default_reference_set_id: None,
        active_revision_id: None,
        metadata_json: "{}".to_owned(),
        created_at: now(),
        updated_at: now(),
    }
}

fn scene_profile(id: &str, project_id: &str, name: &str) -> SceneProfile {
    SceneProfile {
        id: id.to_owned(),
        project_id: project_id.to_owned(),
        name: name.to_owned(),
        description: "scene description".to_owned(),
        environment_prompt: "environment".to_owned(),
        lighting_prompt: Some("lighting".to_owned()),
        negative_prompt: Some("empty".to_owned()),
        default_style_profile_id: None,
        default_reference_set_id: None,
        active_revision_id: None,
        created_at: now(),
        updated_at: now(),
    }
}

fn prop_profile(id: &str, project_id: &str, name: &str) -> PropProfile {
    PropProfile {
        id: id.to_owned(),
        project_id: project_id.to_owned(),
        name: name.to_owned(),
        description: "prop description".to_owned(),
        canonical_prompt: "prop prompt".to_owned(),
        material_prompt: Some("brass".to_owned()),
        scale_prompt: Some("hand-sized".to_owned()),
        default_reference_set_id: None,
        active_revision_id: None,
        created_at: now(),
        updated_at: now(),
    }
}

fn style_profile(id: &str, project_id: &str, name: &str) -> StyleProfile {
    StyleProfile {
        id: id.to_owned(),
        project_id: project_id.to_owned(),
        name: name.to_owned(),
        style_prompt: "ink anime".to_owned(),
        color_prompt: Some("violet".to_owned()),
        line_prompt: Some("precise".to_owned()),
        negative_prompt: Some("photo".to_owned()),
        output_notes: Some("keep faces clear".to_owned()),
        active_revision_id: None,
        created_at: now(),
        updated_at: now(),
    }
}

fn reference_set(id: &str, project_id: &str, name: &str) -> ReferenceSet {
    ReferenceSet {
        id: id.to_owned(),
        project_id: project_id.to_owned(),
        name: name.to_owned(),
        purpose: ReferenceSetPurpose::Character,
        description: "reference description".to_owned(),
        owner_profile_type: None,
        owner_profile_id: None,
        active_revision_id: None,
        created_at: now(),
        updated_at: now(),
    }
}

fn item(reference_set_id: &str, asset_id: &str, ordinal: i64, primary: bool) -> ReferenceSetItem {
    ReferenceSetItem {
        reference_set_id: reference_set_id.to_owned(),
        asset_id: asset_id.to_owned(),
        ordinal,
        role: None,
        is_primary: primary,
        created_at: now(),
    }
}

fn revision(profile_type: ProfileType, profile_id: &str) -> ProfileRevision {
    ProfileRevision {
        id: REVISION_ID.to_owned(),
        profile_type,
        profile_id: profile_id.to_owned(),
        revision_number: 1,
        content_json: "{\"version\":1}".to_owned(),
        content_sha256: "a".repeat(64),
        status: ProfileRevisionStatus::Active,
        created_at: now(),
        created_by: Some("dev048".to_owned()),
    }
}

fn profile_binding(
    id: &str,
    shot_id: &str,
    role: BindingRole,
    profile_type: ProfileType,
    profile_id: &str,
    costume_variant_id: Option<&str>,
    ordinal: i64,
) -> ShotProfileBinding {
    ShotProfileBinding {
        id: id.to_owned(),
        shot_id: shot_id.to_owned(),
        role,
        profile_type,
        profile_id: profile_id.to_owned(),
        costume_variant_id: costume_variant_id.map(str::to_owned),
        ordinal,
        inheritance_mode: InheritanceMode::Explicit,
        created_at: now(),
        updated_at: now(),
    }
}

fn reference_binding(
    id: &str,
    shot_id: &str,
    role: BindingRole,
    reference_set_id: &str,
    ordinal: i64,
) -> ShotReferenceSetBinding {
    ShotReferenceSetBinding {
        id: id.to_owned(),
        shot_id: shot_id.to_owned(),
        role,
        reference_set_id: reference_set_id.to_owned(),
        ordinal,
        required: true,
        inheritance_mode: InheritanceMode::Explicit,
        created_at: now(),
        updated_at: now(),
    }
}

#[tokio::test]
async fn dev048_fresh_migration_001_to_025_creates_only_the_frozen_tables() {
    let directory = tempdir().unwrap();
    let pool = initialize(&directory.path().join("fresh.db"))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        25
    );
    let required_tables = [
        "profile_revisions",
        "reference_sets",
        "style_profiles",
        "character_profiles",
        "scene_profiles",
        "prop_profiles",
        "costume_variants",
        "reference_set_items",
        "shot_profile_bindings",
        "shot_reference_set_bindings",
        "production_preparation_snapshots",
        "script_sources",
        "script_import_drafts",
    ];
    for table in required_tables {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1,
            "missing table {table}"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0,
            "fresh table {table} must be empty"
        );
    }
    pool.close().await;
}

#[tokio::test]
async fn dev048_021_to_025_preserves_all_legacy_sentinels_and_leaves_new_tables_empty() {
    let (directory, pool) = setup().await;
    insert_legacy_sentinels(&pool).await;
    let before = legacy_counts(&pool).await;
    remove_022_for_upgrade_fixture(&pool).await;
    pool.close().await;

    let upgraded = initialize(&directory.path().join("dev048.db"))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&upgraded)
            .await
            .unwrap(),
        25
    );
    assert_eq!(legacy_counts(&upgraded).await, before);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT prompt_text FROM shots WHERE id = 'shot-sentinel'")
            .fetch_one(&upgraded)
            .await
            .unwrap(),
        "legacy prompt"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM reference_anchors WHERE id = 'anc_dev048'"
        )
        .fetch_one(&upgraded)
        .await
        .unwrap(),
        "Legacy Anchor"
    );
    for table in [
        "profile_revisions",
        "reference_sets",
        "style_profiles",
        "character_profiles",
        "scene_profiles",
        "prop_profiles",
        "costume_variants",
        "reference_set_items",
        "shot_profile_bindings",
        "shot_reference_set_bindings",
        "production_preparation_snapshots",
        "script_sources",
        "script_import_drafts",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&upgraded)
                .await
                .unwrap(),
            0,
            "upgrade must not backfill {table}"
        );
    }
}

#[tokio::test]
async fn dev052_existing_023_to_025_creates_preparation_snapshot_table() {
    let (directory, pool) = setup().await;
    remove_024_for_upgrade_fixture(&pool).await;
    pool.close().await;

    let upgraded = initialize(&directory.path().join("dev048.db"))
        .await
        .expect("DEV-052 migration upgrade should initialize");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&upgraded)
            .await
            .unwrap(),
        25
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name = 'production_preparation_snapshots'",
        )
        .fetch_one(&upgraded)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_preparation_snapshots",)
            .fetch_one(&upgraded)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn dev048_profile_service_persists_all_four_profiles_and_supports_update_delete() {
    let (_directory, pool) = setup().await;
    let (profiles, _reference_sets) = services(&pool);
    let style = profiles
        .create_style(CreateStyleProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            name: "  Style  ".to_owned(),
            style_prompt: "ink anime".to_owned(),
            color_prompt: Some("violet".to_owned()),
            line_prompt: None,
            negative_prompt: None,
            output_notes: None,
        })
        .await
        .unwrap();
    let character = profiles
        .create_character(CreateCharacterProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            name: "Character".to_owned(),
            description: "description".to_owned(),
            canonical_prompt: "character".to_owned(),
            negative_prompt: "blurry".to_owned(),
            default_style_profile_id: None,
            default_reference_set_id: None,
            metadata_json: "{}".to_owned(),
        })
        .await
        .unwrap();
    let scene = profiles
        .create_scene(CreateSceneProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            name: "Scene".to_owned(),
            description: "description".to_owned(),
            environment_prompt: "environment".to_owned(),
            lighting_prompt: Some("lighting".to_owned()),
            negative_prompt: None,
            default_style_profile_id: Some(style.id.clone()),
            default_reference_set_id: None,
        })
        .await
        .unwrap();
    let prop = profiles
        .create_prop(CreatePropProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            name: "Prop".to_owned(),
            description: "description".to_owned(),
            canonical_prompt: "prop".to_owned(),
            material_prompt: None,
            scale_prompt: None,
            default_reference_set_id: None,
        })
        .await
        .unwrap();
    assert_eq!(profiles.list(PROJECT_ONE, None).await.unwrap().len(), 4);
    assert!(matches!(
        profiles
            .get(PROJECT_ONE, ProfileType::Character, &character.id)
            .await
            .unwrap(),
        ConsistencyProfileRecord::Character(_)
    ));

    let updated = profiles
        .update_character(UpdateCharacterProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            profile_id: character.id.clone(),
            name: "Updated Character".to_owned(),
            description: "updated".to_owned(),
            canonical_prompt: "updated character".to_owned(),
            negative_prompt: "none".to_owned(),
            default_style_profile_id: Some(style.id.clone()),
            default_reference_set_id: None,
            metadata_json: "{\"updated\":true}".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(updated.id, character.id);
    assert_eq!(updated.project_id, PROJECT_ONE);
    assert_eq!(updated.created_at, character.created_at);
    assert_eq!(updated.name, "Updated Character");

    profiles
        .update_scene(UpdateSceneProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            profile_id: scene.id.clone(),
            name: "Updated Scene".to_owned(),
            description: "updated".to_owned(),
            environment_prompt: "updated environment".to_owned(),
            lighting_prompt: None,
            negative_prompt: None,
            default_style_profile_id: Some(style.id.clone()),
            default_reference_set_id: None,
        })
        .await
        .unwrap();
    profiles
        .update_prop(UpdatePropProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            profile_id: prop.id.clone(),
            name: "Updated Prop".to_owned(),
            description: "updated".to_owned(),
            canonical_prompt: "updated prop".to_owned(),
            material_prompt: None,
            scale_prompt: None,
            default_reference_set_id: None,
        })
        .await
        .unwrap();
    profiles
        .update_style(UpdateStyleProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            profile_id: style.id.clone(),
            name: "Updated Style".to_owned(),
            style_prompt: "updated style".to_owned(),
            color_prompt: None,
            line_prompt: None,
            negative_prompt: None,
            output_notes: None,
        })
        .await
        .unwrap();
    for (profile_type, id) in [
        (ProfileType::Character, character.id.as_str()),
        (ProfileType::Scene, scene.id.as_str()),
        (ProfileType::Prop, prop.id.as_str()),
        (ProfileType::Style, style.id.as_str()),
    ] {
        profiles
            .delete(PROJECT_ONE, profile_type, id)
            .await
            .unwrap();
    }
    assert!(profiles.list(PROJECT_ONE, None).await.unwrap().is_empty());
}

#[tokio::test]
async fn dev048_costume_and_revision_persistence_are_round_trip_and_revision_is_immutable() {
    let (_directory, pool) = setup().await;
    let (profiles, _reference_sets) = services(&pool);
    let character = profiles
        .create_character(CreateCharacterProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            name: "Character".to_owned(),
            description: String::new(),
            canonical_prompt: "character".to_owned(),
            negative_prompt: String::new(),
            default_style_profile_id: None,
            default_reference_set_id: None,
            metadata_json: "{}".to_owned(),
        })
        .await
        .unwrap();
    let costume = profiles
        .create_costume(CreateCostumeVariantRequest {
            project_id: PROJECT_ONE.to_owned(),
            character_profile_id: character.id.clone(),
            name: "Travel coat".to_owned(),
            prompt_fragment: "dark coat".to_owned(),
            reference_set_id: None,
            is_default: true,
            ordinal: 0,
        })
        .await
        .unwrap();
    assert_eq!(
        profiles
            .list_costumes(PROJECT_ONE, &character.id)
            .await
            .unwrap(),
        vec![costume.clone()]
    );
    let updated_costume = profiles
        .update_costume(UpdateCostumeVariantRequest {
            project_id: PROJECT_ONE.to_owned(),
            costume_variant_id: costume.id.clone(),
            name: "Updated coat".to_owned(),
            prompt_fragment: "updated coat".to_owned(),
            reference_set_id: None,
            is_default: false,
            ordinal: 1,
        })
        .await
        .unwrap();
    assert_eq!(updated_costume.id, costume.id);
    profiles
        .delete_costume(PROJECT_ONE, &costume.id)
        .await
        .unwrap();

    let repository = profile_repository(&pool);
    let revision = revision(ProfileType::Character, &character.id);
    repository.insert_profile_revision(&revision).await.unwrap();
    assert_eq!(
        repository
            .find_profile_revision(&revision.id)
            .await
            .unwrap(),
        Some(revision.clone())
    );
    assert_eq!(
        repository
            .list_profile_revisions(ProfileType::Character, &character.id)
            .await
            .unwrap(),
        vec![revision.clone()]
    );
    let duplicate = ProfileRevision {
        id: "prv-dev048-duplicate".to_owned(),
        content_json: "{\"version\":2}".to_owned(),
        ..revision.clone()
    };
    assert!(repository
        .insert_profile_revision(&duplicate)
        .await
        .is_err());
    assert_eq!(
        repository
            .find_profile_revision(&revision.id)
            .await
            .unwrap(),
        Some(revision)
    );
}

#[tokio::test]
async fn dev048_reference_set_crud_order_and_atomic_item_rollback_work() {
    let (_directory, pool) = setup().await;
    for asset_id in ["ast_a", "ast_b", "ast_c"] {
        insert_asset(&pool, asset_id, PROJECT_ONE, "image").await;
    }
    let repository =
        crate::infrastructure::database::repositories::SqliteReferenceSetRepository::new(
            pool.clone(),
        );
    let set = reference_set(REFERENCE_SET_ID, PROJECT_ONE, "References");
    repository.insert_reference_set(&set).await.unwrap();
    let original = vec![
        item(&set.id, "ast_a", 0, true),
        item(&set.id, "ast_b", 1, false),
    ];
    repository.replace_items(&set.id, &original).await.unwrap();
    assert_eq!(repository.list_items(&set.id).await.unwrap(), original);
    let replacement = vec![
        item(&set.id, "ast_c", 0, true),
        item(&set.id, "ast_missing", 1, false),
    ];
    assert!(repository
        .replace_items(&set.id, &replacement)
        .await
        .is_err());
    assert_eq!(repository.list_items(&set.id).await.unwrap(), original);

    let mut duplicate_asset = vec![
        item(&set.id, "ast_a", 0, true),
        item(&set.id, "ast_a", 1, false),
    ];
    assert!(repository
        .replace_items(&set.id, &duplicate_asset)
        .await
        .is_err());
    duplicate_asset[1].asset_id = "ast_b".to_owned();
    duplicate_asset[1].ordinal = 0;
    assert!(repository
        .replace_items(&set.id, &duplicate_asset)
        .await
        .is_err());
    assert_eq!(repository.list_items(&set.id).await.unwrap(), original);

    let updated = ReferenceSet {
        name: "Updated References".to_owned(),
        updated_at: now(),
        ..set.clone()
    };
    assert!(repository.update_reference_set(&updated).await.unwrap());
    assert_eq!(
        repository
            .find_reference_set(PROJECT_ONE, &set.id)
            .await
            .unwrap(),
        Some(updated)
    );
    assert_eq!(
        repository
            .list_reference_sets(PROJECT_ONE, Some(ReferenceSetPurpose::Character))
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(repository
        .delete_reference_set(PROJECT_ONE, &set.id)
        .await
        .unwrap());
    assert!(repository
        .find_reference_set(PROJECT_ONE, &set.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn dev048_reference_set_service_rejects_cross_project_media_duplicates_and_limit() {
    let (_directory, pool) = setup().await;
    for asset_id in [
        "ast_image",
        "ast_image_2",
        "ast_video",
        "ast_audio",
        "ast_other",
    ] {
        insert_asset(
            &pool,
            asset_id,
            if asset_id == "ast_other" {
                PROJECT_TWO
            } else {
                PROJECT_ONE
            },
            if asset_id == "ast_video" {
                "video"
            } else if asset_id == "ast_audio" {
                "audio"
            } else {
                "image"
            },
        )
        .await;
    }
    let (_profiles, reference_sets) = services(&pool);
    let create = |items: Vec<ReferenceSetItemRequest>| CreateReferenceSetRequest {
        project_id: PROJECT_ONE.to_owned(),
        name: "Service Set".to_owned(),
        purpose: ReferenceSetPurpose::Shot,
        description: String::new(),
        owner_profile_type: None,
        owner_profile_id: None,
        items,
    };
    let image = |asset_id: &str, ordinal: i64| ReferenceSetItemRequest {
        asset_id: asset_id.to_owned(),
        ordinal,
        role: None,
        is_primary: ordinal == 0,
    };
    let set = reference_sets
        .create(create(vec![image("ast_image", 0)]))
        .await
        .unwrap();
    assert!(reference_sets
        .create(create(vec![image("ast_video", 0)]))
        .await
        .is_err());
    assert!(reference_sets
        .create(create(vec![image("ast_audio", 0)]))
        .await
        .is_err());
    assert!(reference_sets
        .create(create(vec![image("ast_other", 0)]))
        .await
        .is_err());
    assert!(reference_sets
        .replace_items(
            PROJECT_ONE,
            &set.id,
            vec![image("ast_image", 0), image("ast_image", 1)]
        )
        .await
        .is_err());
    assert!(reference_sets
        .replace_items(
            PROJECT_ONE,
            &set.id,
            vec![image("ast_image", 0), image("ast_image_2", 0)]
        )
        .await
        .is_err());
    let too_many = (0..21).map(|ordinal| image("ast_image", ordinal)).collect();
    assert!(reference_sets
        .replace_items(PROJECT_ONE, &set.id, too_many)
        .await
        .is_err());
}

#[tokio::test]
async fn dev048_owner_and_default_relationships_reject_cross_project_profiles_and_reference_sets() {
    let (_directory, pool) = setup().await;
    let (profiles, reference_sets) = services(&pool);
    let style_two = profiles
        .create_style(CreateStyleProfileRequest {
            project_id: PROJECT_TWO.to_owned(),
            name: "Other Style".to_owned(),
            style_prompt: "other".to_owned(),
            color_prompt: None,
            line_prompt: None,
            negative_prompt: None,
            output_notes: None,
        })
        .await
        .unwrap();
    assert!(profiles
        .create_character(CreateCharacterProfileRequest {
            project_id: PROJECT_ONE.to_owned(),
            name: "Cross Project Character".to_owned(),
            description: String::new(),
            canonical_prompt: "character".to_owned(),
            negative_prompt: String::new(),
            default_style_profile_id: Some(style_two.id.clone()),
            default_reference_set_id: None,
            metadata_json: "{}".to_owned(),
        })
        .await
        .is_err());

    let other_character = profiles
        .create_character(CreateCharacterProfileRequest {
            project_id: PROJECT_TWO.to_owned(),
            name: "Other Character".to_owned(),
            description: String::new(),
            canonical_prompt: "character".to_owned(),
            negative_prompt: String::new(),
            default_style_profile_id: None,
            default_reference_set_id: None,
            metadata_json: "{}".to_owned(),
        })
        .await
        .unwrap();
    assert!(reference_sets
        .create(CreateReferenceSetRequest {
            project_id: PROJECT_ONE.to_owned(),
            name: "Cross Owner".to_owned(),
            purpose: ReferenceSetPurpose::Character,
            description: String::new(),
            owner_profile_type: Some(ProfileType::Character),
            owner_profile_id: Some(other_character.id),
            items: Vec::new(),
        })
        .await
        .is_err());
}

#[tokio::test]
async fn dev048_shot_profile_and_reference_bindings_round_trip_and_rollback_atomically() {
    let (_directory, pool) = setup().await;
    insert_shot(&pool, SHOT_ID, PROJECT_ONE).await;
    let profile_repository = profile_repository(&pool);
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Character(character_profile(
            CHARACTER_ID,
            PROJECT_ONE,
            "Character A",
        )))
        .await
        .unwrap();
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Character(character_profile(
            "cp-dev048-character-b",
            PROJECT_ONE,
            "Character B",
        )))
        .await
        .unwrap();
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Scene(scene_profile(
            SCENE_ID,
            PROJECT_ONE,
            "Scene",
        )))
        .await
        .unwrap();
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Prop(prop_profile(
            PROP_ID,
            PROJECT_ONE,
            "Prop A",
        )))
        .await
        .unwrap();
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Prop(prop_profile(
            "pp-dev048-prop-b",
            PROJECT_ONE,
            "Prop B",
        )))
        .await
        .unwrap();
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Style(style_profile(
            STYLE_ID,
            PROJECT_ONE,
            "Style",
        )))
        .await
        .unwrap();
    profile_repository
        .insert_costume_variant(&CostumeVariant {
            id: COSTUME_ID.to_owned(),
            character_profile_id: CHARACTER_ID.to_owned(),
            name: "Costume".to_owned(),
            prompt_fragment: "coat".to_owned(),
            reference_set_id: None,
            is_default: true,
            ordinal: 0,
            active_revision_id: None,
            created_at: now(),
            updated_at: now(),
        })
        .await
        .unwrap();
    let reference_repository = reference_set_repository(&pool);
    for (id, purpose) in [
        (REFERENCE_SET_ID, ReferenceSetPurpose::Character),
        ("rs-dev048-scene", ReferenceSetPurpose::Scene),
        ("rs-dev048-prop", ReferenceSetPurpose::Prop),
        ("rs-dev048-style", ReferenceSetPurpose::Style),
        ("rs-dev048-shot", ReferenceSetPurpose::Shot),
    ] {
        let mut set = reference_set(id, PROJECT_ONE, id);
        set.purpose = purpose;
        reference_repository
            .insert_reference_set(&set)
            .await
            .unwrap();
    }

    let repository =
        crate::infrastructure::database::repositories::SqliteShotConsistencyRepository::new(
            pool.clone(),
        );
    let profiles = vec![
        profile_binding(
            "spb-char-a",
            SHOT_ID,
            BindingRole::Character,
            ProfileType::Character,
            CHARACTER_ID,
            Some(COSTUME_ID),
            0,
        ),
        profile_binding(
            "spb-char-b",
            SHOT_ID,
            BindingRole::Character,
            ProfileType::Character,
            "cp-dev048-character-b",
            None,
            1,
        ),
        profile_binding(
            "spb-prop-a",
            SHOT_ID,
            BindingRole::Prop,
            ProfileType::Prop,
            PROP_ID,
            None,
            0,
        ),
        profile_binding(
            "spb-prop-b",
            SHOT_ID,
            BindingRole::Prop,
            ProfileType::Prop,
            "pp-dev048-prop-b",
            None,
            1,
        ),
        profile_binding(
            "spb-scene",
            SHOT_ID,
            BindingRole::Scene,
            ProfileType::Scene,
            SCENE_ID,
            None,
            0,
        ),
        profile_binding(
            "spb-style",
            SHOT_ID,
            BindingRole::Style,
            ProfileType::Style,
            STYLE_ID,
            None,
            0,
        ),
    ];
    repository
        .replace_profile_bindings(SHOT_ID, &profiles)
        .await
        .unwrap();
    assert_eq!(
        repository
            .list_profile_bindings(SHOT_ID)
            .await
            .unwrap()
            .len(),
        6
    );
    let invalid_profile = profile_binding(
        "spb-invalid",
        SHOT_ID,
        BindingRole::Character,
        ProfileType::Character,
        CHARACTER_ID,
        Some("cv-missing"),
        0,
    );
    assert!(repository
        .replace_profile_bindings(SHOT_ID, &[invalid_profile])
        .await
        .is_err());
    assert_eq!(
        repository.list_profile_bindings(SHOT_ID).await.unwrap(),
        profiles
    );

    let references = vec![
        reference_binding(
            "srb-character",
            SHOT_ID,
            BindingRole::Character,
            REFERENCE_SET_ID,
            0,
        ),
        reference_binding("srb-prop", SHOT_ID, BindingRole::Prop, "rs-dev048-prop", 0),
        reference_binding(
            "srb-scene",
            SHOT_ID,
            BindingRole::Scene,
            "rs-dev048-scene",
            0,
        ),
        reference_binding(
            "srb-shot",
            SHOT_ID,
            BindingRole::ShotReference,
            "rs-dev048-shot",
            0,
        ),
        reference_binding(
            "srb-style",
            SHOT_ID,
            BindingRole::Style,
            "rs-dev048-style",
            0,
        ),
    ];
    repository
        .replace_reference_set_bindings(SHOT_ID, &references)
        .await
        .unwrap();
    assert_eq!(
        repository
            .list_reference_set_bindings(SHOT_ID)
            .await
            .unwrap()
            .len(),
        5
    );
    let invalid_reference = reference_binding(
        "srb-invalid",
        SHOT_ID,
        BindingRole::ShotReference,
        "rs-missing",
        0,
    );
    assert!(repository
        .replace_reference_set_bindings(SHOT_ID, &[invalid_reference])
        .await
        .is_err());
    assert_eq!(
        repository
            .list_reference_set_bindings(SHOT_ID)
            .await
            .unwrap(),
        references
    );
}

#[tokio::test]
async fn dev048_reference_anchor_adapter_preserves_order_primary_anchor_assets_and_media() {
    let (_directory, pool) = setup().await;
    for asset_id in ["ast_anchor_b", "ast_anchor_a", "ast_anchor_c"] {
        insert_asset(&pool, asset_id, PROJECT_ONE, "image").await;
    }
    insert_anchor(&pool, &["ast_anchor_b", "ast_anchor_a", "ast_anchor_c"]).await;
    let anchor_repository =
        crate::infrastructure::database::repositories::SqliteReferenceAnchorRepository::new(
            pool.clone(),
        );
    let anchor_id = ReferenceAnchorId::parse(ANCHOR_ID).unwrap();
    let original_anchor = anchor_repository
        .find(PROJECT_ONE, &anchor_id)
        .await
        .unwrap()
        .unwrap();
    let original_assets: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, storage_path, sha256 FROM assets WHERE id IN ('ast_anchor_a', 'ast_anchor_b', 'ast_anchor_c') ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let (_profiles, reference_sets) = services(&pool);
    let converted = reference_sets
        .create_from_anchor(PROJECT_ONE, ANCHOR_ID, Some("Converted Anchor".to_owned()))
        .await
        .unwrap();
    let repository =
        crate::infrastructure::database::repositories::SqliteReferenceSetRepository::new(
            pool.clone(),
        );
    let items = repository.list_items(&converted.id).await.unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| item.asset_id.as_str())
            .collect::<Vec<_>>(),
        vec!["ast_anchor_b", "ast_anchor_a", "ast_anchor_c"]
    );
    assert_eq!(
        items.iter().map(|item| item.ordinal).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        items.iter().map(|item| item.is_primary).collect::<Vec<_>>(),
        vec![true, false, false]
    );
    assert!(items.iter().all(|item| item.role.is_none()));
    assert_eq!(
        anchor_repository
            .find(PROJECT_ONE, &anchor_id)
            .await
            .unwrap()
            .unwrap(),
        original_anchor
    );
    let after_assets: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, storage_path, sha256 FROM assets WHERE id IN ('ast_anchor_a', 'ast_anchor_b', 'ast_anchor_c') ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(after_assets, original_assets);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets WHERE id LIKE 'ast_anchor_%'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
}

#[test]
fn dev048_version_migration_and_scope_gate_is_explicit() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let migration_dir = root.join("migrations");
    let migrations = fs::read_dir(&migration_dir)
        .unwrap()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| name.ends_with(".sql"))
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
    assert!(migrations.iter().all(|name| {
        name.get(..3)
            .and_then(|prefix| prefix.parse::<u32>().ok())
            .is_some_and(|version| version <= 25)
    }));
    let package = fs::read_to_string(root.parent().unwrap().join("package.json")).unwrap();
    assert!(package.contains("\"version\": \"0.7.0\""));
    let cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"0.7.0\""));
    let backup =
        fs::read_to_string(root.join("src/application/project_backup_service.rs")).unwrap();
    assert!(backup.contains("const BACKUP_VERSION: u32 = 15"));
    let manifest =
        fs::read_to_string(root.join("src/application/project_manifest_service.rs")).unwrap();
    assert!(manifest.contains("const MANIFEST_VERSION: u32 = 2"));
    let migration =
        fs::read_to_string(migration_dir.join("022_consistency_profiles_and_reference_sets.sql"))
            .unwrap();
    for forbidden in [
        "shot_context_snapshots",
        "shot_readiness_cache",
        "storyboard",
        "scheduler",
    ] {
        assert!(
            !migration.contains(forbidden),
            "022 must not create {forbidden}"
        );
    }
}
