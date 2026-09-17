use crate::application::ports::{
    ArtifactRecord, ArtifactRepository, ArtifactReviewQueueFilter, ArtifactReviewQueuePage,
    AssetRepository, RepositoryError,
};
use crate::domain::{Artifact, ArtifactReview, ArtifactReviewDecision, AssetId, TaskId};
use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use super::{
    asset::SqliteAssetRepository, format_datetime, map_domain_error, map_sqlx_error, parse_datetime,
};

#[derive(Clone)]
pub struct SqliteArtifactRepository {
    pool: SqlitePool,
}

impl SqliteArtifactRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn hydrate_records(
        &self,
        mappings: Vec<ArtifactMappingRow>,
    ) -> Result<Vec<ArtifactRecord>, RepositoryError> {
        if mappings.is_empty() {
            return Ok(Vec::new());
        }
        let asset_ids = mappings
            .iter()
            .map(|mapping| mapping.asset_id.clone())
            .collect::<Vec<_>>();
        let asset_ids = asset_ids
            .into_iter()
            .map(AssetId::parse)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_domain_error("artifact asset_id", error))?;
        let assets = SqliteAssetRepository::new(self.pool.clone())
            .find_many_by_ids(&asset_ids)
            .await?;
        let assets_by_id = assets
            .into_iter()
            .map(|asset| (asset.id.as_str().to_owned(), asset))
            .collect::<std::collections::HashMap<_, _>>();

        let mut versions_query = QueryBuilder::<Sqlite>::new(
            "SELECT asset_id, MAX(version_number) AS version FROM asset_versions WHERE asset_id IN (",
        );
        {
            let mut separated = versions_query.separated(", ");
            for id in &asset_ids {
                separated.push_bind(id.as_str());
            }
        }
        versions_query.push(") GROUP BY asset_id");
        let versions = versions_query
            .build_query_as::<ArtifactVersionRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(|row| {
                u32::try_from(row.version.max(1))
                    .map(|version| (row.asset_id, version))
                    .map_err(|_| {
                        RepositoryError::serialization(
                            "artifact version",
                            "version is out of range",
                        )
                    })
            })
            .collect::<Result<std::collections::HashMap<_, _>, _>>()?;

        let mut reviews_query = QueryBuilder::<Sqlite>::new(
            "SELECT id, project_id, artifact_id, decision, comment, revision, created_at, updated_at
             FROM artifact_reviews WHERE artifact_id IN (",
        );
        {
            let mut separated = reviews_query.separated(", ");
            for id in &asset_ids {
                separated.push_bind(id.as_str());
            }
        }
        reviews_query.push(")");
        let reviews = reviews_query
            .build_query_as::<ArtifactReviewRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(ArtifactReviewRow::try_into_domain)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|review| (review.artifact_id.as_str().to_owned(), review))
            .collect::<std::collections::HashMap<_, _>>();

        let mut records = Vec::with_capacity(mappings.len());
        for row in mappings {
            let asset = assets_by_id.get(&row.asset_id).cloned().ok_or_else(|| {
                RepositoryError::not_found("artifact asset", row.asset_id.clone())
            })?;
            let task_id = TaskId::parse(row.task_id.clone())
                .map_err(|error| map_domain_error("artifact task_id", error))?;
            let asset_id = AssetId::parse(row.asset_id.clone())
                .map_err(|error| map_domain_error("artifact asset_id", error))?;
            let version = versions.get(&row.asset_id).copied().unwrap_or(1);
            let artifact = Artifact {
                asset,
                task_id,
                output_id: row.output_id,
                ordinal: u32::try_from(row.ordinal).map_err(|_| {
                    RepositoryError::serialization(
                        "artifact ordinal",
                        format!("invalid value {}", row.ordinal),
                    )
                })?,
                version,
            };
            records.push(ArtifactRecord {
                artifact,
                review: reviews.get(asset_id.as_str()).cloned(),
            });
        }
        Ok(records)
    }
}

#[async_trait]
impl ArtifactRepository for SqliteArtifactRepository {
    async fn list_for_tasks(
        &self,
        project_id: &str,
        task_ids: &[TaskId],
    ) -> Result<Vec<ArtifactRecord>, RepositoryError> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT toa.task_id, toa.output_id, toa.ordinal, toa.asset_id
             FROM task_output_assets toa
             INNER JOIN tasks t ON t.id = toa.task_id
             INNER JOIN assets a ON a.id = toa.asset_id AND a.project_id = t.project_id
             WHERE t.project_id = ",
        );
        query.push_bind(project_id).push(" AND toa.task_id IN (");
        {
            let mut separated = query.separated(", ");
            for task_id in task_ids {
                separated.push_bind(task_id.as_str());
            }
        }
        query.push(") ORDER BY toa.task_id, toa.output_id, toa.ordinal, toa.asset_id");
        let rows = query
            .build_query_as::<ArtifactMappingRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        self.hydrate_records(rows).await
    }

    async fn find_artifact(
        &self,
        project_id: &str,
        artifact_id: &AssetId,
    ) -> Result<Option<Artifact>, RepositoryError> {
        let row = sqlx::query_as::<_, ArtifactMappingRow>(
            "SELECT toa.task_id, toa.output_id, toa.ordinal, toa.asset_id
             FROM task_output_assets toa
             INNER JOIN tasks t ON t.id = toa.task_id AND t.project_id = ?
             INNER JOIN assets a ON a.id = toa.asset_id AND a.project_id = t.project_id
             WHERE toa.asset_id = ? LIMIT 1",
        )
        .bind(project_id)
        .bind(artifact_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let Some(row) = row else { return Ok(None) };
        Ok(self
            .hydrate_records(vec![row])
            .await?
            .into_iter()
            .next()
            .map(|record| record.artifact))
    }

    async fn find_artifact_by_id(
        &self,
        artifact_id: &AssetId,
    ) -> Result<Option<Artifact>, RepositoryError> {
        let row = sqlx::query_as::<_, ArtifactMappingRow>(
            "SELECT toa.task_id, toa.output_id, toa.ordinal, toa.asset_id
             FROM task_output_assets toa
             INNER JOIN tasks t ON t.id = toa.task_id
             INNER JOIN assets a ON a.id = toa.asset_id AND a.project_id = t.project_id
             WHERE toa.asset_id = ? LIMIT 1",
        )
        .bind(artifact_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let Some(row) = row else { return Ok(None) };
        Ok(self
            .hydrate_records(vec![row])
            .await?
            .into_iter()
            .next()
            .map(|record| record.artifact))
    }

    async fn list_review_queue(
        &self,
        project_id: &str,
        filter: ArtifactReviewQueueFilter,
        limit: usize,
        offset: usize,
    ) -> Result<ArtifactReviewQueuePage, RepositoryError> {
        let (decision_filter, ordering) = match filter {
            ArtifactReviewQueueFilter::Pending => ("decision = 'PENDING'", "review.created_at ASC"),
            ArtifactReviewQueueFilter::Completed => (
                "decision IN ('APPROVED', 'REJECTED')",
                "review.updated_at DESC, review.artifact_id ASC",
            ),
        };
        let count_query = format!(
            "SELECT COUNT(*) FROM artifact_reviews WHERE project_id = ? AND {decision_filter}"
        );
        let total = sqlx::query_scalar::<_, i64>(&count_query)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_error)?
            .max(0) as usize;
        let list_query = format!(
            "SELECT toa.task_id, toa.output_id, toa.ordinal, toa.asset_id
             FROM artifact_reviews review
             INNER JOIN assets a ON a.id = review.artifact_id AND a.project_id = review.project_id
             INNER JOIN task_output_assets toa ON toa.asset_id = a.id
             INNER JOIN tasks t ON t.id = toa.task_id AND t.project_id = review.project_id
             WHERE review.project_id = ? AND {decision_filter}
             ORDER BY {ordering}, toa.task_id, toa.output_id, toa.ordinal
             LIMIT ? OFFSET ?"
        );
        let mappings = sqlx::query_as::<_, ArtifactMappingRow>(&list_query)
            .bind(project_id)
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(ArtifactReviewQueuePage {
            items: self.hydrate_records(mappings).await?,
            total,
        })
    }

    async fn find_review(
        &self,
        project_id: &str,
        artifact_id: &AssetId,
    ) -> Result<Option<ArtifactReview>, RepositoryError> {
        sqlx::query_as::<_, ArtifactReviewRow>(
            "SELECT id, project_id, artifact_id, decision, comment, revision, created_at, updated_at
             FROM artifact_reviews WHERE project_id = ? AND artifact_id = ?",
        )
        .bind(project_id)
        .bind(artifact_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(ArtifactReviewRow::try_into_domain)
        .transpose()
    }

    async fn update_review_if_revision(
        &self,
        review: &ArtifactReview,
        expected_revision: i64,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE artifact_reviews
             SET decision = ?, comment = ?, revision = ?, updated_at = ?
             WHERE project_id = ? AND artifact_id = ? AND revision = ?",
        )
        .bind(review.decision.as_str())
        .bind(&review.comment)
        .bind(review.revision)
        .bind(format_datetime(review.updated_at))
        .bind(&review.project_id)
        .bind(review.artifact_id.as_str())
        .bind(expected_revision)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }
}

#[derive(sqlx::FromRow)]
struct ArtifactMappingRow {
    task_id: String,
    output_id: String,
    ordinal: i64,
    asset_id: String,
}

#[derive(sqlx::FromRow)]
struct ArtifactVersionRow {
    asset_id: String,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct ArtifactReviewRow {
    id: String,
    project_id: String,
    artifact_id: String,
    decision: String,
    comment: String,
    revision: i64,
    created_at: String,
    updated_at: String,
}

impl ArtifactReviewRow {
    fn try_into_domain(self) -> Result<ArtifactReview, RepositoryError> {
        Ok(ArtifactReview {
            id: self.id,
            project_id: self.project_id,
            artifact_id: AssetId::parse(self.artifact_id)
                .map_err(|error| map_domain_error("artifact review artifact_id", error))?,
            decision: ArtifactReviewDecision::parse(&self.decision)
                .map_err(|error| map_domain_error("artifact review decision", error))?,
            comment: self.comment,
            revision: self.revision,
            created_at: parse_datetime("artifact review created_at", &self.created_at)?,
            updated_at: parse_datetime("artifact review updated_at", &self.updated_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteArtifactRepository;
    use crate::{
        application::ports::{
            ArtifactRepository, ArtifactReviewQueueFilter, AssetRepository, TaskOutputAssetMapping,
        },
        domain::{ArtifactReviewDecision, Asset, AssetId, TaskId},
        infrastructure::database::{pool::initialize, SqliteAssetRepository},
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use tempfile::tempdir;

    async fn setup() -> (tempfile::TempDir, sqlx::SqlitePool, TaskId) {
        let directory = tempdir().expect("temporary directory should be created");
        let pool = initialize(&directory.path().join("artifact.db"))
            .await
            .expect("all migrations should apply");
        for statement in [
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-artifact', 'Artifact', 'C:/artifact', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO workflows (id, name, category, mode, created_at, updated_at)
             VALUES ('workflow-artifact', 'Workflow', 'image', 'text_to_image', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('wfv_artifact', 'workflow-artifact', '1', '{}', 'sha', '2026-01-01T00:00:00Z')",
            "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('rcp_artifact', 'wfv_artifact', '1', 1, 'schema_version: 1', 'sha', '2026-01-01T00:00:00Z')",
            "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at, finished_at)
             VALUES ('tsk_artifact_fixture', 'project-artifact', 'workflow-artifact', 'wfv_artifact', 'rcp_artifact', 'SUCCEEDED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:01Z')",
        ] {
            sqlx::query(statement)
                .execute(&pool)
                .await
                .expect("artifact fixture row should insert");
        }
        (
            directory,
            pool,
            TaskId::parse("tsk_artifact_fixture").expect("task id should be valid"),
        )
    }

    fn generated_image(
        task_id: &TaskId,
        id: &str,
        output: &str,
        ordinal: u32,
    ) -> (Asset, TaskOutputAssetMapping) {
        let now = Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 2 + ordinal as u32)
            .unwrap();
        let asset_id = AssetId::parse(id).unwrap();
        let asset = Asset::new_generated_image(
            asset_id.clone(),
            "project-artifact",
            format!("Generated {ordinal}"),
            format!("{id}.png"),
            format!("C:/artifact/assets/generated/image/{id}.png"),
            "a".repeat(64),
            "image/png",
            640,
            480,
            1024,
            task_id.clone(),
            json!({"outputId": output, "position": ordinal}),
            now,
        )
        .unwrap();
        let mapping = TaskOutputAssetMapping {
            task_id: task_id.clone(),
            output_id: output.to_owned(),
            ordinal,
            asset_id,
            created_at: now,
        };
        (asset, mapping)
    }

    #[tokio::test]
    async fn successful_task_persists_multiple_independently_reviewable_artifacts() {
        let (_directory, pool, task_id) = setup().await;
        let first = generated_image(&task_id, "ast_artifact_first", "output-a", 0);
        let second = generated_image(&task_id, "ast_artifact_second", "output-b", 1);
        SqliteAssetRepository::new(pool.clone())
            .insert_generated_outputs(
                &[first.0.clone(), second.0.clone()],
                &[first.1.clone(), second.1.clone()],
            )
            .await
            .expect("generated outputs should be persisted atomically");

        let repository = SqliteArtifactRepository::new(pool.clone());
        let records = repository
            .list_for_tasks("project-artifact", std::slice::from_ref(&task_id))
            .await
            .expect("task artifacts should load");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].artifact.task_id, task_id);
        assert_eq!(records[0].artifact.version, 1);
        assert_eq!(records[1].artifact.version, 1);
        assert!(records.iter().all(|record| {
            record.review.as_ref().is_some_and(|review| {
                review.decision == ArtifactReviewDecision::Pending && review.revision == 0
            })
        }));

        let queue = repository
            .list_review_queue(
                "project-artifact",
                ArtifactReviewQueueFilter::Pending,
                20,
                0,
            )
            .await
            .expect("review queue should load");
        assert_eq!(queue.total, 2);
        assert_eq!(queue.items.len(), 2);
        assert!(repository
            .list_for_tasks("another-project", std::slice::from_ref(&task_id))
            .await
            .unwrap()
            .is_empty());
        assert!(repository
            .find_artifact("another-project", &first.0.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn artifact_review_persistence_uses_revision_compare_and_swap() {
        let (_directory, pool, task_id) = setup().await;
        let (asset, mapping) = generated_image(&task_id, "ast_artifact_cas", "output", 0);
        SqliteAssetRepository::new(pool.clone())
            .insert_generated_outputs(std::slice::from_ref(&asset), std::slice::from_ref(&mapping))
            .await
            .unwrap();
        let repository = SqliteArtifactRepository::new(pool);
        let mut review = repository
            .find_review("project-artifact", &asset.id)
            .await
            .unwrap()
            .expect("registration should create a pending review");
        review
            .submit(
                ArtifactReviewDecision::Approved,
                "looks good".into(),
                0,
                Utc::now(),
            )
            .unwrap();
        assert!(repository
            .update_review_if_revision(&review, 0)
            .await
            .unwrap());
        assert!(!repository
            .update_review_if_revision(&review, 0)
            .await
            .unwrap());
        assert_eq!(
            repository
                .find_review("project-artifact", &asset.id)
                .await
                .unwrap()
                .unwrap()
                .decision,
            ArtifactReviewDecision::Approved
        );
    }

    #[tokio::test]
    async fn completed_review_history_preserves_comments_revisions_and_artifact_isolation_after_reopen(
    ) {
        let (directory, pool, task_id) = setup().await;
        let approved = generated_image(&task_id, "ast_artifact_approved", "output-a", 0);
        let rejected = generated_image(&task_id, "ast_artifact_rejected", "output-b", 1);
        SqliteAssetRepository::new(pool.clone())
            .insert_generated_outputs(
                &[approved.0.clone(), rejected.0.clone()],
                &[approved.1.clone(), rejected.1.clone()],
            )
            .await
            .unwrap();
        let repository = SqliteArtifactRepository::new(pool.clone());

        let mut approved_review = repository
            .find_review("project-artifact", &approved.0.id)
            .await
            .unwrap()
            .expect("approved artifact should have a pending review record");
        approved_review
            .submit(
                ArtifactReviewDecision::Approved,
                String::new(),
                0,
                Utc::now(),
            )
            .unwrap();
        assert!(repository
            .update_review_if_revision(&approved_review, 0)
            .await
            .unwrap());

        let rejection_comment = "测试驳回：画面不符合预期。";
        let mut rejected_review = repository
            .find_review("project-artifact", &rejected.0.id)
            .await
            .unwrap()
            .expect("rejected artifact should have a pending review record");
        rejected_review
            .submit(
                ArtifactReviewDecision::Rejected,
                rejection_comment.to_owned(),
                0,
                Utc::now(),
            )
            .unwrap();
        assert!(repository
            .update_review_if_revision(&rejected_review, 0)
            .await
            .unwrap());

        assert_eq!(
            repository
                .list_review_queue(
                    "project-artifact",
                    ArtifactReviewQueueFilter::Pending,
                    20,
                    0
                )
                .await
                .unwrap()
                .total,
            0,
            "terminal reviews must leave the pending queue"
        );
        let history = repository
            .list_review_queue(
                "project-artifact",
                ArtifactReviewQueueFilter::Completed,
                20,
                0,
            )
            .await
            .unwrap();
        assert_eq!(history.total, 2);
        assert_eq!(history.items.len(), 2);

        drop(repository);
        pool.close().await;
        let reopened_pool = initialize(&directory.path().join("artifact.db"))
            .await
            .expect("the same SQLite database should reopen");
        let reopened_repository = SqliteArtifactRepository::new(reopened_pool.clone());
        let persisted_history = reopened_repository
            .list_review_queue(
                "project-artifact",
                ArtifactReviewQueueFilter::Completed,
                20,
                0,
            )
            .await
            .unwrap();
        assert_eq!(persisted_history.total, 2);
        assert_eq!(
            persisted_history
                .items
                .iter()
                .filter(|record| record.review.is_some())
                .count(),
            2,
            "each Artifact keeps exactly its own persisted review record"
        );
        let approved_history = persisted_history
            .items
            .iter()
            .find(|record| record.review.as_ref().unwrap().artifact_id == approved.0.id)
            .unwrap()
            .review
            .as_ref()
            .unwrap();
        assert_eq!(approved_history.decision, ArtifactReviewDecision::Approved);
        assert!(approved_history.comment.is_empty());
        assert_eq!(approved_history.revision, 1);

        let rejected_history = persisted_history
            .items
            .iter()
            .find(|record| record.review.as_ref().unwrap().artifact_id == rejected.0.id)
            .unwrap()
            .review
            .as_ref()
            .unwrap();
        assert_eq!(rejected_history.decision, ArtifactReviewDecision::Rejected);
        assert_eq!(rejected_history.comment, rejection_comment);
        assert_eq!(rejected_history.revision, 1);
        assert!(reopened_repository
            .list_review_queue(
                "another-project",
                ArtifactReviewQueueFilter::Completed,
                20,
                0
            )
            .await
            .unwrap()
            .items
            .is_empty());
        reopened_pool.close().await;
    }
}
