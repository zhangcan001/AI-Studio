use ai_studio_lib::application::consistency_scope_binding_service::{
    ConsistencyProfileBindingInput, ConsistencyReferenceSetBindingInput,
    ConsistencyScopeBindingService,
};
use ai_studio_lib::application::ports::{
    Clock, ConsistencyProfileRepository, ConsistencyScopeRepository, ProductionStructureRepository,
    ReferenceSetRepository, ShotConsistencyRepository, ShotRecord, ShotRepository,
};
use ai_studio_lib::application::shot_consistency_binding_service::ShotConsistencyBindingService;
use ai_studio_lib::domain::consistency::{
    generate_consistency_id, BindingRole, ConsistencyIdKind, ConsistencyProfileRecord,
    InheritanceMode, ProfileType, ReferenceSetPurpose, ShotProfileBinding, ShotReferenceSetBinding,
};
use ai_studio_lib::domain::{
    CharacterProfile, ProductionEpisode, ProductionEpisodeId, ProductionScene, ProductionSceneId,
    ProductionSeries, ProductionSeriesId,
};
use ai_studio_lib::infrastructure::database::{
    initialize,
    repositories::{
        SqliteConsistencyProfileRepository, SqliteConsistencyScopeRepository,
        SqliteProductionStructureRepository, SqliteReferenceSetRepository,
        SqliteShotConsistencyRepository, SqliteShotRepository,
    },
};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::SqlitePool;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};

const PROJECT_ID: &str = "project-1";
const SHOT_ID: &str = "sht-054";

struct FixedClock(DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

fn now(second: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_735_689_600 + second, 0)
        .single()
        .expect("valid fixture time")
}

async fn database() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory");
    let pool = initialize(&directory.path().join("dev054-bindings.db"))
        .await
        .expect("database should initialize");
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES ('project-1', 'Project', NULL, 'C:/project', ?, ?)",
    )
    .bind(now(0).to_rfc3339())
    .bind(now(0).to_rfc3339())
    .execute(&pool)
    .await
    .expect("project fixture should insert");
    (directory, pool)
}

async fn insert_shot(pool: &SqlitePool, shot_id: &str, project_id: &str) {
    SqliteShotRepository::new(pool.clone())
        .insert(&ShotRecord {
            id: shot_id.to_owned(),
            project_id: project_id.to_owned(),
            ordinal: 0,
            name: shot_id.to_owned(),
            prompt_text: String::new(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            created_at: now(0),
            updated_at: now(0),
        })
        .await
        .expect("shot fixture should insert");
}

async fn insert_character(
    pool: &SqlitePool,
    profile_id: &str,
) -> Arc<SqliteConsistencyProfileRepository> {
    let repository = Arc::new(SqliteConsistencyProfileRepository::new(pool.clone()));
    repository
        .insert_profile(&ConsistencyProfileRecord::Character(CharacterProfile {
            id: profile_id.to_owned(),
            project_id: PROJECT_ID.to_owned(),
            name: format!("Character {profile_id}"),
            description: String::new(),
            canonical_prompt: "hero".to_owned(),
            negative_prompt: String::new(),
            default_style_profile_id: None,
            default_reference_set_id: None,
            active_revision_id: None,
            metadata_json: "{}".to_owned(),
            created_at: now(0),
            updated_at: now(0),
        }))
        .await
        .expect("character fixture should insert");
    repository
}

async fn insert_character_reference_set(pool: &SqlitePool, reference_set_id: &str) {
    SqliteReferenceSetRepository::new(pool.clone())
        .insert_reference_set(&ai_studio_lib::domain::consistency::ReferenceSet {
            id: reference_set_id.to_owned(),
            project_id: PROJECT_ID.to_owned(),
            name: format!("References {reference_set_id}"),
            purpose: ReferenceSetPurpose::Character,
            description: String::new(),
            owner_profile_type: None,
            owner_profile_id: None,
            active_revision_id: None,
            created_at: now(0),
            updated_at: now(0),
        })
        .await
        .expect("reference set fixture should insert");
}

fn profile_input(profile_id: &str, mode: InheritanceMode) -> ConsistencyProfileBindingInput {
    ConsistencyProfileBindingInput {
        id: None,
        role: BindingRole::Character,
        profile_type: ProfileType::Character,
        profile_id: profile_id.to_owned(),
        costume_variant_id: None,
        ordinal: 0,
        inheritance_mode: mode,
    }
}

fn reference_input(
    reference_set_id: &str,
    mode: InheritanceMode,
) -> ConsistencyReferenceSetBindingInput {
    ConsistencyReferenceSetBindingInput {
        id: None,
        role: BindingRole::Character,
        reference_set_id: reference_set_id.to_owned(),
        ordinal: 0,
        required: true,
        inheritance_mode: mode,
    }
}

#[tokio::test]
async fn scope_remove_then_shot_explicit_readd_and_replace_are_real() {
    let (_directory, pool) = database().await;
    insert_shot(&pool, SHOT_ID, PROJECT_ID).await;
    let profile_id = generate_consistency_id(ConsistencyIdKind::CharacterProfile);
    let profile_repository = insert_character(&pool, &profile_id).await;
    let reference_repository = Arc::new(SqliteReferenceSetRepository::new(pool.clone()));
    let structure_repository = Arc::new(SqliteProductionStructureRepository::new(pool.clone()));
    let series = structure_repository
        .create_series(&ProductionSeries {
            id: ProductionSeriesId::parse("ser_054").unwrap(),
            project_id: PROJECT_ID.to_owned(),
            ordinal: 0,
            name: "Series".to_owned(),
            description: String::new(),
            created_at: now(0),
            updated_at: now(0),
        })
        .await
        .unwrap();
    let episode = structure_repository
        .create_episode(
            PROJECT_ID,
            &ProductionEpisode {
                id: ProductionEpisodeId::parse("epi_054").unwrap(),
                series_id: series.id.clone(),
                ordinal: 0,
                name: "Episode".to_owned(),
                description: String::new(),
                created_at: now(0),
                updated_at: now(0),
            },
        )
        .await
        .unwrap();
    let scene = structure_repository
        .create_scene(
            PROJECT_ID,
            &ProductionScene {
                id: ProductionSceneId::parse("scn_054").unwrap(),
                episode_id: episode.id.clone(),
                ordinal: 0,
                name: "Scene".to_owned(),
                description: String::new(),
                created_at: now(0),
                updated_at: now(0),
            },
        )
        .await
        .unwrap();
    structure_repository
        .assign_shots_atomic(PROJECT_ID, &scene.id, &[SHOT_ID.to_owned()], now(0))
        .await
        .unwrap();

    let scope_repository: Arc<dyn ConsistencyScopeRepository> =
        Arc::new(SqliteConsistencyScopeRepository::new(pool.clone()));
    let scope_service = ConsistencyScopeBindingService::new_with_clock(
        scope_repository,
        profile_repository.clone(),
        reference_repository.clone(),
        structure_repository.clone(),
        Arc::new(FixedClock(now(1))),
    );
    scope_service
        .replace_binding_pack(
            PROJECT_ID,
            ai_studio_lib::domain::consistency::ConsistencyScopeType::Project,
            PROJECT_ID,
            &[profile_input(&profile_id, InheritanceMode::Explicit)],
            &[],
        )
        .await
        .unwrap();
    scope_service
        .replace_binding_pack(
            PROJECT_ID,
            ai_studio_lib::domain::consistency::ConsistencyScopeType::Scene,
            scene.id.as_str(),
            &[profile_input(&profile_id, InheritanceMode::Remove)],
            &[],
        )
        .await
        .unwrap();
    let scene_pack = scope_service
        .get_binding_pack(
            PROJECT_ID,
            ai_studio_lib::domain::consistency::ConsistencyScopeType::Scene,
            scene.id.as_str(),
        )
        .await
        .unwrap();
    assert_eq!(scene_pack.ancestors.len(), 3);
    assert_eq!(
        scene_pack.direct_profile_bindings[0].inheritance_mode,
        InheritanceMode::Remove
    );

    scope_service
        .replace_binding_pack(
            PROJECT_ID,
            ai_studio_lib::domain::consistency::ConsistencyScopeType::Scene,
            scene.id.as_str(),
            &[profile_input(&profile_id, InheritanceMode::Replace)],
            &[],
        )
        .await
        .unwrap();
    let shot_service = ShotConsistencyBindingService::new(
        Arc::new(SqliteShotConsistencyRepository::new(pool.clone())),
        Arc::new(SqliteShotRepository::new(pool)),
        profile_repository,
        reference_repository,
        Arc::new(FixedClock(now(2))),
    );
    shot_service
        .replace_binding_pack(
            PROJECT_ID,
            SHOT_ID,
            &[profile_input(&profile_id, InheritanceMode::Explicit)],
            &[],
        )
        .await
        .unwrap();
    let shot_pack = shot_service
        .get_binding_pack(PROJECT_ID, SHOT_ID)
        .await
        .unwrap();
    assert_eq!(shot_pack.profile_bindings.len(), 1);
    assert_eq!(shot_pack.profile_bindings[0].profile_id, profile_id);
}

#[tokio::test]
async fn shot_binding_rejects_cross_project_and_preserves_created_at() {
    let (_directory, pool) = database().await;
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES ('project-2', 'Other', NULL, 'C:/other', ?, ?)",
    )
    .bind(now(0).to_rfc3339())
    .bind(now(0).to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();
    insert_shot(&pool, SHOT_ID, PROJECT_ID).await;
    insert_shot(&pool, "sht-054-other", "project-2").await;
    let profile_id = generate_consistency_id(ConsistencyIdKind::CharacterProfile);
    let profile_repository = insert_character(&pool, &profile_id).await;
    let reference_repository = Arc::new(SqliteReferenceSetRepository::new(pool.clone()));
    let service = ShotConsistencyBindingService::new(
        Arc::new(SqliteShotConsistencyRepository::new(pool.clone())),
        Arc::new(SqliteShotRepository::new(pool)),
        profile_repository,
        reference_repository,
        Arc::new(FixedClock(now(3))),
    );
    service
        .replace_binding_pack(
            PROJECT_ID,
            SHOT_ID,
            &[profile_input(&profile_id, InheritanceMode::Explicit)],
            &[],
        )
        .await
        .unwrap();
    let original = service.get_binding_pack(PROJECT_ID, SHOT_ID).await.unwrap();
    let id = original.profile_bindings[0].id.clone();
    let created_at = original.profile_bindings[0].created_at;
    let mut update = profile_input(&profile_id, InheritanceMode::Replace);
    update.id = Some(id);
    service
        .replace_binding_pack(PROJECT_ID, SHOT_ID, &[update], &[])
        .await
        .unwrap();
    let updated = service.get_binding_pack(PROJECT_ID, SHOT_ID).await.unwrap();
    assert_eq!(updated.profile_bindings[0].created_at, created_at);
    assert_eq!(updated.profile_bindings[0].updated_at, now(3));

    let error = service
        .replace_binding_pack(
            "project-2",
            SHOT_ID,
            &[profile_input(&profile_id, InheritanceMode::Explicit)],
            &[],
        )
        .await
        .expect_err("a project must not edit another project's shot");
    assert!(error.to_string().contains("CONSISTENCY_SHOT_NOT_FOUND"));
}

#[tokio::test]
async fn sqlite_combined_shot_replace_rolls_back_profile_when_reference_insert_fails() {
    let (_directory, pool) = database().await;
    insert_shot(&pool, SHOT_ID, PROJECT_ID).await;
    let reference_set_id = generate_consistency_id(ConsistencyIdKind::ReferenceSet);
    insert_character_reference_set(&pool, &reference_set_id).await;
    let repository = SqliteShotConsistencyRepository::new(pool);
    let original_profile_id = generate_consistency_id(ConsistencyIdKind::CharacterProfile);
    let original_profile_binding = ShotProfileBinding {
        id: generate_consistency_id(ConsistencyIdKind::ShotProfileBinding),
        shot_id: SHOT_ID.to_owned(),
        role: BindingRole::Character,
        profile_type: ProfileType::Character,
        profile_id: original_profile_id,
        costume_variant_id: None,
        ordinal: 0,
        inheritance_mode: InheritanceMode::Explicit,
        created_at: now(0),
        updated_at: now(0),
    };
    let original_reference_binding = ShotReferenceSetBinding {
        id: generate_consistency_id(ConsistencyIdKind::ShotReferenceSetBinding),
        shot_id: SHOT_ID.to_owned(),
        role: BindingRole::Character,
        reference_set_id: reference_set_id.clone(),
        ordinal: 0,
        required: true,
        inheritance_mode: InheritanceMode::Explicit,
        created_at: now(0),
        updated_at: now(0),
    };
    repository
        .replace_binding_pack(
            SHOT_ID,
            &[original_profile_binding.clone()],
            &[original_reference_binding.clone()],
        )
        .await
        .unwrap();
    let replacement = ShotProfileBinding {
        id: generate_consistency_id(ConsistencyIdKind::ShotProfileBinding),
        profile_id: generate_consistency_id(ConsistencyIdKind::CharacterProfile),
        ..original_profile_binding.clone()
    };
    let invalid_reference = ShotReferenceSetBinding {
        id: generate_consistency_id(ConsistencyIdKind::ShotReferenceSetBinding),
        reference_set_id: generate_consistency_id(ConsistencyIdKind::ReferenceSet),
        ..original_reference_binding.clone()
    };
    assert!(repository
        .replace_binding_pack(SHOT_ID, &[replacement], &[invalid_reference])
        .await
        .is_err());
    assert_eq!(
        repository.list_profile_bindings(SHOT_ID).await.unwrap(),
        vec![original_profile_binding]
    );
    assert_eq!(
        repository
            .list_reference_set_bindings(SHOT_ID)
            .await
            .unwrap(),
        vec![original_reference_binding]
    );
}
