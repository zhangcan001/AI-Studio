use crate::application::ports::{
    RepositoryError, RuntimeRecipeRecord, RuntimeWorkflowVersionRecord, WorkflowRuntimeRepository,
};
use async_trait::async_trait;
use sqlx::SqlitePool;
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
}
