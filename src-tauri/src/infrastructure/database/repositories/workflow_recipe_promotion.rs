use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    RepositoryError, WorkflowRecipePromotionRecord, WorkflowRecipePromotionRepository,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Clone)]
pub struct SqliteWorkflowRecipePromotionRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowRecipePromotionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkflowRecipePromotionRepository for SqliteWorkflowRecipePromotionRepository {
    async fn list(&self) -> Result<Vec<WorkflowRecipePromotionRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, PromotionRow>(
            "SELECT workflow_version_id, recipe_id, promoted_at
             FROM workflow_recipe_promotions
             ORDER BY workflow_version_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(PromotionRow::try_into_record)
            .collect()
    }

    async fn promote(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        promoted_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        promote_in_transaction(
            &mut transaction,
            workflow_version_id,
            recipe_id,
            promoted_at,
        )
        .await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }
}

async fn promote_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_version_id: &str,
    recipe_id: &str,
    promoted_at: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    let version_exists =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions WHERE id = ?")
            .bind(workflow_version_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
    if version_exists == 0 {
        return Err(RepositoryError::not_found(
            "workflow version",
            workflow_version_id,
        ));
    }

    let archived = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(archived, 0)
         FROM workflow_runtime_states
         WHERE workflow_version_id = ?",
    )
    .bind(workflow_version_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .unwrap_or(0);
    if archived != 0 {
        return Err(RepositoryError::integrity(
            "cannot promote a recipe in an archived workflow version",
        ));
    }

    let recipe_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM recipes
         WHERE id = ? AND workflow_version_id = ?",
    )
    .bind(recipe_id)
    .bind(workflow_version_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if recipe_exists == 0 {
        return Err(RepositoryError::not_found(
            "recipe for workflow version",
            format!("{workflow_version_id}:{recipe_id}"),
        ));
    }

    sqlx::query(
        "INSERT INTO workflow_recipe_promotions (workflow_version_id, recipe_id, promoted_at)
         VALUES (?, ?, ?)
         ON CONFLICT(workflow_version_id) DO UPDATE SET
             recipe_id = excluded.recipe_id,
             promoted_at = excluded.promoted_at",
    )
    .bind(workflow_version_id)
    .bind(recipe_id)
    .bind(format_datetime(promoted_at))
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct PromotionRow {
    workflow_version_id: String,
    recipe_id: String,
    promoted_at: String,
}

impl PromotionRow {
    fn try_into_record(self) -> Result<WorkflowRecipePromotionRecord, RepositoryError> {
        Ok(WorkflowRecipePromotionRecord {
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            promoted_at: parse_datetime(
                "workflow recipe promotion promoted_at",
                &self.promoted_at,
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowRecipePromotionRepository;
    use crate::application::ports::WorkflowRecipePromotionRepository;
    use crate::infrastructure::database::initialize;
    use chrono::{TimeZone, Utc};
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    async fn setup() -> (SqlitePool, SqliteWorkflowRecipePromotionRepository) {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        let timestamp = "2026-09-10T00:00:00Z";
        sqlx::query(
            "INSERT INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('promotion-workflow', 'Promotion', 'video', 'text_to_video', 'promotion-version', ?, ?)",
        )
        .bind(timestamp)
        .bind(timestamp)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('promotion-version', 'promotion-workflow', '1.0.0', '{}', 'workflow-sha', ?)",
        )
        .bind(timestamp)
        .execute(&pool)
        .await
        .unwrap();
        for recipe_id in ["recipe-a", "recipe-b"] {
            sqlx::query(
                "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
                 VALUES (?, 'promotion-version', ?, 1, 'schema_version: 1', ?, ?)",
            )
            .bind(recipe_id)
            .bind(if recipe_id == "recipe-a" { "1.0.0" } else { "2.0.0" })
            .bind(format!("{recipe_id}-sha"))
            .bind(timestamp)
            .execute(&pool)
            .await
            .unwrap();
        }
        (
            pool.clone(),
            SqliteWorkflowRecipePromotionRepository::new(pool),
        )
    }

    #[tokio::test]
    async fn promotion_is_exact_idempotent_and_replaces_atomically() {
        let (_pool, repository) = setup().await;
        let at = Utc.with_ymd_and_hms(2026, 9, 10, 0, 0, 0).unwrap();
        repository
            .promote("promotion-version", "recipe-a", at)
            .await
            .unwrap();
        repository
            .promote("promotion-version", "recipe-a", at)
            .await
            .unwrap();
        repository
            .promote("promotion-version", "recipe-b", at)
            .await
            .unwrap();
        let rows = repository.list().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].workflow_version_id, "promotion-version");
        assert_eq!(rows[0].recipe_id, "recipe-b");
    }

    #[tokio::test]
    async fn rejects_unknown_recipe_and_archived_version() {
        let (pool, repository) = setup().await;
        let at = Utc.with_ymd_and_hms(2026, 9, 10, 0, 0, 0).unwrap();
        let error = repository
            .promote("promotion-version", "other", at)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("recipe for workflow version"));
        sqlx::query(
            "INSERT INTO workflow_runtime_states (workflow_version_id, enabled, updated_at, archived, archived_at)
             VALUES ('promotion-version', 0, ?, 1, ?)",
        )
        .bind(at.to_rfc3339())
        .bind(at.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        let error = repository
            .promote("promotion-version", "recipe-a", at)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("archived workflow version"));
    }

    #[tokio::test]
    async fn deleting_workflow_version_cleans_promotion() {
        let (pool, repository) = setup().await;
        let at = Utc.with_ymd_and_hms(2026, 9, 10, 0, 0, 0).unwrap();
        repository
            .promote("promotion-version", "recipe-a", at)
            .await
            .unwrap();
        sqlx::query("DELETE FROM recipes WHERE workflow_version_id = 'promotion-version'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM workflow_versions WHERE id = 'promotion-version'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(repository.list().await.unwrap().is_empty());
    }
}
