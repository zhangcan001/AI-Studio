use super::{map_sqlx_error, parse_json};
use crate::application::ports::{
    GenerationDefinition, GenerationDefinitionRepository, RepositoryError,
};
use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteGenerationDefinitionRepository {
    pool: SqlitePool,
}

impl SqliteGenerationDefinitionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GenerationDefinitionRepository for SqliteGenerationDefinitionRepository {
    async fn find(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Option<GenerationDefinition>, RepositoryError> {
        let row = sqlx::query_as::<_, DefinitionRow>(
            "SELECT
                w.id AS workflow_id,
                wv.id AS workflow_version_id,
                r.id AS recipe_id,
                wv.api_workflow_json,
                r.recipe_yaml
             FROM workflows w
             INNER JOIN workflow_versions wv ON wv.workflow_id = w.id
             INNER JOIN recipes r ON r.workflow_version_id = wv.id
             WHERE wv.id = ?
               AND r.id = ?
               AND r.workflow_version_id = ?",
        )
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(workflow_version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(DefinitionRow::try_into_domain).transpose()
    }
}

#[derive(sqlx::FromRow)]
struct DefinitionRow {
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
    api_workflow_json: String,
    recipe_yaml: String,
}

impl DefinitionRow {
    fn try_into_domain(self) -> Result<GenerationDefinition, RepositoryError> {
        let workflow_json = parse_json(
            "generation definition api_workflow_json",
            Some(&self.api_workflow_json),
        )?
        .ok_or_else(|| {
            RepositoryError::serialization(
                "generation definition api_workflow_json",
                "missing value",
            )
        })?;

        Ok(GenerationDefinition {
            workflow_id: self.workflow_id,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            workflow_json,
            recipe_yaml: self.recipe_yaml,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteGenerationDefinitionRepository;
    use crate::application::ports::GenerationDefinitionRepository;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, SqliteGenerationDefinitionRepository) {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteGenerationDefinitionRepository::new(pool.clone());
        (directory, pool, repository)
    }

    #[tokio::test]
    async fn finds_joined_workflow_recipe_definition() {
        let (_directory, pool, repository) = setup().await;
        sqlx::query("UPDATE workflow_versions SET api_workflow_json = ? WHERE id = ?")
            .bind(r#"{"3":{"inputs":{},"class_type":"KSampler"}}"#)
            .bind("workflow-version-1")
            .execute(&pool)
            .await
            .expect("workflow fixture should update");

        let definition = repository
            .find("workflow-version-1", "recipe-1")
            .await
            .expect("definition lookup should succeed")
            .expect("definition should exist");

        assert_eq!(definition.workflow_id, "workflow-1");
        assert_eq!(definition.workflow_version_id, "workflow-version-1");
        assert_eq!(definition.recipe_id, "recipe-1");
        assert_eq!(definition.workflow_json["3"]["class_type"], "KSampler");
        assert!(definition.recipe_yaml.contains("schema_version"));
    }

    #[tokio::test]
    async fn rejects_recipe_from_another_workflow_version() {
        let (_directory, _pool, repository) = setup().await;

        let definition = repository
            .find("other-workflow-version", "recipe-1")
            .await
            .expect("definition lookup should succeed");

        assert!(definition.is_none());
    }
}
