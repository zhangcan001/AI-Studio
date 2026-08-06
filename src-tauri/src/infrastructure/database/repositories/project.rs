use super::map_sqlx_error;
use crate::application::ports::{ProjectRepository, RepositoryError};
use async_trait::async_trait;
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
    async fn get_storage_root(&self, project_id: &str) -> Result<Option<PathBuf>, RepositoryError> {
        let root_path =
            sqlx::query_scalar::<_, Option<String>>("SELECT root_path FROM projects WHERE id = ?")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_sqlx_error)?
                .flatten();

        root_path
            .map(|value| {
                if value.trim().is_empty() {
                    Err(RepositoryError::integrity(
                        "project root_path must not be empty",
                    ))
                } else {
                    Ok(PathBuf::from(value))
                }
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteProjectRepository;
    use crate::application::ports::ProjectRepository;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use sqlx::SqlitePool;
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
}
