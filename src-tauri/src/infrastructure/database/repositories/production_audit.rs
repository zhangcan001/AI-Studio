use super::map_sqlx_error;
use crate::application::ports::{
    ProductionAuditAssetRecord, ProductionAuditBatchItemRecord, ProductionAuditBatchRecord,
    ProductionAuditGraph, ProductionAuditPreparationSnapshotRecord, ProductionAuditRepository,
    ProductionAuditRunRecord, ProductionAuditShotLinkRecord, ProductionAuditShotRecord,
    ProductionAuditSnapshotDetailRecord, ProductionAuditSnapshotRecord,
    ProductionAuditStageItemRecord, ProductionAuditStageRecord, ProductionAuditTaskOutputRecord,
    ProductionAuditTaskRecord, RepositoryError,
};
use async_trait::async_trait;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone)]
pub struct SqliteProductionAuditRepository {
    pool: SqlitePool,
}

impl SqliteProductionAuditRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionAuditRepository for SqliteProductionAuditRepository {
    async fn load_project_graph(
        &self,
        project_id: &str,
    ) -> Result<ProductionAuditGraph, RepositoryError> {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = ?")
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        if exists == 0 {
            return Err(RepositoryError::not_found("project", project_id));
        }

        let runs = sqlx::query_as::<_, DbRun>(
            "SELECT id, project_id, name, status, created_at, updated_at, started_at, finished_at
             FROM production_runs WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let stages = sqlx::query_as::<_, DbStage>(
            "SELECT s.id, s.run_id, s.ordinal, s.stage_type, s.status, s.production_batch_id,
                    s.created_at, s.updated_at, s.started_at, s.finished_at
             FROM production_stages s JOIN production_runs r ON r.id = s.run_id
             WHERE r.project_id = ? ORDER BY s.run_id, s.ordinal, s.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let batches = sqlx::query_as::<_, DbBatch>(
            "SELECT id, project_id, name, status, created_at, updated_at
             FROM production_batches WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let batch_items = sqlx::query_as::<_, DbBatchItem>(
            "SELECT i.id, i.batch_id, i.ordinal, i.status, i.task_id, i.retry_of_item_id,
                    i.error_code, i.error_message, i.created_at, i.updated_at
             FROM production_batch_items i JOIN production_batches b ON b.id = i.batch_id
             WHERE b.project_id = ? ORDER BY i.batch_id, i.ordinal, i.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let stage_items = sqlx::query_as::<_, DbStageItem>(
            "SELECT i.id, i.stage_id, i.ordinal, i.status, i.production_batch_item_id,
                    i.task_id, i.asset_id, i.parent_stage_item_id, i.error_code
             FROM production_stage_items i
             JOIN production_stages s ON s.id = i.stage_id
             JOIN production_runs r ON r.id = s.run_id
             WHERE r.project_id = ? ORDER BY i.stage_id, i.ordinal, i.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let tasks = sqlx::query_as::<_, DbTask>(
            "SELECT id, project_id, status, error_code, created_at,
                    COALESCE(finished_at, started_at, queued_at, created_at) AS updated_at,
                    finished_at
             FROM tasks WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let snapshots = sqlx::query_as::<_, DbSnapshot>(
            "SELECT s.id, s.task_id, s.created_at
             FROM generation_snapshots s JOIN tasks t ON t.id = s.task_id
             WHERE t.project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let preparation_snapshots = sqlx::query_as::<_, DbPreparationSnapshot>(
            "SELECT s.id, s.project_id, s.shot_id, s.stage, s.context_hash,
                    s.production_batch_id, s.production_batch_item_id, s.created_at
             FROM production_preparation_snapshots s
             JOIN production_batches b ON b.id = s.production_batch_id
             JOIN production_batch_items i ON i.id = s.production_batch_item_id
             JOIN shots sh ON sh.id = s.shot_id
             WHERE s.project_id = ? AND b.project_id = ? AND sh.project_id = ?
             ORDER BY s.created_at ASC, s.id ASC",
        )
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let assets = sqlx::query_as::<_, DbAsset>(
            "SELECT id, project_id, name, source_task_id, created_at, updated_at
             FROM assets WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let task_outputs = sqlx::query_as::<_, DbTaskOutput>(
            "SELECT o.task_id, o.output_id, o.ordinal, o.asset_id, o.created_at
             FROM task_output_assets o JOIN tasks t ON t.id = o.task_id
             WHERE t.project_id = ? ORDER BY o.task_id, o.output_id, o.ordinal",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let shots = sqlx::query_as::<_, DbShot>(
            "SELECT id, project_id, name, selected_image_asset_id, selected_video_asset_id,
                    created_at, updated_at
             FROM shots WHERE project_id = ? ORDER BY ordinal, id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let shot_links = sqlx::query_as::<_, DbShotLink>(
            "SELECT l.id, l.shot_id, l.stage, l.task_id, l.production_batch_item_id, l.created_at
             FROM shot_generation_links l JOIN shots s ON s.id = l.shot_id
             WHERE s.project_id = ? ORDER BY l.shot_id, l.created_at, l.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();

        Ok(ProductionAuditGraph {
            runs,
            stages,
            batches,
            batch_items,
            stage_items,
            tasks,
            snapshots,
            preparation_snapshots,
            assets,
            task_outputs,
            shots,
            shot_links,
        })
    }

    async fn find_snapshot_detail(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
    ) -> Result<Option<ProductionAuditSnapshotDetailRecord>, RepositoryError> {
        sqlx::query_as::<_, DbSnapshotDetail>(
            "SELECT s.id, s.project_id, s.shot_id, s.stage, s.context_hash,
                    s.production_batch_id, s.production_batch_item_id,
                    s.snapshot_json, s.created_at
             FROM production_preparation_snapshots s
             JOIN production_batches b ON b.id = s.production_batch_id
             JOIN production_batch_items i ON i.id = s.production_batch_item_id
             JOIN shots sh ON sh.id = s.shot_id
             WHERE s.project_id = ? AND b.project_id = ? AND sh.project_id = ?
               AND s.production_batch_item_id = ?",
        )
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(production_batch_item_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
        .map(|row| row.map(Into::into))
    }
}

macro_rules! db_record {
    ($name:ident => $record:ty { $($field:ident : $field_ty:ty),* $(,)? }) => {
        #[derive(Debug, FromRow)]
        struct $name { $( $field: $field_ty, )* }
        impl From<$name> for $record {
            fn from(value: $name) -> Self { Self { $( $field: value.$field, )* } }
        }
    };
}

db_record!(DbRun => ProductionAuditRunRecord {
    id: String, project_id: String, name: String, status: String, created_at: String,
    updated_at: String, started_at: Option<String>, finished_at: Option<String>
});
db_record!(DbStage => ProductionAuditStageRecord {
    id: String, run_id: String, ordinal: i64, stage_type: String, status: String,
    production_batch_id: Option<String>, created_at: String, updated_at: String,
    started_at: Option<String>, finished_at: Option<String>
});
db_record!(DbBatch => ProductionAuditBatchRecord {
    id: String, project_id: String, name: String, status: String, created_at: String,
    updated_at: String
});
db_record!(DbBatchItem => ProductionAuditBatchItemRecord {
    id: String, batch_id: String, ordinal: i64, status: String, task_id: Option<String>,
    retry_of_item_id: Option<String>, error_code: Option<String>, error_message: Option<String>,
    created_at: String, updated_at: String
});
db_record!(DbStageItem => ProductionAuditStageItemRecord {
    id: String, stage_id: String, ordinal: i64, status: String,
    production_batch_item_id: Option<String>, task_id: Option<String>, asset_id: Option<String>,
    parent_stage_item_id: Option<String>, error_code: Option<String>
});
db_record!(DbTask => ProductionAuditTaskRecord {
    id: String, project_id: String, status: String, error_code: Option<String>, created_at: String,
    updated_at: String, finished_at: Option<String>
});
db_record!(DbSnapshot => ProductionAuditSnapshotRecord {
    id: String, task_id: String, created_at: String
});
db_record!(DbPreparationSnapshot => ProductionAuditPreparationSnapshotRecord {
    id: String, project_id: String, shot_id: String, stage: String, context_hash: String,
    production_batch_id: String, production_batch_item_id: String, created_at: String
});
db_record!(DbAsset => ProductionAuditAssetRecord {
    id: String, project_id: String, name: String, source_task_id: Option<String>,
    created_at: String, updated_at: String
});
db_record!(DbTaskOutput => ProductionAuditTaskOutputRecord {
    task_id: String, output_id: String, ordinal: i64, asset_id: String, created_at: String
});
db_record!(DbShot => ProductionAuditShotRecord {
    id: String, project_id: String, name: String, selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>, created_at: String, updated_at: String
});
db_record!(DbShotLink => ProductionAuditShotLinkRecord {
    id: String, shot_id: String, stage: String, task_id: Option<String>,
    production_batch_item_id: Option<String>, created_at: String
});
db_record!(DbSnapshotDetail => ProductionAuditSnapshotDetailRecord {
    id: String, project_id: String, shot_id: String, stage: String, context_hash: String,
    production_batch_id: String, production_batch_item_id: String, snapshot_json: String,
    created_at: String
});
