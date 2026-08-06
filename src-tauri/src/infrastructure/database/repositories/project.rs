use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{ProjectRecord, ProjectRepository, RepositoryError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::path::PathBuf;

#[derive(Clone)]
pub struct SqliteProjectRepository {
    pool: SqlitePool,
}

impl SqliteProjectRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectRepository for SqliteProjectRepository {
    async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, description, root_path, created_at, updated_at
             FROM projects
             ORDER BY updated_at DESC, created_at DESC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProjectRow::try_into_domain).collect()
    }

    async fn find_by_id(&self, project_id: &str) -> Result<Option<ProjectRecord>, RepositoryError> {
        let row = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, description, root_path, created_at, updated_at
             FROM projects WHERE id = ?",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(ProjectRow::try_into_domain).transpose()
    }

    async fn insert(&self, project: &ProjectRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.description)
        .bind(project.root_path.to_string_lossy().to_string())
        .bind(format_datetime(project.created_at))
        .bind(format_datetime(project.updated_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn update_metadata(
        &self,
        project_id: &str,
        name: &str,
        description: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ProjectRecord>, RepositoryError> {
        let result = sqlx::query(
            "UPDATE projects
             SET name = ?, description = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(name)
        .bind(description)
        .bind(format_datetime(updated_at))
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_by_id(project_id).await
    }

    async fn get_storage_root(&self, project_id: &str) -> Result<Option<PathBuf>, RepositoryError> {
        Ok(self
            .find_by_id(project_id)
            .await?
            .map(|project| project.root_path))
    }

    async fn ensure_default_project(
        &self,
        project_id: &str,
        name: &str,
        root_path: &PathBuf,
        created_at: DateTime<Utc>,
    ) -> Result<ProjectRecord, RepositoryError> {
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES (?, ?, NULL, ?, ?, ?)
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(project_id)
        .bind(name)
        .bind(root_path.to_string_lossy().to_string())
        .bind(format_datetime(created_at))
        .bind(format_datetime(created_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        self.find_by_id(project_id)
            .await?
            .ok_or_else(|| RepositoryError::not_found("project", project_id))
    }
}

#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: String,
    name: String,
    description: Option<String>,
    root_path: String,
    created_at: String,
    updated_at: String,
}

impl ProjectRow {
    fn try_into_domain(self) -> Result<ProjectRecord, RepositoryError> {
        if self.root_path.trim().is_empty() {
            return Err(RepositoryError::integrity(
                "project root_path must not be empty",
            ));
        }

        Ok(ProjectRecord {
            id: self.id,
            name: self.name,
            description: self.description,
            root_path: PathBuf::from(self.root_path),
            created_at: parse_datetime("project created_at", &self.created_at)?,
            updated_at: parse_datetime("project updated_at", &self.updated_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteProjectRepository;
    use crate::application::ports::{ProjectRecord, ProjectRepository};
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use sqlx::SqlitePool;
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, SqliteProjectRepository) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        (directory, pool.clone(), SqliteProjectRepository::new(pool))
    }

    #[tokio::test]
    async fn reads_project_storage_root_without_writing() {
        let (_directory, _pool, repository) = setup().await;
        assert_eq!(
            repository
                .get_storage_root("project-1")
                .await
                .expect("project lookup"),
            Some("C:/project".into())
        );
        assert_eq!(
            repository
                .get_storage_root("missing")
                .await
                .expect("missing lookup"),
            None
        );
    }

    #[tokio::test]
    async fn ensures_default_project_once_without_overwriting_existing_root() {
        let (_directory, pool, repository) = setup().await;
        let first_root = PathBuf::from("C:/first-project");
        let second_root = PathBuf::from("C:/second-project");
        let at = chrono::Utc::now();

        let first = repository
            .ensure_default_project("prj_default", "Default Project", &first_root, at)
            .await
            .expect("default project should be created");
        let second = repository
            .ensure_default_project(
                "prj_default",
                "Default Project",
                &second_root,
                at + chrono::Duration::seconds(1),
            )
            .await
            .expect("default project should be reused");

        assert_eq!(first.root_path, first_root);
        assert_eq!(second.root_path, first_root);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = 'prj_default'")
                .fetch_one(&pool)
                .await
                .expect("project count"),
            1
        );
    }

    #[tokio::test]
    async fn inserts_lists_and_updates_project_metadata_without_changing_root() {
        let (_directory, _pool, repository) = setup().await;
        let root = PathBuf::from("C:/created-project");
        let created_at = chrono::Utc::now();
        let project = ProjectRecord {
            id: "prj_created".to_owned(),
            name: "Created".to_owned(),
            description: Some("Initial".to_owned()),
            root_path: root.clone(),
            created_at,
            updated_at: created_at,
        };

        repository.insert(&project).await.unwrap();
        let listed = repository.list().await.unwrap();
        assert!(listed.iter().any(|item| item.id == "prj_created"));

        let updated = repository
            .update_metadata(
                "prj_created",
                "Renamed",
                Some("Updated"),
                created_at + chrono::Duration::seconds(1),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.description.as_deref(), Some("Updated"));
        assert_eq!(updated.root_path, root);
    }
}
