use super::{format_datetime, map_sqlx_error};
use crate::application::ports::{
    RepositoryError, RuntimeRecipeRecord, RuntimeWorkflowVersionRecord, WorkflowDeletionCounts,
    WorkflowRuntimeRepository,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct SqliteWorkflowRuntimeRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowRuntimeRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn load(
        &self,
        workflow_version_id: Option<&str>,
    ) -> Result<Vec<RuntimeWorkflowVersionRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, RuntimeWorkflowRow>(
            "SELECT
                wv.id AS workflow_version_id,
                w.id AS workflow_id,
                w.name,
                w.category,
                w.mode,
                wv.version AS workflow_version,
                wv.workflow_sha256,
                wv.package_name,
                CASE WHEN w.current_version_id = wv.id THEN 1 ELSE 0 END AS is_current,
                r.id AS recipe_id,
                r.version AS recipe_version,
                r.schema_version AS recipe_schema_version,
                r.recipe_yaml,
                r.recipe_sha256,
                (SELECT COUNT(*) FROM tasks t WHERE t.workflow_version_id = wv.id) AS total_tasks,
                (SELECT COUNT(*) FROM tasks t WHERE t.workflow_version_id = wv.id
                   AND t.status IN ('CREATED', 'VALIDATING', 'PREPARING', 'QUEUED', 'RUNNING', 'COLLECTING', 'CANCEL_REQUESTED')) AS active_tasks,
                (SELECT COUNT(*) FROM tasks t WHERE t.workflow_version_id = wv.id AND t.status = 'SUCCEEDED') AS successful_tasks,
                (SELECT MAX(finished_at) FROM tasks t WHERE t.workflow_version_id = wv.id AND t.status = 'SUCCEEDED') AS latest_success_at,
                (SELECT MAX(finished_at) FROM tasks t WHERE t.workflow_version_id = wv.id AND t.status = 'FAILED') AS latest_failure_at
             FROM workflows w
             INNER JOIN workflow_versions wv ON wv.workflow_id = w.id
             LEFT JOIN recipes r ON r.workflow_version_id = wv.id
             WHERE (? IS NULL OR wv.id = ?)
             ORDER BY w.name ASC, wv.version ASC, r.version ASC",
        )
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;

        let mut records = BTreeMap::<String, RuntimeWorkflowVersionRecord>::new();
        for row in rows {
            let entry = records
                .entry(row.workflow_version_id.clone())
                .or_insert_with(|| RuntimeWorkflowVersionRecord {
                    workflow_version_id: row.workflow_version_id.clone(),
                    workflow_id: row.workflow_id.clone(),
                    name: row.name.clone(),
                    category: row.category.clone(),
                    mode: row.mode.clone(),
                    workflow_version: row.workflow_version.clone(),
                    workflow_sha256: row.workflow_sha256.clone(),
                    package_name: row.package_name.clone(),
                    is_current: row.is_current != 0,
                    recipes: Vec::new(),
                    active_tasks: row.active_tasks.max(0) as u64,
                    total_tasks: row.total_tasks.max(0) as u64,
                    has_successful_run: row.successful_tasks > 0,
                    latest_success_at: row.latest_success_at.clone(),
                    latest_failure_at: row.latest_failure_at.clone(),
                });
            if let (
                Some(recipe_id),
                Some(recipe_version),
                Some(schema_version),
                Some(recipe_yaml),
                Some(recipe_sha256),
            ) = (
                row.recipe_id,
                row.recipe_version,
                row.recipe_schema_version,
                row.recipe_yaml,
                row.recipe_sha256,
            ) {
                entry.recipes.push(RuntimeRecipeRecord {
                    recipe_id,
                    version: recipe_version,
                    schema_version: schema_version as u32,
                    recipe_yaml,
                    recipe_sha256,
                });
            }
        }
        Ok(records.into_values().collect())
    }
}

#[async_trait]
impl WorkflowRuntimeRepository for SqliteWorkflowRuntimeRepository {
    async fn list_versions(&self) -> Result<Vec<RuntimeWorkflowVersionRecord>, RepositoryError> {
        self.load(None).await
    }

    async fn find_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<Option<RuntimeWorkflowVersionRecord>, RepositoryError> {
        Ok(self
            .load(Some(workflow_version_id))
            .await?
            .into_iter()
            .next())
    }

    async fn inspect_deletion(
        &self,
        workflow_version_id: &str,
    ) -> Result<Option<WorkflowDeletionCounts>, RepositoryError> {
        let row = sqlx::query_as::<_, WorkflowDeletionCountsRow>(
            "SELECT
                (SELECT COUNT(*) FROM tasks t WHERE t.workflow_version_id = ?
                   AND t.status IN ('CREATED', 'VALIDATING', 'PREPARING', 'QUEUED', 'RUNNING', 'COLLECTING', 'CANCEL_REQUESTED')) AS active_task_count,
                (SELECT COUNT(*) FROM tasks t WHERE t.workflow_version_id = ?) AS historical_task_count,
                (SELECT COUNT(*) FROM production_batch_items pbi
                   INNER JOIN production_batches pb ON pb.id = pbi.batch_id
                   WHERE pbi.workflow_version_id = ?
                     AND pbi.status IN ('PENDING', 'DISPATCHING', 'DISPATCHED')
                     AND pb.status IN ('READY', 'RUNNING', 'PAUSED')) AS active_queue_item_count,
                (SELECT COUNT(*) FROM production_batch_items pbi WHERE pbi.workflow_version_id = ?) AS production_batch_item_count,
                ((SELECT COUNT(*) FROM presets p WHERE p.workflow_version_id = ?)
                 + (SELECT COUNT(*) FROM project_templates pt WHERE pt.workflow_version_id = ?)
                 + (SELECT COUNT(*) FROM shot_stage_configs ssc WHERE ssc.workflow_version_id = ?)) AS other_reference_count
             WHERE EXISTS (SELECT 1 FROM workflow_versions WHERE id = ?)",
        )
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.map(|row| WorkflowDeletionCounts {
            active_task_count: row.active_task_count.max(0) as u64,
            active_queue_item_count: row.active_queue_item_count.max(0) as u64,
            historical_task_count: row.historical_task_count.max(0) as u64,
            production_batch_item_count: row.production_batch_item_count.max(0) as u64,
            other_reference_count: row.other_reference_count.max(0) as u64,
        }))
    }

    async fn delete_version(
        &self,
        workflow_version_id: &str,
        workflow_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        delete_version(
            &mut transaction,
            workflow_version_id,
            workflow_id,
            updated_at,
        )
        .await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }
}

async fn delete_version(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_version_id: &str,
    workflow_id: &str,
    updated_at: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workflow_versions WHERE id = ? AND workflow_id = ?",
    )
    .bind(workflow_version_id)
    .bind(workflow_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if exists == 0 {
        return Err(RepositoryError::not_found(
            "workflow version",
            workflow_version_id,
        ));
    }

    let references = sqlx::query_scalar::<_, i64>(
        "SELECT
            (SELECT COUNT(*) FROM tasks WHERE workflow_version_id = ?)
          + (SELECT COUNT(*) FROM production_batch_items WHERE workflow_version_id = ?)
          + (SELECT COUNT(*) FROM presets WHERE workflow_version_id = ?)
          + (SELECT COUNT(*) FROM project_templates WHERE workflow_version_id = ?)
          + (SELECT COUNT(*) FROM shot_stage_configs WHERE workflow_version_id = ?)",
    )
    .bind(workflow_version_id)
    .bind(workflow_version_id)
    .bind(workflow_version_id)
    .bind(workflow_version_id)
    .bind(workflow_version_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if references > 0 {
        return Err(RepositoryError::integrity(
            "workflow version still has historical or configuration references",
        ));
    }

    sqlx::query(
        "UPDATE workflows
         SET current_version_id = (
             SELECT id FROM workflow_versions
             WHERE workflow_id = ? AND id <> ?
             ORDER BY version DESC LIMIT 1
         ), updated_at = ?
         WHERE id = ?",
    )
    .bind(workflow_id)
    .bind(workflow_version_id)
    .bind(format_datetime(updated_at))
    .bind(workflow_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    sqlx::query("DELETE FROM workflow_runtime_states WHERE workflow_version_id = ?")
        .bind(workflow_version_id)
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    sqlx::query("DELETE FROM recipes WHERE workflow_version_id = ?")
        .bind(workflow_version_id)
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    sqlx::query("DELETE FROM workflow_versions WHERE id = ?")
        .bind(workflow_version_id)
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    sqlx::query(
        "DELETE FROM workflows
         WHERE id = ? AND NOT EXISTS (SELECT 1 FROM workflow_versions WHERE workflow_id = ?)",
    )
    .bind(workflow_id)
    .bind(workflow_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct RuntimeWorkflowRow {
    workflow_version_id: String,
    workflow_id: String,
    name: String,
    category: String,
    mode: String,
    workflow_version: String,
    workflow_sha256: String,
    package_name: Option<String>,
    is_current: i64,
    recipe_id: Option<String>,
    recipe_version: Option<String>,
    recipe_schema_version: Option<i64>,
    recipe_yaml: Option<String>,
    recipe_sha256: Option<String>,
    total_tasks: i64,
    active_tasks: i64,
    successful_tasks: i64,
    latest_success_at: Option<String>,
    latest_failure_at: Option<String>,
}

#[derive(sqlx::FromRow)]
struct WorkflowDeletionCountsRow {
    active_task_count: i64,
    historical_task_count: i64,
    active_queue_item_count: i64,
    production_batch_item_count: i64,
    other_reference_count: i64,
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowRuntimeRepository;
    use crate::application::ports::WorkflowRuntimeRepository;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use tempfile::tempdir;

    #[tokio::test]
    async fn lists_canonical_ids_recipes_and_task_evidence() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteWorkflowRuntimeRepository::new(pool.clone());
        let records = repository.list_versions().await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].workflow_version_id, "workflow-version-1");
        assert_eq!(records[0].recipes[0].recipe_id, "recipe-1");

        sqlx::query(
            "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at, finished_at)
             VALUES ('runtime-task', 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-1', 'SUCCEEDED', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(
            repository
                .find_version("workflow-version-1")
                .await
                .unwrap()
                .unwrap()
                .has_successful_run
        );
    }

    #[tokio::test]
    async fn deletion_inspection_counts_history_and_active_queue_references() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteWorkflowRuntimeRepository::new(pool.clone());

        sqlx::query(
            "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at, finished_at)
             VALUES ('historical-task', 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-1', 'SUCCEEDED', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_batches (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES ('active-batch', 'project-1', 'Active', 'READY', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_batch_items (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, created_at, updated_at)
             VALUES ('pending-item', 'active-batch', 0, 'workflow-version-1', 'recipe-1', '{}', 'PENDING', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let counts = repository
            .inspect_deletion("workflow-version-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(counts.active_task_count, 0);
        assert_eq!(counts.active_queue_item_count, 1);
        assert_eq!(counts.historical_task_count, 1);
        assert_eq!(counts.production_batch_item_count, 1);

        let error = repository
            .delete_version("workflow-version-1", "workflow-1", chrono::Utc::now())
            .await
            .expect_err("historical references must protect the version");
        assert!(error.to_string().contains("historical"));
    }

    #[tokio::test]
    async fn unreferenced_version_deletes_registration_and_parent_workflow_atomically() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteWorkflowRuntimeRepository::new(pool.clone());

        repository
            .delete_version("workflow-version-1", "workflow-1", chrono::Utc::now())
            .await
            .unwrap();

        assert!(repository.list_versions().await.unwrap().is_empty());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
}
