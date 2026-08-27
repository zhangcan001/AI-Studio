//! DEV-049 migration and pure context-resolution contract tests.

// The application module is not yet re-exported by the library crate. Keep
// this test-local bridge until Main wires the finalized B APIs publicly.
mod domain {
    pub use ai_studio_lib::domain::*;
}

mod ports {
    #[path = "../../src/application/ports/repository_error.rs"]
    pub mod repository_error;
    pub use repository_error::RepositoryError;
    #[path = "../../src/application/ports/clock.rs"]
    pub mod clock;
    pub use clock::{Clock, MonotonicEventClock};
    #[path = "../../src/application/ports/project_repository.rs"]
    pub mod project_repository;
    pub use project_repository::{ProjectRecord, ProjectRepository};
    #[path = "../../src/application/ports/production_structure_repository.rs"]
    pub mod production_structure_repository;
    pub use production_structure_repository::{
        ProductionStructureRepository, ProductionStructureTreeData,
    };
    #[path = "../../src/application/ports/shot_bulk_repository.rs"]
    pub mod shot_bulk_repository;
    pub use shot_bulk_repository::{ShotBulkData, ShotBulkRepository, ShotStagePromptRecord};
    #[path = "../../src/application/ports/shot_repository.rs"]
    pub mod shot_repository;
    pub use shot_repository::{
        ShotData, ShotGenerationLinkRecord, ShotRecord, ShotReferenceAssetRecord, ShotRepository,
        ShotStageConfigRecord,
    };
    #[path = "../../src/application/ports/asset_repository.rs"]
    pub mod asset_repository;
    pub use asset_repository::{AssetRepository, TaskOutputAssetMapping};
    #[path = "../../src/application/ports/consistency_profile_repository.rs"]
    pub mod consistency_profile_repository;
    pub use consistency_profile_repository::ConsistencyProfileRepository;
    #[path = "../../src/application/ports/consistency_scope_repository.rs"]
    pub mod consistency_scope_repository;
    pub use consistency_scope_repository::ConsistencyScopeRepository;
    #[path = "../../src/application/ports/reference_set_repository.rs"]
    pub mod reference_set_repository;
    pub use reference_set_repository::ReferenceSetRepository;
    #[path = "../../src/application/ports/shot_consistency_repository.rs"]
    pub mod shot_consistency_repository;
    pub use shot_consistency_repository::ShotConsistencyRepository;
}
#[path = "../src/application/prompt_context_builder.rs"]
pub mod prompt_context_builder;
#[path = "../src/application/shot_context_resolver.rs"]
pub mod shot_context_resolver;
#[path = "../src/application/shot_reference_pack_builder.rs"]
pub mod shot_reference_pack_builder;

mod application {
    pub mod ports {
        pub use crate::ports::*;
    }
    pub use crate::prompt_context_builder;
    pub use crate::shot_reference_pack_builder;
}

use ai_studio_lib::initialize;
use application::prompt_context_builder::{
    build_prompt_context, select_stage_prompt, PromptContextInput, PromptFragmentInput,
};
use domain::{
    BindingRole, ContextHashInput, ContextSourceScope, InheritanceMode, PromptSegmentKind,
    ResolvedOutputSpec, ResolvedStructure, ShotStage,
};
use serde_json::json;
use shot_context_resolver::{ShotContextResolver, CONTEXT_BATCH_LIMIT};
use sqlx::SqlitePool;
use tempfile::{tempdir, TempDir};

const PROJECT_ID: &str = "dev049-project";
const CREATED_AT: &str = "2026-08-26T00:00:00Z";

async fn database() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("DEV-049 temporary directory should exist");
    let pool = initialize(&directory.path().join("dev049.db"))
        .await
        .expect("DEV-049 database should initialize");
    (directory, pool)
}

async fn table_exists(pool: &SqlitePool, table: &str) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .expect("schema lookup should succeed")
        == 1
}

async fn insert_022_fixture(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES (?, 'DEV-049', '', 'C:/dev049', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("project fixture should insert");

    sqlx::query(
        "INSERT INTO assets
         (id, project_id, type, category, name, original_name, storage_path, sha256,
          mime_type, width, height, file_size, metadata_json, created_at, updated_at)
         VALUES ('asset-dev049', ?, 'image', 'source_image', 'Reference', 'reference.png',
                 'C:/dev049/reference.png', 'sha-dev049', 'image/png', 1, 1, 1, '{}', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("asset fixture should insert");

    sqlx::query(
        "INSERT INTO shots
         (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
         VALUES ('shot-dev049', ?, 0, 'Legacy shot', 'legacy prompt', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("shot fixture should insert");

    sqlx::query(
        "INSERT INTO style_profiles
         (id, project_id, name, style_prompt, created_at, updated_at)
         VALUES ('stp_dev049', ?, 'Legacy style', 'ink', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("022 style fixture should insert");

    sqlx::query(
        "INSERT INTO reference_sets
         (id, project_id, name, purpose, description, created_at, updated_at)
         VALUES ('rs_dev049', ?, 'Legacy references', 'CHARACTER', '', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("022 reference-set fixture should insert");

    sqlx::query(
        "INSERT INTO reference_set_items
         (reference_set_id, asset_id, ordinal, is_primary, created_at)
         VALUES ('rs_dev049', 'asset-dev049', 0, 1, ?)",
    )
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("022 reference item fixture should insert");
}

async fn remove_023_for_upgrade(pool: &SqlitePool) {
    for table in [
        "consistency_scope_reference_set_bindings",
        "consistency_scope_profile_bindings",
    ] {
        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(pool)
            .await
            .expect("023 table should be removable in isolated fixture");
    }
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 23")
        .execute(pool)
        .await
        .expect("023 migration marker should be removable in isolated fixture");
}

#[tokio::test]
async fn dev049_fresh_001_to_025_creates_only_the_scope_tables() {
    let (_directory, pool) = database().await;

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        25
    );
    for table in [
        "consistency_scope_profile_bindings",
        "consistency_scope_reference_set_bindings",
        "production_preparation_snapshots",
    ] {
        assert!(table_exists(&pool, table).await, "missing table {table}");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0,
            "fresh table {table} must be empty"
        );
    }
    assert!(!table_exists(&pool, "shot_context_snapshots").await);
    assert!(!table_exists(&pool, "shot_readiness_cache").await);
}

#[tokio::test]
async fn dev049_022_to_025_preserves_022_rows_and_leaves_scope_tables_empty() {
    let (directory, pool) = database().await;
    insert_022_fixture(&pool).await;
    remove_023_for_upgrade(&pool).await;
    pool.close().await;

    let upgraded = initialize(&directory.path().join("dev049.db"))
        .await
        .expect("DEV-049 upgrade should initialize");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&upgraded)
            .await
            .unwrap(),
        25
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = 'dev049-project'",)
            .fetch_one(&upgraded)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM style_profiles WHERE id = 'stp_dev049'",
        )
        .fetch_one(&upgraded)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM reference_set_items
             WHERE reference_set_id = 'rs_dev049' AND asset_id = 'asset-dev049'",
        )
        .fetch_one(&upgraded)
        .await
        .unwrap(),
        1
    );
    for table in [
        "consistency_scope_profile_bindings",
        "consistency_scope_reference_set_bindings",
        "production_preparation_snapshots",
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

fn profile_candidate(
    binding_id: &str,
    scope: ContextSourceScope,
    role: BindingRole,
    profile_id: &str,
    ordinal: i64,
    inheritance_mode: InheritanceMode,
) -> shot_reference_pack_builder::ProfileBindingCandidate {
    let profile_type = match role {
        BindingRole::Character => domain::ProfileType::Character,
        BindingRole::Scene => domain::ProfileType::Scene,
        BindingRole::Prop => domain::ProfileType::Prop,
        BindingRole::Style => domain::ProfileType::Style,
        BindingRole::ShotReference => panic!("SHOT_REFERENCE is not a profile binding"),
    };
    shot_reference_pack_builder::ProfileBindingCandidate {
        binding_id: binding_id.to_owned(),
        scope,
        scope_id: scope.as_str().to_owned(),
        role,
        profile_type,
        profile_id: profile_id.to_owned(),
        costume_variant_id: None,
        ordinal,
        inheritance_mode,
    }
}

fn reference_candidate(
    binding_id: &str,
    scope: ContextSourceScope,
    role: BindingRole,
    reference_set_id: &str,
    ordinal: i64,
    inheritance_mode: InheritanceMode,
) -> shot_reference_pack_builder::ReferenceSetBindingCandidate {
    shot_reference_pack_builder::ReferenceSetBindingCandidate {
        binding_id: binding_id.to_owned(),
        scope,
        scope_id: scope.as_str().to_owned(),
        role,
        reference_set_id: reference_set_id.to_owned(),
        ordinal,
        required: true,
        inheritance_mode,
    }
}

fn fragment(text: &str, scope: ContextSourceScope, entity_id: &str) -> PromptFragmentInput {
    PromptFragmentInput::new(text, scope, entity_id)
}

#[test]
fn dev049_profile_merge_covers_hierarchy_replace_remove_readd_and_conflict() {
    let result = shot_reference_pack_builder::merge_profile_bindings(&[
        profile_candidate(
            "project-a",
            ContextSourceScope::Project,
            BindingRole::Character,
            "character-a",
            0,
            InheritanceMode::Inherited,
        ),
        profile_candidate(
            "series-b",
            ContextSourceScope::Series,
            BindingRole::Character,
            "character-b",
            1,
            InheritanceMode::Inherited,
        ),
        profile_candidate(
            "episode-remove",
            ContextSourceScope::Episode,
            BindingRole::Character,
            "character-a",
            0,
            InheritanceMode::Remove,
        ),
        profile_candidate(
            "scene-inherited",
            ContextSourceScope::Scene,
            BindingRole::Character,
            "character-a",
            0,
            InheritanceMode::Inherited,
        ),
        profile_candidate(
            "shot-readd",
            ContextSourceScope::Shot,
            BindingRole::Character,
            "character-a",
            0,
            InheritanceMode::Explicit,
        ),
    ]);
    assert_eq!(
        result
            .bindings
            .iter()
            .map(|binding| binding.profile_id.as_str())
            .collect::<Vec<_>>(),
        vec!["character-a", "character-b"]
    );
    assert_eq!(result.bindings[0].source.scope, ContextSourceScope::Shot);
    assert!(result.tombstones.is_empty());

    let replaced = shot_reference_pack_builder::merge_profile_bindings(&[
        profile_candidate(
            "project-a",
            ContextSourceScope::Project,
            BindingRole::Character,
            "character-a",
            0,
            InheritanceMode::Inherited,
        ),
        profile_candidate(
            "project-b",
            ContextSourceScope::Project,
            BindingRole::Character,
            "character-b",
            1,
            InheritanceMode::Inherited,
        ),
        profile_candidate(
            "scene-replace",
            ContextSourceScope::Scene,
            BindingRole::Character,
            "character-c",
            0,
            InheritanceMode::Replace,
        ),
    ]);
    assert_eq!(replaced.bindings.len(), 1);
    assert_eq!(replaced.bindings[0].profile_id, "character-c");

    let conflict = shot_reference_pack_builder::merge_profile_bindings(&[
        profile_candidate(
            "scene-a",
            ContextSourceScope::Scene,
            BindingRole::Style,
            "style-a",
            0,
            InheritanceMode::Explicit,
        ),
        profile_candidate(
            "scene-b",
            ContextSourceScope::Scene,
            BindingRole::Style,
            "style-b",
            0,
            InheritanceMode::Explicit,
        ),
    ]);
    assert!(conflict.bindings.is_empty());
    assert_eq!(
        conflict.diagnostics[0].code,
        "CONTEXT_PROFILE_ORDINAL_CONFLICT"
    );
}

#[test]
fn dev049_reference_merge_and_prompt_builder_are_deterministic() {
    let references = shot_reference_pack_builder::merge_reference_set_bindings(&[
        reference_candidate(
            "project-a",
            ContextSourceScope::Project,
            BindingRole::Character,
            "refs-a",
            0,
            InheritanceMode::Inherited,
        ),
        reference_candidate(
            "scene-replace",
            ContextSourceScope::Scene,
            BindingRole::Character,
            "refs-b",
            0,
            InheritanceMode::Replace,
        ),
        reference_candidate(
            "episode-remove",
            ContextSourceScope::Episode,
            BindingRole::Character,
            "refs-b",
            0,
            InheritanceMode::Remove,
        ),
        reference_candidate(
            "shot-readd",
            ContextSourceScope::Shot,
            BindingRole::Character,
            "refs-b",
            0,
            InheritanceMode::Explicit,
        ),
    ]);
    assert_eq!(references.bindings.len(), 1);
    assert_eq!(references.bindings[0].reference_set_id, "refs-b");
    assert_eq!(
        references.bindings[0].source.scope,
        ContextSourceScope::Shot
    );

    let mut style = fragment(" style ", ContextSourceScope::Project, "style");
    style.negative_prompt = Some("bad  anatomy".to_owned());
    let mut scene = fragment("scene", ContextSourceScope::Scene, "scene");
    scene.negative_prompt = Some("bad anatomy".to_owned());
    let mut character = fragment("character", ContextSourceScope::Shot, "character");
    character.negative_prompt = Some("low quality".to_owned());
    let input = PromptContextInput {
        global_style: vec![
            style.clone(),
            fragment("style", ContextSourceScope::Project, "style"),
        ],
        scene: vec![scene],
        characters: vec![character],
        costumes: vec![fragment("costume", ContextSourceScope::Shot, "costume")],
        props: vec![fragment("prop", ContextSourceScope::Shot, "prop")],
        shot_action: vec![fragment("action", ContextSourceScope::Shot, "shot")],
        camera: vec![fragment("camera", ContextSourceScope::Shot, "camera")],
        lighting: vec![fragment("light", ContextSourceScope::Scene, "scene")],
        output_specification: vec![fragment("output", ContextSourceScope::Shot, "shot")],
    };
    let context = build_prompt_context(&input);
    assert_eq!(
        context
            .segments
            .iter()
            .map(|segment| segment.kind)
            .collect::<Vec<_>>(),
        vec![
            PromptSegmentKind::GlobalStyle,
            PromptSegmentKind::Scene,
            PromptSegmentKind::Character,
            PromptSegmentKind::Costume,
            PromptSegmentKind::Props,
            PromptSegmentKind::ShotAction,
            PromptSegmentKind::Camera,
            PromptSegmentKind::Lighting,
            PromptSegmentKind::OutputSpecification,
        ]
    );
    assert_eq!(
        context.rendered_text,
        "style\nscene\ncharacter\ncostume\nprop\naction\ncamera\nlight\noutput"
    );
    assert_eq!(context.negative_prompt, "bad anatomy\nlow quality");
    assert_eq!(
        select_stage_prompt(ShotStage::Image, Some(" image "), Some("video"), "legacy"),
        "image"
    );
    assert_eq!(
        select_stage_prompt(ShotStage::Video, Some("image"), Some(" video "), "legacy"),
        "video"
    );
    assert_eq!(
        select_stage_prompt(ShotStage::Image, None, Some("video"), "legacy"),
        "legacy"
    );
}

#[test]
fn dev049_reference_assets_and_context_hash_are_stable_and_sensitive() {
    let assets = shot_reference_pack_builder::order_reference_assets(vec![
        shot_reference_pack_builder::ReferenceAssetCandidate {
            asset_id: "asset-scene".to_owned(),
            sha256: "scene-sha".to_owned(),
            role: BindingRole::Scene,
            binding_ordinal: 0,
            set_ordinal: 0,
            source_reference_set_id: "refs-scene".to_owned(),
            source_profile_id: None,
            source_scope: ContextSourceScope::Scene,
        },
        shot_reference_pack_builder::ReferenceAssetCandidate {
            asset_id: "asset-character".to_owned(),
            sha256: "character-sha".to_owned(),
            role: BindingRole::Character,
            binding_ordinal: 1,
            set_ordinal: 0,
            source_reference_set_id: "refs-character".to_owned(),
            source_profile_id: Some("character".to_owned()),
            source_scope: ContextSourceScope::Project,
        },
        shot_reference_pack_builder::ReferenceAssetCandidate {
            asset_id: "asset-character".to_owned(),
            sha256: "character-sha".to_owned(),
            role: BindingRole::Character,
            binding_ordinal: 1,
            set_ordinal: 1,
            source_reference_set_id: "refs-character".to_owned(),
            source_profile_id: Some("character".to_owned()),
            source_scope: ContextSourceScope::Project,
        },
    ]);
    assert_eq!(
        assets
            .iter()
            .map(|asset| asset.asset_id.as_str())
            .collect::<Vec<_>>(),
        vec!["asset-character", "asset-scene"]
    );

    let items = vec![
        shot_reference_pack_builder::ReferenceSetContentHashItem {
            asset_id: "asset-b".to_owned(),
            asset_sha256: "sha-b".to_owned(),
            ordinal: 1,
            role: Some("DETAIL".to_owned()),
            is_primary: false,
        },
        shot_reference_pack_builder::ReferenceSetContentHashItem {
            asset_id: "asset-a".to_owned(),
            asset_sha256: "sha-a".to_owned(),
            ordinal: 0,
            role: Some("FACE".to_owned()),
            is_primary: true,
        },
    ];
    let hash_a = shot_reference_pack_builder::reference_set_content_hash(
        "refs",
        domain::ReferenceSetPurpose::Character,
        items.clone(),
    );
    let hash_b = shot_reference_pack_builder::reference_set_content_hash(
        "refs",
        domain::ReferenceSetPurpose::Character,
        items.into_iter().rev().collect(),
    );
    assert_eq!(hash_a, hash_b);

    let input = ContextHashInput {
        project_id: "project".to_owned(),
        stage: "image".to_owned(),
        profile_ids: vec!["character".to_owned()],
        profile_content_hashes: vec!["profile-sha".to_owned()],
        asset_ids: vec!["asset-a".to_owned()],
        asset_sha256: vec!["asset-sha".to_owned()],
        scalar_values: json!({"width": 512}),
        output: ResolvedOutputSpec {
            width: Some(512),
            ..ResolvedOutputSpec::default()
        },
        structure: ResolvedStructure::default(),
        ..ContextHashInput::default()
    };
    let stable_a = shot_reference_pack_builder::compute_context_hash(&input);
    let stable_b = shot_reference_pack_builder::compute_context_hash(&input);
    assert_eq!(stable_a, stable_b);
    assert!(serde_json::to_value(&input)
        .unwrap()
        .get("resolved_at")
        .is_none());

    let mut changed = input.clone();
    changed.asset_sha256 = vec!["changed-sha".to_owned()];
    assert_ne!(
        stable_a,
        shot_reference_pack_builder::compute_context_hash(&changed)
    );
}

#[cfg(test)]
mod resolver_contract_tests {
    use super::*;
    use application::ports::{
        AssetRepository, Clock, ConsistencyProfileRepository, ConsistencyScopeRepository,
        ProductionStructureRepository, ProductionStructureTreeData, ProjectRecord,
        ProjectRepository, ReferenceSetRepository, RepositoryError, ShotConsistencyRepository,
        ShotData, ShotGenerationLinkRecord, ShotRecord, ShotReferenceAssetRecord, ShotRepository,
        ShotStageConfigRecord, ShotStagePromptRecord,
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use domain::{
        Asset, AssetId, AssetType, CharacterProfile, ConsistencyProfileRecord,
        ConsistencyScopeType, CostumeVariant, InheritanceMode, ProfileRevision,
        ProfileRevisionStatus, ProfileType, ReferenceSet, ReferenceSetItem, ReferenceSetPurpose,
        ScopedProfileBinding, ScopedReferenceSetBinding, ShotProfileBinding,
        ShotReferenceSetBinding,
    };
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    const WHEN: &str = "2026-08-26T00:00:00Z";
    const PID: &str = "project-resolver";

    #[derive(Default)]
    struct Counts {
        project: AtomicUsize,
        structure: AtomicUsize,
        shots: AtomicUsize,
        scope_profiles: AtomicUsize,
        scope_references: AtomicUsize,
        shot_profiles: AtomicUsize,
        shot_references: AtomicUsize,
        profiles: AtomicUsize,
        costumes: AtomicUsize,
        references: AtomicUsize,
        items: AtomicUsize,
        revisions: AtomicUsize,
        assets: AtomicUsize,
    }

    struct Fixture {
        project: ProjectRecord,
        structure: ProductionStructureTreeData,
        shots: Vec<ShotData>,
        profiles: Vec<ConsistencyProfileRecord>,
        costumes: Vec<CostumeVariant>,
        revisions: Vec<ProfileRevision>,
        reference_sets: Vec<ReferenceSet>,
        items: Vec<ReferenceSetItem>,
        scope_profiles: Vec<ScopedProfileBinding>,
        scope_references: Vec<ScopedReferenceSetBinding>,
        shot_profiles: Vec<ShotProfileBinding>,
        shot_references: Vec<ShotReferenceSetBinding>,
        assets: Vec<Asset>,
        counts: Arc<Counts>,
        now: DateTime<Utc>,
    }

    fn when() -> DateTime<Utc> {
        WHEN.parse().unwrap()
    }

    fn project() -> ProjectRecord {
        ProjectRecord {
            id: PID.to_owned(),
            name: "Resolver project".to_owned(),
            description: None,
            root_path: PathBuf::from("C:/resolver"),
            created_at: when(),
            updated_at: when(),
        }
    }

    fn shot(id: &str) -> ShotData {
        ShotData {
            shot: ShotRecord {
                id: id.to_owned(),
                project_id: PID.to_owned(),
                ordinal: 0,
                name: id.to_owned(),
                prompt_text: "legacy prompt".to_owned(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: None,
                selected_video_asset_id: None,
                created_at: when(),
                updated_at: when(),
            },
            stage_configs: Vec::new(),
            stage_prompts: vec![
                ShotStagePromptRecord {
                    shot_id: id.to_owned(),
                    stage: domain::ShotStage::Image,
                    prompt_text: "image-only prompt".to_owned(),
                    prompt_entry_id: None,
                    prompt_version_id: None,
                    updated_at: when(),
                },
                ShotStagePromptRecord {
                    shot_id: id.to_owned(),
                    stage: domain::ShotStage::Video,
                    prompt_text: "video-only prompt".to_owned(),
                    prompt_entry_id: None,
                    prompt_version_id: None,
                    updated_at: when(),
                },
            ],
            reference_assets: Vec::new(),
            generation_links: Vec::<ShotGenerationLinkRecord>::new(),
        }
    }

    fn asset(id: &str, project_id: &str, asset_type: AssetType) -> Asset {
        Asset {
            id: AssetId::parse(id).unwrap(),
            project_id: project_id.to_owned(),
            asset_type,
            category: "source_image".to_owned(),
            name: id.to_owned(),
            original_name: format!("{id}.png"),
            storage_path: format!("C:/{id}.png"),
            thumbnail_path: None,
            sha256: format!("sha-{id}"),
            mime_type: "image/png".to_owned(),
            width: 100,
            height: 100,
            duration_ms: None,
            file_size: 1,
            source_task_id: None,
            metadata_json: json!({}),
            created_at: when(),
            updated_at: when(),
        }
    }

    fn character(id: &str, active_revision_id: Option<&str>) -> ConsistencyProfileRecord {
        ConsistencyProfileRecord::Character(CharacterProfile {
            id: id.to_owned(),
            project_id: PID.to_owned(),
            name: "Character".to_owned(),
            description: String::new(),
            canonical_prompt: "character prompt".to_owned(),
            negative_prompt: String::new(),
            default_style_profile_id: None,
            default_reference_set_id: None,
            active_revision_id: active_revision_id.map(str::to_owned),
            metadata_json: "{}".to_owned(),
            created_at: when(),
            updated_at: when(),
        })
    }

    fn profile_binding(id: &str, profile_id: &str) -> ShotProfileBinding {
        ShotProfileBinding {
            id: id.to_owned(),
            shot_id: "shot-1".to_owned(),
            role: domain::BindingRole::Character,
            profile_type: ProfileType::Character,
            profile_id: profile_id.to_owned(),
            costume_variant_id: None,
            ordinal: 0,
            inheritance_mode: InheritanceMode::Explicit,
            created_at: when(),
            updated_at: when(),
        }
    }

    fn reference_set(id: &str) -> ReferenceSet {
        ReferenceSet {
            id: id.to_owned(),
            project_id: PID.to_owned(),
            name: "References".to_owned(),
            purpose: ReferenceSetPurpose::Character,
            description: String::new(),
            owner_profile_type: None,
            owner_profile_id: None,
            active_revision_id: None,
            created_at: when(),
            updated_at: when(),
        }
    }

    fn reference_binding(id: &str, set_id: &str) -> ShotReferenceSetBinding {
        ShotReferenceSetBinding {
            id: id.to_owned(),
            shot_id: "shot-1".to_owned(),
            role: domain::BindingRole::Character,
            reference_set_id: set_id.to_owned(),
            ordinal: 0,
            required: true,
            inheritance_mode: InheritanceMode::Explicit,
            created_at: when(),
            updated_at: when(),
        }
    }

    fn base_fixture() -> Fixture {
        Fixture {
            project: project(),
            structure: Default::default(),
            shots: vec![shot("shot-1")],
            profiles: Vec::new(),
            costumes: Vec::new(),
            revisions: Vec::new(),
            reference_sets: Vec::new(),
            items: Vec::new(),
            scope_profiles: Vec::new(),
            scope_references: Vec::new(),
            shot_profiles: Vec::new(),
            shot_references: Vec::new(),
            assets: Vec::new(),
            counts: Arc::new(Counts::default()),
            now: when(),
        }
    }

    struct ProjectFake(Arc<Fixture>);
    struct StructureFake(Arc<Fixture>);
    struct ShotFake(Arc<Fixture>);
    struct ScopeFake(Arc<Fixture>);
    struct ProfileFake(Arc<Fixture>);
    struct ReferenceFake(Arc<Fixture>);
    struct ShotConsistencyFake(Arc<Fixture>);
    struct AssetFake(Arc<Fixture>);
    struct FixedClock(DateTime<Utc>);

    fn unused<T>() -> Result<T, RepositoryError> {
        Err(RepositoryError::database("unused test fake method"))
    }

    #[async_trait]
    impl ProjectRepository for ProjectFake {
        async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
            unused()
        }
        async fn find_by_id(&self, id: &str) -> Result<Option<ProjectRecord>, RepositoryError> {
            self.0.counts.project.fetch_add(1, Ordering::SeqCst);
            Ok((id == self.0.project.id).then(|| self.0.project.clone()))
        }
        async fn insert(&self, _: &ProjectRecord) -> Result<(), RepositoryError> {
            unused()
        }
        async fn update_metadata(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
            _: DateTime<Utc>,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            unused()
        }
        async fn get_storage_root(&self, _: &str) -> Result<Option<PathBuf>, RepositoryError> {
            unused()
        }
        async fn ensure_default_project(
            &self,
            _: &str,
            _: &str,
            _: &PathBuf,
            _: DateTime<Utc>,
        ) -> Result<ProjectRecord, RepositoryError> {
            unused()
        }
    }

    #[async_trait]
    impl ProductionStructureRepository for StructureFake {
        async fn load_tree_data(
            &self,
            _: &str,
        ) -> Result<ProductionStructureTreeData, RepositoryError> {
            self.0.counts.structure.fetch_add(1, Ordering::SeqCst);
            Ok(self.0.structure.clone())
        }
        async fn create_series(
            &self,
            _: &domain::ProductionSeries,
        ) -> Result<domain::ProductionSeries, RepositoryError> {
            unused()
        }
        async fn update_series(
            &self,
            _: &domain::ProductionSeries,
        ) -> Result<domain::ProductionSeries, RepositoryError> {
            unused()
        }
        async fn delete_series(
            &self,
            _: &str,
            _: &domain::ProductionSeriesId,
        ) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn reorder_series(
            &self,
            _: &str,
            _: &[domain::ProductionSeriesId],
            _: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn create_episode(
            &self,
            _: &str,
            _: &domain::ProductionEpisode,
        ) -> Result<domain::ProductionEpisode, RepositoryError> {
            unused()
        }
        async fn update_episode(
            &self,
            _: &domain::ProductionEpisode,
            _: &str,
        ) -> Result<domain::ProductionEpisode, RepositoryError> {
            unused()
        }
        async fn delete_episode(
            &self,
            _: &str,
            _: &domain::ProductionEpisodeId,
        ) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn reorder_episodes(
            &self,
            _: &str,
            _: &domain::ProductionSeriesId,
            _: &[domain::ProductionEpisodeId],
            _: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn create_scene(
            &self,
            _: &str,
            _: &domain::ProductionScene,
        ) -> Result<domain::ProductionScene, RepositoryError> {
            unused()
        }
        async fn update_scene(
            &self,
            _: &domain::ProductionScene,
            _: &str,
        ) -> Result<domain::ProductionScene, RepositoryError> {
            unused()
        }
        async fn delete_scene(
            &self,
            _: &str,
            _: &domain::ProductionSceneId,
        ) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn reorder_scenes(
            &self,
            _: &str,
            _: &domain::ProductionEpisodeId,
            _: &[domain::ProductionSceneId],
            _: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn assign_shots_atomic(
            &self,
            _: &str,
            _: &domain::ProductionSceneId,
            _: &[String],
            _: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn unassign_shots_atomic(
            &self,
            _: &str,
            _: &[String],
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn reorder_scene_shots(
            &self,
            _: &domain::ProductionSceneId,
            _: &[String],
            _: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            unused()
        }
    }

    #[async_trait]
    impl ShotRepository for ShotFake {
        async fn list(&self, _: &str) -> Result<Vec<ShotData>, RepositoryError> {
            self.0.counts.shots.fetch_add(1, Ordering::SeqCst);
            Ok(self.0.shots.clone())
        }
        async fn find(&self, _: &str, _: &str) -> Result<Option<ShotData>, RepositoryError> {
            unused()
        }
        async fn insert(&self, _: &ShotRecord) -> Result<(), RepositoryError> {
            unused()
        }
        async fn update(&self, _: &ShotRecord) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn delete(&self, _: &str, _: &str) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn reorder(
            &self,
            _: &str,
            _: &[String],
            _: DateTime<Utc>,
        ) -> Result<Vec<ShotRecord>, RepositoryError> {
            unused()
        }
        async fn upsert_stage_config(
            &self,
            _: &str,
            _: &ShotStageConfigRecord,
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn replace_reference_assets(
            &self,
            _: &str,
            _: &str,
            _: domain::ShotStage,
            _: &[String],
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn select_image(&self, _: &str, _: &str, _: &str) -> Result<(), RepositoryError> {
            unused()
        }
        async fn select_video(&self, _: &str, _: &str, _: &str) -> Result<(), RepositoryError> {
            unused()
        }
        async fn link_generation(
            &self,
            _: &str,
            _: &str,
            _: domain::ShotStage,
            _: &str,
            _: Option<&str>,
            _: DateTime<Utc>,
        ) -> Result<ShotGenerationLinkRecord, RepositoryError> {
            unused()
        }
    }

    #[async_trait]
    impl ConsistencyScopeRepository for ScopeFake {
        async fn list_profile_bindings_for_project(
            &self,
            _: &str,
        ) -> Result<Vec<ScopedProfileBinding>, RepositoryError> {
            self.0.counts.scope_profiles.fetch_add(1, Ordering::SeqCst);
            Ok(self.0.scope_profiles.clone())
        }
        async fn list_reference_set_bindings_for_project(
            &self,
            _: &str,
        ) -> Result<Vec<ScopedReferenceSetBinding>, RepositoryError> {
            self.0
                .counts
                .scope_references
                .fetch_add(1, Ordering::SeqCst);
            Ok(self.0.scope_references.clone())
        }
        async fn replace_profile_bindings(
            &self,
            _: &str,
            _: ConsistencyScopeType,
            _: &str,
            _: &[ScopedProfileBinding],
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn replace_reference_set_bindings(
            &self,
            _: &str,
            _: ConsistencyScopeType,
            _: &str,
            _: &[ScopedReferenceSetBinding],
        ) -> Result<(), RepositoryError> {
            unused()
        }
    }

    #[async_trait]
    impl ConsistencyProfileRepository for ProfileFake {
        async fn list_profiles(
            &self,
            _: &str,
            kind: ProfileType,
        ) -> Result<Vec<ConsistencyProfileRecord>, RepositoryError> {
            self.0.counts.profiles.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .0
                .profiles
                .iter()
                .filter(|p| p.profile_type() == kind)
                .cloned()
                .collect())
        }
        async fn find_profile(
            &self,
            _: &str,
            _: ProfileType,
            _: &str,
        ) -> Result<Option<ConsistencyProfileRecord>, RepositoryError> {
            unused()
        }
        async fn insert_profile(
            &self,
            _: &ConsistencyProfileRecord,
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn update_profile(
            &self,
            _: &ConsistencyProfileRecord,
        ) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn delete_profile(
            &self,
            _: &str,
            _: ProfileType,
            _: &str,
        ) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn list_costume_variants(
            &self,
            _: &str,
        ) -> Result<Vec<CostumeVariant>, RepositoryError> {
            unused()
        }
        async fn list_costume_variants_many(
            &self,
            _: &[String],
        ) -> Result<Vec<CostumeVariant>, RepositoryError> {
            self.0.counts.costumes.fetch_add(1, Ordering::SeqCst);
            Ok(self.0.costumes.clone())
        }
        async fn find_costume_variant(
            &self,
            _: &str,
        ) -> Result<Option<CostumeVariant>, RepositoryError> {
            unused()
        }
        async fn insert_costume_variant(&self, _: &CostumeVariant) -> Result<(), RepositoryError> {
            unused()
        }
        async fn update_costume_variant(
            &self,
            _: &CostumeVariant,
        ) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn delete_costume_variant(&self, _: &str) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn list_profile_revisions(
            &self,
            _: ProfileType,
            _: &str,
        ) -> Result<Vec<ProfileRevision>, RepositoryError> {
            unused()
        }
        async fn find_profile_revision(
            &self,
            _: &str,
        ) -> Result<Option<ProfileRevision>, RepositoryError> {
            unused()
        }
        async fn find_profile_revisions_many(
            &self,
            ids: &[String],
        ) -> Result<Vec<ProfileRevision>, RepositoryError> {
            self.0.counts.revisions.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .0
                .revisions
                .iter()
                .filter(|r| ids.contains(&r.id))
                .cloned()
                .collect())
        }
        async fn insert_profile_revision(
            &self,
            _: &ProfileRevision,
        ) -> Result<(), RepositoryError> {
            unused()
        }
    }

    #[async_trait]
    impl ReferenceSetRepository for ReferenceFake {
        async fn list_reference_sets(
            &self,
            _: &str,
            _: Option<ReferenceSetPurpose>,
        ) -> Result<Vec<ReferenceSet>, RepositoryError> {
            self.0.counts.references.fetch_add(1, Ordering::SeqCst);
            Ok(self.0.reference_sets.clone())
        }
        async fn find_reference_set(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<ReferenceSet>, RepositoryError> {
            unused()
        }
        async fn insert_reference_set(&self, _: &ReferenceSet) -> Result<(), RepositoryError> {
            unused()
        }
        async fn update_reference_set(&self, _: &ReferenceSet) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn delete_reference_set(&self, _: &str, _: &str) -> Result<bool, RepositoryError> {
            unused()
        }
        async fn list_items(&self, _: &str) -> Result<Vec<ReferenceSetItem>, RepositoryError> {
            unused()
        }
        async fn list_items_many(
            &self,
            ids: &[String],
        ) -> Result<Vec<ReferenceSetItem>, RepositoryError> {
            self.0.counts.items.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .0
                .items
                .iter()
                .filter(|item| ids.contains(&item.reference_set_id))
                .cloned()
                .collect())
        }
        async fn replace_items(
            &self,
            _: &str,
            _: &[ReferenceSetItem],
        ) -> Result<(), RepositoryError> {
            unused()
        }
    }

    #[async_trait]
    impl ShotConsistencyRepository for ShotConsistencyFake {
        async fn list_profile_bindings(
            &self,
            _: &str,
        ) -> Result<Vec<ShotProfileBinding>, RepositoryError> {
            unused()
        }
        async fn list_profile_bindings_many(
            &self,
            ids: &[String],
        ) -> Result<Vec<ShotProfileBinding>, RepositoryError> {
            self.0.counts.shot_profiles.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .0
                .shot_profiles
                .iter()
                .filter(|b| ids.contains(&b.shot_id))
                .cloned()
                .collect())
        }
        async fn replace_profile_bindings(
            &self,
            _: &str,
            _: &[ShotProfileBinding],
        ) -> Result<(), RepositoryError> {
            unused()
        }
        async fn list_reference_set_bindings(
            &self,
            _: &str,
        ) -> Result<Vec<ShotReferenceSetBinding>, RepositoryError> {
            unused()
        }
        async fn list_reference_set_bindings_many(
            &self,
            ids: &[String],
        ) -> Result<Vec<ShotReferenceSetBinding>, RepositoryError> {
            self.0.counts.shot_references.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .0
                .shot_references
                .iter()
                .filter(|b| ids.contains(&b.shot_id))
                .cloned()
                .collect())
        }
        async fn replace_reference_set_bindings(
            &self,
            _: &str,
            _: &[ShotReferenceSetBinding],
        ) -> Result<(), RepositoryError> {
            unused()
        }
    }

    #[async_trait]
    impl AssetRepository for AssetFake {
        async fn insert_many(&self, _: &[Asset]) -> Result<(), RepositoryError> {
            unused()
        }
        async fn find_by_id(&self, _: &AssetId) -> Result<Option<Asset>, RepositoryError> {
            unused()
        }
        async fn find_many_by_ids(&self, ids: &[AssetId]) -> Result<Vec<Asset>, RepositoryError> {
            self.0.counts.assets.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .0
                .assets
                .iter()
                .filter(|a| ids.iter().any(|id| id.as_str() == a.id.as_str()))
                .cloned()
                .collect())
        }
        async fn list_by_source_task(
            &self,
            _: &domain::TaskId,
        ) -> Result<Vec<Asset>, RepositoryError> {
            unused()
        }
        async fn list_recent(&self, _: &str, _: u32) -> Result<Vec<Asset>, RepositoryError> {
            unused()
        }
    }

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    fn make_resolver(fixture: Fixture) -> (ShotContextResolver, Arc<Counts>) {
        let fixture = Arc::new(fixture);
        let counts = Arc::clone(&fixture.counts);
        let resolver = ShotContextResolver::new(
            Arc::new(ProjectFake(Arc::clone(&fixture))),
            Arc::new(StructureFake(Arc::clone(&fixture))),
            Arc::new(ShotFake(Arc::clone(&fixture))),
            Arc::new(ScopeFake(Arc::clone(&fixture))),
            Arc::new(ProfileFake(Arc::clone(&fixture))),
            Arc::new(ReferenceFake(Arc::clone(&fixture))),
            Arc::new(ShotConsistencyFake(Arc::clone(&fixture))),
            Arc::new(AssetFake(Arc::clone(&fixture))),
            Arc::new(FixedClock(fixture.now)),
        );
        (resolver, counts)
    }

    fn codes(context: &domain::ResolvedShotContext) -> Vec<&str> {
        context
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect()
    }

    #[tokio::test]
    async fn resolver_uses_stage_owned_prompt_and_legacy_reference_fallback() {
        let image_asset_id = "ast_legacy_image";
        let video_asset_id = "ast_legacy_video";
        let mut fixture = base_fixture();
        fixture.shots[0].reference_assets = vec![
            ShotReferenceAssetRecord {
                shot_id: "shot-1".to_owned(),
                stage: domain::ShotStage::Image,
                asset_id: image_asset_id.to_owned(),
                ordinal: 0,
            },
            ShotReferenceAssetRecord {
                shot_id: "shot-1".to_owned(),
                stage: domain::ShotStage::Video,
                asset_id: video_asset_id.to_owned(),
                ordinal: 0,
            },
        ];
        fixture.assets = vec![
            asset(image_asset_id, PID, AssetType::Image),
            asset(video_asset_id, PID, AssetType::Image),
        ];
        let (resolver, _) = make_resolver(fixture);

        let image = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Image)
            .await
            .unwrap();
        let video = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Video)
            .await
            .unwrap();
        assert_eq!(
            image.prompt_context.segments.last().unwrap().text,
            "image-only prompt"
        );
        assert_eq!(
            video.prompt_context.segments.last().unwrap().text,
            "video-only prompt"
        );
        assert_eq!(
            image
                .reference_assets
                .iter()
                .map(|a| a.asset_id.as_str())
                .collect::<Vec<_>>(),
            vec![image_asset_id]
        );
        assert_eq!(
            video
                .reference_assets
                .iter()
                .map(|a| a.asset_id.as_str())
                .collect::<Vec<_>>(),
            vec![video_asset_id]
        );
        assert!(!image.legacy.has_reference_pack);
        assert!(image.legacy.uses_legacy_shot_references);
    }

    #[tokio::test]
    async fn resolver_resolves_video_selected_image_and_hashes_it() {
        let mut selected_a = base_fixture();
        selected_a.shots[0].shot.selected_image_asset_id = Some("ast_selected_a".to_owned());
        selected_a.assets = vec![asset("ast_selected_a", PID, AssetType::Image)];
        let (resolver_a, _) = make_resolver(selected_a);
        let context_a = resolver_a
            .resolve_draft(PID, "shot-1", domain::ShotStage::Video)
            .await
            .unwrap();

        let mut selected_b = base_fixture();
        selected_b.shots[0].shot.selected_image_asset_id = Some("ast_selected_b".to_owned());
        selected_b.assets = vec![asset("ast_selected_b", PID, AssetType::Image)];
        let (resolver_b, _) = make_resolver(selected_b);
        let context_b = resolver_b
            .resolve_draft(PID, "shot-1", domain::ShotStage::Video)
            .await
            .unwrap();

        assert_eq!(
            context_a.stage_input.selected_image_asset_id.as_deref(),
            Some("ast_selected_a")
        );
        assert_eq!(
            context_a.stage_input.selected_image_sha256.as_deref(),
            Some("sha-ast_selected_a")
        );
        assert_ne!(
            context_a.resolver_identity.context_hash,
            context_b.resolver_identity.context_hash
        );
    }

    #[tokio::test]
    async fn resolver_image_stage_ignores_selected_image_for_input_and_hash() {
        let mut selected_a = base_fixture();
        selected_a.shots[0].shot.selected_image_asset_id = Some("ast_selected_a".to_owned());
        let (resolver_a, _) = make_resolver(selected_a);
        let context_a = resolver_a
            .resolve_draft(PID, "shot-1", domain::ShotStage::Image)
            .await
            .unwrap();

        let mut selected_b = base_fixture();
        selected_b.shots[0].shot.selected_image_asset_id = Some("ast_selected_b".to_owned());
        let (resolver_b, _) = make_resolver(selected_b);
        let context_b = resolver_b
            .resolve_draft(PID, "shot-1", domain::ShotStage::Image)
            .await
            .unwrap();

        assert_eq!(context_a.stage_input, Default::default());
        assert_eq!(context_b.stage_input, Default::default());
        assert_eq!(
            context_a.resolver_identity.context_hash,
            context_b.resolver_identity.context_hash
        );
    }

    #[tokio::test]
    async fn resolver_validates_video_selected_image_boundaries() {
        let mut missing = base_fixture();
        missing.shots[0].shot.selected_image_asset_id = Some("ast_missing".to_owned());
        let (resolver, _) = make_resolver(missing);
        let context = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Video)
            .await
            .unwrap();
        assert!(codes(&context).contains(&"CONTEXT_SELECTED_IMAGE_NOT_FOUND"));
        assert!(context.partial);

        let mut mismatch = base_fixture();
        mismatch.shots[0].shot.selected_image_asset_id = Some("ast_other_project".to_owned());
        mismatch.assets = vec![asset(
            "ast_other_project",
            "different-project",
            AssetType::Image,
        )];
        let (resolver, _) = make_resolver(mismatch);
        let context = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Video)
            .await
            .unwrap();
        assert!(codes(&context).contains(&"CONTEXT_SELECTED_IMAGE_PROJECT_MISMATCH"));
        assert!(context.partial);

        let mut wrong_type = base_fixture();
        wrong_type.shots[0].shot.selected_image_asset_id = Some("ast_video".to_owned());
        wrong_type.assets = vec![asset("ast_video", PID, AssetType::Video)];
        let (resolver, _) = make_resolver(wrong_type);
        let context = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Video)
            .await
            .unwrap();
        assert!(codes(&context).contains(&"CONTEXT_SELECTED_IMAGE_TYPE_INVALID"));
        assert!(context.partial);
    }

    #[tokio::test]
    async fn resolver_reports_revision_warning_missing_and_hash_diagnostics() {
        let mut missing = base_fixture();
        missing.profiles.push(character("char-live", None));
        missing
            .shot_profiles
            .push(profile_binding("binding-live", "char-live"));
        let (resolver, _) = make_resolver(missing);
        let context = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Image)
            .await
            .unwrap();
        assert!(codes(&context).contains(&"CONTEXT_PROFILE_REVISION_MISSING"));
        assert!(!context.partial);

        let mut invalid = base_fixture();
        invalid
            .profiles
            .push(character("char-invalid", Some("rev-missing")));
        invalid
            .shot_profiles
            .push(profile_binding("binding-invalid", "char-invalid"));
        let (resolver, _) = make_resolver(invalid);
        let context = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Image)
            .await
            .unwrap();
        assert!(codes(&context).contains(&"CONTEXT_PROFILE_REVISION_MISSING"));
        assert!(context.partial);

        let mut wrong_hash = base_fixture();
        wrong_hash
            .profiles
            .push(character("char-hash", Some("rev-hash")));
        wrong_hash
            .shot_profiles
            .push(profile_binding("binding-hash", "char-hash"));
        wrong_hash.revisions.push(ProfileRevision {
            id: "rev-hash".to_owned(),
            profile_type: ProfileType::Character,
            profile_id: "char-hash".to_owned(),
            revision_number: 1,
            content_json: "{}".to_owned(),
            content_sha256: "wrong".to_owned(),
            status: ProfileRevisionStatus::Active,
            created_at: when(),
            created_by: None,
        });
        let (resolver, _) = make_resolver(wrong_hash);
        let context = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Image)
            .await
            .unwrap();
        assert!(codes(&context).contains(&"CONTEXT_PROFILE_REVISION_HASH_MISMATCH"));
        assert!(context.partial);
    }

    #[tokio::test]
    async fn resolver_expands_reference_sets_in_order_and_validates_assets() {
        let mut fixture = base_fixture();
        fixture.reference_sets.push(reference_set("refs-1"));
        fixture
            .shot_references
            .push(reference_binding("ref-binding", "refs-1"));
        fixture.items = vec![
            ReferenceSetItem {
                reference_set_id: "refs-1".to_owned(),
                asset_id: "ast_bbbbb".to_owned(),
                ordinal: 1,
                role: None,
                is_primary: false,
                created_at: when(),
            },
            ReferenceSetItem {
                reference_set_id: "refs-1".to_owned(),
                asset_id: "ast_aaaaa".to_owned(),
                ordinal: 0,
                role: None,
                is_primary: true,
                created_at: when(),
            },
            ReferenceSetItem {
                reference_set_id: "refs-1".to_owned(),
                asset_id: "ast_video".to_owned(),
                ordinal: 2,
                role: None,
                is_primary: false,
                created_at: when(),
            },
            ReferenceSetItem {
                reference_set_id: "refs-1".to_owned(),
                asset_id: "ast_missing".to_owned(),
                ordinal: 3,
                role: None,
                is_primary: false,
                created_at: when(),
            },
        ];
        fixture.assets = vec![
            asset("ast_bbbbb", PID, AssetType::Image),
            asset("ast_aaaaa", PID, AssetType::Image),
            asset("ast_video", PID, AssetType::Video),
        ];
        let (resolver, _) = make_resolver(fixture);
        let context = resolver
            .resolve_draft(PID, "shot-1", domain::ShotStage::Image)
            .await
            .unwrap();
        assert_eq!(
            context
                .reference_assets
                .iter()
                .map(|a| a.asset_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ast_aaaaa", "ast_bbbbb"]
        );
        assert!(codes(&context).contains(&"CONTEXT_IMAGE_REQUIRED"));
        assert!(codes(&context).contains(&"CONTEXT_ASSET_NOT_FOUND"));
        assert!(context.partial);
        assert!(context
            .resolver_identity
            .reference_set_content_hashes
            .contains_key("refs-1"));
    }

    #[tokio::test]
    async fn resolver_batch_has_hard_limit_and_constant_read_counts_for_500_shots() {
        let mut too_many = base_fixture();
        too_many.shots = (0..501).map(|i| shot(&format!("shot-{i}"))).collect();
        let (resolver, counts) = make_resolver(too_many);
        let ids = (0..501).map(|i| format!("shot-{i}")).collect::<Vec<_>>();
        let error = resolver
            .resolve_many_draft(PID, &ids, domain::ShotStage::Image)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("CONTEXT_BATCH_LIMIT"));
        assert_eq!(counts.project.load(Ordering::SeqCst), 0);

        let mut fixture = base_fixture();
        fixture.shots = (0..500).map(|i| shot(&format!("shot-{i}"))).collect();
        let (resolver, counts) = make_resolver(fixture);
        let ids = (0..500).map(|i| format!("shot-{i}")).collect::<Vec<_>>();
        let resolved = resolver
            .resolve_many_draft(PID, &ids, domain::ShotStage::Image)
            .await
            .unwrap();
        assert_eq!(resolved.len(), 500);
        assert_eq!(counts.project.load(Ordering::SeqCst), 1);
        assert_eq!(counts.structure.load(Ordering::SeqCst), 1);
        assert_eq!(counts.shots.load(Ordering::SeqCst), 1);
        assert_eq!(counts.scope_profiles.load(Ordering::SeqCst), 1);
        assert_eq!(counts.scope_references.load(Ordering::SeqCst), 1);
        assert_eq!(counts.shot_profiles.load(Ordering::SeqCst), 1);
        assert_eq!(counts.shot_references.load(Ordering::SeqCst), 1);
        assert_eq!(counts.profiles.load(Ordering::SeqCst), 4);
        assert_eq!(counts.costumes.load(Ordering::SeqCst), 1);
        assert_eq!(counts.references.load(Ordering::SeqCst), 1);
        assert_eq!(counts.items.load(Ordering::SeqCst), 1);
        assert_eq!(counts.revisions.load(Ordering::SeqCst), 1);
        assert_eq!(counts.assets.load(Ordering::SeqCst), 1);
    }
}
