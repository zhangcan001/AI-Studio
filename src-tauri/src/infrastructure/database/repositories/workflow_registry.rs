use super::{format_datetime, map_sqlx_error, parse_datetime, parse_optional_datetime};
use crate::application::ports::{
    RepositoryError, WorkflowRegistryRecord, WorkflowRegistryRepository, WORKFLOW_SOURCE_USER,
    WORKFLOW_STATE_ACTIVE, WORKFLOW_STATE_REMOVED,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};

const WORKFLOW_SELECT: &str = "SELECT
    id, name, category, mode, source_kind, library_state,
    current_version_id, removed_at, created_at, updated_at
    FROM workflows";

#[derive(Clone)]
pub struct SqliteWorkflowRegistryRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowRegistryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkflowRegistryRepository for SqliteWorkflowRegistryRepository {
    async fn list(&self) -> Result<Vec<WorkflowRegistryRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, WorkflowRegistryRow>(&format!(
            "{WORKFLOW_SELECT} ORDER BY name COLLATE NOCASE ASC, id ASC"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(WorkflowRegistryRow::try_into_record)
            .collect()
    }

    async fn get(
        &self,
        workflow_id: &str,
    ) -> Result<Option<WorkflowRegistryRecord>, RepositoryError> {
        read_workflow(&self.pool, workflow_id).await
    }

    async fn rename(
        &self,
        workflow_id: &str,
        name: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowRegistryRecord>, RepositoryError> {
        if name.trim().is_empty() {
            return Err(RepositoryError::integrity(
                "workflow name must not be empty",
            ));
        }

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let result = sqlx::query(
            "UPDATE workflows
             SET name = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(name.trim())
        .bind(format_datetime(updated_at))
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(map_sqlx_error)?;
            return Ok(None);
        }

        let workflow = read_workflow_in_transaction(&mut transaction, workflow_id).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(workflow)
    }

    async fn set_current_version(
        &self,
        workflow_id: &str,
        workflow_version_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_workflow_version(&mut transaction, workflow_id, workflow_version_id).await?;

        sqlx::query(
            "UPDATE workflows
             SET current_version_id = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(workflow_version_id)
        .bind(format_datetime(updated_at))
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn remove(
        &self,
        workflow_id: &str,
        removed_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowRegistryRecord>, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        if read_workflow_in_transaction(&mut transaction, workflow_id)
            .await?
            .is_none()
        {
            transaction.rollback().await.map_err(map_sqlx_error)?;
            return Ok(None);
        }

        let active_tasks = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM tasks
             WHERE workflow_version_id IN (
                 SELECT id FROM workflow_versions WHERE workflow_id = ?
             )
               AND status IN (
                   'CREATED', 'VALIDATING', 'PREPARING', 'QUEUED',
                   'RUNNING', 'COLLECTING', 'CANCEL_REQUESTED'
               )",
        )
        .bind(workflow_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        let active_queue_items = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM production_batch_items pbi
             INNER JOIN production_batches pb ON pb.id = pbi.batch_id
             WHERE pbi.workflow_version_id IN (
                 SELECT id FROM workflow_versions WHERE workflow_id = ?
             )
               AND pbi.status IN ('PENDING', 'DISPATCHING', 'DISPATCHED')
               AND pb.status IN ('READY', 'RUNNING', 'PAUSED')",
        )
        .bind(workflow_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if active_tasks > 0 || active_queue_items > 0 {
            return Err(RepositoryError::integrity(
                "workflow has active tasks or production queue items",
            ));
        }

        sqlx::query(
            "UPDATE workflows
             SET library_state = ?, removed_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(WORKFLOW_STATE_REMOVED)
        .bind(format_datetime(removed_at))
        .bind(format_datetime(removed_at))
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        // Bindings are logical project configuration, not workflow history.
        // Clear them in the same transaction as the logical state change.
        sqlx::query(
            "DELETE FROM project_workflow_bindings
             WHERE workflow_version_id IN (
                 SELECT id FROM workflow_versions WHERE workflow_id = ?
             )",
        )
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let workflow = read_workflow_in_transaction(&mut transaction, workflow_id).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(workflow)
    }

    async fn restore(
        &self,
        workflow_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowRegistryRecord>, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let result = sqlx::query(
            "UPDATE workflows
             SET library_state = ?, removed_at = NULL, updated_at = ?
             WHERE id = ?",
        )
        .bind(WORKFLOW_STATE_ACTIVE)
        .bind(format_datetime(updated_at))
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(map_sqlx_error)?;
            return Ok(None);
        }

        let workflow = read_workflow_in_transaction(&mut transaction, workflow_id).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(workflow)
    }

    async fn purge(&self, workflow_id: &str) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let Some(workflow) = read_workflow_in_transaction(&mut transaction, workflow_id).await?
        else {
            transaction.rollback().await.map_err(map_sqlx_error)?;
            return Ok(false);
        };

        if workflow.source_kind != WORKFLOW_SOURCE_USER {
            return Err(RepositoryError::integrity(
                "PURGE_PRODUCT_BLOCKED: product workflows are never purged",
            ));
        }
        if workflow.library_state != WORKFLOW_STATE_REMOVED {
            return Err(RepositoryError::integrity(
                "workflow must be removed before purge",
            ));
        }

        let references = sqlx::query_as::<_, WorkflowReferenceCounts>(
            "WITH versions AS (
                 SELECT id FROM workflow_versions WHERE workflow_id = ?
             )
             SELECT
                 (SELECT COUNT(*) FROM tasks
                    WHERE workflow_version_id IN (SELECT id FROM versions)) AS task_count,
                 (SELECT COUNT(*) FROM production_batch_items
                    WHERE workflow_version_id IN (SELECT id FROM versions)) AS batch_item_count,
                 (SELECT COUNT(*) FROM presets
                    WHERE workflow_version_id IN (SELECT id FROM versions)) AS preset_count,
                 (SELECT COUNT(*) FROM project_templates
                    WHERE workflow_version_id IN (SELECT id FROM versions)) AS template_count,
                 (SELECT COUNT(*) FROM shot_stage_configs
                    WHERE workflow_version_id IN (SELECT id FROM versions)) AS shot_config_count,
                 (SELECT COUNT(*) FROM benchmark_candidates
                    WHERE workflow_version_id IN (SELECT id FROM versions)) AS benchmark_count,
                 (SELECT COUNT(*) FROM project_workflow_bindings
                    WHERE workflow_version_id IN (SELECT id FROM versions)) AS binding_count,
                 (SELECT COUNT(*) FROM production_stages
                    WHERE workflow_version_id IN (SELECT id FROM versions)
                       OR recipe_id IN (
                           SELECT id FROM recipes
                           WHERE workflow_version_id IN (SELECT id FROM versions)
                       )) AS stage_count,
                 (SELECT COUNT(*) FROM production_run_templates
                    WHERE krea2_workflow_version_id IN (SELECT id FROM versions)
                       OR h3_workflow_version_id IN (SELECT id FROM versions)) AS run_template_count",
        )
        .bind(workflow_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if references.total() > 0 {
            return Err(RepositoryError::integrity(format!(
                "workflow still has references (tasks={}, batches={}, presets={}, templates={}, shots={}, benchmarks={}, bindings={}, stages={}, run_templates={})",
                references.task_count,
                references.batch_item_count,
                references.preset_count,
                references.template_count,
                references.shot_config_count,
                references.benchmark_count,
                references.binding_count,
                references.stage_count,
                references.run_template_count,
            )));
        }

        sqlx::query(
            "DELETE FROM workflow_runtime_artifacts
             WHERE workflow_version_id IN (
                 SELECT id FROM workflow_versions WHERE workflow_id = ?
             )",
        )
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "DELETE FROM workflow_runtime_states
             WHERE workflow_version_id IN (
                 SELECT id FROM workflow_versions WHERE workflow_id = ?
             )",
        )
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "DELETE FROM recipes
             WHERE workflow_version_id IN (
                 SELECT id FROM workflow_versions WHERE workflow_id = ?
             )",
        )
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query("DELETE FROM workflow_versions WHERE workflow_id = ?")
            .bind(workflow_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        let result = sqlx::query("DELETE FROM workflows WHERE id = ?")
            .bind(workflow_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;

        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }
}

async fn read_workflow(
    pool: &SqlitePool,
    workflow_id: &str,
) -> Result<Option<WorkflowRegistryRecord>, RepositoryError> {
    let row = sqlx::query_as::<_, WorkflowRegistryRow>(&format!("{WORKFLOW_SELECT} WHERE id = ?"))
        .bind(workflow_id)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?;
    row.map(WorkflowRegistryRow::try_into_record).transpose()
}

async fn read_workflow_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_id: &str,
) -> Result<Option<WorkflowRegistryRecord>, RepositoryError> {
    let row = sqlx::query_as::<_, WorkflowRegistryRow>(&format!("{WORKFLOW_SELECT} WHERE id = ?"))
        .bind(workflow_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    row.map(WorkflowRegistryRow::try_into_record).transpose()
}

async fn ensure_workflow_version(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_id: &str,
    workflow_version_id: &str,
) -> Result<(), RepositoryError> {
    let workflow_exists =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(workflow_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
    if workflow_exists == 0 {
        return Err(RepositoryError::not_found("workflow", workflow_id));
    }

    let version_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM workflow_versions
         WHERE id = ? AND workflow_id = ?",
    )
    .bind(workflow_version_id)
    .bind(workflow_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if version_exists == 0 {
        return Err(RepositoryError::not_found(
            "workflow version",
            workflow_version_id,
        ));
    }

    let library_state =
        sqlx::query_scalar::<_, String>("SELECT library_state FROM workflows WHERE id = ?")
            .bind(workflow_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
    if library_state != WORKFLOW_STATE_ACTIVE {
        return Err(RepositoryError::integrity(
            "removed workflows cannot select a current version",
        ));
    }

    let runtime_state = sqlx::query_as::<_, (i64, i64)>(
        "SELECT enabled, archived
         FROM workflow_runtime_states
         WHERE workflow_version_id = ?",
    )
    .bind(workflow_version_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if let Some((enabled, archived)) = runtime_state {
        if archived != 0 {
            return Err(RepositoryError::integrity(
                "archived workflow versions cannot become current",
            ));
        }
        if enabled == 0 {
            return Err(RepositoryError::integrity(
                "disabled workflow versions cannot become current",
            ));
        }
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct WorkflowRegistryRow {
    id: String,
    name: String,
    category: String,
    mode: String,
    source_kind: String,
    library_state: String,
    current_version_id: Option<String>,
    removed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl WorkflowRegistryRow {
    fn try_into_record(self) -> Result<WorkflowRegistryRecord, RepositoryError> {
        Ok(WorkflowRegistryRecord {
            id: self.id,
            name: self.name,
            category: self.category,
            mode: self.mode,
            source_kind: self.source_kind,
            library_state: self.library_state,
            current_version_id: self.current_version_id,
            removed_at: parse_optional_datetime(
                "workflow registry removed_at",
                self.removed_at.as_deref(),
            )?,
            created_at: parse_datetime("workflow registry created_at", &self.created_at)?,
            updated_at: parse_datetime("workflow registry updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct WorkflowReferenceCounts {
    task_count: i64,
    batch_item_count: i64,
    preset_count: i64,
    template_count: i64,
    shot_config_count: i64,
    benchmark_count: i64,
    binding_count: i64,
    stage_count: i64,
    run_template_count: i64,
}

impl WorkflowReferenceCounts {
    fn total(&self) -> i64 {
        self.task_count
            + self.batch_item_count
            + self.preset_count
            + self.template_count
            + self.shot_config_count
            + self.benchmark_count
            + self.binding_count
            + self.stage_count
            + self.run_template_count
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowRegistryRepository;
    use crate::application::ports::{WorkflowRegistryRepository, WORKFLOW_SOURCE_USER};
    use crate::infrastructure::database::initialize;
    use chrono::{TimeZone, Utc};
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    async fn setup() -> (SqlitePool, SqliteWorkflowRegistryRepository) {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        sqlx::query(
            "INSERT INTO workflows
             (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('registry-workflow', 'Original', 'video', 'text_to_video', NULL, ?, ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .expect("workflow fixture should insert");
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, package_name,
              package_source_path, created_at)
             VALUES ('registry-version', 'registry-workflow', '1.0.0', '{}', 'workflow-sha',
                     'legacy-package', 'C:/legacy-package', ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .expect("workflow version fixture should insert");
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256,
              created_at)
             VALUES ('registry-recipe', 'registry-version', '1.0.0', 1, 'schema_version: 1',
                     'recipe-sha', ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .expect("recipe fixture should insert");
        (pool.clone(), SqliteWorkflowRegistryRepository::new(pool))
    }

    #[tokio::test]
    async fn logical_lifecycle_keeps_ids_clears_bindings_and_purges_only_after_remove() {
        let (pool, repository) = setup().await;
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

        repository
            .set_current_version("registry-workflow", "registry-version", timestamp)
            .await
            .expect("current version should be set");
        let renamed = repository
            .rename("registry-workflow", "Renamed", timestamp)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(renamed.name, "Renamed");
        assert_eq!(
            renamed.current_version_id.as_deref(),
            Some("registry-version")
        );

        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('registry-project', 'Project', 'C:/project', ?, ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO project_workflow_bindings
             (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at)
             VALUES ('registry-project', 'VIDEO', 'DEFAULT', 'registry-version',
                     'registry-recipe', ?, ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let removed = repository
            .remove("registry-workflow", timestamp)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(removed.library_state, "REMOVED");
        assert!(removed.removed_at.is_some());
        assert_eq!(
            removed.current_version_id.as_deref(),
            Some("registry-version")
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM project_workflow_bindings")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );

        let restored = repository
            .restore("registry-workflow", timestamp)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.library_state, "ACTIVE");
        assert!(restored.removed_at.is_none());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM project_workflow_bindings")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );

        repository
            .remove("registry-workflow", timestamp)
            .await
            .unwrap();
        assert!(repository.purge("registry-workflow").await.unwrap());
        assert!(repository.get("registry-workflow").await.unwrap().is_none());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn remove_blocks_active_task_without_mutating_state() {
        let (pool, repository) = setup().await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('registry-project', 'Project', 'C:/project', ?, ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tasks
             (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at)
             VALUES ('registry-task', 'registry-project', 'registry-workflow',
                     'registry-version', 'registry-recipe', 'RUNNING', ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let error = repository
            .remove(
                "registry-workflow",
                Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
            )
            .await
            .expect_err("active task should block logical removal");
        assert!(error.to_string().contains("active"));
        assert_eq!(
            repository
                .get("registry-workflow")
                .await
                .unwrap()
                .unwrap()
                .library_state,
            "ACTIVE"
        );
        assert_eq!(WORKFLOW_SOURCE_USER, "USER");
    }
}
