use super::{format_datetime, map_sqlx_error, parse_datetime, parse_optional_datetime};
use crate::application::ports::{
    RepairJobRecord, RepairJobRepository, RepositoryError, REPAIR_JOB_RUNNING,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteRepairJobRepository {
    pool: SqlitePool,
}

impl SqliteRepairJobRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct RepairJobRow {
    job_id: String,
    started_at: String,
    completed_at: Option<String>,
    status: String,
    summary_json: String,
}

impl RepairJobRow {
    fn try_into_record(self) -> Result<RepairJobRecord, RepositoryError> {
        Ok(RepairJobRecord {
            started_at: parse_datetime("app_repair_jobs.started_at", &self.started_at)?,
            completed_at: parse_optional_datetime(
                "app_repair_jobs.completed_at",
                self.completed_at.as_deref(),
            )?,
            job_id: self.job_id,
            status: self.status,
            summary_json: self.summary_json,
        })
    }
}

#[async_trait]
impl RepairJobRepository for SqliteRepairJobRepository {
    async fn list(&self) -> Result<Vec<RepairJobRecord>, RepositoryError> {
        sqlx::query_as::<_, RepairJobRow>(
            "SELECT job_id, started_at, completed_at, status, summary_json
             FROM app_repair_jobs ORDER BY job_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(RepairJobRow::try_into_record)
        .collect()
    }

    async fn find(&self, job_id: &str) -> Result<Option<RepairJobRecord>, RepositoryError> {
        sqlx::query_as::<_, RepairJobRow>(
            "SELECT job_id, started_at, completed_at, status, summary_json
             FROM app_repair_jobs WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(RepairJobRow::try_into_record)
        .transpose()
    }

    async fn mark_started(
        &self,
        job_id: &str,
        started_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO app_repair_jobs (job_id, started_at, completed_at, status, summary_json)
             VALUES (?, ?, NULL, ?, '{}')
             ON CONFLICT(job_id) DO UPDATE SET
                started_at = excluded.started_at,
                completed_at = NULL,
                status = excluded.status",
        )
        .bind(job_id)
        .bind(format_datetime(started_at))
        .bind(REPAIR_JOB_RUNNING)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn mark_finished(
        &self,
        job_id: &str,
        status: &str,
        completed_at: DateTime<Utc>,
        summary_json: &str,
    ) -> Result<(), RepositoryError> {
        let updated = sqlx::query(
            "UPDATE app_repair_jobs
             SET status = ?, completed_at = ?, summary_json = ?
             WHERE job_id = ?",
        )
        .bind(status)
        .bind(format_datetime(completed_at))
        .bind(summary_json)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::not_found("repair job", job_id));
        }
        Ok(())
    }

    async fn reset(&self, job_id: &str) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM app_repair_jobs WHERE job_id = ?")
            .bind(job_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteRepairJobRepository;
    use crate::application::ports::{RepairJobRepository, REPAIR_JOB_COMPLETED};
    use crate::infrastructure::database::initialize;
    use chrono::{TimeZone, Utc};
    use tempfile::tempdir;

    #[tokio::test]
    async fn repair_job_markers_round_trip_and_reset() {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        let repository = SqliteRepairJobRepository::new(pool);
        let started = Utc.with_ymd_and_hms(2026, 9, 28, 8, 0, 0).unwrap();
        assert!(repository.find("job_v1").await.unwrap().is_none());
        repository.mark_started("job_v1", started).await.unwrap();
        assert_eq!(
            repository.find("job_v1").await.unwrap().unwrap().status,
            "RUNNING"
        );
        repository
            .mark_finished("job_v1", REPAIR_JOB_COMPLETED, started, r#"{"repaired":1}"#)
            .await
            .unwrap();
        let record = repository.find("job_v1").await.unwrap().unwrap();
        assert_eq!(record.status, REPAIR_JOB_COMPLETED);
        assert_eq!(record.summary_json, r#"{"repaired":1}"#);
        assert_eq!(repository.list().await.unwrap().len(), 1);
        repository.reset("job_v1").await.unwrap();
        assert!(repository.find("job_v1").await.unwrap().is_none());
    }
}
