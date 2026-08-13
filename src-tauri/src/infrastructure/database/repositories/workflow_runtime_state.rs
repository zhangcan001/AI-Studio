use crate::application::ports::{
    RepositoryError, WorkflowRuntimeState, WorkflowRuntimeStateRepository,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteWorkflowRuntimeStateRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowRuntimeStateRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkflowRuntimeStateRepository for SqliteWorkflowRuntimeStateRepository {
    async fn is_enabled(&self, workflow_version_id: &str) -> Result<bool, RepositoryError> {
        let enabled = sqlx::query_scalar::<_, i64>(
            "SELECT enabled FROM workflow_runtime_states WHERE workflow_version_id = ?",
        )
        .bind(workflow_version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        Ok(enabled.unwrap_or(1) != 0)
    }

    async fn set_enabled(
        &self,
        workflow_version_id: &str,
        enabled: bool,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions WHERE id = ?")
                .bind(workflow_version_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|error| RepositoryError::database(error.to_string()))?;
        if exists == 0 {
            return Err(RepositoryError::not_found(
                "workflow version",
                workflow_version_id,
            ));
        }

        let current = self.find_state(workflow_version_id).await?;
        self.set_archived(
            workflow_version_id,
            current.as_ref().is_some_and(|state| state.archived),
            enabled,
            current.and_then(|state| state.archived_at),
            updated_at,
        )
        .await
    }

    async fn find_state(
        &self,
        workflow_version_id: &str,
    ) -> Result<Option<WorkflowRuntimeState>, RepositoryError> {
        let row = sqlx::query_as::<_, RuntimeStateRow>(
            "SELECT workflow_version_id, enabled, archived, archived_at, updated_at
             FROM workflow_runtime_states
             WHERE workflow_version_id = ?",
        )
        .bind(workflow_version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        row.map(RuntimeStateRow::try_into_domain).transpose()
    }

    async fn set_archived(
        &self,
        workflow_version_id: &str,
        archived: bool,
        enabled: bool,
        archived_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions WHERE id = ?")
                .bind(workflow_version_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|error| RepositoryError::database(error.to_string()))?;
        if exists == 0 {
            return Err(RepositoryError::not_found(
                "workflow version",
                workflow_version_id,
            ));
        }

        sqlx::query(
            "INSERT INTO workflow_runtime_states (
                workflow_version_id, enabled, archived, archived_at, updated_at
             ) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(workflow_version_id) DO UPDATE SET
                 enabled = excluded.enabled,
                 archived = excluded.archived,
                 archived_at = excluded.archived_at,
                 updated_at = excluded.updated_at",
        )
        .bind(workflow_version_id)
        .bind(i64::from(enabled))
        .bind(i64::from(archived))
        .bind(archived_at.map(|value| value.to_rfc3339()))
        .bind(updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        Ok(())
    }

    async fn list_states(&self) -> Result<Vec<WorkflowRuntimeState>, RepositoryError> {
        let rows = sqlx::query_as::<_, RuntimeStateRow>(
            "SELECT workflow_version_id, enabled, archived, archived_at, updated_at
             FROM workflow_runtime_states
             ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        rows.into_iter()
            .map(RuntimeStateRow::try_into_domain)
            .collect()
    }
}

#[derive(sqlx::FromRow)]
struct RuntimeStateRow {
    workflow_version_id: String,
    enabled: i64,
    archived: i64,
    archived_at: Option<String>,
    updated_at: String,
}

impl RuntimeStateRow {
    fn try_into_domain(self) -> Result<WorkflowRuntimeState, RepositoryError> {
        let updated_at = DateTime::parse_from_rfc3339(&self.updated_at)
            .map_err(|error| {
                RepositoryError::serialization("workflow runtime updated_at", error.to_string())
            })?
            .with_timezone(&Utc);
        let archived_at = self
            .archived_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|error| {
                RepositoryError::serialization("workflow runtime archived_at", error.to_string())
            })?
            .map(|value| value.with_timezone(&Utc));
        Ok(WorkflowRuntimeState {
            workflow_version_id: self.workflow_version_id,
            enabled: self.enabled != 0,
            archived: self.archived != 0,
            archived_at,
            updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowRuntimeStateRepository;
    use crate::application::ports::WorkflowRuntimeStateRepository;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn missing_state_is_enabled_and_explicit_state_round_trips() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteWorkflowRuntimeStateRepository::new(pool);

        assert!(repository.is_enabled("workflow-version-1").await.unwrap());
        repository
            .set_enabled("workflow-version-1", false, Utc::now())
            .await
            .unwrap();
        assert!(!repository.is_enabled("workflow-version-1").await.unwrap());
        assert_eq!(repository.list_states().await.unwrap().len(), 1);

        repository
            .set_archived(
                "workflow-version-1",
                true,
                false,
                Some(Utc::now()),
                Utc::now(),
            )
            .await
            .unwrap();
        let archived = repository
            .find_state("workflow-version-1")
            .await
            .unwrap()
            .unwrap();
        assert!(archived.archived);
        assert!(!archived.enabled);
        assert!(archived.archived_at.is_some());

        repository
            .set_archived("workflow-version-1", false, false, None, Utc::now())
            .await
            .unwrap();
        assert!(
            !repository
                .find_state("workflow-version-1")
                .await
                .unwrap()
                .unwrap()
                .archived
        );
    }
}
