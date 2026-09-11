use super::{map_sqlx_error, parse_datetime};
use crate::application::ports::{
    ExternalProductionHandoffAssetReference, ExternalProductionHandoffEntityMapping,
    ExternalProductionHandoffEpisode, ExternalProductionHandoffImportPlan,
    ExternalProductionHandoffImportResult, ExternalProductionHandoffPrompt,
    ExternalProductionHandoffRecord, ExternalProductionHandoffRepository,
    ExternalProductionHandoffScene, ExternalProductionHandoffSceneAssignment,
    ExternalProductionHandoffSeries, ExternalProductionHandoffShot,
    ExternalProductionHandoffStageConfig, RepositoryError,
};
use async_trait::async_trait;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[derive(Clone)]
pub struct SqliteExternalProductionHandoffRepository {
    pool: SqlitePool,
}

impl SqliteExternalProductionHandoffRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExternalProductionHandoffRepository for SqliteExternalProductionHandoffRepository {
    async fn find_by_document_hash(
        &self,
        project_id: &str,
        document_sha256: &str,
    ) -> Result<Option<ExternalProductionHandoffImportResult>, RepositoryError> {
        let Some(row) = sqlx::query_as::<_, HandoffRow>(
            "SELECT id, project_id, schema_version, source_agent, source_revision, document_sha256, imported_at
             FROM external_production_handoffs WHERE project_id = ? AND document_sha256 = ?",
        )
        .bind(project_id)
        .bind(document_sha256)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)? else { return Ok(None); };
        let handoff = row.into_record()?;
        let mappings = load_mappings(&self.pool, project_id, &handoff.id).await?;
        Ok(Some(ExternalProductionHandoffImportResult {
            handoff,
            mappings,
            replayed: true,
        }))
    }

    async fn find_by_source_revision(
        &self,
        project_id: &str,
        source_agent: &str,
        source_revision: &str,
    ) -> Result<Option<ExternalProductionHandoffRecord>, RepositoryError> {
        sqlx::query_as::<_, HandoffRow>(
            "SELECT id, project_id, schema_version, source_agent, source_revision, document_sha256, imported_at
             FROM external_production_handoffs
             WHERE project_id = ? AND source_agent = ? AND source_revision = ?
             ORDER BY imported_at DESC, id DESC LIMIT 1",
        )
        .bind(project_id)
        .bind(source_agent)
        .bind(source_revision)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(HandoffRow::into_record)
        .transpose()
    }

    async fn list(
        &self,
        project_id: &str,
    ) -> Result<Vec<ExternalProductionHandoffRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, HandoffRow>(
            "SELECT id, project_id, schema_version, source_agent, source_revision, document_sha256, imported_at
             FROM external_production_handoffs WHERE project_id = ? ORDER BY imported_at DESC, id DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(HandoffRow::into_record).collect()
    }

    async fn mappings(
        &self,
        project_id: &str,
        handoff_id: &str,
    ) -> Result<Vec<ExternalProductionHandoffEntityMapping>, RepositoryError> {
        load_mappings(&self.pool, project_id, handoff_id).await
    }

    async fn import_atomic(
        &self,
        plan: &ExternalProductionHandoffImportPlan,
    ) -> Result<ExternalProductionHandoffImportResult, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        if let Some(row) = sqlx::query_as::<_, HandoffRow>(
            "SELECT id, project_id, schema_version, source_agent, source_revision, document_sha256, imported_at
             FROM external_production_handoffs WHERE project_id = ? AND document_sha256 = ?",
        )
        .bind(&plan.handoff.project_id)
        .bind(&plan.handoff.document_sha256)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)? {
            let handoff = row.into_record()?;
            let mappings = load_mappings_tx(&mut transaction, &plan.handoff.project_id, &handoff.id).await?;
            transaction.commit().await.map_err(map_sqlx_error)?;
            return Ok(ExternalProductionHandoffImportResult { handoff, mappings, replayed: true });
        }
        if let Some(revision) = plan.handoff.source_revision.as_deref() {
            let conflict = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM external_production_handoffs WHERE project_id = ? AND source_agent = ? AND source_revision = ? AND document_sha256 <> ?",
            )
            .bind(&plan.handoff.project_id)
            .bind(&plan.handoff.source_agent)
            .bind(revision)
            .bind(&plan.handoff.document_sha256)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            if conflict > 0 {
                return Err(RepositoryError::integrity(
                    "HANDOFF_SOURCE_REVISION_CONFLICT",
                ));
            }
        }
        ensure_project(&mut transaction, &plan.handoff.project_id).await?;
        for reference in &plan.asset_references {
            let found = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM assets WHERE id = ? AND project_id = ?",
            )
            .bind(&reference.asset_id)
            .bind(&plan.handoff.project_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            if found == 0 {
                return Err(RepositoryError::integrity("HANDOFF_ASSET_PROJECT_MISMATCH"));
            }
        }
        for config in &plan.stage_configs {
            ensure_stage_definition(
                &mut transaction,
                &config.workflow_version_id,
                &config.recipe_id,
            )
            .await?;
        }
        sqlx::query(
            "INSERT INTO external_production_handoffs
             (id, project_id, schema_version, source_agent, source_revision, document_sha256, imported_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&plan.handoff.id)
        .bind(&plan.handoff.project_id)
        .bind(plan.handoff.schema_version as i64)
        .bind(&plan.handoff.source_agent)
        .bind(&plan.handoff.source_revision)
        .bind(&plan.handoff.document_sha256)
        .bind(plan.handoff.imported_at.to_rfc3339())
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let series_offset = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(ordinal) FROM production_series WHERE project_id = ?",
        )
        .bind(&plan.handoff.project_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .unwrap_or(-1)
            + 1;
        for (index, series) in plan.series.iter().enumerate() {
            insert_series(&mut transaction, series, series_offset + index as i64).await?;
        }
        for episode in &plan.episodes {
            insert_episode(&mut transaction, episode).await?;
        }
        for scene in &plan.scenes {
            insert_scene(&mut transaction, scene).await?;
        }
        let shot_offset = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(ordinal) FROM shots WHERE project_id = ?",
        )
        .bind(&plan.handoff.project_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .unwrap_or(-1)
            + 1;
        for (index, shot) in plan.shots.iter().enumerate() {
            insert_shot(&mut transaction, shot, shot_offset + index as i64).await?;
        }
        for prompt in &plan.prompts {
            insert_prompt(&mut transaction, prompt).await?;
        }
        for config in &plan.stage_configs {
            insert_stage_config(&mut transaction, config).await?;
        }
        for reference in &plan.asset_references {
            insert_asset_reference(&mut transaction, reference).await?;
        }
        for assignment in &plan.assignments {
            insert_assignment(&mut transaction, assignment).await?;
        }
        for mapping in &plan.mappings {
            insert_mapping(&mut transaction, mapping).await?;
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(ExternalProductionHandoffImportResult {
            handoff: plan.handoff.clone(),
            mappings: plan.mappings.clone(),
            replayed: false,
        })
    }
}

#[derive(FromRow)]
struct HandoffRow {
    id: String,
    project_id: String,
    schema_version: i64,
    source_agent: String,
    source_revision: Option<String>,
    document_sha256: String,
    imported_at: String,
}

impl HandoffRow {
    fn into_record(self) -> Result<ExternalProductionHandoffRecord, RepositoryError> {
        Ok(ExternalProductionHandoffRecord {
            id: self.id,
            project_id: self.project_id,
            schema_version: u32::try_from(self.schema_version).map_err(|_| {
                RepositoryError::serialization("handoff schema version", "negative")
            })?,
            source_agent: self.source_agent,
            source_revision: self.source_revision,
            document_sha256: self.document_sha256,
            imported_at: parse_datetime("handoff imported_at", &self.imported_at)?,
        })
    }
}

async fn load_mappings(
    pool: &SqlitePool,
    project_id: &str,
    handoff_id: &str,
) -> Result<Vec<ExternalProductionHandoffEntityMapping>, RepositoryError> {
    let rows = sqlx::query_as::<_, MappingRow>(
        "SELECT m.handoff_id, m.entity_kind, m.external_id, m.formal_entity_id
         FROM external_production_handoff_entities m
         INNER JOIN external_production_handoffs h ON h.id = m.handoff_id
         WHERE h.project_id = ? AND m.handoff_id = ? ORDER BY m.entity_kind, m.external_id",
    )
    .bind(project_id)
    .bind(handoff_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;
    Ok(rows.into_iter().map(MappingRow::into_value).collect())
}

async fn load_mappings_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    handoff_id: &str,
) -> Result<Vec<ExternalProductionHandoffEntityMapping>, RepositoryError> {
    let rows = sqlx::query_as::<_, MappingRow>(
        "SELECT m.handoff_id, m.entity_kind, m.external_id, m.formal_entity_id
         FROM external_production_handoff_entities m
         INNER JOIN external_production_handoffs h ON h.id = m.handoff_id
         WHERE h.project_id = ? AND m.handoff_id = ? ORDER BY m.entity_kind, m.external_id",
    )
    .bind(project_id)
    .bind(handoff_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(rows.into_iter().map(MappingRow::into_value).collect())
}

#[derive(FromRow)]
struct MappingRow {
    handoff_id: String,
    entity_kind: String,
    external_id: String,
    formal_entity_id: String,
}
impl MappingRow {
    fn into_value(self) -> ExternalProductionHandoffEntityMapping {
        ExternalProductionHandoffEntityMapping {
            handoff_id: self.handoff_id,
            entity_kind: self.entity_kind,
            external_id: self.external_id,
            formal_entity_id: self.formal_entity_id,
        }
    }
}

async fn ensure_project(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<(), RepositoryError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = ?")
        .bind(project_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    if count == 0 {
        return Err(RepositoryError::not_found("project", project_id));
    }
    Ok(())
}

async fn ensure_stage_definition(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_version_id: &str,
    recipe_id: &str,
) -> Result<(), RepositoryError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workflow_versions wv
         INNER JOIN workflows w ON w.id = wv.workflow_id
         INNER JOIN recipes r ON r.workflow_version_id = wv.id AND r.id = ?
         LEFT JOIN workflow_runtime_states wvs
           ON wvs.workflow_version_id = wv.id
         LEFT JOIN workflow_recipe_runtime_states wrs
           ON wrs.workflow_version_id = wv.id AND wrs.recipe_id = r.id
         WHERE wv.id = ? AND w.library_state = 'ACTIVE'
           AND COALESCE(wvs.enabled, 1) = 1
           AND COALESCE(wvs.archived, 0) = 0
           AND COALESCE(wrs.archived, 0) = 0",
    )
    .bind(recipe_id)
    .bind(workflow_version_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if count == 0 {
        return Err(RepositoryError::integrity("HANDOFF_WORKFLOW_UNAVAILABLE"));
    }
    Ok(())
}

async fn insert_series(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffSeries,
    ordinal: i64,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO production_series (id, project_id, ordinal, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&value.id).bind(&value.project_id).bind(ordinal).bind(&value.name).bind(&value.description).bind(value.created_at.to_rfc3339()).bind(value.updated_at.to_rfc3339()).execute(&mut **transaction).await.map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_episode(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffEpisode,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO production_episodes (id, series_id, ordinal, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&value.id).bind(&value.series_id).bind(value.ordinal as i64).bind(&value.name).bind(&value.description).bind(value.created_at.to_rfc3339()).bind(value.updated_at.to_rfc3339()).execute(&mut **transaction).await.map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_scene(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffScene,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO production_scenes (id, episode_id, ordinal, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&value.id).bind(&value.episode_id).bind(value.ordinal as i64).bind(&value.name).bind(&value.description).bind(value.created_at.to_rfc3339()).bind(value.updated_at.to_rfc3339()).execute(&mut **transaction).await.map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_shot(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffShot,
    ordinal: i64,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO shots (id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id, selected_image_asset_id, selected_video_asset_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, ?)")
        .bind(&value.id).bind(&value.project_id).bind(ordinal).bind(&value.name).bind(&value.description).bind(value.created_at.to_rfc3339()).bind(value.updated_at.to_rfc3339()).execute(&mut **transaction).await.map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_prompt(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffPrompt,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO shot_stage_prompts (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at) VALUES (?, ?, ?, NULL, NULL, ?) ON CONFLICT(shot_id, stage) DO UPDATE SET prompt_text = excluded.prompt_text, updated_at = excluded.updated_at")
        .bind(&value.shot_id).bind(&value.stage).bind(&value.prompt_text).bind(value.updated_at.to_rfc3339()).execute(&mut **transaction).await.map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_stage_config(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffStageConfig,
) -> Result<(), RepositoryError> {
    let scalar_values = serde_json::to_string(&value.scalar_values).map_err(|error| {
        RepositoryError::serialization("handoff stage config", error.to_string())
    })?;
    sqlx::query("INSERT INTO shot_stage_configs (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&value.shot_id).bind(&value.stage).bind(&value.workflow_version_id).bind(&value.recipe_id).bind(scalar_values).bind(value.updated_at.to_rfc3339()).execute(&mut **transaction).await.map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_asset_reference(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffAssetReference,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO shot_reference_assets (shot_id, stage, asset_id, ordinal) VALUES (?, ?, ?, ?)",
    )
    .bind(&value.shot_id)
    .bind(&value.stage)
    .bind(&value.asset_id)
    .bind(value.ordinal)
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_assignment(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffSceneAssignment,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO shot_scene_assignments (shot_id, scene_id, ordinal, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&value.shot_id).bind(&value.scene_id).bind(value.ordinal).bind(value.created_at.to_rfc3339()).bind(value.updated_at.to_rfc3339()).execute(&mut **transaction).await.map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_mapping(
    transaction: &mut Transaction<'_, Sqlite>,
    value: &ExternalProductionHandoffEntityMapping,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO external_production_handoff_entities (handoff_id, entity_kind, external_id, formal_entity_id) VALUES (?, ?, ?, ?)")
        .bind(&value.handoff_id).bind(&value.entity_kind).bind(&value.external_id).bind(&value.formal_entity_id).execute(&mut **transaction).await.map_err(map_sqlx_error)?;
    Ok(())
}
