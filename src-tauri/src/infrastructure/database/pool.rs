use crate::error::AppError;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{path::Path, time::Duration};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn initialize(database_path: &Path) -> Result<SqlitePool, AppError> {
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|_| AppError::database("failed to connect to SQLite"))?;

    configure_pragmas(&pool).await?;
    tracing::info!("database connected");

    MIGRATOR.run(&pool).await.map_err(|error| {
        tracing::error!(?error, "database migration failed");
        AppError::database("database migration failed")
    })?;
    tracing::info!("database migration completed");

    Ok(pool)
}

async fn configure_pragmas(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await
        .map_err(|_| AppError::database("failed to enable SQLite foreign keys"))?;

    sqlx::query("PRAGMA journal_mode = WAL")
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::database("failed to enable SQLite WAL mode"))?;

    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(pool)
        .await
        .map_err(|_| AppError::database("failed to set SQLite busy timeout"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::initialize;
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    async fn table_count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN \
             ('projects', 'workflows', 'workflow_versions', 'recipes', 'tasks', 'assets', \
              'generation_snapshots', 'task_events', 'presets', 'task_output_assets', \
               'production_batches', 'production_batch_items', 'asset_tags', 'asset_tag_links',
               'asset_favorites', 'project_templates', 'prompt_entries', 'prompt_versions',
               'shots', 'shot_stage_configs', 'shot_reference_assets', 'shot_generation_links',
               'shot_stage_prompts',
               'asset_video_prompts', 'production_item_reviews', 'benchmark_experiments',
               'benchmark_candidates', 'benchmark_runs', 'benchmark_quality_scores',
               'production_runs', 'production_stages', 'production_stage_items',
               'production_run_templates', 'reference_anchors', 'reference_anchor_assets',
               'asset_versions', 'asset_relations',
               'models', 'model_versions',
               'tools', 'tool_versions', 'tool_capabilities', 'tool_instances',
               'generation_tool_usages', 'generation_asset_versions',
               'production_series', 'production_episodes', 'production_scenes',
               'shot_scene_assignments', 'profile_revisions', 'reference_sets',
               'style_profiles', 'character_profiles', 'scene_profiles', 'prop_profiles',
               'costume_variants', 'reference_set_items', 'shot_profile_bindings',
               'shot_reference_set_bindings', 'consistency_scope_profile_bindings',
               'consistency_scope_reference_set_bindings', 'production_preparation_snapshots',
               'script_sources', 'script_import_drafts',
               'production_package_batch_bindings', 'project_workflow_bindings',
               'workflow_runtime_artifacts', 'workflow_recipe_promotions',
               'workflow_recipe_runtime_states', 'artifact_reviews')",
        )
        .fetch_one(pool)
        .await
        .expect("schema query should succeed")
    }

    #[tokio::test]
    async fn migration_runs_against_temporary_sqlite() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let database_path = temporary_directory.path().join("app.db");

        let pool = initialize(&database_path)
            .await
            .expect("migration should succeed");

        assert_eq!(table_count(&pool).await, 70);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM _sqlx_migrations",)
                .fetch_one(&pool)
                .await
                .expect("latest migration should be readable"),
            39
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
                .fetch_one(&pool)
                .await
                .expect("foreign keys pragma should be readable"),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
                .fetch_one(&pool)
                .await
                .expect("journal mode pragma should be readable")
                .to_ascii_lowercase(),
            "wal"
        );
        let prompt_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('asset_video_prompts') ORDER BY cid",
        )
        .fetch_all(&pool)
        .await
        .expect("asset video prompt columns should be readable");
        assert_eq!(
            prompt_columns,
            vec!["asset_id", "project_id", "prompt_text", "updated_at"]
        );
        let workflow_version_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('workflow_versions') WHERE name = 'package_name'",
        )
        .fetch_all(&pool)
        .await
        .expect("workflow version metadata should be readable");
        assert_eq!(workflow_version_columns, vec!["package_name"]);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_recipe_promotions",)
                .fetch_one(&pool)
                .await
                .expect("promotion table should be readable"),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_recipe_runtime_states",)
                .fetch_one(&pool)
                .await
                .expect("recipe runtime state table should be readable"),
            0
        );
        let project_workflow_binding_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('project_workflow_bindings') ORDER BY cid",
        )
        .fetch_all(&pool)
        .await
        .expect("project workflow binding columns should be readable");
        assert_eq!(
            project_workflow_binding_columns,
            vec![
                "project_id",
                "stage",
                "mode",
                "workflow_version_id",
                "recipe_id",
                "created_at",
                "updated_at"
            ]
        );
        let project_workflow_binding_primary_key = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('project_workflow_bindings')
             WHERE pk > 0 ORDER BY pk",
        )
        .fetch_all(&pool)
        .await
        .expect("project workflow binding primary key should be readable");
        assert_eq!(
            project_workflow_binding_primary_key,
            vec!["project_id", "stage", "mode"]
        );
        let runtime_state_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('workflow_runtime_states') WHERE name IN ('archived', 'archived_at') ORDER BY cid",
        )
        .fetch_all(&pool)
        .await
        .expect("workflow archive metadata should be readable");
        assert_eq!(runtime_state_columns, vec!["archived", "archived_at"]);
        let telemetry_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('tasks') WHERE name IN
             ('generation_execution_id', 'compiled_workflow_sha256', 'runtime_profile',
              'concurrency_class', 'prepare_started_at', 'prepared_at', 'submitted_at',
              'execution_started_at', 'execution_finished_at', 'collection_finished_at')
             ORDER BY cid",
        )
        .fetch_all(&pool)
        .await
        .expect("task telemetry metadata should be readable");
        assert_eq!(telemetry_columns.len(), 10);
        let idempotency_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('tasks') WHERE name IN
             ('submission_idempotency_key', 'submission_attempt', 'parent_task_id')
             ORDER BY cid",
        )
        .fetch_all(&pool)
        .await
        .expect("submission identity metadata should be readable");
        assert_eq!(
            idempotency_columns,
            vec![
                "submission_idempotency_key",
                "submission_attempt",
                "parent_task_id"
            ]
        );
        let model_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('models') ORDER BY cid",
        )
        .fetch_all(&pool)
        .await
        .expect("model columns should be readable");
        assert_eq!(
            model_columns,
            vec![
                "id",
                "name",
                "provider",
                "type",
                "description",
                "metadata_json",
                "created_at"
            ]
        );
        let model_version_columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('model_versions') ORDER BY cid",
        )
        .fetch_all(&pool)
        .await
        .expect("model version columns should be readable");
        assert_eq!(
            model_version_columns,
            vec![
                "id",
                "model_id",
                "version",
                "capabilities_json",
                "parameter_schema_json",
                "created_at"
            ]
        );
        for (table, column) in [
            ("prompt_versions", "model_version_id"),
            ("generation_snapshots", "model_version_id"),
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!(
                    "SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = '{column}'"
                ))
                .fetch_one(&pool)
                .await
                .expect("model provenance column should be readable"),
                1
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_tasks_project_submission_idempotency'",
            )
            .fetch_one(&pool)
            .await
            .expect("submission identity index should be readable"),
            1
        );

        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-migration', 'Migration', 'C:/migration', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("migration project fixture should insert");
        sqlx::query(
            "INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at)
             VALUES ('asset-migration', 'project-migration', 'image', 'source_image', 'Image', 'image.png', 'C:/migration/image.png', 'sha', 'image/png', 1, 1, 1, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("migration asset fixture should insert");
        sqlx::query(
            "INSERT INTO asset_video_prompts (asset_id, project_id, prompt_text, updated_at)
             VALUES ('asset-migration', 'project-migration', 'move slowly', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("asset video prompt fixture should insert");
        sqlx::query("DELETE FROM assets WHERE id = 'asset-migration'")
            .execute(&pool)
            .await
            .expect("asset deletion should succeed");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM asset_video_prompts WHERE asset_id = 'asset-migration'",
            )
            .fetch_one(&pool)
            .await
            .expect("asset video prompt count should be readable"),
            0
        );

        pool.close().await;
        assert!(database_path.is_file());
    }

    #[tokio::test]
    async fn repeated_migration_is_safe() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let database_path = temporary_directory.path().join("app.db");

        let first_pool = initialize(&database_path)
            .await
            .expect("first migration should succeed");
        first_pool.close().await;

        let second_pool = initialize(&database_path)
            .await
            .expect("second migration should succeed");
        assert_eq!(table_count(&second_pool).await, 70);
        second_pool.close().await;
    }

    #[tokio::test]
    async fn migration_033_through_038_preserves_rows_and_migrates_exact_artifact_review_links() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let database_path = temporary_directory.path().join("legacy-032.db");
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&database_path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("legacy database should connect");

        for migration in [
            include_str!("../../../migrations/001_initial.sql"),
            include_str!("../../../migrations/002_browse_indexes.sql"),
            include_str!("../../../migrations/003_presets.sql"),
            include_str!("../../../migrations/004_video_outputs.sql"),
            include_str!("../../../migrations/005_workflow_runtime_state.sql"),
            include_str!("../../../migrations/006_production_queue.sql"),
            include_str!("../../../migrations/007_production_queue_operations.sql"),
            include_str!("../../../migrations/008_organization.sql"),
            include_str!("../../../migrations/009_prompt_library.sql"),
            include_str!("../../../migrations/010_shot_production.sql"),
            include_str!("../../../migrations/011_asset_video_prompt.sql"),
            include_str!("../../../migrations/012_production_item_review.sql"),
            include_str!("../../../migrations/013_workflow_archive_and_package_metadata.sql"),
            include_str!("../../../migrations/014_workflow_benchmark.sql"),
            include_str!("../../../migrations/015_runtime_provenance.sql"),
            include_str!("../../../migrations/016_generation_telemetry.sql"),
            include_str!("../../../migrations/017_submission_idempotency.sql"),
            include_str!("../../../migrations/018_production_orchestrator.sql"),
            include_str!("../../../migrations/019_shot_stage_prompts.sql"),
            include_str!("../../../migrations/020_reference_anchors.sql"),
            include_str!("../../../migrations/021_production_structure.sql"),
            include_str!("../../../migrations/022_consistency_profiles_and_reference_sets.sql"),
            include_str!("../../../migrations/023_consistency_scope_bindings.sql"),
            include_str!("../../../migrations/024_production_preparation_snapshots.sql"),
            include_str!("../../../migrations/025_script_draft_foundation.sql"),
            include_str!("../../../migrations/026_production_package_batch_bindings.sql"),
            include_str!("../../../migrations/027_project_workflow_bindings.sql"),
            include_str!("../../../migrations/028_workflow_registry_v2.sql"),
            include_str!(
                "../../../migrations/029_workflow_registry_runtime_artifact_reconciliation.sql"
            ),
            include_str!("../../../migrations/030_workflow_recipe_promotions.sql"),
            include_str!("../../../migrations/031_workflow_recipe_archive_state.sql"),
            include_str!("../../../migrations/032_external_production_handoffs.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&pool)
                .await
                .expect("pre-033 migrations should apply");
        }

        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('legacy-v2-project', 'Legacy v2', 'C:/legacy-v2',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy project fixture should insert");
        sqlx::query(
            "INSERT INTO workflows
             (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('legacy-v2-workflow', 'Legacy workflow', 'image', 'text_to_image',
                     'legacy-v2-version', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy workflow fixture should insert");
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('legacy-v2-version', 'legacy-v2-workflow', '1', '{}', 'legacy-sha',
                     '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy workflow version fixture should insert");
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('legacy-v2-recipe', 'legacy-v2-version', '1', 1, 'schema_version: 1',
                     'legacy-recipe-sha', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy recipe fixture should insert");
        sqlx::query(
            "INSERT INTO tasks
             (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at)
             VALUES ('legacy-v2-task', 'legacy-v2-project', 'legacy-v2-workflow',
                     'legacy-v2-version', 'legacy-v2-recipe', 'SUCCEEDED',
                     '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy task fixture should insert");
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path, sha256,
              mime_type, width, height, file_size, source_task_id, metadata_json, created_at, updated_at)
             VALUES ('ast_legacy_v2_asset', 'legacy-v2-project', 'image', 'source_image',
                     'Legacy asset', 'legacy.png', 'C:/legacy-v2/legacy.png', 'asset-sha',
                     'image/png', 1280, 720, 1024, 'legacy-v2-task', '{}',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy asset fixture should insert");
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path, sha256,
              mime_type, width, height, file_size, source_task_id, metadata_json, created_at, updated_at)
             VALUES ('ast_legacy_v2_unreviewed', 'legacy-v2-project', 'image', 'source_image',
                     'Unreviewed output', 'other.png', 'C:/legacy-v2/other.png', 'other-sha',
                     'image/png', 1280, 720, 2048, 'legacy-v2-task', '{}',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("unreviewed output fixture should insert");
        sqlx::query(
            "INSERT INTO production_batches
             (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES ('legacy-v2-batch', 'legacy-v2-project', 'Legacy batch', 'COMPLETED', 0,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy batch fixture should insert");
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
              created_at, updated_at)
             VALUES ('legacy-v2-item', 'legacy-v2-batch', 0, 'legacy-v2-version',
                     'legacy-v2-recipe', '{}', 'SUCCEEDED',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy batch item fixture should insert");
        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
             VALUES ('legacy-v2-shot', 'legacy-v2-project', 0, 'Legacy shot', 'legacy prompt',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy shot fixture should insert");
        sqlx::query(
            "INSERT INTO production_item_reviews
             (id, project_id, production_batch_id, production_batch_item_id, task_id,
              result_asset_id, review_status, review_note, version, lineage_key,
              parent_batch_id, parent_item_id, created_at, updated_at)
             VALUES ('legacy-v2-review', 'legacy-v2-project', 'legacy-v2-batch',
                     'legacy-v2-item', 'legacy-v2-task', 'ast_legacy_v2_asset', 'APPROVED',
                     'preserve me', 1, 'legacy-v2-lineage', NULL, NULL,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy review fixture should insert");
        sqlx::query(
            "INSERT INTO task_output_assets (task_id, output_id, ordinal, asset_id, created_at)
             VALUES ('legacy-v2-task', 'legacy-output', 0, 'ast_legacy_v2_asset',
                     '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("legacy output mapping should insert");
        sqlx::query(
            "INSERT INTO task_output_assets (task_id, output_id, ordinal, asset_id, created_at)
             VALUES ('legacy-v2-task', 'legacy-output-unreviewed', 0,
                     'ast_legacy_v2_unreviewed', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("unreviewed output mapping should insert");

        let before: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM projects WHERE id = 'legacy-v2-project'),
               (SELECT COUNT(*) FROM assets WHERE id = 'ast_legacy_v2_asset'),
               (SELECT COUNT(*) FROM shots WHERE id = 'legacy-v2-shot'),
               (SELECT COUNT(*) FROM tasks WHERE id = 'legacy-v2-task'),
               (SELECT COUNT(*) FROM production_item_reviews WHERE id = 'legacy-v2-review')",
        )
        .fetch_one(&pool)
        .await
        .expect("legacy row snapshot should be readable");

        sqlx::raw_sql(include_str!(
            "../../../migrations/033_asset_library_data_layer.sql"
        ))
        .execute(&pool)
        .await
        .expect("migration 033 should apply to the legacy database");

        let after: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM projects WHERE id = 'legacy-v2-project'),
               (SELECT COUNT(*) FROM assets WHERE id = 'ast_legacy_v2_asset'),
               (SELECT COUNT(*) FROM shots WHERE id = 'legacy-v2-shot'),
               (SELECT COUNT(*) FROM tasks WHERE id = 'legacy-v2-task'),
               (SELECT COUNT(*) FROM production_item_reviews WHERE id = 'legacy-v2-review')",
        )
        .fetch_one(&pool)
        .await
        .expect("post-033 row snapshot should be readable");
        assert_eq!(after, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('asset_versions', 'asset_relations')",
            )
            .fetch_one(&pool)
            .await
            .expect("new asset library tables should be readable"),
            2
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
                .fetch_one(&pool)
                .await
                .expect("foreign keys pragma should be readable"),
            1
        );
        sqlx::raw_sql(include_str!(
            "../../../migrations/034_prompt_studio_model_foundation.sql"
        ))
        .execute(&pool)
        .await
        .expect("migration 034 should apply to the legacy database");

        let after_034: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM projects WHERE id = 'legacy-v2-project'),
               (SELECT COUNT(*) FROM assets WHERE id = 'ast_legacy_v2_asset'),
               (SELECT COUNT(*) FROM shots WHERE id = 'legacy-v2-shot'),
               (SELECT COUNT(*) FROM tasks WHERE id = 'legacy-v2-task'),
               (SELECT COUNT(*) FROM production_item_reviews WHERE id = 'legacy-v2-review')",
        )
        .fetch_one(&pool)
        .await
        .expect("post-034 row snapshot should be readable");
        assert_eq!(after_034, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('models', 'model_versions')",
            )
            .fetch_one(&pool)
            .await
            .expect("model tables should be readable"),
            2
        );
        for (table, column) in [
            ("prompt_versions", "model_version_id"),
            ("generation_snapshots", "model_version_id"),
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!(
                    "SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = '{column}'"
                ))
                .fetch_one(&pool)
                .await
                .expect("model provenance column should be readable"),
                1
            );
        }
        sqlx::raw_sql(include_str!(
            "../../../migrations/035_local_tool_hub_data_foundation.sql"
        ))
        .execute(&pool)
        .await
        .expect("migration 035 should apply to the legacy database");

        let after_035: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM projects WHERE id = 'legacy-v2-project'),
               (SELECT COUNT(*) FROM assets WHERE id = 'ast_legacy_v2_asset'),
               (SELECT COUNT(*) FROM shots WHERE id = 'legacy-v2-shot'),
               (SELECT COUNT(*) FROM tasks WHERE id = 'legacy-v2-task'),
               (SELECT COUNT(*) FROM production_item_reviews WHERE id = 'legacy-v2-review')",
        )
        .fetch_one(&pool)
        .await
        .expect("post-035 row snapshot should be readable");
        assert_eq!(after_035, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN
                 ('tools', 'tool_versions', 'tool_capabilities', 'tool_instances')",
            )
            .fetch_one(&pool)
            .await
            .expect("tool tables should be readable"),
            4
        );
        sqlx::raw_sql(include_str!(
            "../../../migrations/036_provenance_lineage.sql"
        ))
        .execute(&pool)
        .await
        .expect("migration 036 should apply to the legacy database");
        let after_036: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM projects WHERE id = 'legacy-v2-project'),
               (SELECT COUNT(*) FROM assets WHERE id = 'ast_legacy_v2_asset'),
               (SELECT COUNT(*) FROM shots WHERE id = 'legacy-v2-shot'),
               (SELECT COUNT(*) FROM tasks WHERE id = 'legacy-v2-task'),
               (SELECT COUNT(*) FROM production_item_reviews WHERE id = 'legacy-v2-review')",
        )
        .fetch_one(&pool)
        .await
        .expect("post-036 row snapshot should be readable");
        assert_eq!(after_036, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN
                 ('generation_tool_usages', 'generation_asset_versions')",
            )
            .fetch_one(&pool)
            .await
            .expect("lineage tables should be readable"),
            2
        );
        sqlx::raw_sql(include_str!(
            "../../../migrations/037_prompt_version_provenance.sql"
        ))
        .execute(&pool)
        .await
        .expect("migration 037 should apply to the legacy database");
        sqlx::raw_sql(include_str!("../../../migrations/038_artifact_review.sql"))
            .execute(&pool)
            .await
            .expect("migration 038 should apply to the legacy database");
        let migrated_review: (String, String) = sqlx::query_as(
            "SELECT decision, comment FROM artifact_reviews
             WHERE project_id = 'legacy-v2-project' AND artifact_id = 'ast_legacy_v2_asset'",
        )
        .fetch_one(&pool)
        .await
        .expect("exact legacy artifact review should migrate");
        assert_eq!(
            migrated_review,
            ("APPROVED".to_owned(), "preserve me".to_owned())
        );
        let unlinked_review: (String, String) = sqlx::query_as(
            "SELECT decision, comment FROM artifact_reviews
             WHERE project_id = 'legacy-v2-project' AND artifact_id = 'ast_legacy_v2_unreviewed'",
        )
        .fetch_one(&pool)
        .await
        .expect("unlinked output should remain pending, not inherit another output review");
        assert_eq!(unlinked_review, ("PENDING".to_owned(), String::new()));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_item_reviews WHERE id = 'legacy-v2-review'",
            )
            .fetch_one(&pool)
            .await
            .expect("legacy review should remain available"),
            1
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn adding_migrations_011_to_018_preserves_existing_project_runtime_rows() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let database_path = temporary_directory.path().join("legacy-app.db");
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&database_path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("legacy database should connect");

        for migration in [
            include_str!("../../../migrations/001_initial.sql"),
            include_str!("../../../migrations/002_browse_indexes.sql"),
            include_str!("../../../migrations/003_presets.sql"),
            include_str!("../../../migrations/004_video_outputs.sql"),
            include_str!("../../../migrations/005_workflow_runtime_state.sql"),
            include_str!("../../../migrations/006_production_queue.sql"),
            include_str!("../../../migrations/007_production_queue_operations.sql"),
            include_str!("../../../migrations/008_organization.sql"),
            include_str!("../../../migrations/009_prompt_library.sql"),
            include_str!("../../../migrations/010_shot_production.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&pool)
                .await
                .expect("legacy migrations should apply");
        }

        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("legacy foreign keys should be enabled");
        sqlx::query("INSERT INTO projects (id, name, root_path, created_at, updated_at) VALUES ('legacy-project', 'Legacy', 'C:/legacy', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at) VALUES ('legacy-workflow', 'Legacy', 'image', 'text_to_image', 'legacy-version', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at) VALUES ('legacy-version', 'legacy-workflow', '1', '{}', 'sha', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at) VALUES ('legacy-recipe', 'legacy-version', '1', 1, 'schema_version: 1', 'sha', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at) VALUES ('legacy-task', 'legacy-project', 'legacy-workflow', 'legacy-version', 'legacy-recipe', 'SUCCEEDED', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO task_events
             (id, task_id, sequence, event_type, payload_json, created_at)
             VALUES (
                'legacy-submission-event', 'legacy-task', 1,
                'TASK_SUBMISSION_PREPARED',
                ?,
                '2026-01-01T00:00:01Z'
             )",
        )
        .bind(r#"{"submissionIdempotencyKey":"legacy-request"}"#)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at) VALUES ('legacy-asset', 'legacy-project', 'image', 'source_image', 'Legacy', 'legacy.png', 'C:/legacy/legacy.png', 'sha', 'image/png', 1, 1, 1, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO production_batches (id, project_id, name, status, continue_on_failure, created_at, updated_at) VALUES ('legacy-batch', 'legacy-project', 'Legacy', 'COMPLETED', 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO production_batch_items (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, created_at, updated_at) VALUES ('legacy-item', 'legacy-batch', 0, 'legacy-version', 'legacy-recipe', '{}', 'SUCCEEDED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO shots (id, project_id, ordinal, name, prompt_text, created_at, updated_at) VALUES ('legacy-shot', 'legacy-project', 0, 'Legacy shot', 'legacy', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();

        let before: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM projects WHERE id = 'legacy-project'),
               (SELECT COUNT(*) FROM tasks WHERE id = 'legacy-task'),
               (SELECT COUNT(*) FROM assets WHERE id = 'legacy-asset'),
               (SELECT COUNT(*) FROM production_batches WHERE id = 'legacy-batch'),
               (SELECT COUNT(*) FROM production_batch_items WHERE id = 'legacy-item'),
               (SELECT COUNT(*) FROM shots WHERE id = 'legacy-shot'),
               (SELECT COUNT(*) FROM shot_generation_links)",
        )
        .fetch_one(&pool)
        .await
        .expect("legacy snapshot should be readable");

        sqlx::raw_sql(include_str!(
            "../../../migrations/011_asset_video_prompt.sql"
        ))
        .execute(&pool)
        .await
        .expect("migration 011 should apply to the legacy database");

        for migration in [
            include_str!("../../../migrations/012_production_item_review.sql"),
            include_str!("../../../migrations/013_workflow_archive_and_package_metadata.sql"),
            include_str!("../../../migrations/014_workflow_benchmark.sql"),
            include_str!("../../../migrations/015_runtime_provenance.sql"),
            include_str!("../../../migrations/016_generation_telemetry.sql"),
            include_str!("../../../migrations/017_submission_idempotency.sql"),
            include_str!("../../../migrations/018_production_orchestrator.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&pool)
                .await
                .expect("migrations 012-018 should apply to the 011 database");
        }

        let after: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM projects WHERE id = 'legacy-project'),
               (SELECT COUNT(*) FROM tasks WHERE id = 'legacy-task'),
               (SELECT COUNT(*) FROM assets WHERE id = 'legacy-asset'),
               (SELECT COUNT(*) FROM production_batches WHERE id = 'legacy-batch'),
               (SELECT COUNT(*) FROM production_batch_items WHERE id = 'legacy-item'),
               (SELECT COUNT(*) FROM shots WHERE id = 'legacy-shot'),
               (SELECT COUNT(*) FROM shot_generation_links)",
        )
        .fetch_one(&pool)
        .await
        .expect("legacy snapshot should remain readable");
        assert_eq!(after, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
                .fetch_one(&pool)
                .await
                .expect("legacy foreign keys pragma should be readable"),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN
                ('production_item_reviews', 'benchmark_experiments', 'benchmark_candidates',
                 'benchmark_runs', 'benchmark_quality_scores', 'production_runs',
                 'production_stages', 'production_stage_items', 'production_run_templates')",
            )
            .fetch_one(&pool)
            .await
            .expect("post-011 tables should be readable"),
            9
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN
                 ('idx_benchmark_runs_experiment_candidate', 'idx_benchmark_runs_task',
                  'idx_benchmark_quality_candidate', 'idx_production_runs_project_updated',
                  'idx_production_stages_run_status', 'idx_production_stage_items_task',
                  'idx_production_run_templates_project')",
            )
            .fetch_one(&pool)
            .await
            .expect("orchestrator indexes should be readable"),
            7
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN
                 ('idx_tasks_project_submission_idempotency', 'idx_production_runs_project_status',
                  'idx_production_stages_batch', 'idx_production_stage_items_asset',
                  'idx_production_stage_items_source_asset')",
            )
            .fetch_one(&pool)
            .await
            .expect("lineage indexes should be readable"),
            5
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT name FROM pragma_table_info('workflow_versions') WHERE name = 'package_name'",
            )
            .fetch_one(&pool)
            .await
            .expect("workflow package metadata should be readable"),
            "package_name"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name IN
                 ('app_version', 'build_commit', 'workflow_version', 'workflow_sha256',
                  'recipe_version', 'recipe_sha256', 'package_name', 'package_source_path',
                  'dynamic_binding_targets_json')",
            )
            .fetch_one(&pool)
            .await
            .expect("task runtime provenance columns should be readable"),
            9
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name IN
                 ('generation_execution_id', 'compiled_workflow_sha256', 'runtime_profile',
                  'concurrency_class', 'prepare_started_at', 'prepared_at', 'submitted_at',
                  'execution_started_at', 'execution_finished_at', 'collection_finished_at')",
            )
            .fetch_one(&pool)
            .await
            .expect("task telemetry columns should be readable"),
            10
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT submission_idempotency_key FROM tasks WHERE id = 'legacy-task'",
            )
            .fetch_one(&pool)
            .await
            .expect("legacy submission key should be backfilled"),
            "legacy-request"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT submission_attempt FROM tasks WHERE id = 'legacy-task'",
            )
            .fetch_one(&pool)
            .await
            .expect("legacy submission attempt should be normalized"),
            1
        );
        let preserved_rows: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM projects WHERE id = 'legacy-project'),
               (SELECT COUNT(*) FROM tasks WHERE id = 'legacy-task'),
               (SELECT COUNT(*) FROM assets WHERE id = 'legacy-asset'),
               (SELECT COUNT(*) FROM production_batches WHERE id = 'legacy-batch'),
               (SELECT COUNT(*) FROM production_batch_items WHERE id = 'legacy-item'),
               (SELECT COUNT(*) FROM shots WHERE id = 'legacy-shot'),
               (SELECT COUNT(*) FROM shot_generation_links)",
        )
        .fetch_one(&pool)
        .await
        .expect("post-015 legacy rows should remain readable");
        assert_eq!(preserved_rows, before);
        pool.close().await;
    }

    #[tokio::test]
    async fn migration_019_backfills_both_stages_and_preserves_fk_cascade_behavior() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let database_path = temporary_directory.path().join("legacy-018.db");
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&database_path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("018 database should connect");

        for migration in [
            include_str!("../../../migrations/001_initial.sql"),
            include_str!("../../../migrations/002_browse_indexes.sql"),
            include_str!("../../../migrations/003_presets.sql"),
            include_str!("../../../migrations/004_video_outputs.sql"),
            include_str!("../../../migrations/005_workflow_runtime_state.sql"),
            include_str!("../../../migrations/006_production_queue.sql"),
            include_str!("../../../migrations/007_production_queue_operations.sql"),
            include_str!("../../../migrations/008_organization.sql"),
            include_str!("../../../migrations/009_prompt_library.sql"),
            include_str!("../../../migrations/010_shot_production.sql"),
            include_str!("../../../migrations/011_asset_video_prompt.sql"),
            include_str!("../../../migrations/012_production_item_review.sql"),
            include_str!("../../../migrations/013_workflow_archive_and_package_metadata.sql"),
            include_str!("../../../migrations/014_workflow_benchmark.sql"),
            include_str!("../../../migrations/015_runtime_provenance.sql"),
            include_str!("../../../migrations/016_generation_telemetry.sql"),
            include_str!("../../../migrations/017_submission_idempotency.sql"),
            include_str!("../../../migrations/018_production_orchestrator.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&pool)
                .await
                .expect("018 migrations should apply");
        }
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys should be enabled");
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('migration-project', 'Migration', 'C:/migration', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO prompt_entries
             (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at)
             VALUES ('migration-entry', 'migration-project', 'prompt', 'Legacy', 'legacy', '[]', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO prompt_versions
             (id, prompt_id, version, text, created_at)
             VALUES ('migration-version', 'migration-entry', 1, 'legacy prompt', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        for (id, name, prompt, entry_id, version_id, ordinal) in [
            (
                "migration-shot-a",
                "Shot A",
                "legacy prompt",
                Some("migration-entry"),
                Some("migration-version"),
                0_i64,
            ),
            ("migration-shot-b", "Shot B", "", None, None, 1_i64),
            (
                "migration-shot-c",
                "Shot C",
                "no provenance",
                None,
                None,
                2_i64,
            ),
        ] {
            sqlx::query(
                "INSERT INTO shots
                 (id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id, created_at, updated_at)
                 VALUES (?, 'migration-project', ?, ?, ?, ?, ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .bind(id)
            .bind(ordinal)
            .bind(name)
            .bind(prompt)
            .bind(entry_id)
            .bind(version_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        sqlx::raw_sql(include_str!(
            "../../../migrations/019_shot_stage_prompts.sql"
        ))
        .execute(&pool)
        .await
        .expect("migration 019 should apply to an 018 database");

        let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT shot_id, stage, prompt_entry_id, prompt_version_id
             FROM shot_stage_prompts ORDER BY shot_id, stage",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 6);
        assert_eq!(
            rows.iter()
                .filter(|row| row.0 == "migration-shot-a")
                .map(|row| (row.1.as_str(), row.2.as_deref(), row.3.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                ("image", Some("migration-entry"), Some("migration-version")),
                ("video", Some("migration-entry"), Some("migration-version")),
            ]
        );
        assert!(rows
            .iter()
            .filter(|row| row.0 == "migration-shot-b")
            .all(|row| row.2.is_none() && row.3.is_none()));

        sqlx::query("DELETE FROM prompt_entries WHERE id = 'migration-entry'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT prompt_entry_id, prompt_version_id
             FROM shot_stage_prompts WHERE shot_id = 'migration-shot-a' ORDER BY stage",
        )
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .all(|row| row.0.is_none() && row.1.is_none()));

        sqlx::query("DELETE FROM shots WHERE id = 'migration-shot-c'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_stage_prompts WHERE shot_id = 'migration-shot-c'",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );

        sqlx::raw_sql(include_str!(
            "../../../migrations/020_reference_anchors.sql"
        ))
        .execute(&pool)
        .await
        .expect("migration 020 should apply to a 019 database");
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path,
              sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at)
             VALUES ('migration-anchor-asset', 'migration-project', 'image', 'source_image',
                     'Anchor', 'anchor.png', 'C:/migration/anchor.png', 'sha', 'image/png',
                     1, 1, 1, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO reference_anchors
             (id, project_id, kind, name, normalized_name, description, created_at, updated_at)
             VALUES ('migration-anchor', 'migration-project', 'CHARACTER', 'Anchor', 'anchor', '',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO reference_anchor_assets
             (anchor_id, asset_id, ordinal, created_at)
             VALUES ('migration-anchor', 'migration-anchor-asset', 0, '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM assets WHERE id = 'migration-anchor-asset'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM reference_anchor_assets WHERE anchor_id = 'migration-anchor'",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }
}
