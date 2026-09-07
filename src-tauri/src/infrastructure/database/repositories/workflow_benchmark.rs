use super::map_sqlx_error;
use crate::application::ports::{
    RepositoryError, WorkflowBenchmarkCandidateRecord, WorkflowBenchmarkDraft,
    WorkflowBenchmarkExperimentRecord, WorkflowBenchmarkQualityRecord, WorkflowBenchmarkQueueLink,
    WorkflowBenchmarkRepository, WorkflowBenchmarkReviewRecord, WorkflowBenchmarkRunRecord,
    WorkflowBenchmarkSnapshot,
};
use async_trait::async_trait;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone)]
pub struct SqliteWorkflowBenchmarkRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowBenchmarkRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct DbExperiment {
    id: String,
    project_id: String,
    name: String,
    media_type: String,
    status: String,
    base_values_json: String,
    asset_ids_json: String,
    winner_candidate_id: Option<String>,
    production_batch_id: Option<String>,
    seed_strategy: String,
    fixed_seed: Option<String>,
    repeat_count: i64,
    recommendation_type: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<DbExperiment> for WorkflowBenchmarkExperimentRecord {
    fn from(value: DbExperiment) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            media_type: value.media_type,
            status: value.status,
            base_values_json: value.base_values_json,
            asset_ids_json: value.asset_ids_json,
            winner_candidate_id: value.winner_candidate_id,
            production_batch_id: value.production_batch_id,
            seed_strategy: value.seed_strategy,
            fixed_seed: value.fixed_seed,
            repeat_count: value.repeat_count,
            recommendation_type: value.recommendation_type,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct DbCandidate {
    id: String,
    position: i64,
    workflow_version_id: String,
    recipe_id: String,
    preset_id: Option<String>,
    preset_name: Option<String>,
    label: String,
    values_json: String,
    asset_ids_json: String,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    workflow_id: Option<String>,
    workflow_version: Option<String>,
    workflow_sha256: Option<String>,
    recipe_version: Option<String>,
    recipe_sha256: Option<String>,
    runtime_package: Option<String>,
    runtime_profile: Option<String>,
}

impl From<DbCandidate> for WorkflowBenchmarkCandidateRecord {
    fn from(value: DbCandidate) -> Self {
        Self {
            id: value.id,
            position: value.position,
            workflow_version_id: value.workflow_version_id,
            recipe_id: value.recipe_id,
            preset_id: value.preset_id,
            preset_name: value.preset_name,
            label: value.label,
            values_json: value.values_json,
            asset_ids_json: value.asset_ids_json,
            production_batch_item_id: value.production_batch_item_id,
            task_id: value.task_id,
            workflow_id: value.workflow_id,
            workflow_version: value.workflow_version,
            workflow_sha256: value.workflow_sha256,
            recipe_version: value.recipe_version,
            recipe_sha256: value.recipe_sha256,
            runtime_package: value.runtime_package,
            runtime_profile: value.runtime_profile,
        }
    }
}

#[derive(Debug, FromRow)]
struct DbRun {
    id: String,
    candidate_id: String,
    run_number: i64,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    snapshot_id: Option<String>,
    output_asset_id: Option<String>,
    generation_execution_id: Option<String>,
    compiled_workflow_sha256: Option<String>,
    runtime_profile: Option<String>,
    concurrency_class: Option<String>,
    queue_wait_ms: Option<i64>,
    prepare_ms: Option<i64>,
    submit_ms: Option<i64>,
    comfy_execution_ms: Option<i64>,
    collect_ms: Option<i64>,
    total_ms: Option<i64>,
    status: Option<String>,
    error_code: Option<String>,
    output_file_size: Option<i64>,
}

impl From<DbRun> for WorkflowBenchmarkRunRecord {
    fn from(value: DbRun) -> Self {
        Self {
            id: value.id,
            candidate_id: value.candidate_id,
            run_number: value.run_number,
            production_batch_item_id: value.production_batch_item_id,
            task_id: value.task_id,
            snapshot_id: value.snapshot_id,
            output_asset_id: value.output_asset_id,
            generation_execution_id: value.generation_execution_id,
            compiled_workflow_sha256: value.compiled_workflow_sha256,
            runtime_profile: value.runtime_profile,
            concurrency_class: value.concurrency_class,
            queue_wait_ms: value.queue_wait_ms,
            prepare_ms: value.prepare_ms,
            submit_ms: value.submit_ms,
            comfy_execution_ms: value.comfy_execution_ms,
            collect_ms: value.collect_ms,
            total_ms: value.total_ms,
            status: value.status,
            error_code: value.error_code,
            output_file_size: value.output_file_size,
        }
    }
}

#[derive(Debug, FromRow)]
struct DbQuality {
    candidate_id: String,
    prompt_adherence: Option<i64>,
    visual_quality: Option<i64>,
    motion_quality: Option<i64>,
    reference_consistency: Option<i64>,
    overall: Option<i64>,
    note: Option<String>,
}

impl From<DbQuality> for WorkflowBenchmarkQualityRecord {
    fn from(value: DbQuality) -> Self {
        Self {
            candidate_id: value.candidate_id,
            prompt_adherence: value.prompt_adherence,
            visual_quality: value.visual_quality,
            motion_quality: value.motion_quality,
            reference_consistency: value.reference_consistency,
            overall: value.overall,
            note: value.note,
        }
    }
}

#[derive(Debug, FromRow)]
struct DbReview {
    production_batch_item_id: String,
    review_status: String,
    review_note: String,
}

impl From<DbReview> for WorkflowBenchmarkReviewRecord {
    fn from(value: DbReview) -> Self {
        Self {
            production_batch_item_id: value.production_batch_item_id,
            review_status: value.review_status,
            review_note: value.review_note,
        }
    }
}

#[async_trait]
impl WorkflowBenchmarkRepository for SqliteWorkflowBenchmarkRepository {
    async fn list_experiments(
        &self,
        project_id: &str,
        limit: i64,
    ) -> Result<Vec<WorkflowBenchmarkExperimentRecord>, RepositoryError> {
        sqlx::query_as::<_, DbExperiment>(
            "SELECT id, project_id, name, media_type, status, base_values_json,
                    asset_ids_json, winner_candidate_id, production_batch_id,
                    seed_strategy, fixed_seed, repeat_count, recommendation_type,
                    created_at, updated_at
             FROM benchmark_experiments
             WHERE project_id = ?
             ORDER BY created_at DESC, id ASC
             LIMIT ?",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn load_experiment_snapshot(
        &self,
        project_id: &str,
        experiment_id: &str,
        updated_at: &str,
    ) -> Result<Option<WorkflowBenchmarkSnapshot>, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let Some(experiment) = sqlx::query_as::<_, DbExperiment>(
            "SELECT id, project_id, name, media_type, status, base_values_json,
                    asset_ids_json, winner_candidate_id, production_batch_id,
                    seed_strategy, fixed_seed, repeat_count, recommendation_type,
                    created_at, updated_at
             FROM benchmark_experiments WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(experiment_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        else {
            return Ok(None);
        };

        sqlx::query(
            "UPDATE benchmark_runs AS r
             SET task_id = COALESCE((SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id), r.task_id),
                 snapshot_id = COALESCE((SELECT s.id FROM generation_snapshots s WHERE s.task_id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id))), r.snapshot_id),
                 output_asset_id = COALESCE((SELECT MIN(oa.asset_id) FROM task_output_assets oa WHERE oa.task_id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id))), r.output_asset_id),
                 generation_execution_id = COALESCE((SELECT t.generation_execution_id FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id))), r.generation_execution_id),
                 compiled_workflow_sha256 = COALESCE((SELECT t.compiled_workflow_sha256 FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id))), r.compiled_workflow_sha256),
                 runtime_profile = COALESCE((SELECT t.runtime_profile FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id))), r.runtime_profile),
                 concurrency_class = COALESCE((SELECT t.concurrency_class FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id))), r.concurrency_class),
                 queue_wait_ms = COALESCE((SELECT CAST((julianday(t.execution_started_at) - julianday(t.queued_at)) * 86400000 AS INTEGER) FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id)) AND t.queued_at IS NOT NULL AND t.execution_started_at IS NOT NULL), r.queue_wait_ms),
                 prepare_ms = COALESCE((SELECT CAST((julianday(t.prepared_at) - julianday(t.prepare_started_at)) * 86400000 AS INTEGER) FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id)) AND t.prepare_started_at IS NOT NULL AND t.prepared_at IS NOT NULL), r.prepare_ms),
                 submit_ms = COALESCE((SELECT CAST((julianday(t.submitted_at) - julianday(t.prepared_at)) * 86400000 AS INTEGER) FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id)) AND t.prepared_at IS NOT NULL AND t.submitted_at IS NOT NULL), r.submit_ms),
                 comfy_execution_ms = COALESCE((SELECT CAST((julianday(t.execution_finished_at) - julianday(t.execution_started_at)) * 86400000 AS INTEGER) FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id)) AND t.execution_started_at IS NOT NULL AND t.execution_finished_at IS NOT NULL), r.comfy_execution_ms),
                 collect_ms = COALESCE((SELECT CAST((julianday(t.collection_finished_at) - julianday(t.execution_finished_at)) * 86400000 AS INTEGER) FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id)) AND t.execution_finished_at IS NOT NULL AND t.collection_finished_at IS NOT NULL), r.collect_ms),
                 total_ms = COALESCE((SELECT CAST((julianday(t.collection_finished_at) - julianday(t.created_at)) * 86400000 AS INTEGER) FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id)) AND t.created_at IS NOT NULL AND t.collection_finished_at IS NOT NULL), r.total_ms),
                 status = COALESCE((SELECT t.status FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id))), (SELECT i.status FROM production_batch_items i WHERE i.id = r.production_batch_item_id), r.status),
                 error_code = COALESCE((SELECT t.error_code FROM tasks t WHERE t.id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id))), (SELECT i.error_code FROM production_batch_items i WHERE i.id = r.production_batch_item_id), r.error_code),
                 output_file_size = COALESCE(r.output_file_size, (SELECT MAX(a.file_size) FROM task_output_assets oa INNER JOIN assets a ON a.id = oa.asset_id WHERE oa.task_id = COALESCE(r.task_id, (SELECT i.task_id FROM production_batch_items i WHERE i.id = r.production_batch_item_id)))),
                 updated_at = ?
             WHERE r.experiment_id = ?",
        )
        .bind(updated_at)
        .bind(&experiment.id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let candidates = sqlx::query_as::<_, DbCandidate>(
            "SELECT id, position, workflow_version_id, recipe_id,
                    preset_id, preset_name, label, values_json, asset_ids_json,
                    production_batch_item_id, task_id, workflow_id, workflow_version,
                    workflow_sha256, recipe_version, recipe_sha256, runtime_package,
                    runtime_profile
             FROM benchmark_candidates
             WHERE experiment_id = ? ORDER BY position ASC",
        )
        .bind(&experiment.id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let runs = sqlx::query_as::<_, DbRun>(
            "SELECT r.id, r.candidate_id, r.run_number, r.production_batch_item_id,
                    COALESCE(r.task_id, i.task_id) AS task_id,
                    COALESCE(r.snapshot_id, s.id) AS snapshot_id,
                    COALESCE(r.output_asset_id, (
                        SELECT MIN(oa.asset_id) FROM task_output_assets oa
                        WHERE oa.task_id = COALESCE(r.task_id, i.task_id)
                    )) AS output_asset_id,
                    COALESCE(r.generation_execution_id, t.generation_execution_id) AS generation_execution_id,
                    COALESCE(r.compiled_workflow_sha256, t.compiled_workflow_sha256) AS compiled_workflow_sha256,
                    COALESCE(r.runtime_profile, t.runtime_profile) AS runtime_profile,
                    COALESCE(r.concurrency_class, t.concurrency_class) AS concurrency_class,
                    COALESCE(r.queue_wait_ms,
                        CASE WHEN t.queued_at IS NOT NULL AND t.execution_started_at IS NOT NULL
                             THEN CAST((julianday(t.execution_started_at) - julianday(t.queued_at)) * 86400000 AS INTEGER)
                        END) AS queue_wait_ms,
                    COALESCE(r.prepare_ms,
                        CASE WHEN t.prepare_started_at IS NOT NULL AND t.prepared_at IS NOT NULL
                             THEN CAST((julianday(t.prepared_at) - julianday(t.prepare_started_at)) * 86400000 AS INTEGER)
                        END) AS prepare_ms,
                    COALESCE(r.submit_ms,
                        CASE WHEN t.prepared_at IS NOT NULL AND t.submitted_at IS NOT NULL
                             THEN CAST((julianday(t.submitted_at) - julianday(t.prepared_at)) * 86400000 AS INTEGER)
                        END) AS submit_ms,
                    COALESCE(r.comfy_execution_ms,
                        CASE WHEN t.execution_started_at IS NOT NULL AND t.execution_finished_at IS NOT NULL
                             THEN CAST((julianday(t.execution_finished_at) - julianday(t.execution_started_at)) * 86400000 AS INTEGER)
                        END) AS comfy_execution_ms,
                    COALESCE(r.collect_ms,
                        CASE WHEN t.execution_finished_at IS NOT NULL AND t.collection_finished_at IS NOT NULL
                             THEN CAST((julianday(t.collection_finished_at) - julianday(t.execution_finished_at)) * 86400000 AS INTEGER)
                        END) AS collect_ms,
                    COALESCE(r.total_ms,
                        CASE WHEN t.created_at IS NOT NULL AND t.collection_finished_at IS NOT NULL
                             THEN CAST((julianday(t.collection_finished_at) - julianday(t.created_at)) * 86400000 AS INTEGER)
                        END) AS total_ms,
                    COALESCE(t.status, i.status, r.status) AS status,
                    COALESCE(r.error_code, t.error_code, i.error_code) AS error_code,
                    COALESCE(r.output_file_size, (
                        SELECT MAX(a.file_size) FROM task_output_assets oa
                        INNER JOIN assets a ON a.id = oa.asset_id
                        WHERE oa.task_id = COALESCE(r.task_id, i.task_id)
                    )) AS output_file_size
             FROM benchmark_runs r
             LEFT JOIN production_batch_items i ON i.id = r.production_batch_item_id
             LEFT JOIN tasks t ON t.id = COALESCE(r.task_id, i.task_id)
             LEFT JOIN generation_snapshots s ON s.task_id = COALESCE(r.task_id, i.task_id)
             WHERE r.experiment_id = ?
             ORDER BY r.candidate_id ASC, r.run_number ASC",
        )
        .bind(&experiment.id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let quality = sqlx::query_as::<_, DbQuality>(
            "SELECT q.candidate_id, q.prompt_adherence, q.visual_quality,
                    q.motion_quality, q.reference_consistency, q.overall, q.note
             FROM benchmark_quality_scores q
             INNER JOIN benchmark_candidates c ON c.id = q.candidate_id
             WHERE c.experiment_id = ?",
        )
        .bind(&experiment.id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let mut reviews = Vec::new();
        for candidate in &candidates {
            let Some(item_id) = candidate.production_batch_item_id.as_deref() else {
                continue;
            };
            if let Some(review) = sqlx::query_as::<_, DbReview>(
                "SELECT production_batch_item_id, review_status, review_note
                 FROM production_item_reviews
                 WHERE production_batch_item_id = ? ORDER BY version DESC LIMIT 1",
            )
            .bind(item_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?
            {
                reviews.push(review.into());
            }
        }

        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(Some(WorkflowBenchmarkSnapshot {
            experiment: experiment.into(),
            candidates: candidates.into_iter().map(Into::into).collect(),
            runs: runs.into_iter().map(Into::into).collect(),
            quality: quality.into_iter().map(Into::into).collect(),
            reviews,
        }))
    }

    async fn create_draft_atomic(
        &self,
        draft: &WorkflowBenchmarkDraft,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let experiment = &draft.experiment;
        sqlx::query(
            "INSERT INTO benchmark_experiments
             (id, project_id, name, media_type, status, base_values_json, asset_ids_json,
              winner_candidate_id, production_batch_id, seed_strategy, fixed_seed,
              repeat_count, recommendation_type, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'DRAFT', ?, ?, NULL, NULL, ?, ?, ?, NULL, ?, ?)",
        )
        .bind(&experiment.id)
        .bind(&experiment.project_id)
        .bind(&experiment.name)
        .bind(&experiment.media_type)
        .bind(&experiment.base_values_json)
        .bind(&experiment.asset_ids_json)
        .bind(&experiment.seed_strategy)
        .bind(&experiment.fixed_seed)
        .bind(i64::from(draft.repeat_count))
        .bind(&experiment.created_at)
        .bind(&experiment.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        for candidate in &draft.candidates {
            sqlx::query(
                "INSERT INTO benchmark_candidates
                (id, experiment_id, position, workflow_version_id, recipe_id, preset_id,
                  preset_name, label, values_json, asset_ids_json, production_batch_item_id,
                  task_id, workflow_id, workflow_version, workflow_sha256, recipe_version,
                  recipe_sha256, runtime_package, runtime_profile, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&candidate.id)
            .bind(&experiment.id)
            .bind(candidate.position)
            .bind(&candidate.workflow_version_id)
            .bind(&candidate.recipe_id)
            .bind(&candidate.preset_id)
            .bind(&candidate.preset_name)
            .bind(&candidate.label)
            .bind(&candidate.values_json)
            .bind(&candidate.asset_ids_json)
            .bind(&candidate.workflow_id)
            .bind(&candidate.workflow_version)
            .bind(&candidate.workflow_sha256)
            .bind(&candidate.recipe_version)
            .bind(&candidate.recipe_sha256)
            .bind(&candidate.runtime_package)
            .bind(&candidate.runtime_profile)
            .bind(&experiment.created_at)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            for run_number in 1..=draft.repeat_count {
                sqlx::query(
                    "INSERT INTO benchmark_runs
                     (id, experiment_id, candidate_id, run_number, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(format!("bmr_{}", uuid::Uuid::new_v4().simple()))
                .bind(&experiment.id)
                .bind(&candidate.id)
                .bind(i64::from(run_number))
                .bind(&experiment.created_at)
                .bind(&experiment.updated_at)
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
            }
        }
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn candidate_belongs_to_experiment(
        &self,
        project_id: &str,
        experiment_id: &str,
        candidate_id: &str,
    ) -> Result<bool, RepositoryError> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM benchmark_candidates c
             INNER JOIN benchmark_experiments e ON e.id = c.experiment_id
             WHERE c.id = ? AND e.id = ? AND e.project_id = ?",
        )
        .bind(candidate_id)
        .bind(experiment_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(exists != 0)
    }

    async fn set_winner(
        &self,
        project_id: &str,
        experiment_id: &str,
        candidate_id: Option<&str>,
        updated_at: &str,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE benchmark_experiments SET winner_candidate_id = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(candidate_id)
        .bind(updated_at)
        .bind(experiment_id)
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() != 0)
    }

    async fn set_recommendation(
        &self,
        project_id: &str,
        experiment_id: &str,
        recommendation_type: Option<&str>,
        updated_at: &str,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE benchmark_experiments SET recommendation_type = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(recommendation_type)
        .bind(updated_at)
        .bind(experiment_id)
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() != 0)
    }

    async fn save_quality(
        &self,
        candidate_id: &str,
        prompt_adherence: Option<i64>,
        visual_quality: Option<i64>,
        motion_quality: Option<i64>,
        reference_consistency: Option<i64>,
        overall: Option<i64>,
        note: Option<&str>,
        now: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO benchmark_quality_scores
             (id, candidate_id, prompt_adherence, visual_quality, motion_quality,
              reference_consistency, overall, note, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(candidate_id) DO UPDATE SET
                prompt_adherence = excluded.prompt_adherence,
                visual_quality = excluded.visual_quality,
                motion_quality = excluded.motion_quality,
                reference_consistency = excluded.reference_consistency,
                overall = excluded.overall,
                note = excluded.note,
                updated_at = excluded.updated_at",
        )
        .bind(format!("bqs_{}", uuid::Uuid::new_v4().simple()))
        .bind(candidate_id)
        .bind(prompt_adherence)
        .bind(visual_quality)
        .bind(motion_quality)
        .bind(reference_consistency)
        .bind(overall)
        .bind(note)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn clone_experiment_atomic(
        &self,
        project_id: &str,
        experiment_id: &str,
        new_id: &str,
        name: &str,
        now: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let experiment = sqlx::query_as::<_, DbExperiment>(
            "SELECT id, project_id, name, media_type, status, base_values_json,
                    asset_ids_json, winner_candidate_id, production_batch_id,
                    seed_strategy, fixed_seed, repeat_count, recommendation_type,
                    created_at, updated_at
             FROM benchmark_experiments WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(experiment_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .ok_or_else(|| RepositoryError::not_found("benchmark", experiment_id))?;
        let candidates = sqlx::query_as::<_, DbCandidate>(
            "SELECT id, position, workflow_version_id, recipe_id,
                    preset_id, preset_name, label, values_json, asset_ids_json,
                    production_batch_item_id, task_id, workflow_id, workflow_version,
                    workflow_sha256, recipe_version, recipe_sha256, runtime_package,
                    runtime_profile
             FROM benchmark_candidates
             WHERE experiment_id = ? ORDER BY position ASC",
        )
        .bind(experiment_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        let repeat_count = u32::try_from(experiment.repeat_count)
            .unwrap_or(3)
            .clamp(1, 10);

        sqlx::query(
            "INSERT INTO benchmark_experiments
             (id, project_id, name, media_type, status, base_values_json, asset_ids_json,
              winner_candidate_id, production_batch_id, seed_strategy, fixed_seed,
              repeat_count, recommendation_type, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'DRAFT', ?, ?, NULL, NULL, ?, ?, ?, NULL, ?, ?)",
        )
        .bind(new_id)
        .bind(project_id)
        .bind(name)
        .bind(&experiment.media_type)
        .bind(&experiment.base_values_json)
        .bind(&experiment.asset_ids_json)
        .bind(&experiment.seed_strategy)
        .bind(&experiment.fixed_seed)
        .bind(i64::from(repeat_count))
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        for candidate in candidates {
            let candidate_id = format!("bmc_{}", uuid::Uuid::new_v4().simple());
            sqlx::query(
                "INSERT INTO benchmark_candidates
                 (id, experiment_id, position, workflow_version_id, recipe_id, preset_id,
                  preset_name, label, values_json, asset_ids_json, production_batch_item_id,
                  task_id, workflow_id, workflow_version, workflow_sha256, recipe_version,
                  recipe_sha256, runtime_package, runtime_profile, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&candidate_id)
            .bind(new_id)
            .bind(candidate.position)
            .bind(&candidate.workflow_version_id)
            .bind(&candidate.recipe_id)
            .bind(&candidate.preset_id)
            .bind(&candidate.preset_name)
            .bind(&candidate.label)
            .bind(&candidate.values_json)
            .bind(&candidate.asset_ids_json)
            .bind(&candidate.workflow_id)
            .bind(&candidate.workflow_version)
            .bind(&candidate.workflow_sha256)
            .bind(&candidate.recipe_version)
            .bind(&candidate.recipe_sha256)
            .bind(&candidate.runtime_package)
            .bind(&candidate.runtime_profile)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            for run_number in 1..=repeat_count {
                sqlx::query(
                    "INSERT INTO benchmark_runs
                     (id, experiment_id, candidate_id, run_number, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(format!("bmr_{}", uuid::Uuid::new_v4().simple()))
                .bind(new_id)
                .bind(&candidate_id)
                .bind(i64::from(run_number))
                .bind(now)
                .bind(now)
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
            }
        }
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn verify_frozen_assets(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<(), RepositoryError> {
        for asset_id in asset_ids {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM assets WHERE id = ? AND project_id = ?",
            )
            .bind(asset_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
            if exists == 0 {
                return Err(RepositoryError::integrity(format!(
                    "Benchmark 冻结素材不存在或不属于当前项目：{asset_id}"
                )));
            }
        }
        Ok(())
    }

    async fn link_queue_atomic(
        &self,
        experiment_id: &str,
        batch_id: &str,
        links: &[WorkflowBenchmarkQueueLink],
        updated_at: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE benchmark_experiments SET production_batch_id = ?, status = 'QUEUED', updated_at = ? WHERE id = ?",
        )
        .bind(batch_id)
        .bind(updated_at)
        .bind(experiment_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        for link in links {
            sqlx::query(
                "UPDATE benchmark_candidates
                 SET production_batch_item_id = ?, values_json = ?
                 WHERE experiment_id = ? AND position = ?",
            )
            .bind(&link.production_batch_item_id)
            .bind(&link.values_json)
            .bind(experiment_id)
            .bind(i64::from(link.candidate_position))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            sqlx::query(
                "UPDATE benchmark_runs
                 SET production_batch_item_id = ?, task_id = NULL, updated_at = ?
                 WHERE experiment_id = ? AND candidate_id = (
                    SELECT id FROM benchmark_candidates
                    WHERE experiment_id = ? AND position = ?
                 ) AND run_number = ?",
            )
            .bind(&link.production_batch_item_id)
            .bind(updated_at)
            .bind(experiment_id)
            .bind(experiment_id)
            .bind(i64::from(link.candidate_position))
            .bind(i64::from(link.run_number))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn mark_queue_link_failed(
        &self,
        experiment_id: &str,
        batch_id: &str,
        updated_at: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE benchmark_experiments
             SET production_batch_id = ?, status = 'FAILED_TO_QUEUE', updated_at = ?
             WHERE id = ?",
        )
        .bind(batch_id)
        .bind(updated_at)
        .bind(experiment_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn set_status(
        &self,
        experiment_id: &str,
        status: &str,
        updated_at: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE benchmark_experiments SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(updated_at)
            .bind(experiment_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn delete_atomic(
        &self,
        project_id: &str,
        experiment_id: &str,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let result =
            sqlx::query("DELETE FROM benchmark_experiments WHERE id = ? AND project_id = ?")
                .bind(experiment_id)
                .bind(project_id)
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(map_sqlx_error)?;
            return Ok(false);
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteWorkflowBenchmarkRepository;
    use crate::application::ports::{
        WorkflowBenchmarkCandidateRecord, WorkflowBenchmarkDraft,
        WorkflowBenchmarkExperimentRecord, WorkflowBenchmarkQueueLink, WorkflowBenchmarkRepository,
    };
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    const NOW: &str = "2026-01-02T00:00:00Z";

    async fn setup() -> (TempDir, SqlitePool, SqliteWorkflowBenchmarkRepository) {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        (
            directory,
            pool.clone(),
            SqliteWorkflowBenchmarkRepository::new(pool),
        )
    }

    fn experiment(id: &str) -> WorkflowBenchmarkExperimentRecord {
        WorkflowBenchmarkExperimentRecord {
            id: id.to_owned(),
            project_id: "project-1".to_owned(),
            name: "Boundary test".to_owned(),
            media_type: "IMAGE".to_owned(),
            status: "DRAFT".to_owned(),
            base_values_json: "{}".to_owned(),
            asset_ids_json: "[]".to_owned(),
            winner_candidate_id: None,
            production_batch_id: None,
            seed_strategy: "FIXED_SEED".to_owned(),
            fixed_seed: Some("42".to_owned()),
            repeat_count: 2,
            recommendation_type: None,
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
        }
    }

    fn candidate(id: &str, position: i64) -> WorkflowBenchmarkCandidateRecord {
        WorkflowBenchmarkCandidateRecord {
            id: id.to_owned(),
            position,
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            preset_id: None,
            preset_name: None,
            label: format!("Candidate {position}"),
            values_json: "{}".to_owned(),
            asset_ids_json: "[]".to_owned(),
            production_batch_item_id: None,
            task_id: None,
            workflow_id: Some("workflow-1".to_owned()),
            workflow_version: Some("1".to_owned()),
            workflow_sha256: Some("sha".to_owned()),
            recipe_version: Some("1".to_owned()),
            recipe_sha256: Some("sha".to_owned()),
            runtime_package: None,
            runtime_profile: None,
        }
    }

    fn draft(
        experiment_id: &str,
        candidates: Vec<WorkflowBenchmarkCandidateRecord>,
    ) -> WorkflowBenchmarkDraft {
        WorkflowBenchmarkDraft {
            experiment: experiment(experiment_id),
            candidates,
            repeat_count: 2,
        }
    }

    #[tokio::test]
    async fn create_draft_rolls_back_experiment_candidates_and_runs_together() {
        let (_directory, pool, repository) = setup().await;
        let result = repository
            .create_draft_atomic(&draft(
                "benchmark-atomic-draft",
                vec![candidate("candidate-a", 0), candidate("candidate-b", 0)],
            ))
            .await;

        assert!(result.is_err(), "duplicate candidate positions should fail");
        for (table, id) in [
            ("benchmark_experiments", "benchmark-atomic-draft"),
            ("benchmark_candidates", "candidate-a"),
            ("benchmark_candidates", "candidate-b"),
        ] {
            let count =
                sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table} WHERE id = ?"))
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .expect("rollback verification query should succeed");
            assert_eq!(count, 0, "{table}/{id} should not survive rollback");
        }
        let run_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM benchmark_runs WHERE experiment_id = ?",
        )
        .bind("benchmark-atomic-draft")
        .fetch_one(&pool)
        .await
        .expect("run rollback verification query should succeed");
        assert_eq!(run_count, 0);
    }

    #[tokio::test]
    async fn queue_link_rolls_back_partial_updates_and_compensation_is_durable() {
        let (_directory, pool, repository) = setup().await;
        repository
            .create_draft_atomic(&draft(
                "benchmark-atomic-link",
                vec![candidate("candidate-link", 0)],
            ))
            .await
            .expect("draft should be created");
        sqlx::query(
            "INSERT INTO production_batches
             (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES ('batch-link', 'project-1', 'Link test', 'QUEUED', 0, ?, ?)",
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("batch fixture should insert");

        let result = repository
            .link_queue_atomic(
                "benchmark-atomic-link",
                "batch-link",
                &[WorkflowBenchmarkQueueLink {
                    production_batch_item_id: "missing-item".to_owned(),
                    candidate_position: 0,
                    run_number: 1,
                    values_json: "{\"prompt\":\"test\"}".to_owned(),
                }],
                NOW,
            )
            .await;
        assert!(
            result.is_err(),
            "invalid queue item should fail the transaction"
        );

        let experiment_state = sqlx::query_as::<_, (Option<String>, String)>(
            "SELECT production_batch_id, status FROM benchmark_experiments WHERE id = ?",
        )
        .bind("benchmark-atomic-link")
        .fetch_one(&pool)
        .await
        .expect("experiment rollback verification query should succeed");
        assert_eq!(experiment_state, (None, "DRAFT".to_owned()));
        let candidate_state = sqlx::query_as::<_, (Option<String>, String)>(
            "SELECT production_batch_item_id, values_json FROM benchmark_candidates WHERE id = ?",
        )
        .bind("candidate-link")
        .fetch_one(&pool)
        .await
        .expect("candidate rollback verification query should succeed");
        assert_eq!(candidate_state, (None, "{}".to_owned()));

        repository
            .mark_queue_link_failed("benchmark-atomic-link", "batch-link", NOW)
            .await
            .expect("queue failure compensation should persist");
        let compensated = sqlx::query_as::<_, (Option<String>, String)>(
            "SELECT production_batch_id, status FROM benchmark_experiments WHERE id = ?",
        )
        .bind("benchmark-atomic-link")
        .fetch_one(&pool)
        .await
        .expect("compensation verification query should succeed");
        assert_eq!(
            compensated,
            (Some("batch-link".to_owned()), "FAILED_TO_QUEUE".to_owned())
        );
    }

    #[tokio::test]
    async fn clone_creates_independent_draft_candidates_and_runs() {
        let (_directory, pool, repository) = setup().await;
        repository
            .create_draft_atomic(&draft(
                "benchmark-atomic-clone-source",
                vec![
                    candidate("candidate-clone-a", 0),
                    candidate("candidate-clone-b", 1),
                ],
            ))
            .await
            .expect("source draft should be created");

        repository
            .clone_experiment_atomic(
                "project-1",
                "benchmark-atomic-clone-source",
                "benchmark-atomic-clone-copy",
                "Cloned benchmark",
                NOW,
            )
            .await
            .expect("clone should be created atomically");
        let snapshot = repository
            .load_experiment_snapshot("project-1", "benchmark-atomic-clone-copy", NOW)
            .await
            .expect("clone snapshot should load")
            .expect("clone should exist");
        assert_eq!(snapshot.experiment.id, "benchmark-atomic-clone-copy");
        assert_eq!(snapshot.experiment.status, "DRAFT");
        assert_eq!(snapshot.candidates.len(), 2);
        assert_eq!(snapshot.runs.len(), 4);
        assert!(snapshot
            .candidates
            .iter()
            .all(|candidate| candidate.production_batch_item_id.is_none()));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM benchmark_candidates WHERE experiment_id = ?",
            )
            .bind("benchmark-atomic-clone-source")
            .fetch_one(&pool)
            .await
            .expect("source candidate count should be queryable"),
            2
        );
    }

    #[tokio::test]
    async fn delete_atomic_removes_experiment_children_and_is_project_scoped() {
        let (_directory, pool, repository) = setup().await;
        repository
            .create_draft_atomic(&draft(
                "benchmark-atomic-delete",
                vec![candidate("candidate-delete", 0)],
            ))
            .await
            .expect("draft should be created");

        assert!(!repository
            .delete_atomic("other-project", "benchmark-atomic-delete")
            .await
            .expect("wrong-project delete should succeed without deleting"));
        assert!(repository
            .delete_atomic("project-1", "benchmark-atomic-delete")
            .await
            .expect("project-scoped delete should succeed"));
        for table in [
            "benchmark_experiments",
            "benchmark_candidates",
            "benchmark_runs",
        ] {
            let count = sqlx::query_scalar::<_, i64>(&format!(
                "SELECT COUNT(*) FROM {table} WHERE {} = ?",
                if table == "benchmark_experiments" {
                    "id"
                } else if table == "benchmark_candidates" {
                    "experiment_id"
                } else {
                    "experiment_id"
                }
            ))
            .bind("benchmark-atomic-delete")
            .fetch_one(&pool)
            .await
            .expect("delete verification query should succeed");
            assert_eq!(count, 0, "{table} children should be deleted");
        }
    }
}
