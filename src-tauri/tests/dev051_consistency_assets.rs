//! DEV-051 command-contract and real SQLite service/repository tests.
//!
//! The production command module is intentionally not registered here: Main
//! owns AppState and command registration.  The fixture below includes the
//! existing application services and SQLite repositories so the behavioral
//! tests exercise the same public service seams that the commands delegate to.

#![allow(dead_code, unused_imports)]

mod domain {
    pub use ai_studio_lib::domain::*;
}

mod ports {
    pub use ai_studio_lib::{
        AssetRepository, Clock, ProjectRecord, ProjectRepository, RepositoryError,
        TaskOutputAssetMapping, TaskRepository,
    };

    pub mod asset_deletion_repository {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/application/ports/asset_deletion_repository.rs"
        ));
    }
    pub use asset_deletion_repository::{AssetDeletionReferences, AssetDeletionRepository};

    pub mod asset_usage_repository {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/application/ports/asset_usage_repository.rs"
        ));
    }
    pub use asset_usage_repository::{
        AssetUsageItem, AssetUsageRepository, AssetUsageSummary, ProfileUsageSummary,
        ReferenceSetUsageSummary,
    };

    pub mod consistency_profile_repository {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/application/ports/consistency_profile_repository.rs"
        ));
    }
    pub use consistency_profile_repository::ConsistencyProfileRepository;

    pub mod reference_anchor_repository {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/application/ports/reference_anchor_repository.rs"
        ));
    }
    pub use reference_anchor_repository::{ReferenceAnchorRecord, ReferenceAnchorRepository};

    pub mod reference_set_repository {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/application/ports/reference_set_repository.rs"
        ));
    }
    pub use reference_set_repository::ReferenceSetRepository;
}

#[path = "../src/application/consistency_profile_service.rs"]
pub mod consistency_profile_service;
#[path = "../src/application/reference_set_service.rs"]
pub mod reference_set_service;

mod application {
    pub mod ports {
        pub use crate::ports::*;
    }

    pub use crate::consistency_profile_service;
    pub use crate::reference_set_service;
}

mod infrastructure {
    pub mod database {
        pub use ai_studio_lib::initialize;

        pub mod repositories {
            use crate::application::ports::RepositoryError;
            use chrono::{DateTime, Utc};
            use serde_json::Value;
            use sqlx::{Sqlite, Transaction};

            pub(super) fn map_sqlx_error(error: sqlx::Error) -> RepositoryError {
                if let sqlx::Error::Database(database_error) = &error {
                    let message = database_error.message().to_owned();
                    let lowercase = message.to_ascii_lowercase();
                    if lowercase.contains("constraint")
                        || lowercase.contains("unique")
                        || lowercase.contains("foreign key")
                    {
                        return RepositoryError::integrity(message);
                    }
                }
                RepositoryError::database(error.to_string())
            }

            pub(super) fn map_domain_error(
                context: &str,
                error: impl std::fmt::Display,
            ) -> RepositoryError {
                RepositoryError::integrity(format!("{context}: {error}"))
            }

            pub(super) fn format_datetime(value: DateTime<Utc>) -> String {
                value.to_rfc3339()
            }

            pub(super) fn parse_datetime(
                field: &str,
                value: &str,
            ) -> Result<DateTime<Utc>, RepositoryError> {
                DateTime::parse_from_rfc3339(value)
                    .map(|parsed| parsed.with_timezone(&Utc))
                    .map_err(|error| RepositoryError::serialization(field, error.to_string()))
            }

            pub(super) fn parse_json(
                context: &str,
                value: Option<&str>,
            ) -> Result<Option<Value>, RepositoryError> {
                value
                    .map(|value| {
                        serde_json::from_str(value).map_err(|error| {
                            RepositoryError::serialization(context, error.to_string())
                        })
                    })
                    .transpose()
            }

            pub(super) fn serialize_json(
                context: &str,
                value: Option<&Value>,
            ) -> Result<Option<String>, RepositoryError> {
                value
                    .map(|value| {
                        serde_json::to_string(value).map_err(|error| {
                            RepositoryError::serialization(context, error.to_string())
                        })
                    })
                    .transpose()
            }

            pub(super) fn i64_to_u64(context: &str, value: i64) -> Result<u64, RepositoryError> {
                u64::try_from(value).map_err(|_| {
                    RepositoryError::serialization(
                        context,
                        format!("negative value {value} is invalid"),
                    )
                })
            }

            pub mod asset {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/infrastructure/database/repositories/asset.rs"
                ));
            }
            pub mod asset_deletion {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/infrastructure/database/repositories/asset_deletion.rs"
                ));
            }
            pub mod asset_usage {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/infrastructure/database/repositories/asset_usage.rs"
                ));
            }
            pub mod consistency_profile {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/infrastructure/database/repositories/consistency_profile.rs"
                ));
            }
            pub mod project {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/infrastructure/database/repositories/project.rs"
                ));
            }
            pub mod reference_anchor {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/infrastructure/database/repositories/reference_anchor.rs"
                ));
            }
            pub mod reference_set {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/infrastructure/database/repositories/reference_set.rs"
                ));
            }

            pub use ai_studio_lib::SqliteTaskRepository;
            pub use asset::SqliteAssetRepository;
            pub use asset_deletion::SqliteAssetDeletionRepository;
            pub use asset_usage::SqliteAssetUsageRepository;
            pub use consistency_profile::SqliteConsistencyProfileRepository;
            pub use project::SqliteProjectRepository;
            pub use reference_anchor::SqliteReferenceAnchorRepository;
            pub use reference_set::SqliteReferenceSetRepository;

            #[cfg(test)]
            pub(crate) mod test_support {
                use sqlx::SqlitePool;

                pub(crate) async fn seed_task_dependencies(pool: &SqlitePool) {
                    sqlx::query(
                        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
                         VALUES ('project-1', 'Project', NULL, 'C:/project', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    )
                    .execute(pool)
                    .await
                    .expect("project fixture should insert");
                    sqlx::query(
                        "INSERT INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at)
                         VALUES ('workflow-1', 'Workflow', 'test', 'image', 'workflow-version-1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    )
                    .execute(pool)
                    .await
                    .expect("workflow fixture should insert");
                    sqlx::query(
                        "INSERT INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
                         VALUES ('workflow-version-1', 'workflow-1', '1', '{}', 'sha', '2026-01-01T00:00:00Z')",
                    )
                    .execute(pool)
                    .await
                    .expect("workflow version fixture should insert");
                    sqlx::query(
                        "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
                         VALUES ('recipe-1', 'workflow-version-1', '1', 1, 'schema_version: 1', 'sha', '2026-01-01T00:00:00Z')",
                    )
                    .execute(pool)
                    .await
                    .expect("recipe fixture should insert");
                }
            }
        }
    }
}

use application::consistency_profile_service::{
    ConsistencyProfileService, CreateCharacterProfileRequest, CreateCostumeVariantRequest,
    UpdateCharacterProfileRequest, UpdateCostumeVariantRequest,
};
use application::ports::{
    AssetDeletionRepository, AssetUsageRepository, Clock, ConsistencyProfileRepository,
    ProjectRepository, ReferenceAnchorRepository, ReferenceSetRepository,
};
use application::reference_set_service::{
    CreateReferenceSetRequest, ReferenceSetItemRequest, ReferenceSetService,
    UpdateReferenceSetRequest,
};
use chrono::{DateTime, TimeZone, Utc};
use domain::consistency::{ProfileType, ReferenceSetPurpose};
use infrastructure::database::repositories::{
    SqliteAssetDeletionRepository, SqliteAssetRepository, SqliteAssetUsageRepository,
    SqliteConsistencyProfileRepository, SqliteProjectRepository, SqliteReferenceAnchorRepository,
    SqliteReferenceSetRepository,
};
use sqlx::SqlitePool;
use std::{fs, sync::Arc};
use tempfile::{tempdir, TempDir};

const PROJECT_ID: &str = "project-dev051";
const CREATED_AT: &str = "2026-08-26T00:00:00Z";

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

async fn database() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory should exist");
    let pool = infrastructure::database::initialize(&directory.path().join("dev051.db"))
        .await
        .expect("database should initialize");
    insert_project(&pool, PROJECT_ID).await;
    (directory, pool)
}

async fn insert_project(pool: &SqlitePool, project_id: &str) {
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

async fn insert_asset(
    pool: &SqlitePool,
    asset_id: &str,
    project_id: &str,
    name: &str,
    thumbnail_path: Option<&str>,
    width: i64,
    height: i64,
) {
    sqlx::query(
        "INSERT INTO assets
         (id, project_id, type, category, name, original_name, storage_path,
          thumbnail_path, sha256, mime_type, width, height, duration_ms, file_size,
          source_task_id, metadata_json, created_at, updated_at)
         VALUES (?, ?, 'image', 'source_image', ?, ?, ?, ?, ?, 'image/png', ?, ?, NULL, 1, NULL, '{}', ?, ?)",
    )
    .bind(asset_id)
    .bind(project_id)
    .bind(name)
    .bind(format!("{asset_id}.png"))
    .bind(format!("C:/{project_id}/{asset_id}.png"))
    .bind(thumbnail_path)
    .bind(format!("sha-{asset_id}"))
    .bind(width)
    .bind(height)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("asset fixture should insert");
}

fn services(pool: &SqlitePool) -> (ConsistencyProfileService, ReferenceSetService) {
    let profile_repository: Arc<dyn ConsistencyProfileRepository> =
        Arc::new(SqliteConsistencyProfileRepository::new(pool.clone()));
    let reference_set_repository: Arc<dyn ReferenceSetRepository> =
        Arc::new(SqliteReferenceSetRepository::new(pool.clone()));
    let project_repository: Arc<dyn ProjectRepository> =
        Arc::new(SqliteProjectRepository::new(pool.clone()));
    let asset_repository: Arc<dyn application::ports::AssetRepository> =
        Arc::new(SqliteAssetRepository::new(pool.clone()));
    let anchor_repository: Arc<dyn ReferenceAnchorRepository> =
        Arc::new(SqliteReferenceAnchorRepository::new(pool.clone()));
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(now()));

    let profiles = ConsistencyProfileService::new(
        profile_repository.clone(),
        reference_set_repository.clone(),
        project_repository.clone(),
        clock.clone(),
    );
    let reference_sets = ReferenceSetService::new(
        reference_set_repository,
        profile_repository,
        asset_repository,
        anchor_repository,
        project_repository,
        clock,
    );
    (profiles, reference_sets)
}

#[tokio::test]
async fn character_profile_and_costume_crud_round_trip_through_real_services() {
    let (_directory, pool) = database().await;
    let (profiles, _reference_sets) = services(&pool);

    let character = profiles
        .create_character(CreateCharacterProfileRequest {
            project_id: PROJECT_ID.to_owned(),
            name: "  主角  ".to_owned(),
            description: "稳定角色身份".to_owned(),
            canonical_prompt: "young hero".to_owned(),
            negative_prompt: "extra fingers".to_owned(),
            default_style_profile_id: None,
            default_reference_set_id: None,
            metadata_json: "{}".to_owned(),
        })
        .await
        .expect("character should be created");

    let listed = profiles
        .list(PROJECT_ID, Some(ProfileType::Character))
        .await
        .expect("character list should load");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id(), character.id);

    let updated = profiles
        .update_character(UpdateCharacterProfileRequest {
            project_id: PROJECT_ID.to_owned(),
            profile_id: character.id.clone(),
            name: "更新后的主角".to_owned(),
            description: "更新描述".to_owned(),
            canonical_prompt: "updated hero".to_owned(),
            negative_prompt: "blur".to_owned(),
            default_style_profile_id: None,
            default_reference_set_id: None,
            metadata_json: "{\"stable\":true}".to_owned(),
        })
        .await
        .expect("character should be updated");
    assert_eq!(updated.id, character.id);
    assert_eq!(updated.name, "更新后的主角");
    assert_eq!(updated.created_at, character.created_at);

    let costume = profiles
        .create_costume(CreateCostumeVariantRequest {
            project_id: PROJECT_ID.to_owned(),
            character_profile_id: character.id.clone(),
            name: "旅行装".to_owned(),
            prompt_fragment: "dark travel coat".to_owned(),
            reference_set_id: None,
            is_default: true,
            ordinal: 0,
        })
        .await
        .expect("costume should be created");
    assert_eq!(
        profiles
            .list_costumes(PROJECT_ID, &character.id)
            .await
            .expect("costume list should load"),
        vec![costume.clone()]
    );

    let updated_costume = profiles
        .update_costume(UpdateCostumeVariantRequest {
            project_id: PROJECT_ID.to_owned(),
            costume_variant_id: costume.id.clone(),
            name: "更新旅行装".to_owned(),
            prompt_fragment: "updated coat".to_owned(),
            reference_set_id: None,
            is_default: false,
            ordinal: 1,
        })
        .await
        .expect("costume should be updated");
    assert_eq!(updated_costume.id, costume.id);
    assert_eq!(updated_costume.ordinal, 1);
    profiles
        .delete_costume(PROJECT_ID, &costume.id)
        .await
        .expect("costume should be deleted");
    assert!(profiles
        .list_costumes(PROJECT_ID, &character.id)
        .await
        .expect("costume list should reload")
        .is_empty());
}

#[tokio::test]
async fn reference_set_detail_returns_ordered_batch_asset_projection() {
    let (_directory, pool) = database().await;
    insert_asset(
        &pool,
        "ast_dev051_first",
        PROJECT_ID,
        "First reference",
        Some("thumb-first.png"),
        640,
        480,
    )
    .await;
    insert_asset(
        &pool,
        "ast_dev051_second",
        PROJECT_ID,
        "Second reference",
        None,
        1920,
        1080,
    )
    .await;
    let (_profiles, reference_sets) = services(&pool);

    let created = reference_sets
        .create(CreateReferenceSetRequest {
            project_id: PROJECT_ID.to_owned(),
            name: "角色参考集".to_owned(),
            purpose: ReferenceSetPurpose::Character,
            description: "主角的稳定参考".to_owned(),
            owner_profile_type: None,
            owner_profile_id: None,
            items: vec![
                ReferenceSetItemRequest {
                    asset_id: "ast_dev051_first".to_owned(),
                    ordinal: 0,
                    role: Some("FACE".to_owned()),
                    is_primary: true,
                },
                ReferenceSetItemRequest {
                    asset_id: "ast_dev051_second".to_owned(),
                    ordinal: 1,
                    role: Some("FULL_BODY".to_owned()),
                    is_primary: false,
                },
            ],
        })
        .await
        .expect("reference set should be created");

    let detail = reference_sets
        .get_detail(PROJECT_ID, &created.id)
        .await
        .expect("reference set detail should load");
    assert_eq!(detail.reference_set.id, created.id);
    assert_eq!(detail.items.len(), 2);
    assert_eq!(detail.items[0].asset_id, "ast_dev051_first");
    assert_eq!(detail.items[0].asset_name, "First reference");
    assert_eq!(detail.items[0].thumbnail_available, true);
    assert_eq!((detail.items[0].width, detail.items[0].height), (640, 480));
    assert_eq!(detail.items[0].role.as_deref(), Some("FACE"));
    assert!(detail.items[0].is_primary);
    assert_eq!(detail.items[1].asset_id, "ast_dev051_second");
    assert!(!detail.items[1].thumbnail_available);
    assert_eq!(
        (detail.items[1].width, detail.items[1].height),
        (1920, 1080)
    );
}

#[tokio::test]
async fn usage_and_deletion_blocker_fixtures_are_project_scoped() {
    let (_directory, pool) = database().await;
    insert_asset(
        &pool,
        "ast_dev051_usage",
        PROJECT_ID,
        "Usage asset",
        None,
        128,
        128,
    )
    .await;
    let (profiles, reference_sets) = services(&pool);
    let reference_set = reference_sets
        .create(CreateReferenceSetRequest {
            project_id: PROJECT_ID.to_owned(),
            name: "Usage references".to_owned(),
            purpose: ReferenceSetPurpose::Character,
            description: String::new(),
            owner_profile_type: None,
            owner_profile_id: None,
            items: vec![ReferenceSetItemRequest {
                asset_id: "ast_dev051_usage".to_owned(),
                ordinal: 0,
                role: None,
                is_primary: true,
            }],
        })
        .await
        .expect("usage reference set should be created");
    let character = profiles
        .create_character(CreateCharacterProfileRequest {
            project_id: PROJECT_ID.to_owned(),
            name: "Usage character".to_owned(),
            description: String::new(),
            canonical_prompt: "character".to_owned(),
            negative_prompt: String::new(),
            default_style_profile_id: None,
            default_reference_set_id: Some(reference_set.id.clone()),
            metadata_json: "{}".to_owned(),
        })
        .await
        .expect("usage character should be created");
    let reference_set = reference_sets
        .update(UpdateReferenceSetRequest {
            project_id: PROJECT_ID.to_owned(),
            reference_set_id: reference_set.id.clone(),
            name: reference_set.name.clone(),
            purpose: ReferenceSetPurpose::Character,
            description: reference_set.description.clone(),
            owner_profile_type: Some(ProfileType::Character),
            owner_profile_id: Some(character.id.clone()),
            items: vec![ReferenceSetItemRequest {
                asset_id: "ast_dev051_usage".to_owned(),
                ordinal: 0,
                role: None,
                is_primary: true,
            }],
        })
        .await
        .expect("reference set owner should be assigned");

    let usage_repository = SqliteAssetUsageRepository::new(pool.clone());
    let asset_id = domain::AssetId::parse("ast_dev051_usage").expect("asset id should parse");
    let asset_usage = usage_repository
        .asset_usage(PROJECT_ID, &asset_id)
        .await
        .expect("asset usage should load");
    assert_eq!(asset_usage.asset_id, "ast_dev051_usage");
    assert!(asset_usage
        .reference_sets
        .iter()
        .any(|item| item.reference_set_id.as_deref() == Some(reference_set.id.as_str())));
    assert!(asset_usage
        .profiles
        .iter()
        .any(|item| item.entity_id == character.id));
    assert!(asset_usage.blocking_count > 0);

    let profile_usage = usage_repository
        .profile_usage(PROJECT_ID, ProfileType::Character, &character.id)
        .await
        .expect("profile usage should load");
    assert_eq!(profile_usage.profile_id, character.id);
    assert!(profile_usage
        .reference_sets
        .iter()
        .any(|item| item.reference_set_id.as_deref() == Some(reference_set.id.as_str())));

    let reference_set_usage = usage_repository
        .reference_set_usage(PROJECT_ID, &reference_set.id)
        .await
        .expect("reference set usage should load");
    assert_eq!(reference_set_usage.reference_set_id, reference_set.id);
    assert_eq!(reference_set_usage.item_count, 1);
    assert!(reference_set_usage
        .profile_defaults
        .iter()
        .any(|item| item.entity_id == character.id));

    let deletion_repository = SqliteAssetDeletionRepository::new(pool.clone());
    let deletion_references = deletion_repository
        .references_for(PROJECT_ID, &[asset_id])
        .await
        .expect("asset deletion references should load");
    assert_eq!(deletion_references.len(), 1);
    assert!(
        deletion_references[0]
            .reference_set_ids
            .contains(&reference_set.id),
        "live ReferenceSet relation must remain a deletion blocker"
    );
}

#[tokio::test]
async fn explicit_anchor_conversion_preserves_legacy_anchor_rows() {
    let (_directory, pool) = database().await;
    insert_asset(
        &pool,
        "ast_dev051_anchor_a",
        PROJECT_ID,
        "Anchor A",
        None,
        256,
        256,
    )
    .await;
    insert_asset(
        &pool,
        "ast_dev051_anchor_b",
        PROJECT_ID,
        "Anchor B",
        None,
        512,
        512,
    )
    .await;
    sqlx::query(
        "INSERT INTO reference_anchors
         (id, project_id, kind, name, normalized_name, description, created_at, updated_at)
         VALUES ('anc_dev051_legacy', ?, 'CHARACTER', 'Legacy Anchor', 'legacy anchor', 'legacy description', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&pool)
    .await
    .expect("legacy anchor should insert");
    for (asset_id, ordinal) in [
        ("ast_dev051_anchor_b", 3_i64),
        ("ast_dev051_anchor_a", 7_i64),
    ] {
        sqlx::query(
            "INSERT INTO reference_anchor_assets (anchor_id, asset_id, ordinal, created_at)
             VALUES ('anc_dev051_legacy', ?, ?, ?)",
        )
        .bind(asset_id)
        .bind(ordinal)
        .bind(CREATED_AT)
        .execute(&pool)
        .await
        .expect("legacy anchor asset should insert");
    }
    let before = sqlx::query_as::<_, (String, i64)>(
        "SELECT asset_id, ordinal FROM reference_anchor_assets
         WHERE anchor_id = 'anc_dev051_legacy' ORDER BY ordinal ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("legacy anchor rows should load");

    let (_profiles, reference_sets) = services(&pool);
    let converted = reference_sets
        .create_from_anchor(
            PROJECT_ID,
            "anc_dev051_legacy",
            Some("Converted references".to_owned()),
        )
        .await
        .expect("explicit anchor conversion should succeed");
    assert_eq!(converted.name, "Converted references");
    let converted_items = SqliteReferenceSetRepository::new(pool.clone())
        .list_items(&converted.id)
        .await
        .expect("converted items should load");
    assert_eq!(
        converted_items
            .iter()
            .map(|item| (item.asset_id.as_str(), item.ordinal))
            .collect::<Vec<_>>(),
        vec![("ast_dev051_anchor_b", 0), ("ast_dev051_anchor_a", 1),]
    );

    let after = sqlx::query_as::<_, (String, i64)>(
        "SELECT asset_id, ordinal FROM reference_anchor_assets
         WHERE anchor_id = 'anc_dev051_legacy' ORDER BY ordinal ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("legacy anchor rows should remain");
    assert_eq!(after, before);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM reference_anchors WHERE id = 'anc_dev051_legacy'",
        )
        .fetch_one(&pool)
        .await
        .expect("legacy anchor should remain"),
        "Legacy Anchor"
    );
}

#[test]
fn command_contract_is_stable_camel_case_and_explicitly_wired_for_later_main_registration() {
    let command_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/commands/consistency_assets.rs"
    );
    let source = fs::read_to_string(command_path).expect("DEV-051 command source should exist");
    assert!(source.contains("#[serde(rename_all = \"camelCase\")]"));
    assert!(source.contains("#[tauri::command(rename_all = \"camelCase\")]"));

    for command in [
        "consistency_profile_list",
        "consistency_profile_get",
        "character_profile_create",
        "character_profile_update",
        "scene_profile_create",
        "scene_profile_update",
        "prop_profile_create",
        "prop_profile_update",
        "style_profile_create",
        "style_profile_update",
        "consistency_profile_delete",
        "costume_variant_list",
        "costume_variant_get",
        "costume_variant_create",
        "costume_variant_update",
        "costume_variant_delete",
        "reference_set_list",
        "reference_set_detail_get",
        "reference_set_create",
        "reference_set_update",
        "reference_set_delete",
        "reference_set_create_from_anchor",
        "asset_usage_get",
        "profile_usage_get",
        "reference_set_usage_get",
    ] {
        assert!(
            source.contains(&format!("pub async fn {command}")),
            "missing command {command}"
        );
    }
    for field in [
        "id",
        "project_id",
        "profile_type",
        "default_reference_set_id",
        "default_style_profile_id",
        "active_revision_id",
        "created_at",
        "updated_at",
    ] {
        assert!(
            source.contains(&format!("pub {field}:")),
            "profile view is missing {field}"
        );
    }
    assert!(source.contains("pub struct ConsistencyProfileView"));
    assert!(!source.contains("pub enum ConsistencyProfileView"));
    assert!(source.contains("reference_set_service\n        .create_from_anchor"));
}
