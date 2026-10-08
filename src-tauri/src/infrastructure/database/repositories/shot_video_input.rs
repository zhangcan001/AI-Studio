use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    RepositoryError, ShotVideoInputAsset, ShotVideoInputRepository, ShotVideoInputScope,
    ShotVideoInputSet, ShotVideoInputToken,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct SqliteShotVideoInputRepository {
    pool: SqlitePool,
}

#[cfg(test)]
mod tests;

impl SqliteShotVideoInputRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct InputSetRow {
    instance_id: String,
    revision: i64,
    updated_at: String,
}

#[derive(FromRow)]
struct InputAssetRow {
    input_key: String,
    ordinal: i64,
    asset_id: String,
}

fn validate_inputs(inputs: &[ShotVideoInputAsset]) -> Result<(), RepositoryError> {
    let mut ordinals: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for input in inputs {
        if !matches!(
            input.input_key.as_str(),
            "first_frame"
                | "last_frame"
                | "reference_images"
                | "reference_videos"
                | "reference_audios"
        ) || input.asset_id.trim().is_empty()
        {
            return Err(RepositoryError::integrity("invalid video input slot"));
        }
        ordinals
            .entry(&input.input_key)
            .or_default()
            .push(input.ordinal);
    }
    for (key, mut values) in ordinals {
        values.sort_unstable();
        if values.iter().enumerate().any(|(i, v)| *v != i as i64)
            || (matches!(key, "first_frame" | "last_frame") && values.len() != 1)
        {
            return Err(RepositoryError::integrity(
                "video input ordinals must be dense per slot",
            ));
        }
    }
    Ok(())
}

#[async_trait]
impl ShotVideoInputRepository for SqliteShotVideoInputRepository {
    async fn find(
        &self,
        scope: &ShotVideoInputScope,
    ) -> Result<Option<ShotVideoInputSet>, RepositoryError> {
        // One read transaction: the token and its children are a single snapshot.
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        let header = sqlx::query_as::<_, InputSetRow>(
            "SELECT instance_id, revision, updated_at FROM shot_video_input_sets
             WHERE project_id = ? AND shot_id = ? AND workflow_version_id = ? AND recipe_id = ?",
        )
        .bind(&scope.project_id)
        .bind(&scope.shot_id)
        .bind(&scope.workflow_version_id)
        .bind(&scope.recipe_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;
        let Some(header) = header else {
            return Ok(None);
        };
        let rows = sqlx::query_as::<_, InputAssetRow>(
            "SELECT input_key, ordinal, asset_id FROM shot_video_input_assets
             WHERE project_id = ? AND shot_id = ? AND workflow_version_id = ? AND recipe_id = ?
             ORDER BY input_key, ordinal",
        )
        .bind(&scope.project_id)
        .bind(&scope.shot_id)
        .bind(&scope.workflow_version_id)
        .bind(&scope.recipe_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(Some(ShotVideoInputSet {
            scope: scope.clone(),
            token: ShotVideoInputToken {
                instance_id: header.instance_id,
                revision: header.revision,
            },
            inputs: rows
                .into_iter()
                .map(|r| ShotVideoInputAsset {
                    input_key: r.input_key,
                    ordinal: r.ordinal,
                    asset_id: r.asset_id,
                })
                .collect(),
            updated_at: parse_datetime("video input updated_at", &header.updated_at)?,
        }))
    }

    async fn replace(
        &self,
        scope: &ShotVideoInputScope,
        expected: Option<&ShotVideoInputToken>,
        inputs: &[ShotVideoInputAsset],
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShotVideoInputSet>, RepositoryError> {
        validate_inputs(inputs)?;
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        // Write first, rather than upgrading a stale read transaction to a writer.
        let (changed, token) = match expected {
            Some(expected) => {
                let revision = expected
                    .revision
                    .checked_add(1)
                    .filter(|_| expected.revision > 0)
                    .ok_or_else(|| RepositoryError::integrity("invalid video input revision"))?;
                let changed = sqlx::query(
                    "UPDATE shot_video_input_sets SET revision = revision + 1, updated_at = ?
                     WHERE project_id = ? AND shot_id = ? AND workflow_version_id = ? AND recipe_id = ?
                     AND instance_id = ? AND revision = ?",
                )
                .bind(format_datetime(updated_at)).bind(&scope.project_id).bind(&scope.shot_id)
                .bind(&scope.workflow_version_id).bind(&scope.recipe_id)
                .bind(&expected.instance_id).bind(expected.revision)
                .execute(&mut *tx).await.map_err(map_sqlx_error)?.rows_affected();
                (
                    changed,
                    ShotVideoInputToken {
                        instance_id: expected.instance_id.clone(),
                        revision,
                    },
                )
            }
            None => {
                let instance_id = format!("vin_{}", Uuid::new_v4());
                let changed = sqlx::query(
                    "INSERT INTO shot_video_input_sets
                     (project_id, shot_id, workflow_version_id, recipe_id, instance_id, revision, updated_at)
                     VALUES (?, ?, ?, ?, ?, 1, ?)
                     ON CONFLICT(project_id, shot_id, workflow_version_id, recipe_id) DO NOTHING",
                )
                .bind(&scope.project_id).bind(&scope.shot_id).bind(&scope.workflow_version_id)
                .bind(&scope.recipe_id).bind(&instance_id).bind(format_datetime(updated_at))
                .execute(&mut *tx).await.map_err(map_sqlx_error)?.rows_affected();
                (
                    changed,
                    ShotVideoInputToken {
                        instance_id,
                        revision: 1,
                    },
                )
            }
        };
        if changed == 0 {
            return Ok(None);
        }
        sqlx::query(
            "DELETE FROM shot_video_input_assets
             WHERE project_id = ? AND shot_id = ? AND workflow_version_id = ? AND recipe_id = ?",
        )
        .bind(&scope.project_id)
        .bind(&scope.shot_id)
        .bind(&scope.workflow_version_id)
        .bind(&scope.recipe_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;
        for input in inputs {
            sqlx::query(
                "INSERT INTO shot_video_input_assets
                 (project_id, shot_id, workflow_version_id, recipe_id, input_key, ordinal, asset_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&scope.project_id)
            .bind(&scope.shot_id)
            .bind(&scope.workflow_version_id)
            .bind(&scope.recipe_id)
            .bind(&input.input_key)
            .bind(input.ordinal)
            .bind(&input.asset_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        }
        tx.commit().await.map_err(map_sqlx_error)?;
        let mut inputs = inputs.to_vec();
        inputs.sort_by(|a, b| (&a.input_key, a.ordinal).cmp(&(&b.input_key, b.ordinal)));
        Ok(Some(ShotVideoInputSet {
            scope: scope.clone(),
            token,
            inputs,
            updated_at,
        }))
    }
}
