use super::map_sqlx_error;
use crate::application::ports::{
    ManifestAnchorAssetRecord, ManifestAnchorRecord, ManifestAssignmentRecord,
    ManifestCharacterProfileRecord, ManifestConsistencyRecords, ManifestCostumeVariantRecord,
    ManifestEpisodeRecord, ManifestGenerationLinkRecord, ManifestProjectRecord,
    ManifestPropProfileRecord, ManifestReferenceRecord, ManifestReferenceSetItemRecord,
    ManifestReferenceSetRecord, ManifestSceneProfileRecord, ManifestSceneRecord,
    ManifestScopeProfileBindingRecord, ManifestScopeReferenceSetBindingRecord,
    ManifestSeriesRecord, ManifestShotProfileBindingRecord, ManifestShotRecord,
    ManifestShotReferenceSetBindingRecord, ManifestStageConfigRecord, ManifestStyleProfileRecord,
    ProjectManifestRepository, ProjectManifestSnapshot, RepositoryError,
};
use async_trait::async_trait;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[derive(Clone)]
pub struct SqliteProjectManifestRepository {
    pool: SqlitePool,
}

impl SqliteProjectManifestRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectManifestRepository for SqliteProjectManifestRepository {
    async fn load_manifest_snapshot(
        &self,
        project_id: &str,
    ) -> Result<ProjectManifestSnapshot, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let project = sqlx::query_as::<_, DbProject>(
            "SELECT id, name, description FROM projects WHERE id = ?",
        )
        .bind(project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .ok_or_else(|| RepositoryError::not_found("project", project_id))?
        .into();

        let table_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name IN
               ('production_series', 'production_episodes', 'production_scenes',
                'shot_scene_assignments')",
        )
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        let (series, episodes, scenes, assignments) = if table_count == 0 {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        } else if table_count != 4 {
            return Err(RepositoryError::database(
                "生产结构表不完整，请先应用 migration 021",
            ));
        } else {
            let series = sqlx::query_as::<_, DbSeries>(
                "SELECT id, ordinal, name, description FROM production_series
                 WHERE project_id = ? ORDER BY ordinal, id",
            )
            .bind(project_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(Into::into)
            .collect();
            let episodes = sqlx::query_as::<_, DbEpisode>(
                "SELECT e.id, e.series_id, e.ordinal, e.name, e.description
                 FROM production_episodes e JOIN production_series s ON s.id = e.series_id
                 WHERE s.project_id = ? ORDER BY e.series_id, e.ordinal, e.id",
            )
            .bind(project_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(Into::into)
            .collect();
            let scenes = sqlx::query_as::<_, DbScene>(
                "SELECT c.id, c.episode_id, c.ordinal, c.name, c.description
                 FROM production_scenes c
                 JOIN production_episodes e ON e.id = c.episode_id
                 JOIN production_series s ON s.id = e.series_id
                 WHERE s.project_id = ? ORDER BY c.episode_id, c.ordinal, c.id",
            )
            .bind(project_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(Into::into)
            .collect();
            let assignments = sqlx::query_as::<_, DbAssignment>(
                "SELECT a.shot_id, a.scene_id, a.ordinal
                 FROM shot_scene_assignments a
                 JOIN production_scenes c ON c.id = a.scene_id
                 JOIN production_episodes e ON e.id = c.episode_id
                 JOIN production_series s ON s.id = e.series_id
                 JOIN shots h ON h.id = a.shot_id
                 WHERE s.project_id = ? AND h.project_id = ?
                 ORDER BY a.scene_id, a.ordinal, a.shot_id",
            )
            .bind(project_id)
            .bind(project_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(Into::into)
            .collect();
            (series, episodes, scenes, assignments)
        };

        let shots = sqlx::query_as::<_, DbShot>(
            "SELECT id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id,
                    selected_image_asset_id, selected_video_asset_id
             FROM shots WHERE project_id = ? ORDER BY ordinal, id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let configs = sqlx::query_as::<_, DbStageConfig>(
            "SELECT shot_id, stage, workflow_version_id, recipe_id, scalar_values_json
             FROM shot_stage_configs
             WHERE shot_id IN (SELECT id FROM shots WHERE project_id = ?)
             ORDER BY shot_id, stage",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let references = sqlx::query_as::<_, DbReference>(
            "SELECT r.shot_id, r.stage, r.asset_id, r.ordinal
             FROM shot_reference_assets r
             JOIN shots s ON s.id = r.shot_id
             WHERE s.project_id = ?
             ORDER BY r.shot_id, r.stage, r.ordinal, r.asset_id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let links = sqlx::query_as::<_, DbGenerationLink>(
            "SELECT l.shot_id, l.stage, t.status AS task_status
             FROM shot_generation_links l
             JOIN shots s ON s.id = l.shot_id
             LEFT JOIN tasks t ON t.id = l.task_id
             WHERE s.project_id = ?
             ORDER BY l.shot_id, l.stage, l.created_at DESC, l.id DESC",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let anchors = sqlx::query_as::<_, DbAnchor>(
            "SELECT id, kind, name, description FROM reference_anchors
             WHERE project_id = ? ORDER BY kind, name, id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let anchor_assets = sqlx::query_as::<_, DbAnchorAsset>(
            "SELECT m.anchor_id, m.asset_id, m.ordinal
             FROM reference_anchor_assets m
             JOIN reference_anchors a ON a.id = m.anchor_id
             WHERE a.project_id = ? ORDER BY m.anchor_id, m.ordinal, m.asset_id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(Into::into)
        .collect();
        let consistency = load_consistency(&mut transaction, project_id).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;

        Ok(ProjectManifestSnapshot {
            project,
            series,
            episodes,
            scenes,
            assignments,
            shots,
            configs,
            references,
            links,
            anchors,
            anchor_assets,
            consistency,
        })
    }
}

async fn load_consistency(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<ManifestConsistencyRecords, RepositoryError> {
    let character_profiles = sqlx::query_as::<_, DbCharacterProfile>(
        "SELECT id, name, description, canonical_prompt, negative_prompt,
                default_style_profile_id, default_reference_set_id, active_revision_id
         FROM character_profiles WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let scene_profiles = sqlx::query_as::<_, DbSceneProfile>(
        "SELECT id, name, description, environment_prompt, lighting_prompt,
                negative_prompt, default_style_profile_id, default_reference_set_id,
                active_revision_id
         FROM scene_profiles WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let prop_profiles = sqlx::query_as::<_, DbPropProfile>(
        "SELECT id, name, description, canonical_prompt, material_prompt, scale_prompt,
                default_reference_set_id, active_revision_id
         FROM prop_profiles WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let style_profiles = sqlx::query_as::<_, DbStyleProfile>(
        "SELECT id, name, style_prompt, color_prompt, line_prompt, negative_prompt,
                output_notes, active_revision_id
         FROM style_profiles WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let costume_variants = sqlx::query_as::<_, DbCostumeVariant>(
        "SELECT v.id, v.character_profile_id, v.name, v.prompt_fragment,
                v.reference_set_id, v.is_default, v.ordinal, v.active_revision_id
         FROM costume_variants v JOIN character_profiles p ON p.id = v.character_profile_id
         WHERE p.project_id = ? ORDER BY v.character_profile_id, v.ordinal, v.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let reference_sets = sqlx::query_as::<_, DbReferenceSet>(
        "SELECT id, name, purpose, description, owner_profile_type,
                owner_profile_id, active_revision_id
         FROM reference_sets WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let reference_set_items = sqlx::query_as::<_, DbReferenceSetItem>(
        "SELECT i.reference_set_id, i.asset_id, i.ordinal, i.role, i.is_primary
         FROM reference_set_items i JOIN reference_sets r ON r.id = i.reference_set_id
         WHERE r.project_id = ? ORDER BY i.reference_set_id, i.ordinal, i.asset_id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let shot_profile_bindings = sqlx::query_as::<_, DbShotProfileBinding>(
        "SELECT b.id, b.shot_id, b.role, b.profile_type, b.profile_id,
                b.costume_variant_id, b.ordinal, b.inheritance_mode
         FROM shot_profile_bindings b JOIN shots s ON s.id = b.shot_id
         WHERE s.project_id = ? ORDER BY b.shot_id, b.role, b.ordinal, b.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let shot_reference_set_bindings = sqlx::query_as::<_, DbShotReferenceSetBinding>(
        "SELECT b.id, b.shot_id, b.role, b.reference_set_id, b.ordinal,
                b.required, b.inheritance_mode
         FROM shot_reference_set_bindings b JOIN shots s ON s.id = b.shot_id
         WHERE s.project_id = ? ORDER BY b.shot_id, b.role, b.ordinal, b.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let scope_profile_bindings = sqlx::query_as::<_, DbScopeProfileBinding>(
        "SELECT id, project_id, scope_type, scope_id, role, profile_type,
                profile_id, costume_variant_id, ordinal, inheritance_mode
         FROM consistency_scope_profile_bindings
         WHERE project_id = ? ORDER BY scope_type, scope_id, role, ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();
    let scope_reference_set_bindings = sqlx::query_as::<_, DbScopeReferenceSetBinding>(
        "SELECT id, project_id, scope_type, scope_id, role, reference_set_id,
                ordinal, required, inheritance_mode
         FROM consistency_scope_reference_set_bindings
         WHERE project_id = ? ORDER BY scope_type, scope_id, role, ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?
    .into_iter()
    .map(Into::into)
    .collect();

    Ok(ManifestConsistencyRecords {
        character_profiles,
        scene_profiles,
        prop_profiles,
        style_profiles,
        costume_variants,
        reference_sets,
        reference_set_items,
        shot_profile_bindings,
        shot_reference_set_bindings,
        scope_profile_bindings,
        scope_reference_set_bindings,
    })
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

db_record!(DbProject => ManifestProjectRecord { id: String, name: String, description: Option<String> });
db_record!(DbSeries => ManifestSeriesRecord { id: String, ordinal: i64, name: String, description: String });
db_record!(DbEpisode => ManifestEpisodeRecord { id: String, series_id: String, ordinal: i64, name: String, description: String });
db_record!(DbScene => ManifestSceneRecord { id: String, episode_id: String, ordinal: i64, name: String, description: String });
db_record!(DbAssignment => ManifestAssignmentRecord { shot_id: String, scene_id: String, ordinal: i64 });
db_record!(DbShot => ManifestShotRecord {
    id: String, ordinal: i64, name: String, prompt_text: String, prompt_entry_id: Option<String>,
    prompt_version_id: Option<String>, selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>
});
db_record!(DbStageConfig => ManifestStageConfigRecord {
    shot_id: String, stage: String, workflow_version_id: String, recipe_id: String,
    scalar_values_json: String
});
db_record!(DbReference => ManifestReferenceRecord { shot_id: String, stage: String, asset_id: String, ordinal: i64 });
db_record!(DbGenerationLink => ManifestGenerationLinkRecord { shot_id: String, stage: String, task_status: Option<String> });
db_record!(DbAnchor => ManifestAnchorRecord { id: String, kind: String, name: String, description: String });
db_record!(DbAnchorAsset => ManifestAnchorAssetRecord { anchor_id: String, asset_id: String, ordinal: i64 });
db_record!(DbCharacterProfile => ManifestCharacterProfileRecord {
    id: String, name: String, description: String, canonical_prompt: String, negative_prompt: String,
    default_style_profile_id: Option<String>, default_reference_set_id: Option<String>, active_revision_id: Option<String>
});
db_record!(DbSceneProfile => ManifestSceneProfileRecord {
    id: String, name: String, description: String, environment_prompt: String,
    lighting_prompt: Option<String>, negative_prompt: Option<String>, default_style_profile_id: Option<String>,
    default_reference_set_id: Option<String>, active_revision_id: Option<String>
});
db_record!(DbPropProfile => ManifestPropProfileRecord {
    id: String, name: String, description: String, canonical_prompt: String,
    material_prompt: Option<String>, scale_prompt: Option<String>, default_reference_set_id: Option<String>,
    active_revision_id: Option<String>
});
db_record!(DbStyleProfile => ManifestStyleProfileRecord {
    id: String, name: String, style_prompt: String, color_prompt: Option<String>, line_prompt: Option<String>,
    negative_prompt: Option<String>, output_notes: Option<String>, active_revision_id: Option<String>
});
db_record!(DbCostumeVariant => ManifestCostumeVariantRecord {
    id: String, character_profile_id: String, name: String, prompt_fragment: String,
    reference_set_id: Option<String>, is_default: i64, ordinal: i64, active_revision_id: Option<String>
});
db_record!(DbReferenceSet => ManifestReferenceSetRecord {
    id: String, name: String, purpose: String, description: String, owner_profile_type: Option<String>,
    owner_profile_id: Option<String>, active_revision_id: Option<String>
});
db_record!(DbReferenceSetItem => ManifestReferenceSetItemRecord {
    reference_set_id: String, asset_id: String, ordinal: i64, role: Option<String>, is_primary: i64
});
db_record!(DbShotProfileBinding => ManifestShotProfileBindingRecord {
    id: String, shot_id: String, role: String, profile_type: String, profile_id: String,
    costume_variant_id: Option<String>, ordinal: i64, inheritance_mode: String
});
db_record!(DbShotReferenceSetBinding => ManifestShotReferenceSetBindingRecord {
    id: String, shot_id: String, role: String, reference_set_id: String, ordinal: i64,
    required: i64, inheritance_mode: String
});
db_record!(DbScopeProfileBinding => ManifestScopeProfileBindingRecord {
    id: String, project_id: String, scope_type: String, scope_id: String, role: String,
    profile_type: String, profile_id: String, costume_variant_id: Option<String>, ordinal: i64,
    inheritance_mode: String
});
db_record!(DbScopeReferenceSetBinding => ManifestScopeReferenceSetBindingRecord {
    id: String, project_id: String, scope_type: String, scope_id: String, role: String,
    reference_set_id: String, ordinal: i64, required: i64, inheritance_mode: String
});
