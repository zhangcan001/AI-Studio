use super::{format_datetime, map_sqlx_error};
use crate::application::ports::{
    RepositoryError, WorkflowLibraryRepository, WorkflowPackageRecord, WorkflowPackageRegistration,
    RUNTIME_ARTIFACT_CONFLICT,
};
use async_trait::async_trait;
use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct SqliteWorkflowLibraryRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowLibraryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkflowLibraryRepository for SqliteWorkflowLibraryRepository {
    async fn register_package(
        &self,
        package: &WorkflowPackageRecord,
    ) -> Result<WorkflowPackageRegistration, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let registration = register_package(&mut transaction, package).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(registration)
    }
}

async fn register_package(
    transaction: &mut Transaction<'_, Sqlite>,
    package: &WorkflowPackageRecord,
) -> Result<WorkflowPackageRegistration, RepositoryError> {
    sqlx::query(
        "INSERT INTO workflows (
            id, name, category, mode, current_version_id, created_at, updated_at,
            source_kind, library_state, removed_at
        ) VALUES (?, ?, ?, ?, NULL, ?, ?, ?, 'ACTIVE', NULL)
        ON CONFLICT(id) DO NOTHING",
    )
    .bind(&package.workflow_id)
    .bind(&package.name)
    .bind(&package.category)
    .bind(&package.mode)
    .bind(format_datetime(package.created_at))
    .bind(format_datetime(package.created_at))
    .bind(&package.source_kind)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    if package.source_kind == "PRODUCT" {
        sqlx::query(
            "UPDATE workflows SET source_kind = 'PRODUCT'
             WHERE id = ? AND source_kind <> 'PRODUCT'",
        )
        .bind(&package.workflow_id)
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }

    let existing_version = sqlx::query_as::<_, WorkflowVersionRow>(
        "SELECT id, workflow_sha256
         FROM workflow_versions
         WHERE workflow_id = ? AND version = ?",
    )
    .bind(&package.workflow_id)
    .bind(&package.workflow_version)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    let (workflow_version_id, inserted_workflow_version) = match existing_version {
        Some(existing) if existing.workflow_sha256 == package.workflow_sha256 => {
            sqlx::query(
                "UPDATE workflow_versions
                 SET source_workflow_json = COALESCE(source_workflow_json, ?),
                     recognition_metadata_json = COALESCE(recognition_metadata_json, ?)
                 WHERE id = ?",
            )
            .bind(&package.source_workflow_json)
            .bind(&package.recognition_metadata_json)
            .bind(&existing.id)
            .execute(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
            (existing.id, false)
        }
        Some(_) => {
            return Err(RepositoryError::workflow_version_conflict(
                &package.workflow_id,
                &package.workflow_version,
            ));
        }
        None => {
            let id = format!("wfv_{}", Uuid::new_v4());
            sqlx::query(
                "INSERT INTO workflow_versions (
                    id, workflow_id, version, api_workflow_json, workflow_sha256,
                    package_name, package_source_path, created_at,
                    source_workflow_json, recognition_metadata_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(&package.workflow_id)
            .bind(&package.workflow_version)
            .bind(package.workflow_json.to_string())
            .bind(&package.workflow_sha256)
            .bind(&package.package_name)
            .bind(&package.package_source_path)
            .bind(format_datetime(package.created_at))
            .bind(&package.source_workflow_json)
            .bind(&package.recognition_metadata_json)
            .execute(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
            (id, true)
        }
    };

    let existing_recipe = sqlx::query_as::<_, RecipeVersionRow>(
        "SELECT id, recipe_sha256
         FROM recipes
         WHERE workflow_version_id = ? AND version = ?",
    )
    .bind(&workflow_version_id)
    .bind(&package.recipe_version)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    let (registration, recipe_id) = match existing_recipe {
        Some(existing) if existing.recipe_sha256 == package.recipe_sha256 => {
            if inserted_workflow_version {
                (WorkflowPackageRegistration::Inserted, existing.id)
            } else {
                (WorkflowPackageRegistration::Reused, existing.id)
            }
        }
        Some(_) => {
            return Err(RepositoryError::recipe_version_conflict(
                &workflow_version_id,
                &package.recipe_version,
            ))
        }
        None => {
            let recipe_id = format!("rcp_{}", Uuid::new_v4());
            sqlx::query(
                "INSERT INTO recipes (
                    id, workflow_version_id, version, schema_version,
                    recipe_yaml, recipe_sha256, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&recipe_id)
            .bind(&workflow_version_id)
            .bind(&package.recipe_version)
            .bind(i64::from(package.recipe_schema_version))
            .bind(&package.recipe_yaml)
            .bind(&package.recipe_sha256)
            .bind(format_datetime(package.created_at))
            .execute(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
            (WorkflowPackageRegistration::Inserted, recipe_id)
        }
    };

    let supersession = crate::application::builtin_runtime_packages::h3_base_supersession(package);
    let (old_package, old_workflow_sha, old_recipe_sha) =
        supersession.unwrap_or(("", String::new(), String::new()));
    sqlx::query(
        "UPDATE workflows SET current_version_id = ?, updated_at = ?
         WHERE id = ? AND (current_version_id IS NULL OR (
           source_kind = 'PRODUCT' AND library_state = 'ACTIVE'
           AND EXISTS (SELECT 1 FROM workflow_runtime_artifacts a
             WHERE a.workflow_version_id = workflows.current_version_id
               AND a.package_name = ? AND a.source_kind = 'PRODUCT'
               AND a.workflow_sha256 = ? AND a.recipe_sha256 = ?)
           AND NOT EXISTS (SELECT 1 FROM workflow_runtime_states s
             WHERE s.workflow_version_id = workflows.current_version_id
               AND (s.enabled = 0 OR s.archived = 1))
           AND NOT EXISTS (SELECT 1 FROM workflow_recipe_promotions p
             WHERE p.workflow_version_id = workflows.current_version_id)))",
    )
    .bind(&workflow_version_id)
    .bind(format_datetime(package.created_at))
    .bind(&package.workflow_id)
    .bind(old_package)
    .bind(old_workflow_sha)
    .bind(old_recipe_sha)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    register_runtime_artifact(transaction, package, &workflow_version_id, &recipe_id).await?;
    if package.source_kind == "PRODUCT" {
        migrate_stale_bindings(transaction, &package.workflow_id, package.created_at).await?;
    }
    Ok(registration)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StaleBindingMigration {
    pub project_bindings: u64,
    pub shot_stage_configs: u64,
    pub shot_video_input_sets: u64,
    pub project_templates: u64,
}

/// Editable selections (project bindings, shot stage configs, project templates)
/// that still name a non-current version of this PRODUCT workflow follow it to
/// the current version and its single recipe, in the registering transaction.
/// Idempotent; old versions/recipes, shot input sets of the old pair, frozen
/// batches and run/task history are never touched. Other workflows never move.
pub(crate) async fn migrate_stale_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_id: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<StaleBindingMigration, RepositoryError> {
    let mut report = StaleBindingMigration::default();
    let current = sqlx::query_scalar::<_, String>(
        "SELECT w.current_version_id FROM workflows w
         WHERE w.id = ? AND w.source_kind = 'PRODUCT' AND w.library_state = 'ACTIVE'
           AND w.current_version_id IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM workflow_runtime_states s
             WHERE s.workflow_version_id = w.current_version_id
               AND (s.enabled = 0 OR s.archived = 1))",
    )
    .bind(workflow_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    let Some(current) = current else {
        return Ok(report);
    };
    let recipes = sqlx::query_scalar::<_, String>(
        "SELECT id FROM recipes WHERE workflow_version_id = ? ORDER BY id",
    )
    .bind(&current)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    let [recipe] = recipes.as_slice() else {
        return Ok(report);
    };
    let stamp = format_datetime(now);
    const STALE: &str = "workflow_version_id IN (SELECT id FROM workflow_versions
        WHERE workflow_id = ? AND id <> ?)";

    report.project_bindings = sqlx::query(&format!(
        "UPDATE project_workflow_bindings
         SET workflow_version_id = ?, recipe_id = ?, revision = revision + 1, updated_at = ?
         WHERE {STALE}"
    ))
    .bind(&current)
    .bind(recipe)
    .bind(&stamp)
    .bind(workflow_id)
    .bind(&current)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .rows_affected();

    let stale_configs = sqlx::query_as::<_, (String, String, String, String)>(&format!(
        "SELECT c.shot_id, s.project_id, c.workflow_version_id, c.recipe_id
         FROM shot_stage_configs c INNER JOIN shots s ON s.id = c.shot_id
         WHERE c.{STALE}"
    ))
    .bind(workflow_id)
    .bind(&current)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    for (shot_id, project_id, old_version, old_recipe) in &stale_configs {
        // The shot keeps its chosen reference media: a copy of the old pair's
        // input set is created for the new pair unless one already exists.
        let copied = sqlx::query(
            "INSERT INTO shot_video_input_sets
             (project_id, shot_id, workflow_version_id, recipe_id, instance_id, revision, updated_at)
             SELECT project_id, shot_id, ?, ?, ?, revision, ?
             FROM shot_video_input_sets
             WHERE project_id = ? AND shot_id = ? AND workflow_version_id = ? AND recipe_id = ?
             ON CONFLICT(project_id, shot_id, workflow_version_id, recipe_id) DO NOTHING",
        )
        .bind(&current)
        .bind(recipe)
        .bind(format!("vin_{}", Uuid::new_v4()))
        .bind(&stamp)
        .bind(project_id)
        .bind(shot_id)
        .bind(old_version)
        .bind(old_recipe)
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?
        .rows_affected();
        if copied > 0 {
            sqlx::query(
                "INSERT INTO shot_video_input_assets
                 (project_id, shot_id, workflow_version_id, recipe_id, input_key, ordinal, asset_id)
                 SELECT project_id, shot_id, ?, ?, input_key, ordinal, asset_id
                 FROM shot_video_input_assets
                 WHERE project_id = ? AND shot_id = ? AND workflow_version_id = ? AND recipe_id = ?",
            )
            .bind(&current)
            .bind(recipe)
            .bind(project_id)
            .bind(shot_id)
            .bind(old_version)
            .bind(old_recipe)
            .execute(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
            report.shot_video_input_sets += copied;
        }
        report.shot_stage_configs += sqlx::query(
            "UPDATE shot_stage_configs SET workflow_version_id = ?, recipe_id = ?, updated_at = ?
             WHERE shot_id = ? AND workflow_version_id = ? AND recipe_id = ?",
        )
        .bind(&current)
        .bind(recipe)
        .bind(&stamp)
        .bind(shot_id)
        .bind(old_version)
        .bind(old_recipe)
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?
        .rows_affected();
    }

    report.project_templates = sqlx::query(&format!(
        "UPDATE project_templates SET workflow_version_id = ?, recipe_id = ?, updated_at = ?
         WHERE {STALE}"
    ))
    .bind(&current)
    .bind(recipe)
    .bind(&stamp)
    .bind(workflow_id)
    .bind(&current)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .rows_affected();

    if report != StaleBindingMigration::default() {
        tracing::info!(
            workflow_id,
            current_version_id = %current,
            recipe_id = %recipe,
            project_bindings = report.project_bindings,
            shot_stage_configs = report.shot_stage_configs,
            shot_video_input_sets = report.shot_video_input_sets,
            project_templates = report.project_templates,
            "workflow library: migrated stale selections to the current workflow version"
        );
    }
    Ok(report)
}

async fn register_runtime_artifact(
    transaction: &mut Transaction<'_, Sqlite>,
    package: &WorkflowPackageRecord,
    workflow_version_id: &str,
    recipe_id: &str,
) -> Result<(), RepositoryError> {
    let existing_for_pair = sqlx::query_as::<_, RuntimeArtifactRow>(
        "SELECT workflow_version_id, recipe_id, workflow_sha256, recipe_sha256
         FROM workflow_runtime_artifacts
         WHERE workflow_version_id = ? AND recipe_id = ?",
    )
    .bind(workflow_version_id)
    .bind(recipe_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if let Some(existing) = existing_for_pair {
        if existing.workflow_sha256 == package.workflow_sha256
            && existing.recipe_sha256 == package.recipe_sha256
        {
            // The first Registry registration is the canonical package for
            // this exact pair. A duplicate package with identical bytes is
            // harmless and must not replace it by insertion order.
            return Ok(());
        }
        return Err(RepositoryError::integrity(format!(
            "{RUNTIME_ARTIFACT_CONFLICT}: exact workflow version/recipe is registered with different bytes"
        )));
    }

    let existing = sqlx::query_as::<_, RuntimeArtifactRow>(
        "SELECT workflow_version_id, recipe_id, workflow_sha256, recipe_sha256
         FROM workflow_runtime_artifacts WHERE package_name = ?",
    )
    .bind(&package.package_name)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if let Some(existing) = existing {
        if existing.workflow_version_id != workflow_version_id
            || existing.recipe_id != recipe_id
            || existing.workflow_sha256 != package.workflow_sha256
            || existing.recipe_sha256 != package.recipe_sha256
        {
            return Err(RepositoryError::integrity(
                "runtime package is already mapped to a different immutable workflow/recipe",
            ));
        }
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO workflow_runtime_artifacts (
            id, workflow_version_id, recipe_id, package_name, source_kind,
            package_source_path, workflow_sha256, recipe_sha256, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(format!("wra_{}", Uuid::new_v4()))
    .bind(workflow_version_id)
    .bind(recipe_id)
    .bind(&package.package_name)
    .bind(&package.source_kind)
    .bind(&package.package_source_path)
    .bind(&package.workflow_sha256)
    .bind(&package.recipe_sha256)
    .bind(format_datetime(package.created_at))
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct WorkflowVersionRow {
    id: String,
    workflow_sha256: String,
}

#[derive(sqlx::FromRow)]
struct RecipeVersionRow {
    id: String,
    recipe_sha256: String,
}

#[derive(sqlx::FromRow)]
struct RuntimeArtifactRow {
    workflow_version_id: String,
    recipe_id: String,
    workflow_sha256: String,
    recipe_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowLibraryRepository;
    use crate::application::ports::{
        WorkflowLibraryRepository, WorkflowPackageRecord, WorkflowPackageRegistration,
    };
    use crate::infrastructure::database::initialize;
    use chrono::Utc;
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, SqliteWorkflowLibraryRepository) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        (
            directory,
            pool.clone(),
            SqliteWorkflowLibraryRepository::new(pool),
        )
    }

    fn package(workflow_hash: &str, recipe_hash: &str) -> WorkflowPackageRecord {
        WorkflowPackageRecord {
            workflow_id: "wfl_test".to_owned(),
            source_kind: "USER".to_owned(),
            package_name: "test-package".to_owned(),
            package_source_path: None,
            name: "Test Workflow".to_owned(),
            category: "image".to_owned(),
            mode: "text_to_image".to_owned(),
            workflow_version: "1.0.0".to_owned(),
            workflow_json: json!({"3": {"inputs": {}, "class_type": "KSampler"}}),
            workflow_sha256: workflow_hash.to_owned(),
            source_workflow_json: None,
            recognition_metadata_json: None,
            recipe_version: "1.0.0".to_owned(),
            recipe_schema_version: 1,
            recipe_yaml: "schema_version: 1".to_owned(),
            recipe_sha256: recipe_hash.to_owned(),
            created_at: Utc::now(),
        }
    }

    async fn exec(pool: &SqlitePool, sql: &str) {
        sqlx::query(sql).execute(pool).await.expect(sql);
    }

    async fn pair(pool: &SqlitePool, sql: &str) -> (String, String) {
        sqlx::query_as::<_, (String, String)>(sql)
            .fetch_one(pool)
            .await
            .expect(sql)
    }

    /// PRODUCT workflow `wfl_a` with v1/v2/v3 (v3 current) plus an unrelated
    /// workflow `wfl_b`, one shot per selection and a frozen batch on v1.
    async fn stale_fixture(pool: &SqlitePool) {
        let t = "2026-10-01T00:00:00Z";
        exec(pool, &format!("INSERT OR IGNORE INTO projects (id, name, root_path, created_at, updated_at) VALUES ('prj_default', 'default', 'C:/default', '{t}', '{t}')")).await;
        for (w, kind, current) in [("wfl_a", "PRODUCT", "wv_a3"), ("wfl_b", "PRODUCT", "wv_b1")] {
            exec(pool, &format!("INSERT INTO workflows (id, name, category, mode, current_version_id, source_kind, created_at, updated_at) VALUES ('{w}', '{w}', 'video', 'video', '{current}', '{kind}', '{t}', '{t}')")).await;
        }
        for (v, w) in [
            ("wv_a1", "wfl_a"),
            ("wv_a2", "wfl_a"),
            ("wv_a3", "wfl_a"),
            ("wv_b1", "wfl_b"),
        ] {
            exec(pool, &format!("INSERT INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at) VALUES ('{v}', '{w}', '{v}', '{{}}', 'sha', '{t}')")).await;
            exec(pool, &format!("INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at) VALUES ('r_{v}', '{v}', '1', 1, 'x', 'sha', '{t}')")).await;
        }
        for (shot, v) in [
            ("sh_1", "wv_a1"),
            ("sh_2", "wv_a2"),
            ("sh_3", "wv_a3"),
            ("sh_b", "wv_b1"),
        ] {
            exec(pool, &format!("INSERT INTO shots (id, project_id, ordinal, name, prompt_text, created_at, updated_at) VALUES ('{shot}', 'prj_default', {}, '{shot}', '', '{t}', '{t}')", &shot[3..].len() + shot.as_bytes()[3] as usize)).await;
            exec(pool, &format!("INSERT INTO shot_stage_configs (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at) VALUES ('{shot}', 'video', '{v}', 'r_{v}', '{{\"seed\":1}}', '{t}')")).await;
        }
        exec(pool, &format!("INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at) VALUES ('ast_1', 'prj_default', 'image', 'source_image', 'a', 'a', 'assets/a.png', 'sha', 'image/png', 1, 1, 1, '{{}}', '{t}', '{t}')")).await;
        exec(pool, &format!("INSERT INTO shot_video_input_sets (project_id, shot_id, workflow_version_id, recipe_id, instance_id, revision, updated_at) VALUES ('prj_default', 'sh_2', 'wv_a2', 'r_wv_a2', 'vin_old', 4, '{t}')")).await;
        exec(pool, "INSERT INTO shot_video_input_assets (project_id, shot_id, workflow_version_id, recipe_id, input_key, ordinal, asset_id) VALUES ('prj_default', 'sh_2', 'wv_a2', 'r_wv_a2', 'reference_images', 0, 'ast_1')").await;
        exec(pool, &format!("INSERT INTO project_workflow_bindings (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at, revision) VALUES ('prj_default', 'VIDEO', 'DEFAULT', 'wv_a1', 'r_wv_a1', '{t}', '{t}', 3), ('prj_default', 'VIDEO', 'REF2VA_IMAGE', 'wv_a2', 'r_wv_a2', '{t}', '{t}', 1), ('prj_default', 'VIDEO', 'REF2VA_AUDIO', 'wv_b1', 'r_wv_b1', '{t}', '{t}', 1)")).await;
        exec(pool, &format!("INSERT INTO production_batches (id, project_id, name, status, continue_on_failure, created_at, updated_at) VALUES ('pbt_frozen', 'prj_default', 'frozen', 'READY', 0, '{t}', '{t}')")).await;
        exec(pool, &format!("INSERT INTO production_batch_items (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, created_at, updated_at) VALUES ('pbi_frozen', 'pbt_frozen', 0, 'wv_a1', 'r_wv_a1', '{{}}', 'PENDING', '{t}', '{t}')")).await;
    }

    async fn migrate(pool: &SqlitePool, workflow: &str) -> super::StaleBindingMigration {
        let mut tx = pool.begin().await.unwrap();
        let report = super::migrate_stale_bindings(&mut tx, workflow, Utc::now())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        report
    }

    #[tokio::test]
    async fn stale_product_selections_follow_the_current_version_idempotently() {
        let (_directory, pool, _repository) = setup().await;
        stale_fixture(&pool).await;
        let first = migrate(&pool, "wfl_a").await;
        assert_eq!(
            first,
            super::StaleBindingMigration {
                project_bindings: 2,
                shot_stage_configs: 2,
                shot_video_input_sets: 1,
                project_templates: 0,
            }
        );
        for mode in ["DEFAULT", "REF2VA_IMAGE"] {
            assert_eq!(
                pair(&pool, &format!("SELECT workflow_version_id, recipe_id FROM project_workflow_bindings WHERE mode = '{mode}'")).await,
                ("wv_a3".to_owned(), "r_wv_a3".to_owned())
            );
        }
        let revisions: Vec<(String, i64)> =
            sqlx::query_as("SELECT mode, revision FROM project_workflow_bindings ORDER BY mode")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            revisions,
            vec![
                ("DEFAULT".to_owned(), 4),
                ("REF2VA_AUDIO".to_owned(), 1),
                ("REF2VA_IMAGE".to_owned(), 2)
            ]
        );
        for shot in ["sh_1", "sh_2", "sh_3"] {
            assert_eq!(
                pair(&pool, &format!("SELECT workflow_version_id, recipe_id FROM shot_stage_configs WHERE shot_id = '{shot}'")).await,
                ("wv_a3".to_owned(), "r_wv_a3".to_owned())
            );
        }
        // Reference media travel with the shot; the old pair's set is kept.
        let sets: Vec<(String, i64)> = sqlx::query_as("SELECT workflow_version_id, revision FROM shot_video_input_sets WHERE shot_id = 'sh_2' ORDER BY workflow_version_id")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(sets, vec![("wv_a2".to_owned(), 4), ("wv_a3".to_owned(), 4)]);
        assert_eq!(
            pair(&pool, "SELECT workflow_version_id, asset_id FROM shot_video_input_assets WHERE workflow_version_id = 'wv_a3'").await,
            ("wv_a3".to_owned(), "ast_1".to_owned())
        );
        // Cross-workflow selections and frozen batch items never move.
        assert_eq!(
            pair(&pool, "SELECT workflow_version_id, recipe_id FROM project_workflow_bindings WHERE mode = 'REF2VA_AUDIO'").await,
            ("wv_b1".to_owned(), "r_wv_b1".to_owned())
        );
        assert_eq!(
            pair(&pool, "SELECT workflow_version_id, recipe_id FROM shot_stage_configs WHERE shot_id = 'sh_b'").await,
            ("wv_b1".to_owned(), "r_wv_b1".to_owned())
        );
        assert_eq!(
            pair(&pool, "SELECT workflow_version_id, recipe_id FROM production_batch_items WHERE id = 'pbi_frozen'").await,
            ("wv_a1".to_owned(), "r_wv_a1".to_owned())
        );
        // Old versions and recipes remain.
        let kept: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM recipes WHERE workflow_version_id IN ('wv_a1', 'wv_a2')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(kept, 2);
        // Idempotent: a second pass changes nothing, including revisions.
        assert_eq!(
            migrate(&pool, "wfl_a").await,
            super::StaleBindingMigration::default()
        );
        let again: i64 = sqlx::query_scalar(
            "SELECT revision FROM project_workflow_bindings WHERE mode = 'DEFAULT'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(again, 4);
        assert_eq!(
            migrate(&pool, "wfl_b").await,
            super::StaleBindingMigration::default()
        );
    }

    #[tokio::test]
    async fn user_and_disabled_workflows_keep_their_selected_versions() {
        let (_directory, pool, _repository) = setup().await;
        stale_fixture(&pool).await;
        exec(
            &pool,
            "UPDATE workflows SET source_kind = 'USER' WHERE id = 'wfl_a'",
        )
        .await;
        assert_eq!(
            migrate(&pool, "wfl_a").await,
            super::StaleBindingMigration::default()
        );
        exec(
            &pool,
            "UPDATE workflows SET source_kind = 'PRODUCT' WHERE id = 'wfl_a'",
        )
        .await;
        exec(&pool, "INSERT INTO workflow_runtime_states (workflow_version_id, enabled, archived, updated_at) VALUES ('wv_a3', 0, 0, '2026-10-01T00:00:00Z')").await;
        assert_eq!(
            migrate(&pool, "wfl_a").await,
            super::StaleBindingMigration::default()
        );
        assert_eq!(
            pair(&pool, "SELECT workflow_version_id, recipe_id FROM shot_stage_configs WHERE shot_id = 'sh_1'").await,
            ("wv_a1".to_owned(), "r_wv_a1".to_owned())
        );
    }

    #[tokio::test]
    async fn import_pins_to_superseded_product_versions_resolve_to_current() {
        use crate::application::ports::{GenerationDefinitionRepository, SupersededGenerationPair};
        use crate::infrastructure::database::repositories::SqliteGenerationDefinitionRepository;
        let (_directory, pool, _repository) = setup().await;
        stale_fixture(&pool).await;
        let definitions = SqliteGenerationDefinitionRepository::new(pool.clone());
        let current = Some(SupersededGenerationPair {
            workflow_version_id: "wv_a3".into(),
            recipe_id: "r_wv_a3".into(),
            workflow_name: "wfl_a".into(),
            current_version: "wv_a3".into(),
        });
        for (v, r) in [
            ("wv_a1", "r_wv_a1"),
            ("wv_a2", "r_wv_a2"),
            ("wv_a1", "r_wv_a2"),
            ("wv_a3", "r_wv_a1"),
        ] {
            assert_eq!(
                definitions.resolve_superseded_pair(v, r).await.unwrap(),
                current,
                "{v}/{r}"
            );
        }
        // Current, unknown and cross-workflow pairs are left to the normal checks.
        for (v, r) in [
            ("wv_a3", "r_wv_a3"),
            ("wv_missing", "r_wv_a1"),
            ("wv_a1", "r_missing"),
            ("wv_a1", "r_wv_b1"),
            ("wv_b1", "r_wv_a1"),
        ] {
            assert_eq!(
                definitions.resolve_superseded_pair(v, r).await.unwrap(),
                None,
                "{v}/{r}"
            );
        }
        // Ambiguous recipes, USER, disabled or archived current versions never resolve.
        exec(&pool, "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at) VALUES ('r_extra', 'wv_a3', '2', 1, 'x', 'sha', '2026-10-01T00:00:00Z')").await;
        assert_eq!(
            definitions
                .resolve_superseded_pair("wv_a1", "r_wv_a1")
                .await
                .unwrap(),
            None
        );
        exec(
            &pool,
            "UPDATE recipes SET workflow_version_id = 'wv_b1' WHERE id = 'r_extra'",
        )
        .await;
        exec(
            &pool,
            "UPDATE workflows SET source_kind = 'USER' WHERE id = 'wfl_a'",
        )
        .await;
        assert_eq!(
            definitions
                .resolve_superseded_pair("wv_a1", "r_wv_a1")
                .await
                .unwrap(),
            None
        );
        exec(
            &pool,
            "UPDATE workflows SET source_kind = 'PRODUCT' WHERE id = 'wfl_a'",
        )
        .await;
        exec(&pool, "INSERT INTO workflow_runtime_states (workflow_version_id, enabled, archived, updated_at) VALUES ('wv_a3', 1, 1, '2026-10-01T00:00:00Z')").await;
        assert_eq!(
            definitions
                .resolve_superseded_pair("wv_a1", "r_wv_a1")
                .await
                .unwrap(),
            None
        );
        exec(&pool, "UPDATE workflow_runtime_states SET enabled = 0, archived = 0 WHERE workflow_version_id = 'wv_a3'").await;
        assert_eq!(
            definitions
                .resolve_superseded_pair("wv_a1", "r_wv_a1")
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn registers_then_reuses_same_package() {
        let (_directory, pool, repository) = setup().await;
        let package = package("workflow-hash", "recipe-hash");
        assert_eq!(
            repository.register_package(&package).await.unwrap(),
            WorkflowPackageRegistration::Inserted
        );
        assert_eq!(
            repository.register_package(&package).await.unwrap(),
            WorkflowPackageRegistration::Reused
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn persists_original_payload_and_recognition_metadata_with_version() {
        let (_directory, pool, repository) = setup().await;
        let mut package = package("workflow-hash", "recipe-hash");
        package.source_workflow_json = Some(r#"{"ui":{"nodes":[]}}"#.to_owned());
        package.recognition_metadata_json = Some(
            r#"{"recognitionEngine":"WORKFLOW_RECOGNITION_V3","recognitionEngineVersion":"3"}"#
                .to_owned(),
        );

        repository.register_package(&package).await.unwrap();
        let persisted = sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT source_workflow_json, recognition_metadata_json FROM workflow_versions WHERE workflow_id = ?",
        )
        .bind(&package.workflow_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            persisted.0.as_deref(),
            package.source_workflow_json.as_deref()
        );
        assert_eq!(
            persisted.1.as_deref(),
            package.recognition_metadata_json.as_deref()
        );

        let mut duplicate = package.clone();
        duplicate.source_workflow_json = Some(r#"{"ui":{"nodes":[1]}}"#.to_owned());
        repository.register_package(&duplicate).await.unwrap();
        let unchanged = sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT source_workflow_json, recognition_metadata_json FROM workflow_versions WHERE workflow_id = ?",
        )
        .bind(&package.workflow_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            unchanged, persisted,
            "duplicate package must not overwrite immutable provenance"
        );
    }

    #[tokio::test]
    async fn rejects_same_workflow_version_with_different_hash() {
        let (_directory, _pool, repository) = setup().await;
        repository
            .register_package(&package("workflow-hash", "recipe-hash"))
            .await
            .unwrap();
        let error = repository
            .register_package(&package("different-workflow", "recipe-hash"))
            .await
            .expect_err("version conflict should be reported");
        assert!(error.to_string().contains("WORKFLOW_VERSION_CONFLICT"));
    }

    #[tokio::test]
    async fn keeps_first_version_current_when_packages_arrive_out_of_order() {
        let (_directory, pool, repository) = setup().await;
        let mut v110 = package("workflow-1.10.0", "recipe-1.10.0");
        v110.workflow_version = "1.10.0".to_owned();
        let mut v190 = package("workflow-1.9.0", "recipe-1.9.0");
        v190.workflow_version = "1.9.0".to_owned();
        v190.package_name = "test-package-1-9".to_owned();
        v110.package_name = "test-package-1-10".to_owned();

        repository.register_package(&v190).await.unwrap();
        repository.register_package(&v110).await.unwrap();

        let current = sqlx::query_scalar::<_, String>(
            "SELECT current_version_id FROM workflows WHERE id = ?",
        )
        .bind("wfl_test")
        .fetch_one(&pool)
        .await
        .unwrap();
        let current_version =
            sqlx::query_scalar::<_, String>("SELECT version FROM workflow_versions WHERE id = ?")
                .bind(current)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(current_version, "1.9.0");

        // Registering a later version must never silently replace the user's
        // default. The first package was 1.9.0, so it remains current even
        // though 1.10.0 arrived afterwards.
    }

    #[tokio::test]
    async fn rejects_same_recipe_version_with_different_hash() {
        let (_directory, _pool, repository) = setup().await;
        repository
            .register_package(&package("workflow-hash", "recipe-hash"))
            .await
            .unwrap();
        let mut changed = package("workflow-hash", "different-recipe");
        changed.recipe_yaml = "schema_version: 1\nname: changed".to_owned();
        let error = repository
            .register_package(&changed)
            .await
            .expect_err("recipe conflict should be reported");
        assert!(error.to_string().contains("RECIPE_VERSION_CONFLICT"));
    }
}
