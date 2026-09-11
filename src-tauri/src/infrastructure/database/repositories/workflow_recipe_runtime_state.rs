use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    RepositoryError, WorkflowRecipeRuntimeState, WorkflowRecipeRuntimeStateRepository,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteWorkflowRecipeRuntimeStateRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowRecipeRuntimeStateRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkflowRecipeRuntimeStateRepository for SqliteWorkflowRecipeRuntimeStateRepository {
    async fn find_state(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Option<WorkflowRecipeRuntimeState>, RepositoryError> {
        let row = sqlx::query_as::<_, RecipeRuntimeStateRow>(
            "SELECT workflow_version_id, recipe_id, archived, archived_at, updated_at
             FROM workflow_recipe_runtime_states
             WHERE workflow_version_id = ? AND recipe_id = ?",
        )
        .bind(workflow_version_id)
        .bind(recipe_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(RecipeRuntimeStateRow::try_into_domain).transpose()
    }

    async fn set_archived(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        archived: bool,
        archived_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM recipes
             WHERE workflow_version_id = ? AND id = ?",
        )
        .bind(workflow_version_id)
        .bind(recipe_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if exists == 0 {
            return Err(RepositoryError::not_found(
                "recipe for workflow version",
                format!("{workflow_version_id}:{recipe_id}"),
            ));
        }

        sqlx::query(
            "INSERT INTO workflow_recipe_runtime_states
                (workflow_version_id, recipe_id, archived, archived_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(workflow_version_id, recipe_id) DO UPDATE SET
                archived = excluded.archived,
                archived_at = excluded.archived_at,
                updated_at = excluded.updated_at",
        )
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(i64::from(archived))
        .bind(archived_at.map(format_datetime))
        .bind(format_datetime(updated_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn list_states(&self) -> Result<Vec<WorkflowRecipeRuntimeState>, RepositoryError> {
        let rows = sqlx::query_as::<_, RecipeRuntimeStateRow>(
            "SELECT workflow_version_id, recipe_id, archived, archived_at, updated_at
             FROM workflow_recipe_runtime_states
             ORDER BY updated_at DESC, workflow_version_id, recipe_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(RecipeRuntimeStateRow::try_into_domain)
            .collect()
    }
}

#[derive(sqlx::FromRow)]
struct RecipeRuntimeStateRow {
    workflow_version_id: String,
    recipe_id: String,
    archived: i64,
    archived_at: Option<String>,
    updated_at: String,
}

impl RecipeRuntimeStateRow {
    fn try_into_domain(self) -> Result<WorkflowRecipeRuntimeState, RepositoryError> {
        Ok(WorkflowRecipeRuntimeState {
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            archived: self.archived != 0,
            archived_at: self
                .archived_at
                .as_deref()
                .map(|value| parse_datetime("workflow recipe archived_at", value))
                .transpose()?,
            updated_at: parse_datetime("workflow recipe updated_at", &self.updated_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowRecipeRuntimeStateRepository;
    use crate::application::ports::WorkflowRecipeRuntimeStateRepository;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn missing_rows_are_active_and_exact_pairs_are_isolated() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        for recipe_id in ["recipe-a", "recipe-b"] {
            sqlx::query(
                "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
                 VALUES (?, 'workflow-version-1', ?, 1, 'schema_version: 1', 'sha', '2026-01-01T00:00:00Z')",
            )
            .bind(recipe_id)
            .bind(recipe_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        let repository = SqliteWorkflowRecipeRuntimeStateRepository::new(pool);

        assert!(repository
            .find_state("workflow-version-1", "recipe-a")
            .await
            .unwrap()
            .is_none());
        repository
            .set_archived(
                "workflow-version-1",
                "recipe-a",
                true,
                Some(Utc::now()),
                Utc::now(),
            )
            .await
            .unwrap();
        assert!(
            repository
                .find_state("workflow-version-1", "recipe-a")
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(repository
            .find_state("workflow-version-1", "recipe-b")
            .await
            .unwrap()
            .is_none());
        repository
            .set_archived("workflow-version-1", "recipe-a", false, None, Utc::now())
            .await
            .unwrap();
        assert!(
            !repository
                .find_state("workflow-version-1", "recipe-a")
                .await
                .unwrap()
                .unwrap()
                .archived
        );
    }
}
