use super::{format_datetime, map_sqlx_error};
use crate::application::ports::{
    RepositoryError, WorkflowLibraryRepository, WorkflowPackageRecord, WorkflowPackageRegistration,
};
use async_trait::async_trait;
use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct SqliteWorkflowLibraryRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowLibraryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkflowLibraryRepository for SqliteWorkflowLibraryRepository {
    async fn register_package(
        &self,
        package: &WorkflowPackageRecord,
    ) -> Result<WorkflowPackageRegistration, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let registration = register_package(&mut transaction, package).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(registration)
    }
}

async fn register_package(
    transaction: &mut Transaction<'_, Sqlite>,
    package: &WorkflowPackageRecord,
) -> Result<WorkflowPackageRegistration, RepositoryError> {
    sqlx::query(
        "INSERT INTO workflows (
            id, name, category, mode, current_version_id, created_at, updated_at
        ) VALUES (?, ?, ?, ?, NULL, ?, ?)
        ON CONFLICT(id) DO NOTHING",
    )
    .bind(&package.workflow_id)
    .bind(&package.name)
    .bind(&package.category)
    .bind(&package.mode)
    .bind(format_datetime(package.created_at))
    .bind(format_datetime(package.created_at))
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    let existing_version = sqlx::query_as::<_, WorkflowVersionRow>(
        "SELECT id, workflow_sha256
         FROM workflow_versions
         WHERE workflow_id = ? AND version = ?",
    )
    .bind(&package.workflow_id)
    .bind(&package.workflow_version)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    let (workflow_version_id, inserted_workflow_version) = match existing_version {
        Some(existing) if existing.workflow_sha256 == package.workflow_sha256 => {
            (existing.id, false)
        }
        Some(_) => {
            return Err(RepositoryError::workflow_version_conflict(
                &package.workflow_id,
                &package.workflow_version,
            ));
        }
        None => {
            let id = format!("wfv_{}", Uuid::new_v4());
            sqlx::query(
                "INSERT INTO workflow_versions (
                    id, workflow_id, version, api_workflow_json, workflow_sha256,
                    package_name, package_source_path, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(&package.workflow_id)
            .bind(&package.workflow_version)
            .bind(package.workflow_json.to_string())
            .bind(&package.workflow_sha256)
            .bind(&package.package_name)
            .bind(&package.package_source_path)
            .bind(format_datetime(package.created_at))
            .execute(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
            (id, true)
        }
    };

    sqlx::query(
        "UPDATE workflow_versions
         SET package_name = ?, package_source_path = ?
         WHERE id = ?",
    )
    .bind(&package.package_name)
    .bind(&package.package_source_path)
    .bind(&workflow_version_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    let existing_recipe = sqlx::query_as::<_, RecipeVersionRow>(
        "SELECT recipe_sha256
         FROM recipes
         WHERE workflow_version_id = ? AND version = ?",
    )
    .bind(&workflow_version_id)
    .bind(&package.recipe_version)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    let registration = match existing_recipe {
        Some(existing) if existing.recipe_sha256 == package.recipe_sha256 => {
            if inserted_workflow_version {
                WorkflowPackageRegistration::Inserted
            } else {
                WorkflowPackageRegistration::Reused
            }
        }
        Some(_) => {
            return Err(RepositoryError::recipe_version_conflict(
                &workflow_version_id,
                &package.recipe_version,
            ))
        }
        None => {
            sqlx::query(
                "INSERT INTO recipes (
                    id, workflow_version_id, version, schema_version,
                    recipe_yaml, recipe_sha256, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("rcp_{}", Uuid::new_v4()))
            .bind(&workflow_version_id)
            .bind(&package.recipe_version)
            .bind(i64::from(package.recipe_schema_version))
            .bind(&package.recipe_yaml)
            .bind(&package.recipe_sha256)
            .bind(format_datetime(package.created_at))
            .execute(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
            WorkflowPackageRegistration::Inserted
        }
    };

    select_latest_current_version(transaction, &package.workflow_id, package.created_at).await?;
    Ok(registration)
}

async fn select_latest_current_version(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_id: &str,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), RepositoryError> {
    #[derive(sqlx::FromRow)]
    struct VersionCandidate {
        id: String,
        version: String,
    }

    let versions = sqlx::query_as::<_, VersionCandidate>(
        "SELECT id, version FROM workflow_versions WHERE workflow_id = ?",
    )
    .bind(workflow_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    let Some(latest) = versions.into_iter().max_by(|left, right| {
        compare_semver(&left.version, &right.version).then_with(|| left.id.cmp(&right.id))
    }) else {
        return Ok(());
    };

    sqlx::query(
        "UPDATE workflows
         SET current_version_id = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(latest.id)
    .bind(format_datetime(updated_at))
    .bind(workflow_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

fn compare_semver(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parts = left
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    let right_parts = right
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    (0..3)
        .map(|index| {
            left_parts
                .get(index)
                .copied()
                .unwrap_or(0)
                .cmp(&right_parts.get(index).copied().unwrap_or(0))
        })
        .find(|ordering| *ordering != std::cmp::Ordering::Equal)
        .unwrap_or_else(|| left.cmp(right))
}

#[derive(sqlx::FromRow)]
struct WorkflowVersionRow {
    id: String,
    workflow_sha256: String,
}

#[derive(sqlx::FromRow)]
struct RecipeVersionRow {
    recipe_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowLibraryRepository;
    use crate::application::ports::{
        WorkflowLibraryRepository, WorkflowPackageRecord, WorkflowPackageRegistration,
    };
    use crate::infrastructure::database::initialize;
    use chrono::Utc;
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, SqliteWorkflowLibraryRepository) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        (
            directory,
            pool.clone(),
            SqliteWorkflowLibraryRepository::new(pool),
        )
    }

    fn package(workflow_hash: &str, recipe_hash: &str) -> WorkflowPackageRecord {
        WorkflowPackageRecord {
            workflow_id: "wfl_test".to_owned(),
            package_name: "test-package".to_owned(),
            package_source_path: None,
            name: "Test Workflow".to_owned(),
            category: "image".to_owned(),
            mode: "text_to_image".to_owned(),
            workflow_version: "1.0.0".to_owned(),
            workflow_json: json!({"3": {"inputs": {}, "class_type": "KSampler"}}),
            workflow_sha256: workflow_hash.to_owned(),
            recipe_version: "1.0.0".to_owned(),
            recipe_schema_version: 1,
            recipe_yaml: "schema_version: 1".to_owned(),
            recipe_sha256: recipe_hash.to_owned(),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn registers_then_reuses_same_package() {
        let (_directory, pool, repository) = setup().await;
        let package = package("workflow-hash", "recipe-hash");
        assert_eq!(
            repository.register_package(&package).await.unwrap(),
            WorkflowPackageRegistration::Inserted
        );
        assert_eq!(
            repository.register_package(&package).await.unwrap(),
            WorkflowPackageRegistration::Reused
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn rejects_same_workflow_version_with_different_hash() {
        let (_directory, _pool, repository) = setup().await;
        repository
            .register_package(&package("workflow-hash", "recipe-hash"))
            .await
            .unwrap();
        let error = repository
            .register_package(&package("different-workflow", "recipe-hash"))
            .await
            .expect_err("version conflict should be reported");
        assert!(error.to_string().contains("WORKFLOW_VERSION_CONFLICT"));
    }

    #[tokio::test]
    async fn selects_highest_semver_as_current_even_when_packages_arrive_out_of_order() {
        let (_directory, pool, repository) = setup().await;
        let mut v110 = package("workflow-1.10.0", "recipe-1.10.0");
        v110.workflow_version = "1.10.0".to_owned();
        let mut v190 = package("workflow-1.9.0", "recipe-1.9.0");
        v190.workflow_version = "1.9.0".to_owned();

        repository.register_package(&v110).await.unwrap();
        repository.register_package(&v190).await.unwrap();

        let current = sqlx::query_scalar::<_, String>(
            "SELECT current_version_id FROM workflows WHERE id = ?",
        )
        .bind("wfl_test")
        .fetch_one(&pool)
        .await
        .unwrap();
        let current_version =
            sqlx::query_scalar::<_, String>("SELECT version FROM workflow_versions WHERE id = ?")
                .bind(current)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(current_version, "1.10.0");
    }

    #[tokio::test]
    async fn rejects_same_recipe_version_with_different_hash() {
        let (_directory, _pool, repository) = setup().await;
        repository
            .register_package(&package("workflow-hash", "recipe-hash"))
            .await
            .unwrap();
        let mut changed = package("workflow-hash", "different-recipe");
        changed.recipe_yaml = "schema_version: 1\nname: changed".to_owned();
        let error = repository
            .register_package(&changed)
            .await
            .expect_err("recipe conflict should be reported");
        assert!(error.to_string().contains("RECIPE_VERSION_CONFLICT"));
    }
}
