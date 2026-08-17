use super::{format_datetime, map_domain_error, map_sqlx_error, parse_datetime, parse_json};
use crate::application::ports::{
    RepositoryError, ShotBulkData, ShotBulkRepository, ShotData, ShotGenerationLinkRecord,
    ShotRecord, ShotReferenceAssetRecord, ShotRepository, ShotStageConfigRecord,
    ShotStagePromptRecord,
};
use crate::domain::{validate_scalar_values, ShotStage};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone)]
pub struct SqliteShotRepository {
    pool: SqlitePool,
}

impl SqliteShotRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn load_data(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<Option<ShotData>, RepositoryError> {
        let shot = sqlx::query_as::<_, ShotRow>(
            "SELECT id, project_id, ordinal, name, prompt_text, prompt_entry_id,
                    prompt_version_id, selected_image_asset_id, selected_video_asset_id,
                    created_at, updated_at
             FROM shots WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(shot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let Some(shot) = shot else {
            return Ok(None);
        };
        Ok(Some(self.load_related(shot).await?))
    }

    async fn load_related(&self, shot: ShotRow) -> Result<ShotData, RepositoryError> {
        let shot_id = shot.id.clone();
        let stage_rows = sqlx::query_as::<_, ShotStageConfigRow>(
            "SELECT shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at
             FROM shot_stage_configs WHERE shot_id = ? ORDER BY stage",
        )
        .bind(&shot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let reference_rows = sqlx::query_as::<_, ShotReferenceRow>(
            "SELECT shot_id, stage, asset_id, ordinal
             FROM shot_reference_assets WHERE shot_id = ? ORDER BY stage, ordinal, asset_id",
        )
        .bind(&shot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let link_rows = sqlx::query_as::<_, ShotGenerationLinkRow>(
            "SELECT id, shot_id, stage, task_id, production_batch_item_id, created_at
             FROM shot_generation_links WHERE shot_id = ? ORDER BY created_at DESC, id DESC",
        )
        .bind(&shot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(ShotData {
            shot: shot.try_into_domain()?,
            stage_configs: stage_rows
                .into_iter()
                .map(ShotStageConfigRow::try_into_domain)
                .collect::<Result<Vec<_>, _>>()?,
            reference_assets: reference_rows
                .into_iter()
                .map(ShotReferenceRow::try_into_domain)
                .collect::<Result<Vec<_>, _>>()?,
            generation_links: link_rows
                .into_iter()
                .map(ShotGenerationLinkRow::try_into_domain)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[async_trait]
impl ShotRepository for SqliteShotRepository {
    async fn list(&self, project_id: &str) -> Result<Vec<ShotData>, RepositoryError> {
        let rows = sqlx::query_as::<_, ShotRow>(
            "SELECT id, project_id, ordinal, name, prompt_text, prompt_entry_id,
                    prompt_version_id, selected_image_asset_id, selected_video_asset_id,
                    created_at, updated_at
             FROM shots WHERE project_id = ? ORDER BY ordinal ASC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            result.push(self.load_related(row).await?);
        }
        Ok(result)
    }

    async fn find(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<Option<ShotData>, RepositoryError> {
        self.load_data(project_id, shot_id).await
    }

    async fn insert(&self, shot: &ShotRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id,
              selected_image_asset_id, selected_video_asset_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&shot.id)
        .bind(&shot.project_id)
        .bind(shot.ordinal)
        .bind(&shot.name)
        .bind(&shot.prompt_text)
        .bind(&shot.prompt_entry_id)
        .bind(&shot.prompt_version_id)
        .bind(&shot.selected_image_asset_id)
        .bind(&shot.selected_video_asset_id)
        .bind(format_datetime(shot.created_at))
        .bind(format_datetime(shot.updated_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn update(&self, shot: &ShotRecord) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE shots SET name = ?, prompt_text = ?, prompt_entry_id = ?,
                    prompt_version_id = ?, selected_image_asset_id = ?,
                    selected_video_asset_id = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(&shot.name)
        .bind(&shot.prompt_text)
        .bind(&shot.prompt_entry_id)
        .bind(&shot.prompt_version_id)
        .bind(&shot.selected_image_asset_id)
        .bind(&shot.selected_video_asset_id)
        .bind(format_datetime(shot.updated_at))
        .bind(&shot.id)
        .bind(&shot.project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn delete(&self, project_id: &str, shot_id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM shots WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(shot_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn reorder(
        &self,
        project_id: &str,
        ordered_ids: &[String],
        updated_at: DateTime<Utc>,
    ) -> Result<Vec<ShotRecord>, RepositoryError> {
        let current = sqlx::query_scalar::<_, String>(
            "SELECT id FROM shots WHERE project_id = ? ORDER BY ordinal ASC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if current.len() != ordered_ids.len()
            || ordered_ids.iter().collect::<HashSet<_>>().len() != ordered_ids.len()
            || ordered_ids.iter().any(|id| !current.contains(id))
        {
            return Err(RepositoryError::integrity(
                "shot reorder must contain every shot in this project exactly once",
            ));
        }

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let max_ordinal = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(ordinal) FROM shots WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .unwrap_or(-1);
        let temporary_offset = max_ordinal
            .checked_add(1)
            .ok_or_else(|| RepositoryError::integrity("shot ordinal overflow"))?;
        sqlx::query("UPDATE shots SET ordinal = ordinal + ?, updated_at = ? WHERE project_id = ?")
            .bind(temporary_offset)
            .bind(format_datetime(updated_at))
            .bind(project_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        for (ordinal, shot_id) in ordered_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE shots SET ordinal = ?, updated_at = ? WHERE project_id = ? AND id = ?",
            )
            .bind(
                i64::try_from(ordinal)
                    .map_err(|_| RepositoryError::integrity("shot ordinal overflow"))?,
            )
            .bind(format_datetime(updated_at))
            .bind(project_id)
            .bind(shot_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        self.list(project_id)
            .await
            .map(|shots| shots.into_iter().map(|data| data.shot).collect())
    }

    async fn upsert_stage_config(
        &self,
        project_id: &str,
        config: &ShotStageConfigRecord,
    ) -> Result<(), RepositoryError> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM shots WHERE id = ? AND project_id = ?",
        )
        .bind(&config.shot_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if exists == 0 {
            return Err(RepositoryError::not_found("shot", &config.shot_id));
        }
        validate_scalar_values(&config.scalar_values)
            .map_err(|error| map_domain_error("shot scalar values", error))?;
        sqlx::query(
            "INSERT INTO shot_stage_configs
             (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(shot_id, stage) DO UPDATE SET
               workflow_version_id = excluded.workflow_version_id,
               recipe_id = excluded.recipe_id,
               scalar_values_json = excluded.scalar_values_json,
               updated_at = excluded.updated_at",
        )
        .bind(&config.shot_id)
        .bind(config.stage.as_str())
        .bind(&config.workflow_version_id)
        .bind(&config.recipe_id)
        .bind(config.scalar_values.to_string())
        .bind(format_datetime(config.updated_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn replace_reference_assets(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
        asset_ids: &[String],
    ) -> Result<(), RepositoryError> {
        if asset_ids.iter().collect::<HashSet<_>>().len() != asset_ids.len() {
            return Err(RepositoryError::integrity(
                "shot reference assets must not contain duplicates",
            ));
        }
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let shot_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM shots WHERE id = ? AND project_id = ?",
        )
        .bind(shot_id)
        .bind(project_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if shot_exists == 0 {
            return Err(RepositoryError::not_found("shot", shot_id));
        }
        for asset_id in asset_ids {
            let compatible = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM assets WHERE id = ? AND project_id = ? AND type = 'image'",
            )
            .bind(asset_id)
            .bind(project_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            if compatible == 0 {
                return Err(RepositoryError::integrity(format!(
                    "shot {stage:?} reference asset {asset_id} is missing, cross-project, or not an image"
                )));
            }
        }
        sqlx::query("DELETE FROM shot_reference_assets WHERE shot_id = ? AND stage = ?")
            .bind(shot_id)
            .bind(stage.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        for (ordinal, asset_id) in asset_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO shot_reference_assets (shot_id, stage, asset_id, ordinal)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(shot_id)
            .bind(stage.as_str())
            .bind(asset_id)
            .bind(
                i64::try_from(ordinal)
                    .map_err(|_| RepositoryError::integrity("shot reference ordinal overflow"))?,
            )
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn select_image(
        &self,
        project_id: &str,
        shot_id: &str,
        asset_id: &str,
    ) -> Result<(), RepositoryError> {
        select_asset(&self.pool, project_id, shot_id, asset_id, "image", true).await
    }

    async fn select_video(
        &self,
        project_id: &str,
        shot_id: &str,
        asset_id: &str,
    ) -> Result<(), RepositoryError> {
        select_asset(&self.pool, project_id, shot_id, asset_id, "video", false).await
    }

    async fn link_generation(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
        task_id: &str,
        production_batch_item_id: Option<&str>,
        created_at: DateTime<Utc>,
    ) -> Result<ShotGenerationLinkRecord, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let shot_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM shots WHERE id = ? AND project_id = ?",
        )
        .bind(shot_id)
        .bind(project_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if shot_exists == 0 {
            return Err(RepositoryError::not_found("shot", shot_id));
        }
        let task_project =
            sqlx::query_scalar::<_, String>("SELECT project_id FROM tasks WHERE id = ?")
                .bind(task_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?
                .ok_or_else(|| RepositoryError::not_found("task", task_id))?;
        if task_project != project_id {
            return Err(RepositoryError::integrity(
                "shot generation task belongs to another project",
            ));
        }
        if let Some(item_id) = production_batch_item_id {
            let item_project = sqlx::query_scalar::<_, String>(
                "SELECT b.project_id FROM production_batch_items i
                 JOIN production_batches b ON b.id = i.batch_id WHERE i.id = ?",
            )
            .bind(item_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?
            .ok_or_else(|| RepositoryError::not_found("production batch item", item_id))?;
            if item_project != project_id {
                return Err(RepositoryError::integrity(
                    "shot generation batch item belongs to another project",
                ));
            }
        }
        let link = ShotGenerationLinkRecord {
            id: format!("sgl_{}", Uuid::new_v4()),
            shot_id: shot_id.to_owned(),
            stage,
            task_id: Some(task_id.to_owned()),
            production_batch_item_id: production_batch_item_id.map(str::to_owned),
            created_at,
        };
        sqlx::query(
            "INSERT INTO shot_generation_links
             (id, shot_id, stage, task_id, production_batch_item_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&link.id)
        .bind(&link.shot_id)
        .bind(link.stage.as_str())
        .bind(&link.task_id)
        .bind(&link.production_batch_item_id)
        .bind(format_datetime(link.created_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(link)
    }
}

#[async_trait]
impl ShotBulkRepository for SqliteShotRepository {
    async fn list_bulk_data(&self, project_id: &str) -> Result<Vec<ShotBulkData>, RepositoryError> {
        let shots = self.list(project_id).await?;
        let prompt_rows = sqlx::query_as::<_, ShotStagePromptRow>(
            "SELECT p.shot_id, p.stage, p.prompt_text, p.prompt_entry_id,
                    p.prompt_version_id, p.updated_at
             FROM shot_stage_prompts p
             JOIN shots s ON s.id = p.shot_id
             WHERE s.project_id = ?
             ORDER BY p.shot_id, p.stage",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let mut prompts_by_shot =
            std::collections::HashMap::<String, Vec<ShotStagePromptRecord>>::new();
        for row in prompt_rows {
            let prompt = row.try_into_domain()?;
            prompts_by_shot
                .entry(prompt.shot_id.clone())
                .or_default()
                .push(prompt);
        }
        Ok(shots
            .into_iter()
            .map(|data| {
                let mut stage_prompts = prompts_by_shot.remove(&data.shot.id).unwrap_or_default();
                // Direct legacy fixtures may insert a Shot after migration 019
                // without inserting stage rows. Keep them readable using the
                // old snapshot as a compatibility fallback.
                if stage_prompts.is_empty() {
                    stage_prompts = [ShotStage::Image, ShotStage::Video]
                        .into_iter()
                        .map(|stage| ShotStagePromptRecord {
                            shot_id: data.shot.id.clone(),
                            stage,
                            prompt_text: data.shot.prompt_text.clone(),
                            prompt_entry_id: data.shot.prompt_entry_id.clone(),
                            prompt_version_id: data.shot.prompt_version_id.clone(),
                            updated_at: data.shot.updated_at,
                        })
                        .collect();
                }
                ShotBulkData {
                    shot: data.shot,
                    stage_configs: data.stage_configs,
                    stage_prompts,
                }
            })
            .collect())
    }

    async fn insert_shots_atomic(
        &self,
        project_id: &str,
        shots: &[ShotRecord],
        stage_prompts: &[ShotStagePromptRecord],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let mut shot_ids = HashSet::new();
        for shot in shots {
            if shot.project_id != project_id || !shot_ids.insert(shot.id.clone()) {
                return Err(RepositoryError::integrity(
                    "bulk shot insert contains an invalid or duplicate shot",
                ));
            }
            sqlx::query(
                "INSERT INTO shots
                 (id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id,
                  selected_image_asset_id, selected_video_asset_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&shot.id)
            .bind(&shot.project_id)
            .bind(shot.ordinal)
            .bind(&shot.name)
            .bind(&shot.prompt_text)
            .bind(&shot.prompt_entry_id)
            .bind(&shot.prompt_version_id)
            .bind(&shot.selected_image_asset_id)
            .bind(&shot.selected_video_asset_id)
            .bind(format_datetime(shot.created_at))
            .bind(format_datetime(shot.updated_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        insert_stage_prompts(&mut transaction, project_id, stage_prompts).await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn update_stage_prompts_atomic(
        &self,
        project_id: &str,
        updates: &[ShotStagePromptRecord],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        insert_stage_prompts(&mut transaction, project_id, updates).await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn upsert_stage_configs_atomic(
        &self,
        project_id: &str,
        configs: &[ShotStageConfigRecord],
        prompt_updates: &[ShotStagePromptRecord],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        for config in configs {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shots WHERE id = ? AND project_id = ?",
            )
            .bind(&config.shot_id)
            .bind(project_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            if exists == 0 {
                return Err(RepositoryError::not_found("shot", &config.shot_id));
            }
            validate_scalar_values(&config.scalar_values)
                .map_err(|error| map_domain_error("shot scalar values", error))?;
            sqlx::query(
                "INSERT INTO shot_stage_configs
                 (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(shot_id, stage) DO UPDATE SET
                   workflow_version_id = excluded.workflow_version_id,
                   recipe_id = excluded.recipe_id,
                   scalar_values_json = excluded.scalar_values_json,
                   updated_at = excluded.updated_at",
            )
            .bind(&config.shot_id)
            .bind(config.stage.as_str())
            .bind(&config.workflow_version_id)
            .bind(&config.recipe_id)
            .bind(config.scalar_values.to_string())
            .bind(format_datetime(config.updated_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        insert_stage_prompts(&mut transaction, project_id, prompt_updates).await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }
}

async fn insert_stage_prompts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    prompts: &[ShotStagePromptRecord],
) -> Result<(), RepositoryError> {
    let mut seen = HashSet::new();
    for prompt in prompts {
        if !seen.insert((prompt.shot_id.clone(), prompt.stage)) {
            return Err(RepositoryError::integrity(
                "bulk stage prompt updates must not contain duplicate shot/stage pairs",
            ));
        }
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM shots WHERE id = ? AND project_id = ?",
        )
        .bind(&prompt.shot_id)
        .bind(project_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
        if exists == 0 {
            return Err(RepositoryError::not_found("shot", &prompt.shot_id));
        }
        sqlx::query(
            "INSERT INTO shot_stage_prompts
             (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(shot_id, stage) DO UPDATE SET
               prompt_text = excluded.prompt_text,
               prompt_entry_id = excluded.prompt_entry_id,
               prompt_version_id = excluded.prompt_version_id,
               updated_at = excluded.updated_at",
        )
        .bind(&prompt.shot_id)
        .bind(prompt.stage.as_str())
        .bind(&prompt.prompt_text)
        .bind(&prompt.prompt_entry_id)
        .bind(&prompt.prompt_version_id)
        .bind(format_datetime(prompt.updated_at))
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }
    Ok(())
}

async fn select_asset(
    pool: &SqlitePool,
    project_id: &str,
    shot_id: &str,
    asset_id: &str,
    asset_type: &str,
    clear_video: bool,
) -> Result<(), RepositoryError> {
    let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
    let shot_exists =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE id = ? AND project_id = ?")
            .bind(shot_id)
            .bind(project_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
    if shot_exists == 0 {
        return Err(RepositoryError::not_found("shot", shot_id));
    }
    let asset_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM assets WHERE id = ? AND project_id = ? AND type = ?",
    )
    .bind(asset_id)
    .bind(project_id)
    .bind(asset_type)
    .fetch_one(&mut *transaction)
    .await
    .map_err(map_sqlx_error)?;
    if asset_exists == 0 {
        return Err(RepositoryError::integrity(format!(
            "selected {asset_type} asset is missing or belongs to another project"
        )));
    }
    if clear_video {
        sqlx::query(
            "UPDATE shots SET selected_image_asset_id = ?, selected_video_asset_id = NULL
             WHERE id = ? AND project_id = ?",
        )
        .bind(asset_id)
        .bind(shot_id)
        .bind(project_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
    } else {
        sqlx::query("UPDATE shots SET selected_video_asset_id = ? WHERE id = ? AND project_id = ?")
            .bind(asset_id)
            .bind(shot_id)
            .bind(project_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
    }
    transaction.commit().await.map_err(map_sqlx_error)?;
    Ok(())
}

#[derive(FromRow)]
struct ShotRow {
    id: String,
    project_id: String,
    ordinal: i64,
    name: String,
    prompt_text: String,
    prompt_entry_id: Option<String>,
    prompt_version_id: Option<String>,
    selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl ShotRow {
    fn try_into_domain(self) -> Result<ShotRecord, RepositoryError> {
        Ok(ShotRecord {
            id: self.id,
            project_id: self.project_id,
            ordinal: self.ordinal,
            name: self.name,
            prompt_text: self.prompt_text,
            prompt_entry_id: self.prompt_entry_id,
            prompt_version_id: self.prompt_version_id,
            selected_image_asset_id: self.selected_image_asset_id,
            selected_video_asset_id: self.selected_video_asset_id,
            created_at: parse_datetime("shot.created_at", &self.created_at)?,
            updated_at: parse_datetime("shot.updated_at", &self.updated_at)?,
        })
    }
}

#[derive(FromRow)]
struct ShotStageConfigRow {
    shot_id: String,
    stage: String,
    workflow_version_id: String,
    recipe_id: String,
    scalar_values_json: String,
    updated_at: String,
}

impl ShotStageConfigRow {
    fn try_into_domain(self) -> Result<ShotStageConfigRecord, RepositoryError> {
        let scalar_values = parse_json("shot scalar values", Some(&self.scalar_values_json))?
            .ok_or_else(|| RepositoryError::serialization("shot scalar values", "missing value"))?;
        validate_scalar_values(&scalar_values)
            .map_err(|error| map_domain_error("shot scalar values", error))?;
        Ok(ShotStageConfigRecord {
            shot_id: self.shot_id,
            stage: ShotStage::try_from_str(&self.stage)
                .map_err(|error| map_domain_error("shot stage", error))?,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            scalar_values,
            updated_at: parse_datetime("shot stage updated_at", &self.updated_at)?,
        })
    }
}

#[derive(FromRow)]
struct ShotReferenceRow {
    shot_id: String,
    stage: String,
    asset_id: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct ShotStagePromptRow {
    shot_id: String,
    stage: String,
    prompt_text: String,
    prompt_entry_id: Option<String>,
    prompt_version_id: Option<String>,
    updated_at: String,
}

impl ShotStagePromptRow {
    fn try_into_domain(self) -> Result<ShotStagePromptRecord, RepositoryError> {
        Ok(ShotStagePromptRecord {
            shot_id: self.shot_id,
            stage: ShotStage::try_from_str(&self.stage)
                .map_err(|error| map_domain_error("shot prompt stage", error))?,
            prompt_text: self.prompt_text,
            prompt_entry_id: self.prompt_entry_id,
            prompt_version_id: self.prompt_version_id,
            updated_at: parse_datetime("shot stage prompt updated_at", &self.updated_at)?,
        })
    }
}

impl ShotReferenceRow {
    fn try_into_domain(self) -> Result<ShotReferenceAssetRecord, RepositoryError> {
        Ok(ShotReferenceAssetRecord {
            shot_id: self.shot_id,
            stage: ShotStage::try_from_str(&self.stage)
                .map_err(|error| map_domain_error("shot reference stage", error))?,
            asset_id: self.asset_id,
            ordinal: self.ordinal,
        })
    }
}

#[derive(FromRow)]
struct ShotGenerationLinkRow {
    id: String,
    shot_id: String,
    stage: String,
    task_id: Option<String>,
    production_batch_item_id: Option<String>,
    created_at: String,
}

impl ShotGenerationLinkRow {
    fn try_into_domain(self) -> Result<ShotGenerationLinkRecord, RepositoryError> {
        Ok(ShotGenerationLinkRecord {
            id: self.id,
            shot_id: self.shot_id,
            stage: ShotStage::try_from_str(&self.stage)
                .map_err(|error| map_domain_error("shot generation stage", error))?,
            task_id: self.task_id,
            production_batch_item_id: self.production_batch_item_id,
            created_at: parse_datetime("shot generation created_at", &self.created_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteShotRepository;
    use crate::application::ports::{ShotRecord, ShotRepository, ShotStageConfigRecord};
    use crate::domain::ShotStage;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use serde_json::json;
    use tempfile::tempdir;

    fn shot(id: &str, project_id: &str, ordinal: i64) -> ShotRecord {
        let now = Utc::now();
        ShotRecord {
            id: id.to_owned(),
            project_id: project_id.to_owned(),
            ordinal,
            name: format!("镜头 {ordinal}"),
            prompt_text: "测试 Prompt".to_owned(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    async fn insert_asset(pool: &sqlx::SqlitePool, id: &str, project_id: &str, kind: &str) {
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path, sha256,
              mime_type, width, height, file_size, metadata_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(project_id)
        .bind(kind)
        .bind(format!("source_{kind}"))
        .bind(id)
        .bind(id)
        .bind(format!("C:/{id}"))
        .bind("hash")
        .bind(if kind == "image" {
            "image/png"
        } else {
            "video/mp4"
        })
        .bind(1_i64)
        .bind(1_i64)
        .bind(1_i64)
        .bind("{}")
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn shot_repository_is_project_scoped_and_reorders_atomically() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Project 2', 'C:/project-2', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let repository = SqliteShotRepository::new(pool.clone());
        repository
            .insert(&shot("sht-1", "project-1", 0))
            .await
            .unwrap();
        repository
            .insert(&shot("sht-2", "project-1", 1))
            .await
            .unwrap();
        repository
            .insert(&shot("sht-3", "project-2", 0))
            .await
            .unwrap();

        let ordered = repository
            .reorder(
                "project-1",
                &["sht-2".to_owned(), "sht-1".to_owned()],
                Utc::now(),
            )
            .await
            .unwrap();
        assert_eq!(
            ordered
                .iter()
                .map(|shot| shot.id.as_str())
                .collect::<Vec<_>>(),
            ["sht-2", "sht-1"]
        );
        assert_eq!(repository.list("project-2").await.unwrap().len(), 1);
        assert!(repository
            .reorder(
                "project-1",
                &["sht-3".to_owned(), "sht-2".to_owned()],
                Utc::now()
            )
            .await
            .is_err());

        repository
            .upsert_stage_config(
                "project-1",
                &ShotStageConfigRecord {
                    shot_id: "sht-1".to_owned(),
                    stage: ShotStage::Image,
                    workflow_version_id: "workflow-version-1".to_owned(),
                    recipe_id: "recipe-1".to_owned(),
                    scalar_values: json!({"steps": {"type": "integer", "value": 4}}),
                    updated_at: Utc::now(),
                },
            )
            .await
            .unwrap();
        let data = repository
            .find("project-1", "sht-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(data.stage_configs[0].stage, ShotStage::Image);
        assert!(repository
            .find("project-2", "sht-1")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn shot_repository_validates_media_relations_and_generation_links() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteShotRepository::new(pool.clone());
        repository
            .insert(&shot("sht-1", "project-1", 0))
            .await
            .unwrap();
        insert_asset(&pool, "ast-image", "project-1", "image").await;
        insert_asset(&pool, "ast-video", "project-1", "video").await;
        sqlx::query(
            "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at)
             VALUES ('tsk-1', 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-1', 'SUCCEEDED', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        repository
            .replace_reference_assets(
                "project-1",
                "sht-1",
                ShotStage::Video,
                &["ast-image".to_owned()],
            )
            .await
            .unwrap();
        repository
            .select_image("project-1", "sht-1", "ast-image")
            .await
            .unwrap();
        assert!(repository
            .select_video("project-1", "sht-1", "ast-image")
            .await
            .is_err());
        let link = repository
            .link_generation(
                "project-1",
                "sht-1",
                ShotStage::Image,
                "tsk-1",
                None,
                Utc::now(),
            )
            .await
            .unwrap();
        let data = repository
            .find("project-1", "sht-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(data.generation_links[0].id, link.id);
        assert!(repository
            .replace_reference_assets(
                "project-1",
                "sht-1",
                ShotStage::Image,
                &["ast-video".to_owned()]
            )
            .await
            .is_err());
    }
}
