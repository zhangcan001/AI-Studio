use crate::application::ports::{RepositoryError, WorkflowRunRepository};
use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteWorkflowRunRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowRunRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkflowRunRepository for SqliteWorkflowRunRepository {
    async fn has_successful_run(
        &self,
        workflow_id: &str,
        workflow_version: &str,
    ) -> Result<bool, RepositoryError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM tasks t
             INNER JOIN workflow_versions wv ON wv.id = t.workflow_version_id
             WHERE t.workflow_id = ?
               AND wv.version = ?
               AND t.status = 'SUCCEEDED'",
        )
        .bind(workflow_id)
        .bind(workflow_version)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;

        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowRunRepository;
    use crate::application::ports::WorkflowRunRepository;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use tempfile::tempdir;

    #[tokio::test]
    async fn reports_successful_run_by_workflow_version() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteWorkflowRunRepository::new(pool.clone());

        assert!(!repository
            .has_successful_run("workflow-1", "1")
            .await
            .unwrap());

        sqlx::query(
            "INSERT INTO tasks (
                id, project_id, workflow_id, workflow_version_id, recipe_id,
                status, created_at
             ) VALUES ('task-1', 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-1', 'CREATED', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("UPDATE tasks SET status = 'SUCCEEDED' WHERE id = 'task-1'")
            .execute(&pool)
            .await
            .unwrap();

        assert!(repository
            .has_successful_run("workflow-1", "1")
            .await
            .unwrap());
    }
}
