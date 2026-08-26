use super::{map_sqlx_error, parse_json};
use crate::application::ports::{
    AvailableGenerationDefinition, GenerationDefinition, GenerationDefinitionRepository,
    RepositoryError,
};
use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

const FIND_MANY_CHUNK_SIZE: usize = 200;

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
                wv.version AS workflow_version,
                wv.workflow_sha256,
                wv.package_name,
                wv.package_source_path,
                r.version AS recipe_version,
                r.recipe_sha256,
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

    async fn find_many(
        &self,
        pairs: &[(String, String)],
    ) -> Result<Vec<GenerationDefinition>, RepositoryError> {
        if pairs.is_empty() {
            return Ok(Vec::new());
        }

        let mut definitions = Vec::new();
        for chunk in pairs.chunks(FIND_MANY_CHUNK_SIZE) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT
                    w.id AS workflow_id,
                    wv.id AS workflow_version_id,
                    r.id AS recipe_id,
                    wv.version AS workflow_version,
                    wv.workflow_sha256,
                    wv.package_name,
                    wv.package_source_path,
                    r.version AS recipe_version,
                    r.recipe_sha256,
                    wv.api_workflow_json,
                    r.recipe_yaml
                 FROM workflows w
                 INNER JOIN workflow_versions wv ON wv.workflow_id = w.id
                 INNER JOIN recipes r ON r.workflow_version_id = wv.id
                 WHERE (wv.id, r.id) IN (",
            );
            for (index, (workflow_version_id, recipe_id)) in chunk.iter().enumerate() {
                if index > 0 {
                    query.push(", ");
                }
                query
                    .push("(")
                    .push_bind(workflow_version_id.clone())
                    .push(", ")
                    .push_bind(recipe_id.clone())
                    .push(")");
            }
            query.push(") ORDER BY wv.id ASC, r.id ASC");

            let rows = query
                .build_query_as::<DefinitionRow>()
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_error)?;
            definitions.extend(
                rows.into_iter()
                    .map(DefinitionRow::try_into_domain)
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }

        Ok(definitions)
    }

    async fn list_available(&self) -> Result<Vec<AvailableGenerationDefinition>, RepositoryError> {
        let rows = sqlx::query_as::<_, AvailableDefinitionRow>(
            "SELECT
                w.id AS workflow_id,
                wv.id AS workflow_version_id,
                r.id AS recipe_id,
                r.version AS recipe_version,
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
             ORDER BY w.name ASC, wv.version ASC, r.created_at DESC, r.id DESC",
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
                recipe_version: row.recipe_version,
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
    workflow_version: String,
    workflow_sha256: String,
    package_name: Option<String>,
    package_source_path: Option<String>,
    recipe_version: String,
    recipe_sha256: String,
    api_workflow_json: String,
    recipe_yaml: String,
}

#[derive(sqlx::FromRow)]
struct AvailableDefinitionRow {
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
    recipe_version: String,
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
            workflow_version: self.workflow_version,
            workflow_sha256: self.workflow_sha256,
            recipe_version: self.recipe_version,
            recipe_sha256: self.recipe_sha256,
            package_name: self.package_name,
            package_source_path: self.package_source_path,
            workflow_json,
            recipe_yaml: self.recipe_yaml,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteGenerationDefinitionRepository, FIND_MANY_CHUNK_SIZE};
    use crate::application::ports::GenerationDefinitionRepository;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use sqlx::SqlitePool;
    use std::collections::HashSet;
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
    async fn find_many_returns_500_pairs_with_bounded_chunking() {
        let (_directory, pool, repository) = setup().await;
        let mut pairs = Vec::with_capacity(500);
        for index in 0..500 {
            let workflow_id = format!("bulk-workflow-{index}");
            let workflow_version_id = format!("bulk-workflow-version-{index}");
            let recipe_id = format!("bulk-recipe-{index}");
            sqlx::query(
                "INSERT INTO workflows
                    (id, name, category, mode, current_version_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&workflow_id)
            .bind(format!("Bulk Workflow {index}"))
            .bind("test")
            .bind("image")
            .bind(&workflow_version_id)
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .expect("bulk workflow fixture should insert");
            sqlx::query(
                "INSERT INTO workflow_versions
                    (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&workflow_version_id)
            .bind(&workflow_id)
            .bind("1")
            .bind("{}")
            .bind(format!("bulk-workflow-sha-{index}"))
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .expect("bulk workflow version fixture should insert");
            sqlx::query(
                "INSERT INTO recipes
                    (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&recipe_id)
            .bind(&workflow_version_id)
            .bind("1")
            .bind(1)
            .bind("schema_version: 1")
            .bind(format!("bulk-recipe-sha-{index}"))
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .expect("bulk recipe fixture should insert");
            pairs.push((workflow_version_id, recipe_id));
        }

        assert_eq!(pairs.chunks(FIND_MANY_CHUNK_SIZE).len(), 3);
        assert!(pairs.chunks(FIND_MANY_CHUNK_SIZE).len() <= 5);
        let definitions = repository
            .find_many(&pairs)
            .await
            .expect("bulk definition lookup should succeed");

        let returned_pairs = definitions
            .iter()
            .map(|definition| {
                (
                    definition.workflow_version_id.clone(),
                    definition.recipe_id.clone(),
                )
            })
            .collect::<HashSet<_>>();
        assert_eq!(returned_pairs.len(), pairs.len());
        assert!(pairs.iter().all(|pair| returned_pairs.contains(pair)));
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
    async fn lists_all_recipes_for_each_current_workflow_version() {
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

        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].recipe_id, "recipe-2");
        assert_eq!(definitions[0].recipe_version, "1.0.1");
        assert_eq!(definitions[1].recipe_id, "recipe-1");
        assert_eq!(definitions[1].recipe_version, "1");
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
