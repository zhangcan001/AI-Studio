use super::{map_domain_error, map_sqlx_error, parse_json};
use crate::application::ports::{
    AssetUsageItem, AssetUsageRepository, AssetUsageSummary, ProfileUsageSummary,
    ReferenceSetUsageSummary, RepositoryError,
};
use crate::domain::consistency::ProfileType;
use crate::domain::{AssetId, ProductionBatchItemStatus, TaskStatus};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use std::collections::HashSet;

#[derive(Clone)]
pub struct SqliteAssetUsageRepository {
    pool: SqlitePool,
}

impl SqliteAssetUsageRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ReferenceSetAssetRow {
    reference_set_id: String,
    reference_set_name: String,
    purpose: String,
    ordinal: i64,
    role: Option<String>,
}

#[derive(FromRow)]
struct ReferenceSetOwnerRow {
    reference_set_id: String,
    reference_set_name: String,
    profile_type: String,
    profile_id: Option<String>,
    profile_name: String,
}

#[derive(FromRow)]
struct ProfileReferenceRow {
    profile_type: String,
    profile_id: String,
    profile_name: String,
    reference_set_id: String,
    reference_set_name: String,
}

#[derive(FromRow)]
struct CostumeReferenceRow {
    costume_id: String,
    costume_name: String,
    character_profile_id: String,
    character_profile_name: String,
    reference_set_id: String,
    reference_set_name: String,
}

#[derive(FromRow)]
struct ShotReferenceSetRow {
    shot_id: String,
    shot_name: String,
    reference_set_id: String,
    reference_set_name: String,
    role: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct ScopeReferenceSetRow {
    scope_type: String,
    scope_id: String,
    scope_name: String,
    reference_set_id: String,
    reference_set_name: String,
    role: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct SelectedShotRow {
    shot_id: String,
    shot_name: String,
    selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>,
}

#[derive(FromRow)]
struct LegacyAnchorRow {
    anchor_id: String,
    anchor_name: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct LegacyShotReferenceRow {
    shot_id: String,
    shot_name: String,
    stage: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct ProductionQueueRow {
    item_id: String,
    status: String,
    values_json: String,
    task_id: Option<String>,
}

#[derive(FromRow)]
struct TaskReferenceRow {
    task_id: String,
    status: String,
    user_inputs_json: Option<String>,
    resolved_inputs_json: Option<String>,
}

#[derive(FromRow)]
struct SourceTaskRow {
    task_id: String,
    status: String,
}

#[derive(FromRow)]
struct TaskOutputRow {
    task_id: String,
    output_id: String,
    ordinal: i64,
    status: String,
}

#[derive(FromRow)]
struct ProductionStageItemRow {
    stage_item_id: String,
    status: String,
    asset_id: Option<String>,
    source_asset_id: Option<String>,
}

#[derive(FromRow)]
struct ReviewRow {
    review_id: String,
}

#[derive(FromRow)]
struct ShotProfileUsageRow {
    shot_id: String,
    shot_name: String,
    role: String,
    ordinal: i64,
    costume_variant_id: Option<String>,
}

#[derive(FromRow)]
struct ScopeProfileUsageRow {
    scope_type: String,
    scope_id: String,
    scope_name: String,
    role: String,
    ordinal: i64,
    costume_variant_id: Option<String>,
}

#[derive(FromRow)]
struct CostumeVariantUsageRow {
    costume_id: String,
    costume_name: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct ProfileRelationRow {
    profile_type: String,
    profile_id: String,
    profile_name: Option<String>,
    relation_id: Option<String>,
}

#[derive(FromRow)]
struct RevisionUsageRow {
    revision_id: String,
    revision_number: i64,
    status: String,
}

#[derive(FromRow)]
struct ReferenceSetItemUsageRow {
    asset_id: String,
    asset_name: Option<String>,
    ordinal: i64,
    role: Option<String>,
}

const SCOPE_NAME_EXPR: &str = "COALESCE(
    CASE WHEN b.scope_type = 'PROJECT' THEN p.name END,
    CASE WHEN b.scope_type = 'SERIES' THEN series.name END,
    CASE WHEN b.scope_type = 'EPISODE' THEN episode.name END,
    CASE WHEN b.scope_type = 'SCENE' THEN scene.name END,
    b.scope_id
)";

#[async_trait]
impl AssetUsageRepository for SqliteAssetUsageRepository {
    async fn asset_usage(
        &self,
        project_id: &str,
        asset_id: &AssetId,
    ) -> Result<AssetUsageSummary, RepositoryError> {
        load_asset_usage(&self.pool, project_id, asset_id).await
    }

    async fn asset_usage_for(
        &self,
        project_id: &str,
        asset_id: &AssetId,
    ) -> Result<AssetUsageSummary, RepositoryError> {
        load_asset_usage(&self.pool, project_id, asset_id).await
    }

    async fn profile_usage(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<ProfileUsageSummary, RepositoryError> {
        load_profile_usage(&self.pool, project_id, profile_type, profile_id).await
    }

    async fn profile_usage_for(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<ProfileUsageSummary, RepositoryError> {
        load_profile_usage(&self.pool, project_id, profile_type, profile_id).await
    }

    async fn reference_set_usage(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<ReferenceSetUsageSummary, RepositoryError> {
        load_reference_set_usage(&self.pool, project_id, reference_set_id).await
    }

    async fn reference_set_usage_for(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<ReferenceSetUsageSummary, RepositoryError> {
        load_reference_set_usage(&self.pool, project_id, reference_set_id).await
    }
}

async fn load_asset_usage(
    pool: &SqlitePool,
    project_id: &str,
    asset_id: &AssetId,
) -> Result<AssetUsageSummary, RepositoryError> {
    let mut summary = AssetUsageSummary::new(asset_id.as_str());

    // The project/asset membership check is the first read.  Every following
    // query repeats the project predicate because relation tables are not all
    // keyed by project_id and can contain legacy/manual rows.
    let exists =
        sqlx::query_scalar::<_, String>("SELECT id FROM assets WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(asset_id.as_str())
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx_error)?
            .is_some();
    if !exists {
        return Ok(summary);
    }

    let reference_sets = sqlx::query_as::<_, ReferenceSetAssetRow>(
        "SELECT rs.id AS reference_set_id, rs.name AS reference_set_name,
                rs.purpose, rsi.ordinal, rsi.role
         FROM reference_set_items rsi
         INNER JOIN reference_sets rs ON rs.id = rsi.reference_set_id
         WHERE rs.project_id = ? AND rsi.asset_id = ?
         ORDER BY rs.name COLLATE NOCASE ASC, rs.id ASC, rsi.ordinal ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in reference_sets {
        push_unique(
            &mut summary.reference_sets,
            AssetUsageItem::new(
                "REFERENCE_SET",
                row.reference_set_id.clone(),
                row.reference_set_name.clone(),
                "REFERENCE_SET_ITEM",
                None,
                None,
                None,
                None,
                Some(row.reference_set_id),
                true,
                format_reference_set_item_detail(&row.purpose, row.role.as_deref(), row.ordinal),
            ),
        );
    }

    let owners = sqlx::query_as::<_, ReferenceSetOwnerRow>(&format!(
        "SELECT rs.id AS reference_set_id, rs.name AS reference_set_name,
                rs.owner_profile_type AS profile_type, rs.owner_profile_id AS profile_id,
                COALESCE(cp.name, sp.name, pp.name, stp.name, rs.owner_profile_id) AS profile_name
         FROM reference_set_items rsi
         INNER JOIN reference_sets rs ON rs.id = rsi.reference_set_id
         LEFT JOIN character_profiles cp
           ON rs.owner_profile_type = 'CHARACTER'
          AND cp.id = rs.owner_profile_id AND cp.project_id = rs.project_id
         LEFT JOIN scene_profiles sp
           ON rs.owner_profile_type = 'SCENE'
          AND sp.id = rs.owner_profile_id AND sp.project_id = rs.project_id
         LEFT JOIN prop_profiles pp
           ON rs.owner_profile_type = 'PROP'
          AND pp.id = rs.owner_profile_id AND pp.project_id = rs.project_id
         LEFT JOIN style_profiles stp
           ON rs.owner_profile_type = 'STYLE'
          AND stp.id = rs.owner_profile_id AND stp.project_id = rs.project_id
         WHERE rs.project_id = ? AND rsi.asset_id = ?
           AND rs.owner_profile_type IS NOT NULL AND rs.owner_profile_id IS NOT NULL
         ORDER BY rs.name COLLATE NOCASE ASC, rs.id ASC"
    ))
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in owners {
        let Some(profile_id) = row.profile_id else {
            continue;
        };
        push_unique(
            &mut summary.profiles,
            AssetUsageItem::new(
                "PROFILE",
                profile_id,
                row.profile_name,
                "REFERENCE_SET_OWNER",
                None,
                None,
                None,
                Some(row.profile_type),
                Some(row.reference_set_id),
                true,
                format!(
                    "该素材所在的参考集“{}”由此档案拥有。",
                    row.reference_set_name
                ),
            ),
        );
    }

    let default_profiles = sqlx::query_as::<_, ProfileReferenceRow>(
        "SELECT 'CHARACTER' AS profile_type, cp.id AS profile_id, cp.name AS profile_name,
                rs.id AS reference_set_id, rs.name AS reference_set_name
         FROM character_profiles cp
         INNER JOIN reference_sets rs
           ON rs.id = cp.default_reference_set_id AND rs.project_id = cp.project_id
         INNER JOIN reference_set_items rsi ON rsi.reference_set_id = rs.id
         WHERE cp.project_id = ? AND rsi.asset_id = ?
         UNION ALL
         SELECT 'SCENE' AS profile_type, sp.id AS profile_id, sp.name AS profile_name,
                rs.id AS reference_set_id, rs.name AS reference_set_name
         FROM scene_profiles sp
         INNER JOIN reference_sets rs
           ON rs.id = sp.default_reference_set_id AND rs.project_id = sp.project_id
         INNER JOIN reference_set_items rsi ON rsi.reference_set_id = rs.id
         WHERE sp.project_id = ? AND rsi.asset_id = ?
         UNION ALL
         SELECT 'PROP' AS profile_type, pp.id AS profile_id, pp.name AS profile_name,
                rs.id AS reference_set_id, rs.name AS reference_set_name
         FROM prop_profiles pp
         INNER JOIN reference_sets rs
           ON rs.id = pp.default_reference_set_id AND rs.project_id = pp.project_id
         INNER JOIN reference_set_items rsi ON rsi.reference_set_id = rs.id
         WHERE pp.project_id = ? AND rsi.asset_id = ?
         ORDER BY profile_type ASC, profile_name COLLATE NOCASE ASC, profile_id ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .bind(project_id)
    .bind(asset_id.as_str())
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in default_profiles {
        push_unique(
            &mut summary.profiles,
            AssetUsageItem::new(
                "PROFILE",
                row.profile_id,
                row.profile_name,
                "PROFILE_DEFAULT_REFERENCE_SET",
                None,
                None,
                None,
                Some(row.profile_type),
                Some(row.reference_set_id),
                true,
                format!("该档案的默认参考集“{}”包含此素材。", row.reference_set_name),
            ),
        );
    }

    let costume_references = sqlx::query_as::<_, CostumeReferenceRow>(
        "SELECT cv.id AS costume_id, cv.name AS costume_name,
                cp.id AS character_profile_id, cp.name AS character_profile_name,
                rs.id AS reference_set_id, rs.name AS reference_set_name
         FROM costume_variants cv
         INNER JOIN character_profiles cp ON cp.id = cv.character_profile_id
         INNER JOIN reference_sets rs
           ON rs.id = cv.reference_set_id AND rs.project_id = cp.project_id
         INNER JOIN reference_set_items rsi ON rsi.reference_set_id = rs.id
         WHERE cp.project_id = ? AND rsi.asset_id = ?
         ORDER BY cp.name COLLATE NOCASE ASC, cv.ordinal ASC, cv.id ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in costume_references {
        push_unique(
            &mut summary.profiles,
            AssetUsageItem::new(
                "COSTUME_VARIANT",
                row.costume_id,
                row.costume_name,
                "COSTUME_REFERENCE_SET",
                None,
                None,
                None,
                Some("COSTUME".to_owned()),
                Some(row.reference_set_id),
                true,
                format!(
                    "服装变体属于角色档案“{}”，其参考集“{}”包含此素材。",
                    row.character_profile_name, row.reference_set_name
                ),
            ),
        );
    }

    let shot_reference_sets = sqlx::query_as::<_, ShotReferenceSetRow>(
        "SELECT b.shot_id, s.name AS shot_name, b.reference_set_id,
                rs.name AS reference_set_name, b.role, b.ordinal
         FROM shot_reference_set_bindings b
         INNER JOIN shots s ON s.id = b.shot_id
         INNER JOIN reference_sets rs ON rs.id = b.reference_set_id
         INNER JOIN reference_set_items rsi ON rsi.reference_set_id = rs.id
         WHERE s.project_id = ? AND rs.project_id = ? AND rsi.asset_id = ?
         ORDER BY s.ordinal ASC, s.id ASC, b.role ASC, b.ordinal ASC, b.id ASC",
    )
    .bind(project_id)
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in shot_reference_sets {
        push_unique(
            &mut summary.shots,
            AssetUsageItem::new(
                "SHOT",
                row.shot_id.clone(),
                row.shot_name,
                "REFERENCE_SET_SHOT_BINDING",
                None,
                None,
                Some(row.shot_id),
                None,
                Some(row.reference_set_id),
                true,
                format!(
                    "镜头通过 {} 绑定参考集“{}”（第 {} 个）。",
                    row.role,
                    row.reference_set_name,
                    row.ordinal.saturating_add(1)
                ),
            ),
        );
    }

    let scope_reference_sets = sqlx::query_as::<_, ScopeReferenceSetRow>(&format!(
        "SELECT b.scope_type, b.scope_id, {SCOPE_NAME_EXPR} AS scope_name,
                b.reference_set_id, rs.name AS reference_set_name, b.role, b.ordinal
         FROM consistency_scope_reference_set_bindings b
         INNER JOIN reference_sets rs ON rs.id = b.reference_set_id
         INNER JOIN reference_set_items rsi ON rsi.reference_set_id = rs.id
         LEFT JOIN projects p ON b.scope_type = 'PROJECT' AND p.id = b.scope_id
         LEFT JOIN production_series series
           ON b.scope_type = 'SERIES' AND series.id = b.scope_id AND series.project_id = b.project_id
         LEFT JOIN production_episodes episode
           ON b.scope_type = 'EPISODE' AND episode.id = b.scope_id
         LEFT JOIN production_series episode_series
           ON episode_series.id = episode.series_id AND episode_series.project_id = b.project_id
         LEFT JOIN production_scenes scene
           ON b.scope_type = 'SCENE' AND scene.id = b.scope_id
         LEFT JOIN production_episodes scene_episode ON scene_episode.id = scene.episode_id
         LEFT JOIN production_series scene_series
           ON scene_series.id = scene_episode.series_id AND scene_series.project_id = b.project_id
         WHERE b.project_id = ? AND rs.project_id = ? AND rsi.asset_id = ?
         ORDER BY b.scope_type ASC, b.scope_id ASC, b.role ASC, b.ordinal ASC, b.id ASC"
    ))
    .bind(project_id)
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in scope_reference_sets {
        push_unique(
            &mut summary.shots,
            AssetUsageItem::new(
                "SCOPE",
                row.scope_id.clone(),
                row.scope_name,
                "REFERENCE_SET_SCOPE_BINDING",
                Some(row.scope_type),
                Some(row.scope_id),
                None,
                None,
                Some(row.reference_set_id),
                true,
                format!(
                    "范围通过 {} 绑定参考集“{}”（第 {} 个）。",
                    row.role,
                    row.reference_set_name,
                    row.ordinal.saturating_add(1)
                ),
            ),
        );
    }

    let selected_shots = sqlx::query_as::<_, SelectedShotRow>(
        "SELECT id AS shot_id, name AS shot_name,
                selected_image_asset_id, selected_video_asset_id
         FROM shots
         WHERE project_id = ?
           AND (selected_image_asset_id = ? OR selected_video_asset_id = ?)
         ORDER BY ordinal ASC, id ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in selected_shots {
        if row.selected_image_asset_id.as_deref() == Some(asset_id.as_str()) {
            let item = AssetUsageItem::new(
                "SHOT",
                row.shot_id.clone(),
                row.shot_name.clone(),
                "SELECTED_IMAGE_ASSET",
                None,
                None,
                Some(row.shot_id.clone()),
                None,
                None,
                true,
                "该镜头将此素材选为图片关键帧。",
            );
            push_unique(&mut summary.shots, item.clone());
            push_unique(&mut summary.selected_keyframes, item);
        }
        if row.selected_video_asset_id.as_deref() == Some(asset_id.as_str()) {
            let item = AssetUsageItem::new(
                "SHOT",
                row.shot_id.clone(),
                row.shot_name,
                "SELECTED_VIDEO_ASSET",
                None,
                None,
                Some(row.shot_id),
                None,
                None,
                true,
                "该镜头将此素材选为视频关键帧。",
            );
            push_unique(&mut summary.shots, item.clone());
            push_unique(&mut summary.selected_keyframes, item);
        }
    }

    let anchors = sqlx::query_as::<_, LegacyAnchorRow>(
        "SELECT a.id AS anchor_id, a.name AS anchor_name, aa.ordinal
         FROM reference_anchor_assets aa
         INNER JOIN reference_anchors a ON a.id = aa.anchor_id
         WHERE a.project_id = ? AND aa.asset_id = ?
         ORDER BY a.name COLLATE NOCASE ASC, a.id ASC, aa.ordinal ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in anchors {
        push_unique(
            &mut summary.legacy_references,
            AssetUsageItem::new(
                "REFERENCE_ANCHOR",
                row.anchor_id,
                row.anchor_name,
                "REFERENCE_ANCHOR_ASSET",
                None,
                None,
                None,
                None,
                None,
                true,
                format!(
                    "旧版参考锚点仍按第 {} 项使用此素材。",
                    row.ordinal.saturating_add(1)
                ),
            ),
        );
    }

    let legacy_shot_references = sqlx::query_as::<_, LegacyShotReferenceRow>(
        "SELECT r.shot_id, s.name AS shot_name, r.stage, r.ordinal
         FROM shot_reference_assets r
         INNER JOIN shots s ON s.id = r.shot_id
         WHERE s.project_id = ? AND r.asset_id = ?
         ORDER BY s.ordinal ASC, s.id ASC, r.stage ASC, r.ordinal ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in legacy_shot_references {
        push_unique(
            &mut summary.legacy_references,
            AssetUsageItem::new(
                "SHOT",
                row.shot_id.clone(),
                row.shot_name,
                "LEGACY_SHOT_REFERENCE",
                None,
                None,
                Some(row.shot_id),
                None,
                None,
                true,
                format!(
                    "旧版镜头 {} 参考仍按第 {} 项使用此素材。",
                    row.stage,
                    row.ordinal.saturating_add(1)
                ),
            ),
        );
    }

    let source_task = sqlx::query_as::<_, SourceTaskRow>(
        "SELECT t.id AS task_id, t.status
         FROM assets a
         INNER JOIN tasks t ON t.id = a.source_task_id
         WHERE a.project_id = ? AND a.id = ?",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx_error)?;
    if let Some(row) = source_task {
        let status = TaskStatus::try_from_db(&row.status)
            .map_err(|error| map_domain_error("source task status", error))?;
        push_unique(
            &mut summary.production_history,
            AssetUsageItem::new(
                "TASK",
                row.task_id.clone(),
                row.task_id.clone(),
                "ASSET_SOURCE_TASK",
                None,
                None,
                None,
                None,
                None,
                !status.is_terminal(),
                if status.is_terminal() {
                    "该素材由已结束的历史任务生成。"
                } else {
                    "该素材的来源任务仍处于活动状态。"
                },
            ),
        );
    }

    let output_rows = sqlx::query_as::<_, TaskOutputRow>(
        "SELECT o.task_id, o.output_id, o.ordinal, t.status
         FROM task_output_assets o
         INNER JOIN tasks t ON t.id = o.task_id
         WHERE t.project_id = ? AND o.asset_id = ?
         ORDER BY o.task_id ASC, o.output_id ASC, o.ordinal ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in output_rows {
        let status = TaskStatus::try_from_db(&row.status)
            .map_err(|error| map_domain_error("task output status", error))?;
        push_unique(
            &mut summary.production_history,
            AssetUsageItem::new(
                "TASK",
                row.task_id.clone(),
                row.task_id.clone(),
                "TASK_OUTPUT_ASSET",
                None,
                None,
                None,
                None,
                None,
                !status.is_terminal(),
                format!(
                    "该素材是任务 {} 的输出 {}（第 {} 项）。",
                    row.task_id,
                    row.output_id,
                    row.ordinal.saturating_add(1)
                ),
            ),
        );
    }

    let stage_rows = sqlx::query_as::<_, ProductionStageItemRow>(
        "SELECT i.id AS stage_item_id, i.status, i.asset_id, i.source_asset_id
         FROM production_stage_items i
         INNER JOIN production_stages stage ON stage.id = i.stage_id
         INNER JOIN production_runs run ON run.id = stage.run_id
         WHERE run.project_id = ? AND (i.asset_id = ? OR i.source_asset_id = ?)
         ORDER BY i.id ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in stage_rows {
        let selected_as = match (
            row.asset_id.as_deref() == Some(asset_id.as_str()),
            row.source_asset_id.as_deref() == Some(asset_id.as_str()),
        ) {
            (true, true) => "output and source",
            (true, false) => "output",
            (false, true) => "source",
            (false, false) => "asset",
        };
        push_unique(
            &mut summary.production_history,
            AssetUsageItem::new(
                "PRODUCTION_STAGE_ITEM",
                row.stage_item_id.clone(),
                row.stage_item_id.clone(),
                "PRODUCTION_STAGE_ASSET",
                None,
                None,
                None,
                None,
                None,
                is_live_production_status(&row.status),
                format!(
                    "生产阶段项目 {} 将此素材作为 {} 使用（状态 {}）。",
                    row.stage_item_id, selected_as, row.status
                ),
            ),
        );
    }

    let queue_rows = sqlx::query_as::<_, ProductionQueueRow>(
        "SELECT i.id AS item_id, i.status, i.values_json, i.task_id
         FROM production_batch_items i
         INNER JOIN production_batches b ON b.id = i.batch_id
         WHERE b.project_id = ?
         ORDER BY i.id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in queue_rows {
        let values: Value = serde_json::from_str(&row.values_json).map_err(|error| {
            RepositoryError::serialization("production values_json", error.to_string())
        })?;
        if !extract_asset_ids(&values).contains(asset_id.as_str()) {
            continue;
        }
        let status = ProductionBatchItemStatus::parse(&row.status)
            .map_err(|error| map_domain_error("production batch item status", error))?;
        push_unique(
            &mut summary.production_history,
            AssetUsageItem::new(
                "PRODUCTION_ITEM",
                row.item_id.clone(),
                row.item_id.clone(),
                "PRODUCTION_QUEUE_ASSET",
                None,
                None,
                None,
                None,
                None,
                !status.is_terminal(),
                if status.is_terminal() {
                    format!(
                        "生产队列项目 {} 已结束，历史输入仍引用此素材。",
                        row.item_id
                    )
                } else {
                    format!("生产队列项目 {} 仍处于活动状态并引用此素材。", row.item_id)
                },
            ),
        );
    }

    let task_rows = sqlx::query_as::<_, TaskReferenceRow>(
        "SELECT t.id AS task_id, t.status,
                s.user_inputs_json, s.resolved_inputs_json
         FROM tasks t
         LEFT JOIN generation_snapshots s ON s.task_id = t.id
         WHERE t.project_id = ?
         ORDER BY t.id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in task_rows {
        let status = TaskStatus::try_from_db(&row.status)
            .map_err(|error| map_domain_error("task status", error))?;
        let mut task_assets = HashSet::new();
        if let Some(json) = row.user_inputs_json.as_deref() {
            task_assets.extend(parse_snapshot_assets("snapshot user_inputs_json", json)?);
        }
        if let Some(json) = row.resolved_inputs_json.as_deref() {
            task_assets.extend(parse_snapshot_assets(
                "snapshot resolved_inputs_json",
                json,
            )?);
        }
        if !task_assets.contains(asset_id.as_str()) {
            continue;
        }
        push_unique(
            &mut summary.production_history,
            AssetUsageItem::new(
                "TASK",
                row.task_id.clone(),
                row.task_id.clone(),
                "GENERATION_SNAPSHOT_ASSET",
                None,
                None,
                None,
                None,
                None,
                !status.is_terminal(),
                if status.is_terminal() {
                    "历史生成快照仍记录此素材输入。"
                } else {
                    "活动生成任务仍记录此素材输入。"
                },
            ),
        );
    }

    let reviews = sqlx::query_as::<_, ReviewRow>(
        "SELECT id AS review_id
         FROM production_item_reviews
         WHERE project_id = ? AND result_asset_id = ?
         ORDER BY id ASC",
    )
    .bind(project_id)
    .bind(asset_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in reviews {
        push_unique(
            &mut summary.production_history,
            AssetUsageItem::new(
                "REVIEW",
                row.review_id,
                "历史审片版本",
                "PRODUCTION_REVIEW_ASSET",
                None,
                None,
                None,
                None,
                None,
                false,
                "历史审片记录引用此素材，不会阻止删除。",
            ),
        );
    }

    summary.finish();
    Ok(summary)
}

async fn load_profile_usage(
    pool: &SqlitePool,
    project_id: &str,
    profile_type: ProfileType,
    profile_id: &str,
) -> Result<ProfileUsageSummary, RepositoryError> {
    let mut summary = ProfileUsageSummary::new(profile_type, profile_id);
    let table = match profile_type {
        ProfileType::Character => "character_profiles",
        ProfileType::Scene => "scene_profiles",
        ProfileType::Prop => "prop_profiles",
        ProfileType::Style => "style_profiles",
    };
    let profile_exists = sqlx::query_scalar::<_, String>(&format!(
        "SELECT id FROM {table} WHERE project_id = ? AND id = ?"
    ))
    .bind(project_id)
    .bind(profile_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx_error)?
    .is_some();
    if !profile_exists {
        return Ok(summary);
    }

    let shot_bindings = sqlx::query_as::<_, ShotProfileUsageRow>(
        "SELECT b.shot_id, s.name AS shot_name, b.role, b.ordinal, b.costume_variant_id
         FROM shot_profile_bindings b
         INNER JOIN shots s ON s.id = b.shot_id
         WHERE s.project_id = ? AND b.profile_type = ? AND b.profile_id = ?
         ORDER BY s.ordinal ASC, s.id ASC, b.role ASC, b.ordinal ASC, b.id ASC",
    )
    .bind(project_id)
    .bind(profile_type.as_str())
    .bind(profile_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in shot_bindings {
        let costume_detail = row
            .costume_variant_id
            .as_deref()
            .map(|id| format!("，服装变体 {id}"))
            .unwrap_or_default();
        push_unique(
            &mut summary.shot_bindings,
            AssetUsageItem::new(
                "SHOT",
                row.shot_id.clone(),
                row.shot_name,
                "SHOT_PROFILE_BINDING",
                None,
                None,
                Some(row.shot_id),
                Some(profile_type.as_str().to_owned()),
                None,
                true,
                format!(
                    "镜头通过 {} 绑定此档案（第 {} 个{}）。",
                    row.role,
                    row.ordinal.saturating_add(1),
                    costume_detail
                ),
            ),
        );
    }

    let scope_bindings = sqlx::query_as::<_, ScopeProfileUsageRow>(&format!(
        "SELECT b.scope_type, b.scope_id, {SCOPE_NAME_EXPR} AS scope_name,
                b.role, b.ordinal, b.costume_variant_id
         FROM consistency_scope_profile_bindings b
         LEFT JOIN projects p ON b.scope_type = 'PROJECT' AND p.id = b.scope_id
         LEFT JOIN production_series series
           ON b.scope_type = 'SERIES' AND series.id = b.scope_id AND series.project_id = b.project_id
         LEFT JOIN production_episodes episode
           ON b.scope_type = 'EPISODE' AND episode.id = b.scope_id
         LEFT JOIN production_series episode_series
           ON episode_series.id = episode.series_id AND episode_series.project_id = b.project_id
         LEFT JOIN production_scenes scene
           ON b.scope_type = 'SCENE' AND scene.id = b.scope_id
         LEFT JOIN production_episodes scene_episode ON scene_episode.id = scene.episode_id
         LEFT JOIN production_series scene_series
           ON scene_series.id = scene_episode.series_id AND scene_series.project_id = b.project_id
         WHERE b.project_id = ? AND b.profile_type = ? AND b.profile_id = ?
         ORDER BY b.scope_type ASC, b.scope_id ASC, b.role ASC, b.ordinal ASC, b.id ASC"
    ))
    .bind(project_id)
    .bind(profile_type.as_str())
    .bind(profile_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in scope_bindings {
        push_unique(
            &mut summary.scope_bindings,
            AssetUsageItem::new(
                "SCOPE",
                row.scope_id.clone(),
                row.scope_name,
                "SCOPE_PROFILE_BINDING",
                Some(row.scope_type),
                Some(row.scope_id),
                None,
                Some(profile_type.as_str().to_owned()),
                None,
                true,
                format!(
                    "范围通过 {} 绑定此档案（第 {} 个）。",
                    row.role,
                    row.ordinal.saturating_add(1)
                ),
            ),
        );
    }

    let owners = sqlx::query_as::<_, ReferenceSetOwnerRow>(&format!(
        "SELECT rs.id AS reference_set_id, rs.name AS reference_set_name,
                rs.owner_profile_type AS profile_type, rs.owner_profile_id AS profile_id,
                COALESCE(cp.name, sp.name, pp.name, stp.name, rs.owner_profile_id) AS profile_name
         FROM reference_sets rs
         LEFT JOIN character_profiles cp
           ON rs.owner_profile_type = 'CHARACTER'
          AND cp.id = rs.owner_profile_id AND cp.project_id = rs.project_id
         LEFT JOIN scene_profiles sp
           ON rs.owner_profile_type = 'SCENE'
          AND sp.id = rs.owner_profile_id AND sp.project_id = rs.project_id
         LEFT JOIN prop_profiles pp
           ON rs.owner_profile_type = 'PROP'
          AND pp.id = rs.owner_profile_id AND pp.project_id = rs.project_id
         LEFT JOIN style_profiles stp
           ON rs.owner_profile_type = 'STYLE'
          AND stp.id = rs.owner_profile_id AND stp.project_id = rs.project_id
         WHERE rs.project_id = ? AND rs.owner_profile_type = ? AND rs.owner_profile_id = ?
         ORDER BY rs.name COLLATE NOCASE ASC, rs.id ASC"
    ))
    .bind(project_id)
    .bind(profile_type.as_str())
    .bind(profile_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in owners {
        push_unique(
            &mut summary.reference_sets,
            AssetUsageItem::new(
                "REFERENCE_SET",
                row.reference_set_id.clone(),
                row.reference_set_name.clone(),
                "REFERENCE_SET_OWNER",
                None,
                None,
                None,
                Some(profile_type.as_str().to_owned()),
                Some(row.reference_set_id),
                true,
                format!("参考集“{}”将此档案作为 owner。", row.reference_set_name),
            ),
        );
    }

    let default_style_users = if profile_type == ProfileType::Style {
        sqlx::query_as::<_, ProfileRelationRow>(
            "SELECT 'CHARACTER' AS profile_type, cp.id AS profile_id, cp.name AS profile_name,
                    cp.default_style_profile_id AS relation_id
             FROM character_profiles cp
             INNER JOIN style_profiles st
               ON st.id = cp.default_style_profile_id AND st.project_id = cp.project_id
             WHERE cp.project_id = ? AND st.id = ?
             UNION ALL
             SELECT 'SCENE' AS profile_type, sp.id AS profile_id, sp.name AS profile_name,
                    sp.default_style_profile_id AS relation_id
             FROM scene_profiles sp
             INNER JOIN style_profiles st
               ON st.id = sp.default_style_profile_id AND st.project_id = sp.project_id
             WHERE sp.project_id = ? AND st.id = ?
             ORDER BY profile_type ASC, profile_name COLLATE NOCASE ASC, profile_id ASC",
        )
        .bind(project_id)
        .bind(profile_id)
        .bind(project_id)
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx_error)?
    } else {
        Vec::new()
    };
    for row in default_style_users {
        let Some(profile_name) = row.profile_name else {
            continue;
        };
        push_unique(
            &mut summary.default_style_profiles,
            AssetUsageItem::new(
                "PROFILE",
                row.profile_id,
                profile_name,
                "PROFILE_DEFAULT_STYLE",
                None,
                None,
                None,
                Some(row.profile_type),
                None,
                true,
                "该档案被另一个 Profile 作为默认风格使用。",
            ),
        );
    }

    if matches!(profile_type, ProfileType::Character | ProfileType::Scene) {
        let row = if profile_type == ProfileType::Character {
            sqlx::query_as::<_, ProfileRelationRow>(
                "SELECT 'STYLE' AS profile_type, st.id AS profile_id, st.name AS profile_name,
                        cp.default_style_profile_id AS relation_id
                 FROM character_profiles cp
                 LEFT JOIN style_profiles st
                   ON st.id = cp.default_style_profile_id AND st.project_id = cp.project_id
                 WHERE cp.project_id = ? AND cp.id = ?",
            )
            .bind(project_id)
            .bind(profile_id)
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx_error)?
        } else {
            sqlx::query_as::<_, ProfileRelationRow>(
                "SELECT 'STYLE' AS profile_type, st.id AS profile_id, st.name AS profile_name,
                        sp.default_style_profile_id AS relation_id
                 FROM scene_profiles sp
                 LEFT JOIN style_profiles st
                   ON st.id = sp.default_style_profile_id AND st.project_id = sp.project_id
                 WHERE sp.project_id = ? AND sp.id = ?",
            )
            .bind(project_id)
            .bind(profile_id)
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx_error)?
        };
        if let Some(row) = row {
            if let (Some(style_id), Some(style_name)) = (row.relation_id, row.profile_name) {
                push_unique(
                    &mut summary.related_profiles,
                    AssetUsageItem::new(
                        "PROFILE",
                        style_id,
                        style_name,
                        "DEFAULT_STYLE_PROFILE",
                        None,
                        None,
                        None,
                        Some("STYLE".to_owned()),
                        None,
                        false,
                        "此档案指向该默认 Style Profile。",
                    ),
                );
            }
        }
    }

    if profile_type == ProfileType::Character {
        let costumes = sqlx::query_as::<_, CostumeVariantUsageRow>(
            "SELECT cv.id AS costume_id, cv.name AS costume_name, cv.ordinal
             FROM costume_variants cv
             INNER JOIN character_profiles cp ON cp.id = cv.character_profile_id
             WHERE cp.project_id = ? AND cv.character_profile_id = ?
             ORDER BY cv.ordinal ASC, cv.id ASC",
        )
        .bind(project_id)
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx_error)?;
        for row in costumes {
            push_unique(
                &mut summary.costume_variants,
                AssetUsageItem::new(
                    "COSTUME_VARIANT",
                    row.costume_id,
                    row.costume_name,
                    "CHARACTER_COSTUME_VARIANT",
                    None,
                    None,
                    None,
                    Some("COSTUME".to_owned()),
                    None,
                    true,
                    format!(
                        "该服装变体是角色档案的子关系（第 {} 个）。",
                        row.ordinal.saturating_add(1)
                    ),
                ),
            );
        }
    }

    let revisions = sqlx::query_as::<_, RevisionUsageRow>(&format!(
        "SELECT r.id AS revision_id, r.revision_number, r.status
         FROM profile_revisions r
         INNER JOIN {table} p ON p.id = r.profile_id
         WHERE p.project_id = ? AND r.profile_type = ? AND r.profile_id = ?
         ORDER BY r.revision_number ASC, r.id ASC"
    ))
    .bind(project_id)
    .bind(profile_type.as_str())
    .bind(profile_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in revisions {
        push_unique(
            &mut summary.related_profiles,
            AssetUsageItem::new(
                "PROFILE_REVISION",
                row.revision_id,
                format!("Profile revision {}", row.revision_number),
                "PROFILE_REVISION_HISTORY",
                None,
                None,
                None,
                Some(profile_type.as_str().to_owned()),
                None,
                true,
                format!("不可变 Profile revision（{}）。", row.status),
            ),
        );
    }

    summary.finish();
    Ok(summary)
}

async fn load_reference_set_usage(
    pool: &SqlitePool,
    project_id: &str,
    reference_set_id: &str,
) -> Result<ReferenceSetUsageSummary, RepositoryError> {
    let mut summary = ReferenceSetUsageSummary::new(reference_set_id);
    let exists = sqlx::query_scalar::<_, String>(
        "SELECT id FROM reference_sets WHERE project_id = ? AND id = ?",
    )
    .bind(project_id)
    .bind(reference_set_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx_error)?
    .is_some();
    if !exists {
        return Ok(summary);
    }

    summary.item_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM reference_set_items rsi
         INNER JOIN reference_sets rs ON rs.id = rsi.reference_set_id
         WHERE rs.project_id = ? AND rsi.reference_set_id = ?",
    )
    .bind(project_id)
    .bind(reference_set_id)
    .fetch_one(pool)
    .await
    .map_err(map_sqlx_error)?
    .max(0) as usize;

    let owner = sqlx::query_as::<_, ReferenceSetOwnerRow>(&format!(
        "SELECT rs.id AS reference_set_id, rs.name AS reference_set_name,
                rs.owner_profile_type AS profile_type, rs.owner_profile_id AS profile_id,
                COALESCE(cp.name, sp.name, pp.name, stp.name, rs.owner_profile_id) AS profile_name
         FROM reference_sets rs
         LEFT JOIN character_profiles cp
           ON rs.owner_profile_type = 'CHARACTER'
          AND cp.id = rs.owner_profile_id AND cp.project_id = rs.project_id
         LEFT JOIN scene_profiles sp
           ON rs.owner_profile_type = 'SCENE'
          AND sp.id = rs.owner_profile_id AND sp.project_id = rs.project_id
         LEFT JOIN prop_profiles pp
           ON rs.owner_profile_type = 'PROP'
          AND pp.id = rs.owner_profile_id AND pp.project_id = rs.project_id
         LEFT JOIN style_profiles stp
           ON rs.owner_profile_type = 'STYLE'
          AND stp.id = rs.owner_profile_id AND stp.project_id = rs.project_id
         WHERE rs.project_id = ? AND rs.id = ?
           AND rs.owner_profile_type IS NOT NULL AND rs.owner_profile_id IS NOT NULL"
    ))
    .bind(project_id)
    .bind(reference_set_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx_error)?;
    if let Some(row) = owner {
        if let Some(profile_id) = row.profile_id {
            summary.owner = Some(AssetUsageItem::new(
                "PROFILE",
                profile_id,
                row.profile_name,
                "REFERENCE_SET_OWNER",
                None,
                None,
                None,
                Some(row.profile_type),
                Some(row.reference_set_id),
                false,
                "该档案拥有此参考集；owner 元数据本身不阻止删除参考集。",
            ));
        }
    }

    let defaults = sqlx::query_as::<_, ProfileReferenceRow>(
        "SELECT 'CHARACTER' AS profile_type, cp.id AS profile_id, cp.name AS profile_name,
                rs.id AS reference_set_id, rs.name AS reference_set_name
         FROM character_profiles cp
         INNER JOIN reference_sets rs ON rs.id = cp.default_reference_set_id
         WHERE cp.project_id = ? AND rs.project_id = cp.project_id AND rs.id = ?
         UNION ALL
         SELECT 'SCENE' AS profile_type, sp.id AS profile_id, sp.name AS profile_name,
                rs.id AS reference_set_id, rs.name AS reference_set_name
         FROM scene_profiles sp
         INNER JOIN reference_sets rs ON rs.id = sp.default_reference_set_id
         WHERE sp.project_id = ? AND rs.project_id = sp.project_id AND rs.id = ?
         UNION ALL
         SELECT 'PROP' AS profile_type, pp.id AS profile_id, pp.name AS profile_name,
                rs.id AS reference_set_id, rs.name AS reference_set_name
         FROM prop_profiles pp
         INNER JOIN reference_sets rs ON rs.id = pp.default_reference_set_id
         WHERE pp.project_id = ? AND rs.project_id = pp.project_id AND rs.id = ?
         ORDER BY profile_type ASC, profile_name COLLATE NOCASE ASC, profile_id ASC",
    )
    .bind(project_id)
    .bind(reference_set_id)
    .bind(project_id)
    .bind(reference_set_id)
    .bind(project_id)
    .bind(reference_set_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in defaults {
        push_unique(
            &mut summary.profile_defaults,
            AssetUsageItem::new(
                "PROFILE",
                row.profile_id,
                row.profile_name,
                "PROFILE_DEFAULT_REFERENCE_SET",
                None,
                None,
                None,
                Some(row.profile_type),
                Some(row.reference_set_id),
                true,
                format!("该档案将“{}”设为默认参考集。", row.reference_set_name),
            ),
        );
    }

    let costumes = sqlx::query_as::<_, CostumeReferenceRow>(
        "SELECT cv.id AS costume_id, cv.name AS costume_name,
                cp.id AS character_profile_id, cp.name AS character_profile_name,
                rs.id AS reference_set_id, rs.name AS reference_set_name
         FROM costume_variants cv
         INNER JOIN character_profiles cp ON cp.id = cv.character_profile_id
         INNER JOIN reference_sets rs
           ON rs.id = cv.reference_set_id AND rs.project_id = cp.project_id
         WHERE cp.project_id = ? AND rs.id = ?
         ORDER BY cp.name COLLATE NOCASE ASC, cv.ordinal ASC, cv.id ASC",
    )
    .bind(project_id)
    .bind(reference_set_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in costumes {
        push_unique(
            &mut summary.costume_variants,
            AssetUsageItem::new(
                "COSTUME_VARIANT",
                row.costume_id,
                row.costume_name,
                "COSTUME_REFERENCE_SET",
                None,
                None,
                None,
                Some("COSTUME".to_owned()),
                Some(row.reference_set_id),
                true,
                format!(
                    "服装变体属于角色档案“{}”，并引用此参考集。",
                    row.character_profile_name
                ),
            ),
        );
    }

    let shot_bindings = sqlx::query_as::<_, ShotReferenceSetRow>(
        "SELECT b.shot_id, s.name AS shot_name, b.reference_set_id,
                rs.name AS reference_set_name, b.role, b.ordinal
         FROM shot_reference_set_bindings b
         INNER JOIN shots s ON s.id = b.shot_id
         INNER JOIN reference_sets rs ON rs.id = b.reference_set_id
         WHERE s.project_id = ? AND rs.project_id = ? AND b.reference_set_id = ?
         ORDER BY s.ordinal ASC, s.id ASC, b.role ASC, b.ordinal ASC, b.id ASC",
    )
    .bind(project_id)
    .bind(project_id)
    .bind(reference_set_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in shot_bindings {
        push_unique(
            &mut summary.shot_bindings,
            AssetUsageItem::new(
                "SHOT",
                row.shot_id.clone(),
                row.shot_name,
                "SHOT_REFERENCE_SET_BINDING",
                None,
                None,
                Some(row.shot_id),
                None,
                Some(row.reference_set_id),
                true,
                format!(
                    "镜头通过 {} 绑定此参考集（第 {} 个）。",
                    row.role,
                    row.ordinal.saturating_add(1)
                ),
            ),
        );
    }

    let scope_bindings = sqlx::query_as::<_, ScopeReferenceSetRow>(&format!(
        "SELECT b.scope_type, b.scope_id, {SCOPE_NAME_EXPR} AS scope_name,
                b.reference_set_id, rs.name AS reference_set_name, b.role, b.ordinal
         FROM consistency_scope_reference_set_bindings b
         INNER JOIN reference_sets rs ON rs.id = b.reference_set_id
         LEFT JOIN projects p ON b.scope_type = 'PROJECT' AND p.id = b.scope_id
         LEFT JOIN production_series series
           ON b.scope_type = 'SERIES' AND series.id = b.scope_id AND series.project_id = b.project_id
         LEFT JOIN production_episodes episode ON b.scope_type = 'EPISODE' AND episode.id = b.scope_id
         LEFT JOIN production_series episode_series
           ON episode_series.id = episode.series_id AND episode_series.project_id = b.project_id
         LEFT JOIN production_scenes scene ON b.scope_type = 'SCENE' AND scene.id = b.scope_id
         LEFT JOIN production_episodes scene_episode ON scene_episode.id = scene.episode_id
         LEFT JOIN production_series scene_series
           ON scene_series.id = scene_episode.series_id AND scene_series.project_id = b.project_id
         WHERE b.project_id = ? AND rs.project_id = ? AND b.reference_set_id = ?
         ORDER BY b.scope_type ASC, b.scope_id ASC, b.role ASC, b.ordinal ASC, b.id ASC"
    ))
    .bind(project_id)
    .bind(project_id)
    .bind(reference_set_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in scope_bindings {
        push_unique(
            &mut summary.scope_bindings,
            AssetUsageItem::new(
                "SCOPE",
                row.scope_id.clone(),
                row.scope_name,
                "SCOPE_REFERENCE_SET_BINDING",
                Some(row.scope_type),
                Some(row.scope_id),
                None,
                None,
                Some(row.reference_set_id),
                true,
                format!(
                    "范围通过 {} 绑定此参考集（第 {} 个）。",
                    row.role,
                    row.ordinal.saturating_add(1)
                ),
            ),
        );
    }

    let reference_items = sqlx::query_as::<_, ReferenceSetItemUsageRow>(
        "SELECT rsi.asset_id, a.name AS asset_name, rsi.ordinal, rsi.role
         FROM reference_set_items rsi
         INNER JOIN reference_sets rs ON rs.id = rsi.reference_set_id
         LEFT JOIN assets a ON a.id = rsi.asset_id AND a.project_id = rs.project_id
         WHERE rs.project_id = ? AND rsi.reference_set_id = ?
         ORDER BY rsi.ordinal ASC, rsi.asset_id ASC",
    )
    .bind(project_id)
    .bind(reference_set_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    for row in reference_items {
        let display_name = row.asset_name.unwrap_or_else(|| row.asset_id.clone());
        push_unique(
            &mut summary.items,
            AssetUsageItem::new(
                "ASSET",
                row.asset_id,
                display_name,
                "REFERENCE_SET_ITEM",
                None,
                None,
                None,
                None,
                Some(reference_set_id.to_owned()),
                false,
                format_reference_set_item_detail("REFERENCE_SET", row.role.as_deref(), row.ordinal),
            ),
        );
    }

    // `finish` preserves item rows loaded above while adding the semantic
    // relation categories.  Item membership is informational and is not a
    // blocker for deleting the ReferenceSet itself.
    let item_rows = std::mem::take(&mut summary.items);
    summary.finish();
    summary.items.extend(item_rows);
    summary.total = summary.items.len();
    summary.blocking_count = summary.items.iter().filter(|item| item.blocking).count();
    Ok(summary)
}

fn push_unique(items: &mut Vec<AssetUsageItem>, item: AssetUsageItem) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn format_reference_set_item_detail(purpose: &str, role: Option<&str>, ordinal: i64) -> String {
    let role = role
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(|role| format!("，role {role}"))
        .unwrap_or_default();
    format!(
        "{purpose} ReferenceSet 第 {} 项{}引用此素材。",
        ordinal.saturating_add(1),
        role
    )
}

fn is_live_production_status(status: &str) -> bool {
    !matches!(status, "SUCCEEDED" | "FAILED" | "CANCELLED" | "SKIPPED")
}

fn parse_snapshot_assets(context: &str, json: &str) -> Result<HashSet<String>, RepositoryError> {
    let value = parse_json(context, Some(json))?
        .ok_or_else(|| RepositoryError::serialization(context, "missing value"))?;
    Ok(extract_asset_ids(&value))
}

fn extract_asset_ids(value: &Value) -> HashSet<String> {
    let mut output = HashSet::new();
    collect_asset_ids(value, &mut output);
    output
}

fn collect_asset_ids(value: &Value, output: &mut HashSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_asset_ids(item, output);
            }
        }
        Value::Object(object) => {
            if let Some(kind) = object.get("type").and_then(Value::as_str) {
                if matches!(
                    kind,
                    "image_asset"
                        | "video_asset"
                        | "audio_asset"
                        | "image_assets"
                        | "video_assets"
                        | "audio_assets"
                ) {
                    if let Some(asset_id) = object.get("assetId").and_then(Value::as_str) {
                        output.insert(asset_id.to_owned());
                    }
                    if let Some(asset_ids) = object.get("assetIds").and_then(Value::as_array) {
                        output.extend(
                            asset_ids
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned),
                        );
                    }
                }
            }
            for item in object.values() {
                collect_asset_ids(item, output);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_asset_ids, is_live_production_status};
    use serde_json::json;

    #[test]
    fn extracts_asset_inputs_recursively() {
        let ids = extract_asset_ids(&json!({
            "image": {"type": "image_asset", "assetId": "ast_one"},
            "references": {"type": "image_assets", "assetIds": ["ast_two", "ast_three"]}
        }));
        assert_eq!(ids.len(), 3);
        assert!(ids.contains("ast_one"));
        assert!(ids.contains("ast_three"));
    }

    #[test]
    fn only_terminal_production_states_are_non_blocking() {
        assert!(is_live_production_status("RUNNING"));
        assert!(!is_live_production_status("SUCCEEDED"));
        assert!(!is_live_production_status("SKIPPED"));
    }
}
