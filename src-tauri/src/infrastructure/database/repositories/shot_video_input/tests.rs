use super::*;
use crate::application::ports::{
    AssetDeletionRepository, AssetRepository, ProjectRecord, ProjectRepository, ShotRecord,
    ShotRepository,
};
use crate::domain::{Asset, AssetId, AssetType};
use crate::infrastructure::database::{
    initialize,
    repositories::{
        SqliteAssetDeletionRepository, SqliteAssetRepository, SqliteProjectRepository,
        SqliteShotRepository,
    },
};
use serde_json::json;
use tempfile::{tempdir, TempDir};

struct Fixture {
    dir: TempDir,
    pool: SqlitePool,
    repo: SqliteShotVideoInputRepository,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_schema(true).await
    }
    async fn with_schema(current: bool) -> Self {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inputs.db");
        let pool = if current {
            initialize(&path).await.unwrap()
        } else {
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                .foreign_keys(true);
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            let mut historical = sqlx::migrate::Migrator::new(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"),
            )
            .await
            .unwrap();
            historical.migrations = std::borrow::Cow::Owned(
                historical
                    .iter()
                    .filter(|m| m.version <= 42)
                    .cloned()
                    .collect(),
            );
            historical.run(&pool).await.unwrap();
            pool
        };
        let project_repo = SqliteProjectRepository::new(pool.clone());
        let shot_repo = SqliteShotRepository::new(pool.clone());
        let asset_repo = SqliteAssetRepository::new(pool.clone());
        for project in ["p1", "p2"] {
            let now = Utc::now();
            project_repo
                .insert(&ProjectRecord {
                    id: project.into(),
                    name: project.into(),
                    description: None,
                    root_path: dir.path().join(project),
                    created_at: now,
                    updated_at: now,
                })
                .await
                .unwrap();
            shot_repo
                .insert(&ShotRecord {
                    id: format!("shot_{project}"),
                    project_id: project.into(),
                    ordinal: 0,
                    name: "legacy standalone".into(),
                    prompt_text: "move".into(),
                    prompt_entry_id: None,
                    prompt_version_id: None,
                    selected_image_asset_id: None,
                    selected_video_asset_id: None,
                    created_at: now,
                    updated_at: now,
                })
                .await
                .unwrap();
            for (suffix, kind) in [
                ("a", AssetType::Image),
                ("b", AssetType::Image),
                ("v", AssetType::Video),
                ("u", AssetType::Audio),
            ] {
                let mut asset = Asset::new_source_image(
                    AssetId::parse(format!("ast_{project}_{suffix}")).unwrap(),
                    project,
                    "source",
                    "source.png",
                    dir.path().join(project).join(suffix).display().to_string(),
                    "a".repeat(64),
                    "image/png",
                    32,
                    32,
                    100,
                    json!({}),
                    now,
                )
                .unwrap();
                asset.asset_type = kind;
                asset.category = format!("source_{}", kind.as_str());
                if kind != AssetType::Image {
                    asset.duration_ms = Some(3000);
                    asset.mime_type = if kind == AssetType::Video {
                        "video/mp4"
                    } else {
                        "audio/wav"
                    }
                    .into();
                }
                if current {
                    asset_repo.insert_external_source(&asset).await.unwrap();
                } else {
                    asset_repo.insert_many(&[asset]).await.unwrap();
                }
            }
        }
        let repo = SqliteShotVideoInputRepository::new(pool.clone());
        Self { dir, pool, repo }
    }
    fn scope(&self) -> ShotVideoInputScope {
        ShotVideoInputScope {
            project_id: "p1".into(),
            shot_id: "shot_p1".into(),
            workflow_version_id: "version_a".into(),
            recipe_id: "recipe_a".into(),
        }
    }
}

fn input(key: &str, ordinal: i64, id: &str) -> ShotVideoInputAsset {
    ShotVideoInputAsset {
        input_key: key.into(),
        ordinal,
        asset_id: id.into(),
    }
}

#[tokio::test]
async fn explicit_slots_multimedia_order_and_restart_are_persistent() {
    let fixture = Fixture::new().await;
    let scope = fixture.scope();
    // Deliberately unordered payload: ordering is per formal key, not global.
    let inputs = vec![
        input("reference_images", 1, "ast_p1_a"),
        input("reference_videos", 0, "ast_p1_v"),
        input("first_frame", 0, "ast_p1_b"),
        input("last_frame", 0, "ast_p1_a"),
        input("reference_audios", 0, "ast_p1_u"),
        input("reference_images", 0, "ast_p1_b"),
    ];
    let saved = fixture
        .repo
        .replace(&scope, None, &inputs, Utc::now())
        .await
        .unwrap()
        .unwrap();
    let before = fixture.repo.find(&scope).await.unwrap().unwrap();
    assert_eq!(saved, before);
    assert_eq!(
        before
            .inputs
            .iter()
            .filter(|i| i.input_key == "reference_images")
            .map(|i| i.asset_id.as_str())
            .collect::<Vec<_>>(),
        vec!["ast_p1_b", "ast_p1_a"]
    );
    fixture.pool.close().await;
    let reopened = initialize(&fixture.dir.path().join("inputs.db"))
        .await
        .unwrap();
    let repo = SqliteShotVideoInputRepository::new(reopened.clone());
    assert_eq!(repo.find(&scope).await.unwrap().unwrap(), before);
    assert!(SqliteShotRepository::new(reopened.clone())
        .find("p1", "shot_p1")
        .await
        .unwrap()
        .is_some());
    reopened.close().await;
}

#[tokio::test]
async fn recipe_pair_isolation_and_clear_keep_occ_identity() {
    let fixture = Fixture::new().await;
    let scope = fixture.scope();
    let saved = fixture
        .repo
        .replace(
            &scope,
            None,
            &[input("first_frame", 0, "ast_p1_a")],
            Utc::now(),
        )
        .await
        .unwrap()
        .unwrap();
    let mut other = scope.clone();
    other.recipe_id = "recipe_b".into();
    assert!(fixture.repo.find(&other).await.unwrap().is_none());
    other = scope.clone();
    other.workflow_version_id = "version_b".into();
    assert!(fixture.repo.find(&other).await.unwrap().is_none());
    assert!(fixture
        .repo
        .replace(&scope, None, &[], Utc::now())
        .await
        .unwrap()
        .is_none());
    let cleared = fixture
        .repo
        .replace(&scope, Some(&saved.token), &[], Utc::now())
        .await
        .unwrap()
        .unwrap();
    assert!(cleared.inputs.is_empty());
    assert_eq!(cleared.token.instance_id, saved.token.instance_id);
    assert_eq!(cleared.token.revision, 2);
    assert!(fixture
        .repo
        .replace(&scope, Some(&saved.token), &[], Utc::now())
        .await
        .unwrap()
        .is_none());
    assert_eq!(fixture.repo.find(&scope).await.unwrap().unwrap(), cleared);
}

#[tokio::test]
async fn concurrent_writes_have_one_winner_and_no_lost_update() {
    let fixture = Fixture::new().await;
    let scope = fixture.scope();
    let saved = fixture
        .repo
        .replace(&scope, None, &[], Utc::now())
        .await
        .unwrap()
        .unwrap();
    let a = [input("first_frame", 0, "ast_p1_a")];
    let b = [input("first_frame", 0, "ast_p1_b")];
    let (a, b) = tokio::join!(
        fixture
            .repo
            .replace(&scope, Some(&saved.token), &a, Utc::now()),
        fixture
            .repo
            .replace(&scope, Some(&saved.token), &b, Utc::now())
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_ne!(a.is_some(), b.is_some());
    assert_eq!(
        fixture.repo.find(&scope).await.unwrap().unwrap(),
        a.or(b).unwrap()
    );
}

#[tokio::test]
async fn cross_project_scope_and_invalid_slot_replacement_roll_back() {
    let fixture = Fixture::new().await;
    let scope = fixture.scope();
    let saved = fixture
        .repo
        .replace(
            &scope,
            None,
            &[input("first_frame", 0, "ast_p1_a")],
            Utc::now(),
        )
        .await
        .unwrap()
        .unwrap();
    for inputs in [
        vec![input("first_frame", 0, "ast_p2_a")],
        vec![input("last_frame", 1, "ast_p1_b")],
        vec![input("reference_images", 2, "ast_p1_a")],
        vec![
            input("reference_images", 0, "ast_p1_a"),
            input("reference_images", 0, "ast_p1_b"),
        ],
        vec![input("unknown", 0, "ast_p1_b")],
        vec![input("first_frame", 0, "ast_missing")],
    ] {
        assert!(fixture
            .repo
            .replace(&scope, Some(&saved.token), &inputs, Utc::now())
            .await
            .is_err());
        assert_eq!(fixture.repo.find(&scope).await.unwrap().unwrap(), saved);
    }
    let mut wrong = scope.clone();
    wrong.project_id = "p2".into();
    assert!(fixture
        .repo
        .replace(&wrong, None, &[], Utc::now())
        .await
        .is_err());
    assert!(fixture.repo.find(&wrong).await.unwrap().is_none());
}

#[tokio::test]
async fn asset_deletion_is_inspected_and_blocked_but_project_cascade_is_safe() {
    let fixture = Fixture::new().await;
    let scope = fixture.scope();
    fixture
        .repo
        .replace(
            &scope,
            None,
            &[input("first_frame", 0, "ast_p1_a")],
            Utc::now(),
        )
        .await
        .unwrap()
        .unwrap();
    let asset = AssetId::parse("ast_p1_a").unwrap();
    let refs = SqliteAssetDeletionRepository::new(fixture.pool.clone())
        .references_for("p1", &[asset.clone()])
        .await
        .unwrap();
    assert_eq!(refs[0].shot_video_input_ids, ["shot_p1"]);
    assert!(SqliteAssetRepository::new(fixture.pool.clone())
        .delete_by_ids("p1", &[asset.clone()])
        .await
        .is_err());
    assert!(SqliteAssetRepository::new(fixture.pool.clone())
        .find_by_id(&asset)
        .await
        .unwrap()
        .is_some());
    // Existing Asset -> Project FK is non-cascading. An explicit whole-project
    // cleanup transaction removes assets then cascades Shots/input headers.
    // Deferred input FKs protect individual deletes without forbidding this flow.
    let mut cleanup = fixture.pool.begin().await.unwrap();
    sqlx::query("DELETE FROM assets WHERE project_id = 'p1'")
        .execute(&mut *cleanup)
        .await
        .unwrap();
    sqlx::query("DELETE FROM projects WHERE id = 'p1'")
        .execute(&mut *cleanup)
        .await
        .unwrap();
    cleanup.commit().await.unwrap();
    assert!(fixture.repo.find(&scope).await.unwrap().is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM external_asset_imports WHERE project_id = 'p1'"
        )
        .fetch_one(&fixture.pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets WHERE project_id = 'p2'")
            .fetch_one(&fixture.pool)
            .await
            .unwrap(),
        4
    );
}

#[tokio::test]
async fn recreate_shot_cannot_reuse_an_old_input_token() {
    let fixture = Fixture::new().await;
    let scope = fixture.scope();
    let saved = fixture
        .repo
        .replace(&scope, None, &[], Utc::now())
        .await
        .unwrap()
        .unwrap();
    let shots = SqliteShotRepository::new(fixture.pool.clone());
    let original = shots.find("p1", "shot_p1").await.unwrap().unwrap().shot;
    shots.delete("p1", "shot_p1").await.unwrap();
    shots.insert(&original).await.unwrap();
    let recreated = fixture
        .repo
        .replace(&scope, None, &[], Utc::now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.token.revision, recreated.token.revision);
    assert_ne!(saved.token.instance_id, recreated.token.instance_id);
    assert!(fixture
        .repo
        .replace(
            &scope,
            Some(&saved.token),
            &[input("first_frame", 0, "ast_p1_a")],
            Utc::now()
        )
        .await
        .unwrap()
        .is_none());
    assert_eq!(fixture.repo.find(&scope).await.unwrap().unwrap(), recreated);
}

#[tokio::test]
async fn generic_asset_insert_does_not_forge_external_provenance() {
    let fixture = Fixture::new().await;
    let assets = SqliteAssetRepository::new(fixture.pool.clone());
    let mut asset = assets
        .find_by_id(&AssetId::parse("ast_p1_a").unwrap())
        .await
        .unwrap()
        .unwrap();
    let receipt = assets
        .find_external_import("p1", &asset.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.sha256, asset.sha256);
    assert_eq!(receipt.media_type, "image");
    assert!(assets
        .find_external_import("p2", &asset.id)
        .await
        .unwrap()
        .is_none());
    asset.id = AssetId::new();
    asset.metadata_json = json!({"source": "native_import"});
    assets.insert_many(&[asset.clone()]).await.unwrap();
    assert!(assets
        .find_external_import("p1", &asset.id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks")
            .fetch_one(&fixture.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches")
            .fetch_one(&fixture.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn migration_043_upgrades_v42_without_inventing_inputs_or_receipts() {
    let fixture = Fixture::with_schema(false).await;
    let scope = fixture.scope();
    // A true 001..042 schema: never downgrade live tables or migration markers.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&fixture.pool)
            .await
            .unwrap(),
        42
    );
    sqlx::query("UPDATE shots SET selected_image_asset_id = 'ast_p1_a' WHERE id = 'shot_p1'")
        .execute(&fixture.pool)
        .await
        .unwrap();
    fixture.pool.close().await;
    let upgraded = initialize(&fixture.dir.path().join("inputs.db"))
        .await
        .unwrap();
    let shots = SqliteShotRepository::new(upgraded.clone());
    assert_eq!(
        shots
            .find("p1", "shot_p1")
            .await
            .unwrap()
            .unwrap()
            .shot
            .selected_image_asset_id
            .as_deref(),
        Some("ast_p1_a")
    );
    assert!(SqliteShotVideoInputRepository::new(upgraded.clone())
        .find(&scope)
        .await
        .unwrap()
        .is_none());
    assert!(SqliteAssetRepository::new(upgraded.clone())
        .find_external_import("p1", &AssetId::parse("ast_p1_a").unwrap())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&upgraded)
            .await
            .unwrap(),
        43
    );
    upgraded.close().await;
}
