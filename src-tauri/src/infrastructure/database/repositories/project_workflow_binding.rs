use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    ProjectWorkflowBindingRecord, ProjectWorkflowBindingRepository, RepositoryError,
};
use async_trait::async_trait;
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Clone)]
pub struct SqliteProjectWorkflowBindingRepository {
    pool: SqlitePool,
}

impl SqliteProjectWorkflowBindingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectWorkflowBindingRepository for SqliteProjectWorkflowBindingRepository {
    async fn list_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, ProjectWorkflowBindingRow>(
            "SELECT project_id, stage, mode, workflow_version_id, recipe_id,
                    created_at, updated_at
             FROM project_workflow_bindings
             WHERE project_id = ?
             ORDER BY stage ASC, mode ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn replace_for_project(
        &self,
        project_id: &str,
        bindings: &[ProjectWorkflowBindingRecord],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query("DELETE FROM project_workflow_bindings WHERE project_id = ?")
            .bind(project_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;

        for binding in bindings {
            insert_binding(&mut transaction, binding).await?;
        }

        transaction.commit().await.map_err(map_sqlx_error)
    }
}

async fn insert_binding(
    transaction: &mut Transaction<'_, Sqlite>,
    binding: &ProjectWorkflowBindingRecord,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO project_workflow_bindings
            (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&binding.project_id)
    .bind(&binding.stage)
    .bind(&binding.mode)
    .bind(&binding.workflow_version_id)
    .bind(&binding.recipe_id)
    .bind(format_datetime(binding.created_at))
    .bind(format_datetime(binding.updated_at))
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ProjectWorkflowBindingRow {
    project_id: String,
    stage: String,
    mode: String,
    workflow_version_id: String,
    recipe_id: String,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ProjectWorkflowBindingRow> for ProjectWorkflowBindingRecord {
    type Error = RepositoryError;

    fn try_from(row: ProjectWorkflowBindingRow) -> Result<Self, Self::Error> {
        Ok(Self {
            project_id: row.project_id,
            stage: row.stage,
            mode: row.mode,
            workflow_version_id: row.workflow_version_id,
            recipe_id: row.recipe_id,
            created_at: parse_datetime("project workflow binding created_at", &row.created_at)?,
            updated_at: parse_datetime("project workflow binding updated_at", &row.updated_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteProjectWorkflowBindingRepository;
    use crate::application::ports::{
        ProjectWorkflowBindingRecord, ProjectWorkflowBindingRepository,
    };
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, SqliteProjectWorkflowBindingRepository) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        (
            directory,
            pool.clone(),
            SqliteProjectWorkflowBindingRepository::new(pool),
        )
    }

    fn binding(
        stage: &str,
        mode: &str,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> ProjectWorkflowBindingRecord {
        let now = Utc::now();
        ProjectWorkflowBindingRecord {
            project_id: "project-1".to_owned(),
            stage: stage.to_owned(),
            mode: mode.to_owned(),
            workflow_version_id: workflow_version_id.to_owned(),
            recipe_id: recipe_id.to_owned(),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn lists_and_replaces_bindings_atomically() {
        let (_directory, _pool, repository) = setup().await;
        let first = binding("IMAGE", "DEFAULT", "workflow-version-1", "recipe-1");
        let second = binding("VIDEO", "DEFAULT", "workflow-version-1", "recipe-1");
        repository
            .replace_for_project("project-1", &[first.clone(), second.clone()])
            .await
            .expect("initial replacement should succeed");
        assert_eq!(
            repository.list_for_project("project-1").await.unwrap(),
            vec![first, second]
        );

        let replacement = binding(
            "VIDEO",
            "FL2VA_TEXT_TO_VIDEO",
            "workflow-version-1",
            "recipe-1",
        );
        repository
            .replace_for_project("project-1", &[replacement.clone()])
            .await
            .expect("replacement should succeed");
        assert_eq!(
            repository.list_for_project("project-1").await.unwrap(),
            vec![replacement]
        );
    }

    #[tokio::test]
    async fn rolls_back_delete_when_an_insert_fails() {
        let (_directory, _pool, repository) = setup().await;
        let original = binding("IMAGE", "DEFAULT", "workflow-version-1", "recipe-1");
        repository
            .replace_for_project("project-1", &[original.clone()])
            .await
            .unwrap();
        let invalid = binding(
            "IMAGE",
            "FL2VA_TEXT_TO_VIDEO",
            "workflow-version-1",
            "recipe-1",
        );
        assert!(repository
            .replace_for_project("project-1", &[invalid])
            .await
            .is_err());
        assert_eq!(
            repository.list_for_project("project-1").await.unwrap(),
            vec![original]
        );
    }

    #[tokio::test]
    async fn cascades_with_project_delete_and_preserves_soft_workflow_references() {
        let (_directory, pool, repository) = setup().await;
        let stale = binding(
            "VIDEO",
            "DEFAULT",
            "missing-workflow-version",
            "missing-recipe",
        );
        repository
            .replace_for_project("project-1", &[stale.clone()])
            .await
            .unwrap();
        assert_eq!(
            repository.list_for_project("project-1").await.unwrap(),
            vec![stale]
        );
        sqlx::query("DELETE FROM projects WHERE id = 'project-1'")
            .execute(&pool)
            .await
            .expect("project deletion should succeed");
        assert_eq!(
            repository.list_for_project("project-1").await.unwrap(),
            Vec::new()
        );
    }
}
