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
                    binding_instance_id, revision, created_at, updated_at
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

    async fn find_slot(
        &self,
        project_id: &str,
        stage: &str,
        mode: &str,
    ) -> Result<Option<ProjectWorkflowBindingRecord>, RepositoryError> {
        let row = sqlx::query_as::<_, ProjectWorkflowBindingRow>(
            "SELECT project_id, stage, mode, workflow_version_id, recipe_id,
                    binding_instance_id, revision, created_at, updated_at
             FROM project_workflow_bindings
             WHERE project_id = ? AND stage = ? AND mode = ?",
        )
        .bind(project_id)
        .bind(stage)
        .bind(mode)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(TryInto::try_into).transpose()
    }

    async fn insert_slot(
        &self,
        binding: &ProjectWorkflowBindingRecord,
    ) -> Result<(), RepositoryError> {
        insert_binding_pool(&self.pool, binding).await
    }

    async fn update_slot(
        &self,
        project_id: &str,
        stage: &str,
        mode: &str,
        expected_binding_instance_id: &str,
        expected_revision: i64,
        workflow_version_id: &str,
        recipe_id: &str,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query(
            "UPDATE project_workflow_bindings
             SET workflow_version_id = ?, recipe_id = ?, revision = revision + 1,
                 updated_at = ?
             WHERE project_id = ? AND stage = ? AND mode = ?
               AND binding_instance_id = ? AND revision = ?",
        )
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(format_datetime(updated_at))
        .bind(project_id)
        .bind(stage)
        .bind(mode)
        .bind(expected_binding_instance_id)
        .bind(expected_revision)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected())
    }

    async fn delete_slot(
        &self,
        project_id: &str,
        stage: &str,
        mode: &str,
        expected_binding_instance_id: &str,
        expected_revision: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM project_workflow_bindings
             WHERE project_id = ? AND stage = ? AND mode = ?
               AND binding_instance_id = ? AND revision = ?",
        )
        .bind(project_id)
        .bind(stage)
        .bind(mode)
        .bind(expected_binding_instance_id)
        .bind(expected_revision)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected())
    }

    async fn clear_exact_bindings(
        &self,
        bindings: &[ProjectWorkflowBindingRecord],
    ) -> Result<u64, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let mut cleared = 0;
        for binding in bindings {
            let result = sqlx::query(
                "DELETE FROM project_workflow_bindings
                 WHERE project_id = ? AND stage = ? AND mode = ?
                   AND workflow_version_id = ? AND binding_instance_id = ? AND revision = ?",
            )
            .bind(&binding.project_id)
            .bind(&binding.stage)
            .bind(&binding.mode)
            .bind(&binding.workflow_version_id)
            .bind(&binding.binding_instance_id)
            .bind(binding.revision)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            cleared += result.rows_affected();
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(cleared)
    }

    async fn list_for_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, ProjectWorkflowBindingRow>(
            "SELECT project_id, stage, mode, workflow_version_id, recipe_id,
                    binding_instance_id, revision, created_at, updated_at
             FROM project_workflow_bindings
             WHERE workflow_version_id = ?
             ORDER BY project_id ASC, stage ASC, mode ASC",
        )
        .bind(workflow_version_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn clear_by_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<u64, RepositoryError> {
        let result =
            sqlx::query("DELETE FROM project_workflow_bindings WHERE workflow_version_id = ?")
                .bind(workflow_version_id)
                .execute(&self.pool)
                .await
                .map_err(map_sqlx_error)?;
        Ok(result.rows_affected())
    }
}

async fn insert_binding(
    transaction: &mut Transaction<'_, Sqlite>,
    binding: &ProjectWorkflowBindingRecord,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO project_workflow_bindings
            (project_id, stage, mode, workflow_version_id, recipe_id,
             binding_instance_id, revision, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&binding.project_id)
    .bind(&binding.stage)
    .bind(&binding.mode)
    .bind(&binding.workflow_version_id)
    .bind(&binding.recipe_id)
    .bind(&binding.binding_instance_id)
    .bind(binding.revision)
    .bind(format_datetime(binding.created_at))
    .bind(format_datetime(binding.updated_at))
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn insert_binding_pool(
    pool: &SqlitePool,
    binding: &ProjectWorkflowBindingRecord,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO project_workflow_bindings
            (project_id, stage, mode, workflow_version_id, recipe_id,
             binding_instance_id, revision, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&binding.project_id)
    .bind(&binding.stage)
    .bind(&binding.mode)
    .bind(&binding.workflow_version_id)
    .bind(&binding.recipe_id)
    .bind(&binding.binding_instance_id)
    .bind(binding.revision)
    .bind(format_datetime(binding.created_at))
    .bind(format_datetime(binding.updated_at))
    .execute(pool)
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
    binding_instance_id: Option<String>,
    revision: i64,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ProjectWorkflowBindingRow> for ProjectWorkflowBindingRecord {
    type Error = RepositoryError;

    fn try_from(row: ProjectWorkflowBindingRow) -> Result<Self, Self::Error> {
        let binding_instance_id = row
            .binding_instance_id
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| {
                RepositoryError::integrity(format!(
                    "project workflow binding {}/{}/{} has no binding instance id",
                    row.project_id, row.stage, row.mode
                ))
            })?;
        if row.revision < 1 {
            return Err(RepositoryError::integrity(format!(
                "project workflow binding {}/{}/{} has invalid revision {}",
                row.project_id, row.stage, row.mode, row.revision
            )));
        }
        Ok(Self {
            project_id: row.project_id,
            stage: row.stage,
            mode: row.mode,
            workflow_version_id: row.workflow_version_id,
            recipe_id: row.recipe_id,
            binding_instance_id,
            revision: row.revision,
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
    use chrono::{Duration, Utc};
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
            binding_instance_id: format!(
                "bnd_test_{}_{}",
                stage.to_ascii_lowercase(),
                mode.to_ascii_lowercase()
            ),
            revision: 1,
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

    #[tokio::test]
    async fn clears_only_the_requested_workflow_version() {
        let (_directory, _pool, repository) = setup().await;
        let removed = binding("VIDEO", "DEFAULT", "deleted-version", "recipe-1");
        let kept = binding("IMAGE", "DEFAULT", "kept-version", "recipe-2");
        repository
            .replace_for_project("project-1", &[removed.clone(), kept.clone()])
            .await
            .unwrap();

        assert_eq!(
            repository
                .list_for_workflow_version("deleted-version")
                .await
                .unwrap(),
            vec![removed]
        );
        assert_eq!(
            repository
                .clear_by_workflow_version("deleted-version")
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            repository.list_for_project("project-1").await.unwrap(),
            vec![kept]
        );
        assert_eq!(
            repository
                .clear_by_workflow_version("deleted-version")
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn slot_occ_requires_instance_and_revision_and_preserves_creation_time() {
        let (_directory, _pool, repository) = setup().await;
        let original = binding("IMAGE", "DEFAULT", "workflow-version-1", "recipe-1");
        let created_at = original.created_at;
        repository.insert_slot(&original).await.unwrap();

        let updated_at = created_at + Duration::seconds(5);
        assert_eq!(
            repository
                .update_slot(
                    "project-1",
                    "IMAGE",
                    "DEFAULT",
                    &original.binding_instance_id,
                    1,
                    "workflow-version-2",
                    "recipe-2",
                    updated_at,
                )
                .await
                .unwrap(),
            1
        );
        let updated = repository
            .find_slot("project-1", "IMAGE", "DEFAULT")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.binding_instance_id, original.binding_instance_id);
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.created_at, created_at);
        assert_eq!(updated.updated_at, updated_at);

        assert_eq!(
            repository
                .update_slot(
                    "project-1",
                    "IMAGE",
                    "DEFAULT",
                    &original.binding_instance_id,
                    1,
                    "workflow-version-3",
                    "recipe-3",
                    updated_at + Duration::seconds(1),
                )
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            repository
                .delete_slot(
                    "project-1",
                    "IMAGE",
                    "DEFAULT",
                    &original.binding_instance_id,
                    1,
                )
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            repository
                .delete_slot(
                    "project-1",
                    "IMAGE",
                    "DEFAULT",
                    &original.binding_instance_id,
                    2,
                )
                .await
                .unwrap(),
            1
        );

        let mut recreated = binding("IMAGE", "DEFAULT", "workflow-version-4", "recipe-4");
        recreated.binding_instance_id.push_str("_recreated");
        repository.insert_slot(&recreated).await.unwrap();
        assert_ne!(recreated.binding_instance_id, original.binding_instance_id);
        assert_eq!(
            repository
                .update_slot(
                    "project-1",
                    "IMAGE",
                    "DEFAULT",
                    &original.binding_instance_id,
                    2,
                    "workflow-version-5",
                    "recipe-5",
                    updated_at + Duration::seconds(2),
                )
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn concurrent_slot_creates_have_one_winner() {
        let (_directory, pool, repository) = setup().await;
        let first = binding("VIDEO", "DEFAULT", "workflow-version-1", "recipe-1");
        let mut second = binding("VIDEO", "DEFAULT", "workflow-version-2", "recipe-2");
        second.binding_instance_id.push_str("_other");
        let first_repo = repository.clone();
        let second_repo = SqliteProjectWorkflowBindingRepository::new(pool);
        let (first_result, second_result) = tokio::join!(
            first_repo.insert_slot(&first),
            second_repo.insert_slot(&second),
        );
        assert_eq!(first_result.is_ok() as u8 + second_result.is_ok() as u8, 1);
        assert_eq!(
            repository
                .list_for_project("project-1")
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
