//! DEV-054 narrative production integration coverage.
//!
//! This test deliberately uses the public application services over a fresh
//! SQLite database. It proves that hierarchical consistency, prompt context,
//! selected video input, readiness evidence, and an immutable preparation
//! snapshot stay connected across a profile edit.

use ai_studio_lib::application::consistency_scope_binding_service::{
    ConsistencyProfileBindingInput, ConsistencyReferenceSetBindingInput,
    ConsistencyScopeBindingService,
};
use ai_studio_lib::application::ports::{
    AssetRepository, Clock, ConsistencyProfileRepository, ConsistencyScopeRepository,
    ProductionStructureRepository, ProjectRepository, ReferenceSetRepository, ShotBatchRepository,
    ShotConsistencyRepository, ShotRecord, ShotRepository, TaskRepository,
};
use ai_studio_lib::application::shot_batch_service::ShotBatchService;
use ai_studio_lib::application::shot_consistency_binding_service::ShotConsistencyBindingService;
use ai_studio_lib::application::shot_context_resolver::{
    ShotContextResolver, ShotContextResolverError,
};
use ai_studio_lib::domain::consistency::{
    generate_consistency_id, BindingRole, ConsistencyIdKind, ConsistencyProfileRecord,
    ConsistencyScopeType, InheritanceMode, ProfileType, ReferenceSetPurpose,
};
use ai_studio_lib::domain::{
    Asset, AssetId, CharacterProfile, ComfyCapabilityEvidence, CostumeVariant,
    PreparationSnapshotRecord, PreparationSnapshotV1, ProductionBatch, ProductionBatchId,
    ProductionBatchItem, ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus,
    ProductionEpisode, ProductionEpisodeId, ProductionScene, ProductionSceneId, ProductionSeries,
    ProductionSeriesId, PropProfile, ReferenceSet, ReferenceSetItem, SceneProfile, ShotReadiness,
    ShotStage, StyleProfile,
};
use ai_studio_lib::infrastructure::database::{
    initialize,
    repositories::{
        SqliteAssetRepository, SqliteConsistencyProfileRepository,
        SqliteConsistencyScopeRepository, SqliteGenerationDefinitionRepository,
        SqliteProductionQueueRepository, SqliteProductionStructureRepository,
        SqliteProjectRepository, SqliteReferenceSetRepository, SqliteShotConsistencyRepository,
        SqliteShotRepository, SqliteTaskRepository,
    },
};
use ai_studio_lib::infrastructure::time::SystemClock;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};

const PROJECT_ID: &str = "prj_550e8400-e29b-41d4-a716-446655440054";
const LEGACY_PROJECT_ID: &str = "prj_550e8400-e29b-41d4-a716-446655440055";
const SHOT_ID: &str = "shot-dev054-main";
const LEGACY_SHOT_ID: &str = "shot-dev054-legacy";
const SELECTED_IMAGE_ID: &str = "ast_dev054_selected";
const LEGACY_ASSET_ID: &str = "ast_dev054_legacy";
const CHARACTER_ASSET_ID: &str = "ast_dev054_character";
const COSTUME_ASSET_ID: &str = "ast_dev054_costume";
const SCENE_ASSET_ID: &str = "ast_dev054_scene";
const PROP_ASSET_ID: &str = "ast_dev054_prop";
const SHOT_REFERENCE_ASSET_ID: &str = "ast_dev054_shot_reference";
const WORKFLOW_ID: &str = "wf_dev054";
const WORKFLOW_VERSION_ID: &str = "wv_dev054";
const RECIPE_ID: &str = "recipe_dev054";

fn fixture_time(offset_seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_756_000_000 + offset_seconds, 0)
        .single()
        .expect("fixture timestamp should be valid")
}

async fn database() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("DEV-054 temporary directory should exist");
    let pool = initialize(&directory.path().join("dev054-narrative.db"))
        .await
        .expect("DEV-054 database should migrate");

    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES (?, 'DEV-054 Narrative', '', ?, ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(directory.path().to_string_lossy().to_string())
    .bind(fixture_time(0).to_rfc3339())
    .bind(fixture_time(0).to_rfc3339())
    .execute(&pool)
    .await
    .expect("project fixture should insert");
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES (?, 'DEV-054 Legacy', '', ?, ?, ?)",
    )
    .bind(LEGACY_PROJECT_ID)
    .bind(
        directory
            .path()
            .join("legacy")
            .to_string_lossy()
            .to_string(),
    )
    .bind(fixture_time(0).to_rfc3339())
    .bind(fixture_time(0).to_rfc3339())
    .execute(&pool)
    .await
    .expect("legacy project fixture should insert");

    sqlx::query(
        "INSERT INTO workflows (id, name, category, mode, created_at, updated_at)
         VALUES (?, 'DEV-054 Workflow', 'image', 'T2I', ?, ?)",
    )
    .bind(WORKFLOW_ID)
    .bind(fixture_time(0).to_rfc3339())
    .bind(fixture_time(0).to_rfc3339())
    .execute(&pool)
    .await
    .expect("workflow fixture should insert");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES (?, ?, '1', '{}', 'workflow-sha-dev054', ?)",
    )
    .bind(WORKFLOW_VERSION_ID)
    .bind(WORKFLOW_ID)
    .bind(fixture_time(0).to_rfc3339())
    .execute(&pool)
    .await
    .expect("workflow version fixture should insert");
    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
         VALUES (?, ?, '1', 1, 'inputs: {}', 'recipe-sha-dev054', ?)",
    )
    .bind(RECIPE_ID)
    .bind(WORKFLOW_VERSION_ID)
    .bind(fixture_time(0).to_rfc3339())
    .execute(&pool)
    .await
    .expect("recipe fixture should insert");

    (directory, pool)
}

fn source_image_for(project_id: &str, id: &str, name: &str, sha256: &str) -> Asset {
    Asset::new_source_image(
        AssetId::parse(id.to_owned()).expect("fixture asset id should parse"),
        project_id,
        name,
        format!("{name}.png"),
        format!("C:/DEV-054/{id}.png"),
        sha256,
        "image/png",
        1024,
        1024,
        1024,
        json!({"fixture": "DEV-054"}),
        fixture_time(0),
    )
    .expect("source image fixture should be valid")
}

fn source_image(id: &str, name: &str, sha256: &str) -> Asset {
    source_image_for(PROJECT_ID, id, name, sha256)
}

fn reference_set(id: &str, name: &str, purpose: ReferenceSetPurpose) -> ReferenceSet {
    ReferenceSet {
        id: id.to_owned(),
        project_id: PROJECT_ID.to_owned(),
        name: name.to_owned(),
        purpose,
        description: String::new(),
        owner_profile_type: None,
        owner_profile_id: None,
        active_revision_id: None,
        created_at: fixture_time(0),
        updated_at: fixture_time(0),
    }
}

fn reference_item(reference_set_id: &str, asset_id: &str) -> ReferenceSetItem {
    ReferenceSetItem {
        reference_set_id: reference_set_id.to_owned(),
        asset_id: asset_id.to_owned(),
        ordinal: 0,
        role: None,
        is_primary: true,
        created_at: fixture_time(0),
    }
}

fn profile_binding(
    role: BindingRole,
    profile_type: ProfileType,
    profile_id: &str,
    costume_variant_id: Option<&str>,
    inheritance_mode: InheritanceMode,
) -> ConsistencyProfileBindingInput {
    ConsistencyProfileBindingInput {
        id: None,
        role,
        profile_type,
        profile_id: profile_id.to_owned(),
        costume_variant_id: costume_variant_id.map(str::to_owned),
        ordinal: 0,
        inheritance_mode,
    }
}

fn reference_binding(
    role: BindingRole,
    reference_set_id: &str,
    inheritance_mode: InheritanceMode,
) -> ConsistencyReferenceSetBindingInput {
    ConsistencyReferenceSetBindingInput {
        id: None,
        role,
        reference_set_id: reference_set_id.to_owned(),
        ordinal: 0,
        required: true,
        inheritance_mode,
    }
}

async fn seed_fixture(
    pool: &SqlitePool,
) -> (
    Arc<dyn ProjectRepository>,
    Arc<dyn ProductionStructureRepository>,
    Arc<dyn ShotRepository>,
    Arc<dyn AssetRepository>,
    Arc<dyn ConsistencyProfileRepository>,
    Arc<dyn ReferenceSetRepository>,
    Arc<dyn ConsistencyScopeRepository>,
    Arc<dyn ShotConsistencyRepository>,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
) {
    let project_repository: Arc<dyn ProjectRepository> =
        Arc::new(SqliteProjectRepository::new(pool.clone()));
    let structure_repository: Arc<dyn ProductionStructureRepository> =
        Arc::new(SqliteProductionStructureRepository::new(pool.clone()));
    let shot_repository: Arc<dyn ShotRepository> =
        Arc::new(SqliteShotRepository::new(pool.clone()));
    let asset_repository: Arc<dyn AssetRepository> =
        Arc::new(SqliteAssetRepository::new(pool.clone()));
    let profile_repository: Arc<dyn ConsistencyProfileRepository> =
        Arc::new(SqliteConsistencyProfileRepository::new(pool.clone()));
    let reference_set_repository: Arc<dyn ReferenceSetRepository> =
        Arc::new(SqliteReferenceSetRepository::new(pool.clone()));
    let scope_repository: Arc<dyn ConsistencyScopeRepository> =
        Arc::new(SqliteConsistencyScopeRepository::new(pool.clone()));
    let shot_consistency_repository: Arc<dyn ShotConsistencyRepository> =
        Arc::new(SqliteShotConsistencyRepository::new(pool.clone()));

    asset_repository
        .insert_many(&[
            source_image(SELECTED_IMAGE_ID, "Selected frame", "sha-selected-v1"),
            source_image(CHARACTER_ASSET_ID, "Character face", "sha-character-v1"),
            source_image(COSTUME_ASSET_ID, "Costume reference", "sha-costume-v1"),
            source_image(SCENE_ASSET_ID, "Scene reference", "sha-scene-v1"),
            source_image(PROP_ASSET_ID, "Prop reference", "sha-prop-v1"),
            source_image(
                SHOT_REFERENCE_ASSET_ID,
                "Shot reference",
                "sha-shot-reference-v1",
            ),
            source_image_for(
                LEGACY_PROJECT_ID,
                LEGACY_ASSET_ID,
                "Legacy reference",
                "sha-legacy-v1",
            ),
        ])
        .await
        .expect("asset fixtures should insert");

    shot_repository
        .insert(&ShotRecord {
            id: SHOT_ID.to_owned(),
            project_id: PROJECT_ID.to_owned(),
            ordinal: 0,
            name: "Hero enters the warehouse".to_owned(),
            prompt_text: "legacy main prompt".to_owned(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            created_at: fixture_time(0),
            updated_at: fixture_time(0),
        })
        .await
        .expect("main shot fixture should insert");
    shot_repository
        .select_image(PROJECT_ID, SHOT_ID, SELECTED_IMAGE_ID)
        .await
        .expect("main shot should select an image");
    shot_repository
        .insert(&ShotRecord {
            id: LEGACY_SHOT_ID.to_owned(),
            project_id: LEGACY_PROJECT_ID.to_owned(),
            ordinal: 0,
            name: "Legacy shot".to_owned(),
            prompt_text: "legacy prompt fallback".to_owned(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            created_at: fixture_time(0),
            updated_at: fixture_time(0),
        })
        .await
        .expect("legacy shot fixture should insert");
    shot_repository
        .replace_reference_assets(
            LEGACY_PROJECT_ID,
            LEGACY_SHOT_ID,
            ShotStage::Image,
            &[LEGACY_ASSET_ID.to_owned()],
        )
        .await
        .expect("legacy shot reference should insert");

    let series_id = "ser_dev054".to_owned();
    let episode_id = "epi_dev054".to_owned();
    let scene_id = "scn_dev054".to_owned();
    let series = structure_repository
        .create_series(&ProductionSeries {
            id: ProductionSeriesId::parse(series_id.clone()).unwrap(),
            project_id: PROJECT_ID.to_owned(),
            ordinal: 0,
            name: "Narrative series".to_owned(),
            description: String::new(),
            created_at: fixture_time(0),
            updated_at: fixture_time(0),
        })
        .await
        .expect("series fixture should insert");
    let episode = structure_repository
        .create_episode(
            PROJECT_ID,
            &ProductionEpisode {
                id: ProductionEpisodeId::parse(episode_id.clone()).unwrap(),
                series_id: series.id.clone(),
                ordinal: 0,
                name: "Episode one".to_owned(),
                description: String::new(),
                created_at: fixture_time(0),
                updated_at: fixture_time(0),
            },
        )
        .await
        .expect("episode fixture should insert");
    let scene = structure_repository
        .create_scene(
            PROJECT_ID,
            &ProductionScene {
                id: ProductionSceneId::parse(scene_id.clone()).unwrap(),
                episode_id: episode.id.clone(),
                ordinal: 0,
                name: "Warehouse interior".to_owned(),
                description: "A long warehouse with hard morning light".to_owned(),
                created_at: fixture_time(0),
                updated_at: fixture_time(0),
            },
        )
        .await
        .expect("scene fixture should insert");
    structure_repository
        .assign_shots_atomic(
            PROJECT_ID,
            &scene.id,
            &[SHOT_ID.to_owned()],
            fixture_time(0),
        )
        .await
        .expect("shot should be assigned to scene");

    let character_id = generate_consistency_id(ConsistencyIdKind::CharacterProfile);
    let scene_profile_id = generate_consistency_id(ConsistencyIdKind::SceneProfile);
    let prop_id = generate_consistency_id(ConsistencyIdKind::PropProfile);
    let style_id = generate_consistency_id(ConsistencyIdKind::StyleProfile);
    let costume_id = generate_consistency_id(ConsistencyIdKind::CostumeVariant);
    let character_reference_id = generate_consistency_id(ConsistencyIdKind::ReferenceSet);
    let costume_reference_id = generate_consistency_id(ConsistencyIdKind::ReferenceSet);
    let scene_reference_id = generate_consistency_id(ConsistencyIdKind::ReferenceSet);
    let prop_reference_id = generate_consistency_id(ConsistencyIdKind::ReferenceSet);
    let shot_reference_id = generate_consistency_id(ConsistencyIdKind::ReferenceSet);

    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Character(CharacterProfile {
            id: character_id.clone(),
            project_id: PROJECT_ID.to_owned(),
            name: "Hero".to_owned(),
            description: "The protagonist".to_owned(),
            canonical_prompt: "hero with determined eyes".to_owned(),
            negative_prompt: "blurry face".to_owned(),
            default_style_profile_id: None,
            default_reference_set_id: None,
            active_revision_id: None,
            metadata_json: "{}".to_owned(),
            created_at: fixture_time(0),
            updated_at: fixture_time(0),
        }))
        .await
        .expect("character profile should insert");
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Scene(SceneProfile {
            id: scene_profile_id.clone(),
            project_id: PROJECT_ID.to_owned(),
            name: "Warehouse".to_owned(),
            description: "Industrial interior".to_owned(),
            environment_prompt: "warehouse interior".to_owned(),
            lighting_prompt: Some("hard morning light".to_owned()),
            negative_prompt: Some("empty background".to_owned()),
            default_style_profile_id: None,
            default_reference_set_id: None,
            active_revision_id: None,
            created_at: fixture_time(0),
            updated_at: fixture_time(0),
        }))
        .await
        .expect("scene profile should insert");
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Prop(PropProfile {
            id: prop_id.clone(),
            project_id: PROJECT_ID.to_owned(),
            name: "Sword".to_owned(),
            description: "A prop sword".to_owned(),
            canonical_prompt: "steel sword".to_owned(),
            material_prompt: Some("brushed steel".to_owned()),
            scale_prompt: Some("hero scale".to_owned()),
            default_reference_set_id: None,
            active_revision_id: None,
            created_at: fixture_time(0),
            updated_at: fixture_time(0),
        }))
        .await
        .expect("prop profile should insert");
    profile_repository
        .insert_profile(&ConsistencyProfileRecord::Style(StyleProfile {
            id: style_id.clone(),
            project_id: PROJECT_ID.to_owned(),
            name: "Ink style".to_owned(),
            style_prompt: "inked anime linework".to_owned(),
            color_prompt: Some("muted violet palette".to_owned()),
            line_prompt: Some("clean contour lines".to_owned()),
            negative_prompt: Some("photorealism".to_owned()),
            output_notes: Some("cinematic framing".to_owned()),
            active_revision_id: None,
            created_at: fixture_time(0),
            updated_at: fixture_time(0),
        }))
        .await
        .expect("style profile should insert");
    for (id, name, purpose) in [
        (
            character_reference_id.as_str(),
            "Character references",
            ReferenceSetPurpose::Character,
        ),
        (
            costume_reference_id.as_str(),
            "Costume references",
            ReferenceSetPurpose::Costume,
        ),
        (
            scene_reference_id.as_str(),
            "Scene references",
            ReferenceSetPurpose::Scene,
        ),
        (
            prop_reference_id.as_str(),
            "Prop references",
            ReferenceSetPurpose::Prop,
        ),
        (
            shot_reference_id.as_str(),
            "Shot references",
            ReferenceSetPurpose::Shot,
        ),
    ] {
        reference_set_repository
            .insert_reference_set(&reference_set(id, name, purpose))
            .await
            .expect("reference set should insert");
    }
    profile_repository
        .insert_costume_variant(&CostumeVariant {
            id: costume_id.clone(),
            character_profile_id: character_id.clone(),
            name: "Red coat".to_owned(),
            prompt_fragment: "red coat".to_owned(),
            reference_set_id: Some(costume_reference_id.clone()),
            is_default: false,
            ordinal: 0,
            active_revision_id: None,
            created_at: fixture_time(0),
            updated_at: fixture_time(0),
        })
        .await
        .expect("costume variant should insert");
    for (set_id, asset_id) in [
        (character_reference_id.as_str(), CHARACTER_ASSET_ID),
        (costume_reference_id.as_str(), COSTUME_ASSET_ID),
        (scene_reference_id.as_str(), SCENE_ASSET_ID),
        (prop_reference_id.as_str(), PROP_ASSET_ID),
        (shot_reference_id.as_str(), SHOT_REFERENCE_ASSET_ID),
    ] {
        reference_set_repository
            .replace_items(set_id, &[reference_item(set_id, asset_id)])
            .await
            .expect("reference set item should insert");
    }

    (
        project_repository,
        structure_repository,
        shot_repository,
        asset_repository,
        profile_repository,
        reference_set_repository,
        scope_repository,
        shot_consistency_repository,
        character_id,
        scene_profile_id,
        prop_id,
        style_id,
        costume_id,
        character_reference_id,
        scene_reference_id,
        prop_reference_id,
    )
}

#[tokio::test]
async fn narrative_context_is_hierarchical_and_snapshot_is_immutable() {
    let (_directory, pool) = database().await;
    let (
        project_repository,
        structure_repository,
        shot_repository,
        asset_repository,
        profile_repository,
        reference_set_repository,
        scope_repository,
        shot_consistency_repository,
        character_id,
        scene_profile_id,
        prop_id,
        style_id,
        costume_id,
        character_reference_id,
        scene_reference_id,
        prop_reference_id,
    ) = seed_fixture(&pool).await;
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);

    let scope_service = ConsistencyScopeBindingService::new_with_clock(
        scope_repository.clone(),
        profile_repository.clone(),
        reference_set_repository.clone(),
        structure_repository.clone(),
        clock.clone(),
    );
    scope_service
        .replace_binding_pack(
            PROJECT_ID,
            ConsistencyScopeType::Project,
            PROJECT_ID,
            &[
                profile_binding(
                    BindingRole::Character,
                    ProfileType::Character,
                    &character_id,
                    None,
                    InheritanceMode::Explicit,
                ),
                profile_binding(
                    BindingRole::Style,
                    ProfileType::Style,
                    &style_id,
                    None,
                    InheritanceMode::Explicit,
                ),
            ],
            &[reference_binding(
                BindingRole::Character,
                &character_reference_id,
                InheritanceMode::Explicit,
            )],
        )
        .await
        .expect("project binding pack should persist");
    scope_service
        .replace_binding_pack(
            PROJECT_ID,
            ConsistencyScopeType::Scene,
            "scn_dev054",
            &[
                profile_binding(
                    BindingRole::Character,
                    ProfileType::Character,
                    &character_id,
                    None,
                    InheritanceMode::Remove,
                ),
                profile_binding(
                    BindingRole::Scene,
                    ProfileType::Scene,
                    &scene_profile_id,
                    None,
                    InheritanceMode::Explicit,
                ),
                profile_binding(
                    BindingRole::Prop,
                    ProfileType::Prop,
                    &prop_id,
                    None,
                    InheritanceMode::Explicit,
                ),
            ],
            &[
                reference_binding(
                    BindingRole::Character,
                    &character_reference_id,
                    InheritanceMode::Remove,
                ),
                reference_binding(
                    BindingRole::Scene,
                    &scene_reference_id,
                    InheritanceMode::Explicit,
                ),
                reference_binding(
                    BindingRole::Prop,
                    &prop_reference_id,
                    InheritanceMode::Explicit,
                ),
            ],
        )
        .await
        .expect("scene binding pack should persist");

    let shot_service = ShotConsistencyBindingService::new(
        shot_consistency_repository.clone(),
        shot_repository.clone(),
        profile_repository.clone(),
        reference_set_repository.clone(),
        clock.clone(),
    );
    let missing_shot_reference = generate_consistency_id(ConsistencyIdKind::ReferenceSet);
    assert!(shot_service
        .replace_binding_pack(
            PROJECT_ID,
            SHOT_ID,
            &[profile_binding(
                BindingRole::Character,
                ProfileType::Character,
                &character_id,
                Some(&costume_id),
                InheritanceMode::Explicit,
            )],
            &[reference_binding(
                BindingRole::ShotReference,
                &missing_shot_reference,
                InheritanceMode::Explicit,
            )],
        )
        .await
        .is_err());

    let shot_reference_id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM reference_sets WHERE project_id = ? AND purpose = 'SHOT'",
    )
    .bind(PROJECT_ID)
    .fetch_one(&pool)
    .await
    .expect("shot reference set should exist");
    shot_service
        .replace_binding_pack(
            PROJECT_ID,
            SHOT_ID,
            &[profile_binding(
                BindingRole::Character,
                ProfileType::Character,
                &character_id,
                Some(&costume_id),
                InheritanceMode::Explicit,
            )],
            &[reference_binding(
                BindingRole::ShotReference,
                &shot_reference_id,
                InheritanceMode::Explicit,
            )],
        )
        .await
        .expect("shot binding pack should persist");

    let resolver = ShotContextResolver::new(
        project_repository.clone(),
        structure_repository.clone(),
        shot_repository.clone(),
        scope_repository.clone(),
        profile_repository.clone(),
        reference_set_repository.clone(),
        shot_consistency_repository.clone(),
        asset_repository.clone(),
        clock.clone(),
    );
    let image_context = resolver
        .resolve_draft(PROJECT_ID, SHOT_ID, ShotStage::Image)
        .await
        .expect("hierarchical image context should resolve");
    assert!(
        !image_context.partial,
        "unexpected diagnostics: {:?}",
        image_context.diagnostics
    );
    assert_eq!(image_context.profiles.characters.len(), 1);
    assert_eq!(
        image_context.profiles.scene.as_ref().unwrap().profile_id,
        scene_profile_id
    );
    assert_eq!(image_context.profiles.props[0].profile_id, prop_id);
    assert_eq!(
        image_context.profiles.style.as_ref().unwrap().profile_id,
        style_id
    );
    assert_eq!(
        image_context.profiles.characters[0]
            .costume_variant_id
            .as_deref(),
        Some(costume_id.as_str())
    );
    for fragment in [
        "hero with determined eyes",
        "red coat",
        "warehouse interior",
        "steel sword",
        "inked anime linework",
    ] {
        assert!(
            image_context
                .prompt_context
                .rendered_text
                .contains(fragment),
            "prompt should contain {fragment}: {}",
            image_context.prompt_context.rendered_text
        );
    }
    assert!(image_context
        .reference_assets
        .iter()
        .any(|asset| asset.asset_id == COSTUME_ASSET_ID));
    assert!(image_context
        .reference_assets
        .iter()
        .any(|asset| asset.asset_id == SCENE_ASSET_ID));
    assert!(image_context
        .reference_assets
        .iter()
        .any(|asset| asset.asset_id == PROP_ASSET_ID));
    assert!(image_context
        .reference_assets
        .iter()
        .any(|asset| asset.asset_id == SHOT_REFERENCE_ASSET_ID));
    assert!(image_context
        .reference_pack
        .reference_sets
        .iter()
        .all(|set| set.reference_set_id != character_reference_id));
    let scopes = image_context
        .reference_pack
        .source_trace
        .iter()
        .map(|trace| trace.scope)
        .collect::<Vec<_>>();
    assert!(scopes.contains(&ai_studio_lib::domain::ContextSourceScope::Project));
    assert!(scopes.contains(&ai_studio_lib::domain::ContextSourceScope::Scene));
    assert!(scopes.contains(&ai_studio_lib::domain::ContextSourceScope::Shot));
    let original_hash = image_context.resolver_identity.context_hash.clone();

    let mut updated_scene = profile_repository
        .find_profile(PROJECT_ID, ProfileType::Scene, &scene_profile_id)
        .await
        .expect("scene profile should load")
        .expect("scene profile should exist");
    if let ConsistencyProfileRecord::Scene(profile) = &mut updated_scene {
        profile.environment_prompt = "warehouse interior at sunrise".to_owned();
        profile.updated_at = fixture_time(1);
    } else {
        panic!("fixture scene profile has the wrong type");
    }
    assert!(profile_repository
        .update_profile(&updated_scene)
        .await
        .expect("scene profile should update"));
    let updated_context = resolver
        .resolve_draft(PROJECT_ID, SHOT_ID, ShotStage::Image)
        .await
        .expect("updated image context should resolve");
    assert_ne!(
        updated_context.resolver_identity.context_hash,
        original_hash
    );
    assert!(updated_context
        .prompt_context
        .rendered_text
        .contains("warehouse interior at sunrise"));

    let readiness = ShotReadiness::from_gates(
        PROJECT_ID,
        SHOT_ID,
        ShotStage::Image.as_str(),
        &image_context.resolver_identity.context_hash,
        Vec::new(),
        fixture_time(2),
        None,
        false,
        image_context.partial,
    );
    let batch_id = ProductionBatchId::new();
    let item_id = ProductionBatchItemId::new();
    let batch = ProductionBatch {
        id: batch_id.clone(),
        project_id: PROJECT_ID.to_owned(),
        name: "DEV-054 prepared narrative batch".to_owned(),
        status: ProductionBatchStatus::Ready,
        continue_on_failure: false,
        archived_at: None,
        created_at: fixture_time(2),
        updated_at: fixture_time(2),
    };
    let item = ProductionBatchItem {
        id: item_id.clone(),
        batch_id: batch_id.clone(),
        ordinal: 0,
        workflow_version_id: WORKFLOW_VERSION_ID.to_owned(),
        recipe_id: RECIPE_ID.to_owned(),
        values_json: json!({"prompt": image_context.prompt_context.rendered_text}),
        status: ProductionBatchItemStatus::Pending,
        task_id: None,
        retry_of_item_id: None,
        error_code: None,
        error_message: None,
        created_at: fixture_time(2),
        updated_at: fixture_time(2),
    };
    let snapshot = PreparationSnapshotRecord {
        id: "sps_dev054_narrative".to_owned(),
        project_id: PROJECT_ID.to_owned(),
        shot_id: SHOT_ID.to_owned(),
        stage: ShotStage::Image,
        context_hash: image_context.resolver_identity.context_hash.clone(),
        production_batch_id: batch_id.as_str().to_owned(),
        production_batch_item_id: item_id.as_str().to_owned(),
        snapshot: PreparationSnapshotV1::from_context(
            &image_context,
            &readiness,
            json!({"prompt": image_context.prompt_context.rendered_text}),
            ComfyCapabilityEvidence::default(),
            fixture_time(2),
        ),
        created_at: fixture_time(2),
    };
    let batch_repository: Arc<dyn ShotBatchRepository> =
        Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let task_repository: Arc<dyn TaskRepository> =
        Arc::new(SqliteTaskRepository::new(pool.clone()));
    let definition_repository: Arc<
        dyn ai_studio_lib::application::ports::GenerationDefinitionRepository,
    > = Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let batch_service = ShotBatchService::new(
        shot_repository.clone(),
        batch_repository.clone(),
        task_repository,
        asset_repository,
        definition_repository,
        project_repository,
        clock,
    );
    batch_service
        .insert_prepared_batch_with_bindings(
            &batch,
            &[item],
            &[ai_studio_lib::application::ports::ShotBatchBinding {
                shot_id: SHOT_ID.to_owned(),
                stage: ShotStage::Image,
                production_batch_item_id: item_id.as_str().to_owned(),
            }],
            &[snapshot],
        )
        .await
        .expect("prepared narrative snapshot should persist");
    let frozen = batch_service
        .find_preparation_snapshot(PROJECT_ID, item_id.as_str())
        .await
        .expect("frozen snapshot should load")
        .expect("frozen snapshot should exist");
    assert_eq!(
        frozen.context_hash,
        image_context.resolver_identity.context_hash
    );
    assert_eq!(
        frozen.snapshot.prompt.rendered_text,
        image_context.prompt_context.rendered_text
    );
    assert_ne!(
        frozen.snapshot.prompt.rendered_text,
        updated_context.prompt_context.rendered_text
    );

    let video_context = resolver
        .resolve_draft(PROJECT_ID, SHOT_ID, ShotStage::Video)
        .await
        .expect("video context should resolve");
    assert_eq!(
        video_context.stage_input.selected_image_asset_id.as_deref(),
        Some(SELECTED_IMAGE_ID)
    );
    assert_eq!(
        video_context.stage_input.selected_image_sha256.as_deref(),
        Some("sha-selected-v1")
    );
    assert_ne!(
        video_context.resolver_identity.context_hash,
        image_context.resolver_identity.context_hash
    );

    let legacy_context = resolver
        .resolve_draft(LEGACY_PROJECT_ID, LEGACY_SHOT_ID, ShotStage::Image)
        .await
        .expect("legacy context should resolve");
    assert!(!legacy_context.legacy.has_reference_pack);
    assert!(legacy_context.legacy.uses_legacy_shot_references);
    assert_eq!(
        legacy_context.legacy.prompt.as_deref(),
        Some("legacy prompt fallback")
    );
    assert_eq!(legacy_context.reference_assets.len(), 1);
    assert_eq!(
        legacy_context.reference_assets[0].source_scope,
        ai_studio_lib::domain::ContextSourceScope::Legacy
    );

    let five_hundred = vec![SHOT_ID.to_owned(); 500];
    assert_eq!(
        resolver
            .resolve_many_draft(PROJECT_ID, &five_hundred, ShotStage::Image)
            .await
            .unwrap()
            .len(),
        500
    );
    let five_hundred_one = vec![SHOT_ID.to_owned(); 501];
    assert!(matches!(
        resolver
            .resolve_many_draft(PROJECT_ID, &five_hundred_one, ShotStage::Image)
            .await,
        Err(ShotContextResolverError::ContextBatchLimit { limit: 500 })
    ));
}
