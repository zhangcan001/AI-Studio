use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    RepositoryError, WorkflowRuntimeArtifactRecord, WorkflowRuntimeArtifactRepository,
    RUNTIME_ARTIFACT_SOURCE_PRODUCT, RUNTIME_ARTIFACT_SOURCE_USER,
};
use async_trait::async_trait;
use sqlx::SqlitePool;

const ARTIFACT_SELECT: &str = "SELECT
    id, workflow_version_id, recipe_id, package_name, source_kind,
    package_source_path, workflow_sha256, recipe_sha256, created_at
    FROM workflow_runtime_artifacts";

#[derive(Clone)]
pub struct SqliteWorkflowRuntimeArtifactRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowRuntimeArtifactRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkflowRuntimeArtifactRepository for SqliteWorkflowRuntimeArtifactRepository {
    async fn find_exact(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        package_name: &str,
    ) -> Result<Option<WorkflowRuntimeArtifactRecord>, RepositoryError> {
        let row = sqlx::query_as::<_, ArtifactRow>(&format!(
            "{ARTIFACT_SELECT}
             WHERE workflow_version_id = ? AND recipe_id = ? AND package_name = ?"
        ))
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(package_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(ArtifactRow::try_into_record).transpose()
    }

    async fn list(&self) -> Result<Vec<WorkflowRuntimeArtifactRecord>, RepositoryError> {
        self.list_filtered(None, None).await
    }

    async fn list_for_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<Vec<WorkflowRuntimeArtifactRecord>, RepositoryError> {
        self.list_filtered(Some(workflow_version_id), None).await
    }

    async fn list_for_recipe(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Vec<WorkflowRuntimeArtifactRecord>, RepositoryError> {
        self.list_filtered(Some(workflow_version_id), Some(recipe_id))
            .await
    }

    async fn upsert(
        &self,
        artifact: &WorkflowRuntimeArtifactRecord,
    ) -> Result<(), RepositoryError> {
        validate_artifact(artifact)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;

        let hashes = sqlx::query_as::<_, DefinitionHashes>(
            "SELECT wv.workflow_sha256, r.recipe_sha256
             FROM workflow_versions wv
             INNER JOIN recipes r ON r.workflow_version_id = wv.id
             WHERE wv.id = ? AND r.id = ?",
        )
        .bind(&artifact.workflow_version_id)
        .bind(&artifact.recipe_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .ok_or_else(|| {
            RepositoryError::not_found(
                "workflow version/recipe",
                format!("{}/{}", artifact.workflow_version_id, artifact.recipe_id),
            )
        })?;
        if hashes.workflow_sha256 != artifact.workflow_sha256 {
            return Err(RepositoryError::integrity(
                "runtime artifact workflow_sha256 does not match workflow version",
            ));
        }
        if hashes.recipe_sha256 != artifact.recipe_sha256 {
            return Err(RepositoryError::integrity(
                "runtime artifact recipe_sha256 does not match recipe",
            ));
        }

        let existing = sqlx::query_as::<_, ArtifactRow>(&format!(
            "{ARTIFACT_SELECT}
             WHERE workflow_version_id = ? AND recipe_id = ? AND package_name = ?"
        ))
        .bind(&artifact.workflow_version_id)
        .bind(&artifact.recipe_id)
        .bind(&artifact.package_name)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if let Some(existing) = existing {
            if existing.matches_immutable(artifact) {
                transaction.commit().await.map_err(map_sqlx_error)?;
                return Ok(());
            }
            return Err(RepositoryError::integrity(
                "runtime artifact identity already exists with different immutable metadata",
            ));
        }

        let id_taken = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM workflow_runtime_artifacts WHERE id = ?",
        )
        .bind(&artifact.id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if id_taken > 0 {
            return Err(RepositoryError::integrity(
                "runtime artifact id already belongs to another exact mapping",
            ));
        }

        sqlx::query(
            "INSERT INTO workflow_runtime_artifacts (
                id, workflow_version_id, recipe_id, package_name, source_kind,
                package_source_path, workflow_sha256, recipe_sha256, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&artifact.id)
        .bind(&artifact.workflow_version_id)
        .bind(&artifact.recipe_id)
        .bind(&artifact.package_name)
        .bind(&artifact.source_kind)
        .bind(&artifact.package_source_path)
        .bind(&artifact.workflow_sha256)
        .bind(&artifact.recipe_sha256)
        .bind(format_datetime(artifact.created_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        transaction.commit().await.map_err(map_sqlx_error)
    }
}

impl SqliteWorkflowRuntimeArtifactRepository {
    async fn list_filtered(
        &self,
        workflow_version_id: Option<&str>,
        recipe_id: Option<&str>,
    ) -> Result<Vec<WorkflowRuntimeArtifactRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, ArtifactRow>(&format!(
            "{ARTIFACT_SELECT}
             WHERE (? IS NULL OR workflow_version_id = ?)
               AND (? IS NULL OR recipe_id = ?)
             ORDER BY workflow_version_id ASC, recipe_id ASC, package_name ASC, id ASC"
        ))
        .bind(workflow_version_id)
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(recipe_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ArtifactRow::try_into_record).collect()
    }
}

fn validate_artifact(artifact: &WorkflowRuntimeArtifactRecord) -> Result<(), RepositoryError> {
    for (field, value) in [
        ("id", artifact.id.as_str()),
        ("workflow_version_id", artifact.workflow_version_id.as_str()),
        ("recipe_id", artifact.recipe_id.as_str()),
        ("package_name", artifact.package_name.as_str()),
        ("workflow_sha256", artifact.workflow_sha256.as_str()),
        ("recipe_sha256", artifact.recipe_sha256.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(RepositoryError::integrity(format!(
                "runtime artifact {field} must not be empty"
            )));
        }
    }
    if !matches!(
        artifact.source_kind.as_str(),
        RUNTIME_ARTIFACT_SOURCE_PRODUCT | RUNTIME_ARTIFACT_SOURCE_USER
    ) {
        return Err(RepositoryError::integrity(
            "runtime artifact source_kind must be PRODUCT or USER",
        ));
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ArtifactRow {
    id: String,
    workflow_version_id: String,
    recipe_id: String,
    package_name: String,
    source_kind: String,
    package_source_path: Option<String>,
    workflow_sha256: String,
    recipe_sha256: String,
    created_at: String,
}

impl ArtifactRow {
    fn try_into_record(self) -> Result<WorkflowRuntimeArtifactRecord, RepositoryError> {
        Ok(WorkflowRuntimeArtifactRecord {
            id: self.id,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            package_name: self.package_name,
            source_kind: self.source_kind,
            package_source_path: self.package_source_path,
            workflow_sha256: self.workflow_sha256,
            recipe_sha256: self.recipe_sha256,
            created_at: parse_datetime("workflow runtime artifact created_at", &self.created_at)?,
        })
    }

    fn matches_immutable(&self, artifact: &WorkflowRuntimeArtifactRecord) -> bool {
        self.id == artifact.id
            && self.source_kind == artifact.source_kind
            && self.package_source_path == artifact.package_source_path
            && self.workflow_sha256 == artifact.workflow_sha256
            && self.recipe_sha256 == artifact.recipe_sha256
    }
}

#[derive(sqlx::FromRow)]
struct DefinitionHashes {
    workflow_sha256: String,
    recipe_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowRuntimeArtifactRepository;
    use crate::application::ports::{
        WorkflowRuntimeArtifactRecord, WorkflowRuntimeArtifactRepository,
    };
    use crate::infrastructure::database::initialize;
    use chrono::Utc;
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    async fn setup() -> (SqlitePool, SqliteWorkflowRuntimeArtifactRepository) {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        sqlx::query(
            "INSERT INTO workflows
             (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('artifact-workflow', 'Workflow', 'video', 'text_to_video', NULL, ?, ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, package_name,
              created_at)
             VALUES ('artifact-version', 'artifact-workflow', '1.0.0', '{}', 'workflow-sha',
                     'legacy-package', ?)",
        )
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        for (id, version, hash) in [
            ("artifact-recipe-1", "1.0.0", "recipe-sha-1"),
            ("artifact-recipe-2", "1.1.0", "recipe-sha-2"),
        ] {
            sqlx::query(
                "INSERT INTO recipes
                 (id, workflow_version_id, version, schema_version, recipe_yaml,
                  recipe_sha256, created_at)
                 VALUES (?, 'artifact-version', ?, 1, 'schema_version: 1', ?, ?)",
            )
            .bind(id)
            .bind(version)
            .bind(hash)
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        }
        (
            pool.clone(),
            SqliteWorkflowRuntimeArtifactRepository::new(pool),
        )
    }

    fn artifact(id: &str, recipe_id: &str, package_name: &str) -> WorkflowRuntimeArtifactRecord {
        WorkflowRuntimeArtifactRecord {
            id: id.to_owned(),
            workflow_version_id: "artifact-version".to_owned(),
            recipe_id: recipe_id.to_owned(),
            package_name: package_name.to_owned(),
            source_kind: "USER".to_owned(),
            package_source_path: Some(format!("C:/{package_name}")),
            workflow_sha256: "workflow-sha".to_owned(),
            recipe_sha256: if recipe_id.ends_with('1') {
                "recipe-sha-1".to_owned()
            } else {
                "recipe-sha-2".to_owned()
            },
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn exact_recipe_mapping_allows_multiple_packages_without_last_write_wins() {
        let (pool, repository) = setup().await;
        repository
            .upsert(&artifact("artifact-a", "artifact-recipe-1", "package-a"))
            .await
            .unwrap();
        repository
            .upsert(&artifact("artifact-b", "artifact-recipe-2", "package-b"))
            .await
            .unwrap();

        let package_a = repository
            .find_exact("artifact-version", "artifact-recipe-1", "package-a")
            .await
            .unwrap()
            .unwrap();
        let package_b = repository
            .find_exact("artifact-version", "artifact-recipe-2", "package-b")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(package_a.package_name, "package-a");
        assert_eq!(package_b.package_name, "package-b");
        assert_eq!(repository.list().await.unwrap().len(), 2);
        assert_eq!(
            repository
                .list_for_workflow_version("artifact-version")
                .await
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT package_name FROM workflow_versions WHERE id = 'artifact-version'",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "legacy-package"
        );
    }

    #[tokio::test]
    async fn exact_mapping_is_immutable_and_idempotent() {
        let (_pool, repository) = setup().await;
        let first = artifact("artifact-a", "artifact-recipe-1", "package-a");
        repository.upsert(&first).await.unwrap();
        repository.upsert(&first).await.unwrap();

        let mut changed = first;
        changed.package_source_path = Some("C:/changed".to_owned());
        let error = repository
            .upsert(&changed)
            .await
            .expect_err("immutable mapping should reject changed metadata");
        assert!(error.to_string().contains("immutable"));
    }
}
