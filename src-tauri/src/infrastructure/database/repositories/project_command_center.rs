use super::map_sqlx_error;
use crate::application::ports::{
    ProjectCommandCenterAssetCountRecord, ProjectCommandCenterConsistencyRecord,
    ProjectCommandCenterCountRecord, ProjectCommandCenterData,
    ProjectCommandCenterPreparationRecord, ProjectCommandCenterProjectRecord,
    ProjectCommandCenterPromptLibraryRecord, ProjectCommandCenterQueueBatchRecord,
    ProjectCommandCenterQueueItemRecord, ProjectCommandCenterReferenceAnchorRecord,
    ProjectCommandCenterRepository, ProjectCommandCenterSceneRecord,
    ProjectCommandCenterShotConfigRecord, ProjectCommandCenterShotLinkRecord,
    ProjectCommandCenterShotRecord, ProjectCommandCenterStructureRecord, RepositoryError,
};
use async_trait::async_trait;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone)]
pub struct SqliteProjectCommandCenterRepository {
    pool: SqlitePool,
}

impl SqliteProjectCommandCenterRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectCommandCenterRepository for SqliteProjectCommandCenterRepository {
    async fn load_project_command_center_data(
        &self,
        project_id: &str,
    ) -> Result<ProjectCommandCenterData, RepositoryError> {
        let project = sqlx::query_as::<_, DbProject>(
            "SELECT id, name, description, created_at, updated_at
             FROM projects WHERE id = ?",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(Into::into);
        let Some(project) = project else {
            return Err(RepositoryError::not_found("project", project_id));
        };

        let structure = sqlx::query_as::<_, DbStructure>(
            "SELECT
               (SELECT COUNT(*) FROM production_series WHERE project_id = ?) AS series_count,
               (SELECT COUNT(*) FROM production_episodes e
                  JOIN production_series s ON s.id = e.series_id
                  WHERE s.project_id = ?) AS episode_count,
               (SELECT COUNT(*) FROM production_scenes c
                  JOIN production_episodes e ON e.id = c.episode_id
                  JOIN production_series s ON s.id = e.series_id
                  WHERE s.project_id = ?) AS scene_count,
               (SELECT COUNT(*) FROM shot_scene_assignments a
                  JOIN shots sh ON sh.id = a.shot_id
                  WHERE sh.project_id = ?) AS assigned_shot_count,
               (SELECT COUNT(*) FROM shots sh
                  LEFT JOIN shot_scene_assignments a ON a.shot_id = sh.id
                  WHERE sh.project_id = ? AND a.shot_id IS NULL) AS unassigned_shot_count,
               (SELECT MIN(sh.id) FROM shots sh
                  LEFT JOIN shot_scene_assignments a ON a.shot_id = sh.id
                  WHERE sh.project_id = ? AND a.shot_id IS NULL) AS first_unassigned_shot_id,
               (SELECT COUNT(*) FROM production_episodes e
                  LEFT JOIN production_series s ON s.id = e.series_id WHERE s.id IS NULL)
               + (SELECT COUNT(*) FROM production_scenes c
                  LEFT JOIN production_episodes e ON e.id = c.episode_id WHERE e.id IS NULL)
               + (SELECT COUNT(*) FROM shot_scene_assignments a
                  LEFT JOIN shots sh ON sh.id = a.shot_id WHERE sh.id IS NULL) AS orphan_count",
        )
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into();
        let scenes = sqlx::query_as::<_, DbScene>(
            "SELECT c.id, c.name, s.name AS series_name, e.name AS episode_name,
                    COUNT(a.shot_id) AS total,
                    COALESCE(SUM(CASE WHEN sh.selected_video_asset_id IS NOT NULL
                        OR (sh.selected_image_asset_id IS NOT NULL AND NOT EXISTS (
                            SELECT 1 FROM shot_stage_configs vc
                            WHERE vc.shot_id = sh.id AND vc.stage = 'video'
                        )) THEN 1 ELSE 0 END), 0) AS completed
             FROM production_scenes c
             JOIN production_episodes e ON e.id = c.episode_id
             JOIN production_series s ON s.id = e.series_id
             LEFT JOIN shot_scene_assignments a ON a.scene_id = c.id
             LEFT JOIN shots sh ON sh.id = a.shot_id AND sh.project_id = ?
             WHERE s.project_id = ?
             GROUP BY c.id, c.name, s.name, e.name
             ORDER BY s.ordinal, e.ordinal, c.ordinal, c.id",
        )
        .bind(project_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let shots = sqlx::query_as::<_, DbShot>(
            "SELECT id, selected_image_asset_id, selected_video_asset_id
             FROM shots WHERE project_id = ? ORDER BY ordinal ASC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let shot_configs = sqlx::query_as::<_, DbShotConfig>(
            "SELECT c.shot_id, c.stage
             FROM shot_stage_configs c JOIN shots sh ON sh.id = c.shot_id
             WHERE sh.project_id = ? ORDER BY c.shot_id, c.stage",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let shot_links = sqlx::query_as::<_, DbShotLink>(
            "SELECT l.shot_id, l.stage, l.task_id, t.status AS task_status
             FROM shot_generation_links l
             JOIN shots sh ON sh.id = l.shot_id
             LEFT JOIN tasks t ON t.id = l.task_id
             WHERE sh.project_id = ?
             ORDER BY l.shot_id ASC, l.stage ASC, l.created_at DESC, l.id DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let queue_batches = sqlx::query_as::<_, DbQueueBatch>(
            "SELECT id, status, archived_at FROM production_batches
             WHERE project_id = ? ORDER BY id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let queue_items = sqlx::query_as::<_, DbQueueItem>(
            "SELECT i.id, i.batch_id, i.ordinal,
                    (SELECT MIN(l.shot_id) FROM shot_generation_links l
                     WHERE l.production_batch_item_id = i.id) AS shot_id,
                    i.task_id, i.status, i.retry_of_item_id, i.error_code
             FROM production_batch_items i JOIN production_batches b ON b.id = i.batch_id
             WHERE b.project_id = ? ORDER BY i.batch_id ASC, i.ordinal ASC, i.id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let task_counts = sqlx::query_as::<_, DbCount>(
            "SELECT status, COUNT(*) AS count FROM tasks
             WHERE project_id = ? GROUP BY status ORDER BY status",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let asset_counts = sqlx::query_as::<_, DbAssetCount>(
            "SELECT UPPER(type) AS asset_type, COUNT(*) AS count FROM assets
             WHERE project_id = ? GROUP BY UPPER(type) ORDER BY UPPER(type)",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let consistency = sqlx::query_as::<_, DbConsistency>(
            "SELECT
               (SELECT COUNT(*) FROM character_profiles WHERE project_id = ?) AS character_profiles,
               (SELECT COUNT(*) FROM scene_profiles WHERE project_id = ?) AS scene_profiles,
               (SELECT COUNT(*) FROM prop_profiles WHERE project_id = ?) AS prop_profiles,
               (SELECT COUNT(*) FROM style_profiles WHERE project_id = ?) AS style_profiles,
               (SELECT COUNT(*) FROM reference_sets WHERE project_id = ?) AS reference_sets,
               (SELECT COUNT(*) FROM shot_profile_bindings b
                  JOIN shots s ON s.id = b.shot_id
                  WHERE s.project_id = ?) AS shot_profile_bindings,
               (SELECT COUNT(*) FROM shot_reference_set_bindings b
                  JOIN shots s ON s.id = b.shot_id
                  WHERE s.project_id = ?) AS shot_reference_set_bindings,
               (SELECT COUNT(*) FROM consistency_scope_profile_bindings
                  WHERE project_id = ?) AS scope_profile_bindings,
               (SELECT COUNT(*) FROM consistency_scope_reference_set_bindings
                  WHERE project_id = ?) AS scope_reference_set_bindings",
        )
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into();
        let preparation = sqlx::query_as::<_, DbPreparation>(
            "SELECT
               COUNT(*) AS snapshot_count,
               COALESCE(SUM(CASE WHEN s.stage = 'image' THEN 1 ELSE 0 END), 0)
                 AS prepared_image_items,
               COALESCE(SUM(CASE WHEN s.stage = 'video' THEN 1 ELSE 0 END), 0)
                 AS prepared_video_items,
               COALESCE(SUM(CASE WHEN i.status IN
                 ('PENDING', 'DISPATCHING', 'DISPATCHED') THEN 1 ELSE 0 END), 0)
                 AS active_prepared_items,
               MAX(s.created_at) AS latest_prepared_at
             FROM production_preparation_snapshots s
             JOIN production_batches b ON b.id = s.production_batch_id
             JOIN production_batch_items i ON i.id = s.production_batch_item_id
             WHERE s.project_id = ? AND b.project_id = ?",
        )
        .bind(project_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into();
        let reference_anchors = sqlx::query_as::<_, DbReferenceAnchor>(
            "SELECT a.kind, COUNT(aa.asset_id) AS asset_count
             FROM reference_anchors a
             LEFT JOIN reference_anchor_assets aa ON aa.anchor_id = a.id
             WHERE a.project_id = ? GROUP BY a.id, a.kind ORDER BY a.kind, a.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let prompt_entries = sqlx::query_as::<_, DbPromptLibraryEntry>(
            "SELECT e.id, e.name, COUNT(v.id) AS version_count, e.updated_at
             FROM prompt_entries e
             JOIN prompt_versions v ON v.prompt_id = e.id
             WHERE e.project_id = ?
             GROUP BY e.id, e.name, e.updated_at
             ORDER BY e.updated_at DESC, e.id DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();

        Ok(ProjectCommandCenterData {
            project: Some(project),
            structure,
            scenes,
            shots,
            shot_configs,
            shot_links,
            queue_batches,
            queue_items,
            task_counts,
            asset_counts,
            consistency,
            preparation,
            reference_anchors,
            prompt_entries,
        })
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

db_record!(DbProject => ProjectCommandCenterProjectRecord {
    id: String, name: String, description: Option<String>, created_at: String, updated_at: String
});
db_record!(DbStructure => ProjectCommandCenterStructureRecord {
    series_count: i64, episode_count: i64, scene_count: i64, assigned_shot_count: i64,
    unassigned_shot_count: i64, first_unassigned_shot_id: Option<String>, orphan_count: i64
});
db_record!(DbScene => ProjectCommandCenterSceneRecord {
    id: String, name: String, series_name: String, episode_name: String, total: i64, completed: i64
});
db_record!(DbShot => ProjectCommandCenterShotRecord {
    id: String, selected_image_asset_id: Option<String>, selected_video_asset_id: Option<String>
});
db_record!(DbShotConfig => ProjectCommandCenterShotConfigRecord {
    shot_id: String, stage: String
});
db_record!(DbShotLink => ProjectCommandCenterShotLinkRecord {
    shot_id: String, stage: String, task_id: Option<String>, task_status: Option<String>
});
db_record!(DbQueueBatch => ProjectCommandCenterQueueBatchRecord {
    id: String, status: String, archived_at: Option<String>
});
db_record!(DbQueueItem => ProjectCommandCenterQueueItemRecord {
    id: String, batch_id: String, ordinal: i64, shot_id: Option<String>, task_id: Option<String>, status: String,
    retry_of_item_id: Option<String>, error_code: Option<String>
});
db_record!(DbCount => ProjectCommandCenterCountRecord { status: String, count: i64 });
db_record!(DbAssetCount => ProjectCommandCenterAssetCountRecord { asset_type: String, count: i64 });
db_record!(DbConsistency => ProjectCommandCenterConsistencyRecord {
    character_profiles: i64, scene_profiles: i64, prop_profiles: i64, style_profiles: i64,
    reference_sets: i64, shot_profile_bindings: i64, shot_reference_set_bindings: i64,
    scope_profile_bindings: i64, scope_reference_set_bindings: i64
});
db_record!(DbPreparation => ProjectCommandCenterPreparationRecord {
    snapshot_count: i64, prepared_image_items: i64, prepared_video_items: i64,
    active_prepared_items: i64, latest_prepared_at: Option<String>
});
db_record!(DbReferenceAnchor => ProjectCommandCenterReferenceAnchorRecord {
    kind: String, asset_count: i64
});
db_record!(DbPromptLibraryEntry => ProjectCommandCenterPromptLibraryRecord {
    id: String, name: String, version_count: i64, updated_at: String
});
