use super::{map_sqlx_error, parse_json};
use crate::application::ports::{
    AvailableGenerationDefinition, GenerationDefinition, GenerationDefinitionRepository,
    RepositoryError,
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

    async fn list_available(&self) -> Result<Vec<AvailableGenerationDefinition>, RepositoryError> {
        let rows = sqlx::query_as::<_, AvailableDefinitionRow>(
            "SELECT
                w.id AS workflow_id,
                wv.id AS workflow_version_id,
                r.id AS recipe_id,
                w.name,
                w.category,
                w.mode,
                r.recipe_yaml
             FROM workflows w
             INNER JOIN workflow_versions wv ON wv.workflow_id = w.id
             INNER JOIN recipes r ON r.workflow_version_id = wv.id
             LEFT JOIN workflow_runtime_states wrs ON wrs.workflow_version_id = wv.id
             WHERE w.current_version_id = wv.id
               AND COALESCE(wrs.enabled, 1) = 1
               AND COALESCE(wrs.archived, 0) = 0
               AND r.version = (
                   SELECT MAX(latest.version)
                   FROM recipes latest
                   WHERE latest.workflow_version_id = wv.id
               )
             ORDER BY w.name ASC, wv.version ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows
            .into_iter()
            .map(|row| AvailableGenerationDefinition {
                workflow_id: row.workflow_id,
                workflow_version_id: row.workflow_version_id,
                recipe_id: row.recipe_id,
                name: row.name,
                category: row.category,
                mode: row.mode,
                recipe_yaml: row.recipe_yaml,
            })
            .collect())
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

#[derive(sqlx::FromRow)]
struct AvailableDefinitionRow {
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
    name: String,
    category: String,
    mode: String,
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

    #[tokio::test]
    async fn lists_only_the_latest_recipe_for_each_current_workflow_version() {
        let (_directory, pool, repository) = setup().await;
        sqlx::query(
            "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("recipe-2")
        .bind("workflow-version-1")
        .bind("1.0.1")
        .bind(1)
        .bind("schema_version: 1\nid: latest-recipe")
        .bind("sha-latest")
        .bind("2026-01-02T00:00:00Z")
        .execute(&pool)
        .await
        .expect("latest recipe fixture should insert");

        let definitions = repository
            .list_available()
            .await
            .expect("available definitions should load");

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].recipe_id, "recipe-2");
        assert!(definitions[0].recipe_yaml.contains("latest-recipe"));
    }

    #[tokio::test]
    async fn excludes_explicitly_disabled_versions_without_hiding_missing_state() {
        let (_directory, pool, repository) = setup().await;
        assert_eq!(repository.list_available().await.unwrap().len(), 1);

        sqlx::query(
            "INSERT INTO workflow_runtime_states (workflow_version_id, enabled, updated_at)
             VALUES (?, 0, ?)",
        )
        .bind("workflow-version-1")
        .bind("2026-01-02T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        assert!(repository.list_available().await.unwrap().is_empty());
    }
}
