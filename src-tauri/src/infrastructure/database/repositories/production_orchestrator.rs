use super::map_sqlx_error;
use crate::application::ports::{
    ProductionBatchTarget, ProductionOrchestratorRepository, ProductionRunRecord,
    ProductionRunSnapshot, ProductionRunTemplateRecord, ProductionStageItemDraft,
    ProductionStageItemRecord, ProductionStageRecord, ProductionStageStats, RepositoryError,
    SelectedReferenceRecord,
};
use async_trait::async_trait;
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};
use uuid::Uuid;

#[derive(Clone)]
pub struct SqliteProductionOrchestratorRepository {
    pool: SqlitePool,
}

impl SqliteProductionOrchestratorRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct DbRun {
    id: String,
    project_id: String,
    name: String,
    status: String,
    current_stage_ordinal: i64,
    template_id: Option<String>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

impl From<DbRun> for ProductionRunRecord {
    fn from(value: DbRun) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            status: value.status,
            current_stage_ordinal: value.current_stage_ordinal,
            template_id: value.template_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            started_at: value.started_at,
            finished_at: value.finished_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct DbStage {
    id: String,
    run_id: String,
    ordinal: i64,
    stage_type: String,
    status: String,
    workflow_version_id: Option<String>,
    recipe_id: Option<String>,
    production_batch_id: Option<String>,
    frozen_config_json: String,
    prompt: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<DbStage> for ProductionStageRecord {
    fn from(value: DbStage) -> Self {
        Self {
            id: value.id,
            run_id: value.run_id,
            ordinal: value.ordinal,
            stage_type: value.stage_type,
            status: value.status,
            workflow_version_id: value.workflow_version_id,
            recipe_id: value.recipe_id,
            production_batch_id: value.production_batch_id,
            frozen_config_json: value.frozen_config_json,
            prompt: value.prompt,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct DbStageItem {
    id: String,
    stage_id: String,
    ordinal: i64,
    status: String,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    task_status: Option<String>,
    asset_id: Option<String>,
    source_asset_id: Option<String>,
    reference_index: Option<i64>,
    attempt: i64,
    submission_idempotency_key: Option<String>,
    parent_stage_item_id: Option<String>,
    frozen_values_json: String,
    error_code: Option<String>,
    error_message: Option<String>,
}

impl From<DbStageItem> for ProductionStageItemRecord {
    fn from(value: DbStageItem) -> Self {
        Self {
            id: value.id,
            stage_id: value.stage_id,
            ordinal: value.ordinal,
            status: value.status,
            production_batch_item_id: value.production_batch_item_id,
            task_id: value.task_id,
            task_status: value.task_status,
            asset_id: value.asset_id,
            source_asset_id: value.source_asset_id,
            reference_index: value.reference_index,
            attempt: value.attempt,
            submission_idempotency_key: value.submission_idempotency_key,
            parent_stage_item_id: value.parent_stage_item_id,
            frozen_values_json: value.frozen_values_json,
            error_code: value.error_code,
            error_message: value.error_message,
        }
    }
}

#[derive(Debug, FromRow)]
struct DbTemplate {
    id: String,
    project_id: String,
    name: String,
    krea2_workflow_version_id: Option<String>,
    krea2_recipe_id: Option<String>,
    krea2_preset_id: Option<String>,
    default_image_count: i64,
    h3_workflow_version_id: Option<String>,
    h3_recipe_id: Option<String>,
    h3_profile: Option<String>,
    default_duration_seconds: Option<i64>,
    default_width: Option<i64>,
    default_height: Option<i64>,
    created_at: String,
    updated_at: String,
}

impl From<DbTemplate> for ProductionRunTemplateRecord {
    fn from(value: DbTemplate) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            krea2_workflow_version_id: value.krea2_workflow_version_id,
            krea2_recipe_id: value.krea2_recipe_id,
            krea2_preset_id: value.krea2_preset_id,
            default_image_count: value.default_image_count,
            h3_workflow_version_id: value.h3_workflow_version_id,
            h3_recipe_id: value.h3_recipe_id,
            h3_profile: value.h3_profile,
            default_duration_seconds: value.default_duration_seconds,
            default_width: value.default_width,
            default_height: value.default_height,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct DbSelectedReference {
    asset_id: Option<String>,
    reference_index: i64,
}

impl From<DbSelectedReference> for SelectedReferenceRecord {
    fn from(value: DbSelectedReference) -> Self {
        Self {
            asset_id: value.asset_id,
            reference_index: value.reference_index,
        }
    }
}

#[async_trait]
impl ProductionOrchestratorRepository for SqliteProductionOrchestratorRepository {
    async fn template_exists(
        &self,
        project_id: &str,
        template_id: &str,
    ) -> Result<bool, RepositoryError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_run_templates WHERE id = ? AND project_id = ?",
        )
        .bind(template_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(count > 0)
    }

    async fn create_run_atomic(
        &self,
        run: &ProductionRunRecord,
        stages: &[ProductionStageRecord],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "INSERT INTO production_runs
             (id, project_id, name, status, current_stage_ordinal, template_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(&run.project_id)
        .bind(&run.name)
        .bind(&run.status)
        .bind(run.current_stage_ordinal)
        .bind(&run.template_id)
        .bind(&run.created_at)
        .bind(&run.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        for stage in stages {
            insert_stage(&mut transaction, stage).await?;
        }
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn list_runs(
        &self,
        project_id: &str,
        limit: i64,
    ) -> Result<Vec<ProductionRunRecord>, RepositoryError> {
        sqlx::query_as::<_, DbRun>(
            "SELECT id, project_id, name, status, current_stage_ordinal, template_id,
                    created_at, updated_at, started_at, finished_at
             FROM production_runs WHERE project_id = ?
             ORDER BY created_at DESC, id ASC LIMIT ?",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn load_run(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<Option<ProductionRunRecord>, RepositoryError> {
        sqlx::query_as::<_, DbRun>(
            "SELECT id, project_id, name, status, current_stage_ordinal, template_id,
                    created_at, updated_at, started_at, finished_at
             FROM production_runs WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|row| row.map(Into::into))
    }

    async fn load_run_by_id(
        &self,
        run_id: &str,
    ) -> Result<Option<ProductionRunRecord>, RepositoryError> {
        sqlx::query_as::<_, DbRun>(
            "SELECT id, project_id, name, status, current_stage_ordinal, template_id,
                    created_at, updated_at, started_at, finished_at
             FROM production_runs WHERE id = ?",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|row| row.map(Into::into))
    }

    async fn load_run_snapshot(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<Option<ProductionRunSnapshot>, RepositoryError> {
        let Some(run) = self.load_run(project_id, run_id).await? else {
            return Ok(None);
        };
        let stages = sqlx::query_as::<_, DbStage>(
            "SELECT id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
                    production_batch_id, frozen_config_json, prompt, created_at, updated_at
             FROM production_stages WHERE run_id = ? ORDER BY ordinal ASC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let items = sqlx::query_as::<_, DbStageItem>(
            "SELECT si.id, si.stage_id, si.ordinal,
                    CASE
                      WHEN i.status IN ('DISPATCHING', 'DISPATCHED') THEN 'RUNNING'
                      WHEN i.status IS NOT NULL THEN i.status
                      ELSE si.status
                    END AS status,
                    si.production_batch_item_id,
                    COALESCE(i.task_id, si.task_id) AS task_id,
                    t.status AS task_status,
                    COALESCE(si.asset_id, (
                      SELECT MIN(oa.asset_id) FROM task_output_assets oa WHERE oa.task_id = COALESCE(i.task_id, si.task_id)
                    )) AS asset_id,
                    si.source_asset_id, si.reference_index, si.attempt,
                    si.submission_idempotency_key, si.parent_stage_item_id,
                    si.frozen_values_json, si.error_code, si.error_message
             FROM production_stage_items si
             LEFT JOIN production_batch_items i ON i.id = si.production_batch_item_id
             LEFT JOIN tasks t ON t.id = COALESCE(i.task_id, si.task_id)
             INNER JOIN production_stages s ON s.id = si.stage_id
             WHERE s.run_id = ? ORDER BY si.stage_id ASC, si.ordinal ASC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        Ok(Some(ProductionRunSnapshot { run, stages, items }))
    }

    async fn load_stage(
        &self,
        run_id: &str,
        ordinal: i64,
    ) -> Result<Option<ProductionStageRecord>, RepositoryError> {
        sqlx::query_as::<_, DbStage>(
            "SELECT id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
                    production_batch_id, frozen_config_json, prompt, created_at, updated_at
             FROM production_stages WHERE run_id = ? AND ordinal = ?",
        )
        .bind(run_id)
        .bind(ordinal)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|row| row.map(Into::into))
    }

    async fn load_selected_assets(
        &self,
        selection_stage_id: &str,
    ) -> Result<Vec<SelectedReferenceRecord>, RepositoryError> {
        sqlx::query_as::<_, DbSelectedReference>(
            "SELECT asset_id, COALESCE(reference_index, ordinal) AS reference_index
             FROM production_stage_items WHERE stage_id = ? AND status = 'SUCCEEDED'
             AND asset_id IS NOT NULL ORDER BY COALESCE(reference_index, ordinal), ordinal",
        )
        .bind(selection_stage_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn validate_generated_assets(
        &self,
        project_id: &str,
        stage_id: &str,
        asset_ids: &[String],
    ) -> Result<Vec<String>, RepositoryError> {
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT DISTINCT si.asset_id FROM production_stage_items si
             INNER JOIN assets a ON a.id = si.asset_id
             WHERE si.stage_id = ",
        );
        query
            .push_bind(stage_id)
            .push(" AND si.status = 'SUCCEEDED' AND a.project_id = ")
            .push_bind(project_id)
            .push(" AND a.type = 'image' AND si.asset_id IN (");
        {
            let mut separated = query.separated(", ");
            for asset_id in asset_ids {
                separated.push_bind(asset_id);
            }
        }
        query.push(")");
        query
            .build_query_as::<(Option<String>,)>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)
            .map(|rows| rows.into_iter().filter_map(|(id,)| id).collect())
    }

    async fn attach_image_batch_atomic(
        &self,
        run_id: &str,
        stage_id: &str,
        batch_id: &str,
        items: &[ProductionStageItemDraft],
        now: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_stages SET production_batch_id = ?, status = 'READY', updated_at = ?
             WHERE run_id = ? AND ordinal = 0",
        )
        .bind(batch_id)
        .bind(now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        for item in items {
            insert_stage_item(&mut transaction, stage_id, item, now).await?;
        }
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn save_selection_atomic(
        &self,
        run_id: &str,
        selection_stage_id: &str,
        items: &[ProductionStageItemDraft],
        now: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query("DELETE FROM production_stage_items WHERE stage_id = ?")
            .bind(selection_stage_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        for item in items {
            insert_stage_item(&mut transaction, selection_stage_id, item, now).await?;
        }
        sqlx::query(
            "UPDATE production_stages SET status = 'SUCCEEDED', updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(selection_stage_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_stages SET status = 'READY', updated_at = ?
             WHERE run_id = ? AND ordinal = 2 AND production_batch_id IS NULL",
        )
        .bind(now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_runs SET status = 'READY', current_stage_ordinal = 2, updated_at = ?
             WHERE id = ?",
        )
        .bind(now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn prepare_h3_atomic(
        &self,
        run_id: &str,
        stage_id: &str,
        batch_id: &str,
        items: &[ProductionStageItemDraft],
        now: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_stages SET production_batch_id = ?, status = 'READY', updated_at = ? WHERE id = ?",
        )
        .bind(batch_id)
        .bind(now)
        .bind(stage_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        for item in items {
            insert_stage_item(&mut transaction, stage_id, item, now).await?;
        }
        sqlx::query(
            "UPDATE production_runs SET status = 'RUNNING', current_stage_ordinal = 2,
                    started_at = COALESCE(started_at, ?), updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn find_retry_source_item(
        &self,
        stage_id: &str,
        batch_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        sqlx::query_scalar::<_, String>(
            "SELECT production_batch_item_id FROM production_stage_items
             WHERE stage_id = ? AND production_batch_item_id IN (
                 SELECT id FROM production_batch_items WHERE batch_id = ?
             )
             ORDER BY attempt DESC, ordinal ASC LIMIT 1",
        )
        .bind(stage_id)
        .bind(batch_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn retry_metadata(&self, stage_id: &str) -> Result<(i64, i64), RepositoryError> {
        let attempt = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(attempt) FROM production_stage_items WHERE stage_id = ?",
        )
        .bind(stage_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .unwrap_or(0)
        .saturating_add(1);
        let ordinal = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(ordinal) FROM production_stage_items WHERE stage_id = ?",
        )
        .bind(stage_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .unwrap_or(-1)
        .saturating_add(1);
        Ok((attempt, ordinal))
    }

    async fn prepare_retry_atomic(
        &self,
        stage_id: &str,
        items: &[ProductionStageItemDraft],
        now: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        for item in items {
            let parent_stage_item_id = if item.parent_stage_item_id.is_some() {
                item.parent_stage_item_id.clone()
            } else {
                sqlx::query_scalar::<_, String>(
                    "SELECT id FROM production_stage_items
                     WHERE stage_id = ? AND reference_index = ?
                     ORDER BY attempt DESC, ordinal DESC LIMIT 1",
                )
                .bind(stage_id)
                .bind(item.reference_index)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?
            };
            let parent_stage_item_id = parent_stage_item_id.ok_or_else(|| {
                RepositoryError::integrity(format!(
                    "missing retry parent for reference_index {:?}",
                    item.reference_index
                ))
            })?;
            let mut item = item.clone();
            item.parent_stage_item_id = Some(parent_stage_item_id);
            insert_stage_item(&mut transaction, stage_id, &item, now).await?;
        }
        sqlx::query("UPDATE production_stages SET status = 'READY', updated_at = ? WHERE id = ?")
            .bind(now)
            .bind(stage_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn load_cancel_targets(
        &self,
        run_id: &str,
    ) -> Result<(Vec<String>, Vec<ProductionBatchTarget>), RepositoryError> {
        let task_ids = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT COALESCE(i.task_id, si.task_id)
             FROM production_stage_items si
             LEFT JOIN production_batch_items i ON i.id = si.production_batch_item_id
             INNER JOIN production_stages s ON s.id = si.stage_id
             WHERE s.run_id = ? AND COALESCE(i.task_id, si.task_id) IS NOT NULL",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let batches = sqlx::query_as::<_, (String, String)>(
            "SELECT DISTINCT s.production_batch_id, b.status
             FROM production_stages s INNER JOIN production_batches b ON b.id = s.production_batch_id
             WHERE s.run_id = ? AND s.production_batch_id IS NOT NULL",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(|(id, status)| ProductionBatchTarget { id, status })
        .collect();
        Ok((task_ids, batches))
    }

    async fn cancel_run_atomic(&self, run_id: &str, now: &str) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_stages SET status = CASE WHEN status IN ('SUCCEEDED', 'SKIPPED') THEN status ELSE 'CANCELLED' END,
                    updated_at = ? WHERE run_id = ?",
        )
        .bind(now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_stage_items SET status = CASE WHEN status IN ('SUCCEEDED', 'SKIPPED') THEN status ELSE 'CANCELLED' END,
                    updated_at = ? WHERE stage_id IN (SELECT id FROM production_stages WHERE run_id = ?)
                    AND status NOT IN ('SUCCEEDED', 'SKIPPED')",
        )
        .bind(now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_runs SET status = 'CANCELLED', finished_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn save_template(
        &self,
        template: &ProductionRunTemplateRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO production_run_templates
             (id, project_id, name, krea2_workflow_version_id, krea2_recipe_id, krea2_preset_id,
              default_image_count, h3_workflow_version_id, h3_recipe_id, h3_profile,
              default_duration_seconds, default_width, default_height, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&template.id)
        .bind(&template.project_id)
        .bind(&template.name)
        .bind(&template.krea2_workflow_version_id)
        .bind(&template.krea2_recipe_id)
        .bind(&template.krea2_preset_id)
        .bind(template.default_image_count)
        .bind(&template.h3_workflow_version_id)
        .bind(&template.h3_recipe_id)
        .bind(&template.h3_profile)
        .bind(template.default_duration_seconds)
        .bind(template.default_width)
        .bind(template.default_height)
        .bind(&template.created_at)
        .bind(&template.updated_at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn list_templates(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProductionRunTemplateRecord>, RepositoryError> {
        sqlx::query_as::<_, DbTemplate>(
            "SELECT id, project_id, name, krea2_workflow_version_id, krea2_recipe_id,
                    krea2_preset_id, default_image_count, h3_workflow_version_id, h3_recipe_id,
                    h3_profile, default_duration_seconds, default_width, default_height,
                    created_at, updated_at
             FROM production_run_templates WHERE project_id = ?
             ORDER BY updated_at DESC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn load_template(
        &self,
        project_id: &str,
        template_id: &str,
    ) -> Result<Option<ProductionRunTemplateRecord>, RepositoryError> {
        sqlx::query_as::<_, DbTemplate>(
            "SELECT id, project_id, name, krea2_workflow_version_id, krea2_recipe_id,
                    krea2_preset_id, default_image_count, h3_workflow_version_id, h3_recipe_id,
                    h3_profile, default_duration_seconds, default_width, default_height,
                    created_at, updated_at
             FROM production_run_templates WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(template_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|row| row.map(Into::into))
    }

    async fn sync_run_persistence(&self, run_id: &str, now: &str) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE production_stage_items
             SET task_id = COALESCE((SELECT i.task_id FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id), task_id),
                 asset_id = COALESCE(asset_id, (SELECT MIN(oa.asset_id) FROM task_output_assets oa
                     WHERE oa.task_id = COALESCE(production_stage_items.task_id,
                         (SELECT i.task_id FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id)))),
                 status = CASE
                    WHEN (SELECT i.status FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id) IN ('DISPATCHING', 'DISPATCHED') THEN 'RUNNING'
                    WHEN (SELECT i.status FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id) IS NOT NULL THEN (SELECT i.status FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id)
                    ELSE status
                 END,
                 updated_at = ?
             WHERE stage_id IN (SELECT id FROM production_stages WHERE run_id = ?)
               AND production_batch_item_id IS NOT NULL",
        )
        .bind(now)
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn load_batch_status(&self, batch_id: &str) -> Result<Option<String>, RepositoryError> {
        sqlx::query_scalar::<_, String>("SELECT status FROM production_batches WHERE id = ?")
            .bind(batch_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)
    }

    async fn stage_stats(
        &self,
        stage_id: &str,
        batch_id: Option<&str>,
    ) -> Result<ProductionStageStats, RepositoryError> {
        sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN status = 'SUCCEEDED' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status IN ('SUCCEEDED', 'FAILED', 'CANCELLED', 'SKIPPED') THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status IN ('PENDING', 'READY') THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status IN ('RUNNING', 'DISPATCHING', 'DISPATCHED') THEN 1 ELSE 0 END), 0)
             FROM production_stage_items
             WHERE stage_id = ?
               AND (? IS NULL OR production_batch_item_id IN (
                   SELECT id FROM production_batch_items WHERE batch_id = ?
               ))
               AND (
                   reference_index IS NULL OR attempt = (
                       SELECT MAX(latest.attempt)
                       FROM production_stage_items latest
                       WHERE latest.stage_id = production_stage_items.stage_id
                         AND latest.reference_index = production_stage_items.reference_index
                   )
               )",
        )
        .bind(stage_id)
        .bind(batch_id)
        .bind(batch_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|(total, succeeded, terminal, pending, active)| ProductionStageStats {
            total,
            succeeded,
            terminal,
            pending,
            active,
        })
    }

    async fn mark_stage_running(
        &self,
        run_id: &str,
        ordinal: i64,
        now: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_stages SET status = 'RUNNING', started_at = COALESCE(started_at, ?), updated_at = ?
             WHERE run_id = ? AND ordinal = ?",
        )
        .bind(now)
        .bind(now)
        .bind(run_id)
        .bind(ordinal)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_runs SET status = 'RUNNING', current_stage_ordinal = ?, started_at = COALESCE(started_at, ?), updated_at = ?
             WHERE id = ?",
        )
        .bind(ordinal)
        .bind(now)
        .bind(now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn update_stage_status(
        &self,
        run_id: &str,
        ordinal: i64,
        status: &str,
        now: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE production_stages SET status = ?, finished_at = CASE WHEN ? IN ('SUCCEEDED', 'FAILED', 'SKIPPED', 'CANCELLED') THEN COALESCE(finished_at, ?) ELSE finished_at END, updated_at = ?
             WHERE run_id = ? AND ordinal = ?",
        )
        .bind(status)
        .bind(status)
        .bind(now)
        .bind(now)
        .bind(run_id)
        .bind(ordinal)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn update_run_status(
        &self,
        run_id: &str,
        status: &str,
        ordinal: i64,
        finished_at: Option<&str>,
        now: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE production_runs SET status = ?, current_stage_ordinal = ?, finished_at = COALESCE(?, finished_at), updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(ordinal)
        .bind(finished_at)
        .bind(now)
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn reset_run_waiting(&self, run_id: &str, now: &str) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE production_runs SET status = 'WAITING_FOR_SELECTION', updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }
}

async fn insert_stage(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    stage: &ProductionStageRecord,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO production_stages
         (id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
          frozen_config_json, prompt, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&stage.id)
    .bind(&stage.run_id)
    .bind(stage.ordinal)
    .bind(&stage.stage_type)
    .bind(&stage.status)
    .bind(&stage.workflow_version_id)
    .bind(&stage.recipe_id)
    .bind(&stage.frozen_config_json)
    .bind(&stage.prompt)
    .bind(&stage.created_at)
    .bind(&stage.updated_at)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn insert_stage_item(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    stage_id: &str,
    item: &ProductionStageItemDraft,
    now: &str,
) -> Result<(), RepositoryError> {
    let id = format!("prsi_{}", Uuid::new_v4().simple());
    let key = format!("production-stage-item:{id}:attempt:{}", item.attempt);
    sqlx::query(
        "INSERT INTO production_stage_items
         (id, stage_id, ordinal, status, production_batch_item_id, task_id, asset_id,
          source_asset_id, reference_index, attempt, submission_idempotency_key, parent_stage_item_id,
          frozen_values_json, error_code, error_message, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(stage_id)
    .bind(item.ordinal)
    .bind(&item.status)
    .bind(&item.production_batch_item_id)
    .bind(&item.task_id)
    .bind(&item.asset_id)
    .bind(&item.source_asset_id)
    .bind(item.reference_index)
    .bind(item.attempt)
    .bind(Some(key))
    .bind(&item.parent_stage_item_id)
    .bind(&item.frozen_values_json)
    .bind(&item.error_code)
    .bind(&item.error_message)
    .bind(now)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}
