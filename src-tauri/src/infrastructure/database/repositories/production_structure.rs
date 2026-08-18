use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    ProductionStructureRepository, ProductionStructureTreeData, RepositoryError,
};
use crate::domain::{
    ProductionEpisode, ProductionEpisodeId, ProductionScene, ProductionSceneId, ProductionSeries,
    ProductionSeriesId, ShotSceneAssignment,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashSet;

#[derive(Clone)]
pub struct SqliteProductionStructureRepository {
    pool: SqlitePool,
}

impl SqliteProductionStructureRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct SeriesRow {
    id: String,
    project_id: String,
    ordinal: i64,
    name: String,
    description: String,
    created_at: String,
    updated_at: String,
}

impl SeriesRow {
    fn into_domain(self) -> Result<ProductionSeries, RepositoryError> {
        Ok(ProductionSeries {
            id: ProductionSeriesId::parse(self.id).map_err(|error| {
                RepositoryError::serialization("production series id", error.to_string())
            })?,
            project_id: self.project_id,
            ordinal: ordinal(&self.ordinal, "production series ordinal")?,
            name: self.name,
            description: self.description,
            created_at: parse_datetime("production series created_at", &self.created_at)?,
            updated_at: parse_datetime("production series updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct EpisodeRow {
    id: String,
    series_id: String,
    ordinal: i64,
    name: String,
    description: String,
    created_at: String,
    updated_at: String,
}

impl EpisodeRow {
    fn into_domain(self) -> Result<ProductionEpisode, RepositoryError> {
        Ok(ProductionEpisode {
            id: ProductionEpisodeId::parse(self.id).map_err(|error| {
                RepositoryError::serialization("production episode id", error.to_string())
            })?,
            series_id: ProductionSeriesId::parse(self.series_id).map_err(|error| {
                RepositoryError::serialization("production episode series_id", error.to_string())
            })?,
            ordinal: ordinal(&self.ordinal, "production episode ordinal")?,
            name: self.name,
            description: self.description,
            created_at: parse_datetime("production episode created_at", &self.created_at)?,
            updated_at: parse_datetime("production episode updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct SceneRow {
    id: String,
    episode_id: String,
    ordinal: i64,
    name: String,
    description: String,
    created_at: String,
    updated_at: String,
}

impl SceneRow {
    fn into_domain(self) -> Result<ProductionScene, RepositoryError> {
        Ok(ProductionScene {
            id: ProductionSceneId::parse(self.id).map_err(|error| {
                RepositoryError::serialization("production scene id", error.to_string())
            })?,
            episode_id: ProductionEpisodeId::parse(self.episode_id).map_err(|error| {
                RepositoryError::serialization("production scene episode_id", error.to_string())
            })?,
            ordinal: ordinal(&self.ordinal, "production scene ordinal")?,
            name: self.name,
            description: self.description,
            created_at: parse_datetime("production scene created_at", &self.created_at)?,
            updated_at: parse_datetime("production scene updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct ShotAssignmentRow {
    shot_id: String,
    shot_ordinal: i64,
    scene_id: Option<String>,
    ordinal: Option<i64>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

impl ShotAssignmentRow {
    fn into_assignment(self) -> Result<Option<ShotSceneAssignment>, RepositoryError> {
        let Some(scene_id) = self.scene_id else {
            return Ok(None);
        };
        let ordinal_value = self.ordinal.ok_or_else(|| {
            RepositoryError::serialization("shot scene assignment ordinal", "missing value")
        })?;
        let created_at = self.created_at.ok_or_else(|| {
            RepositoryError::serialization("shot scene assignment created_at", "missing value")
        })?;
        let updated_at = self.updated_at.ok_or_else(|| {
            RepositoryError::serialization("shot scene assignment updated_at", "missing value")
        })?;
        Ok(Some(ShotSceneAssignment {
            shot_id: self.shot_id,
            scene_id: ProductionSceneId::parse(scene_id).map_err(|error| {
                RepositoryError::serialization("shot scene assignment scene_id", error.to_string())
            })?,
            ordinal: ordinal(&ordinal_value, "shot scene assignment ordinal")?,
            created_at: parse_datetime("shot scene assignment created_at", &created_at)?,
            updated_at: parse_datetime("shot scene assignment updated_at", &updated_at)?,
        }))
    }
}

#[async_trait]
impl ProductionStructureRepository for SqliteProductionStructureRepository {
    async fn load_tree_data(
        &self,
        project_id: &str,
    ) -> Result<ProductionStructureTreeData, RepositoryError> {
        let series = sqlx::query_as::<_, SeriesRow>(
            "SELECT id, project_id, ordinal, name, description, created_at, updated_at
             FROM production_series
             WHERE project_id = ?
             ORDER BY ordinal ASC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(SeriesRow::into_domain)
        .collect::<Result<Vec<_>, _>>()?;

        let episodes = sqlx::query_as::<_, EpisodeRow>(
            "SELECT e.id, e.series_id, e.ordinal, e.name, e.description, e.created_at, e.updated_at
             FROM production_episodes e
             INNER JOIN production_series s ON s.id = e.series_id
             WHERE s.project_id = ?
             ORDER BY e.series_id ASC, e.ordinal ASC, e.id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(EpisodeRow::into_domain)
        .collect::<Result<Vec<_>, _>>()?;

        let scenes = sqlx::query_as::<_, SceneRow>(
            "SELECT c.id, c.episode_id, c.ordinal, c.name, c.description, c.created_at, c.updated_at
             FROM production_scenes c
             INNER JOIN production_episodes e ON e.id = c.episode_id
             INNER JOIN production_series s ON s.id = e.series_id
             WHERE s.project_id = ?
             ORDER BY c.episode_id ASC, c.ordinal ASC, c.id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(SceneRow::into_domain)
        .collect::<Result<Vec<_>, _>>()?;

        let shot_rows = sqlx::query_as::<_, ShotAssignmentRow>(
            "SELECT s.id AS shot_id, s.ordinal AS shot_ordinal,
                    a.scene_id, a.ordinal, a.created_at, a.updated_at
             FROM shots s
             LEFT JOIN shot_scene_assignments a ON a.shot_id = s.id
             WHERE s.project_id = ?
             ORDER BY CASE WHEN a.scene_id IS NULL THEN 1 ELSE 0 END,
                      a.scene_id ASC, a.ordinal ASC, s.id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let mut project_shot_ids = shot_rows
            .iter()
            .map(|row| (row.shot_ordinal, row.shot_id.clone()))
            .collect::<Vec<_>>();
        project_shot_ids.sort_by_key(|(ordinal, shot_id)| (*ordinal, shot_id.clone()));
        let project_shot_ids = project_shot_ids
            .into_iter()
            .map(|(_, shot_id)| shot_id)
            .collect::<Vec<_>>();
        let assignments = shot_rows
            .into_iter()
            .map(ShotAssignmentRow::into_assignment)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        Ok(ProductionStructureTreeData {
            series,
            episodes,
            scenes,
            assignments,
            project_shot_ids,
        })
    }

    async fn create_series(
        &self,
        series: &ProductionSeries,
    ) -> Result<ProductionSeries, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let next = next_ordinal(
            &mut transaction,
            "production_series",
            "project_id",
            series.project_id.as_str(),
        )
        .await?;
        sqlx::query(
            "INSERT INTO production_series
             (id, project_id, ordinal, name, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(series.id.as_str())
        .bind(&series.project_id)
        .bind(next)
        .bind(&series.name)
        .bind(&series.description)
        .bind(format_datetime(series.created_at))
        .bind(format_datetime(series.updated_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(ProductionSeries {
            ordinal: ordinal(&next, "production series ordinal")?,
            ..series.clone()
        })
    }

    async fn update_series(
        &self,
        series: &ProductionSeries,
    ) -> Result<ProductionSeries, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_series
             SET name = ?, description = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(&series.name)
        .bind(&series.description)
        .bind(format_datetime(series.updated_at))
        .bind(series.id.as_str())
        .bind(&series.project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::not_found(
                "production series",
                series.id.as_str(),
            ));
        }
        Ok(series.clone())
    }

    async fn delete_series(
        &self,
        project_id: &str,
        id: &ProductionSeriesId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM production_series WHERE id = ? AND project_id = ?")
            .bind(id.as_str())
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn reorder_series(
        &self,
        project_id: &str,
        ordered_ids: &[ProductionSeriesId],
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        reorder_table(
            &mut transaction,
            "production_series",
            "project_id",
            project_id,
            "id",
            &ordered_ids
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>(),
            updated_at,
        )
        .await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn create_episode(
        &self,
        project_id: &str,
        episode: &ProductionEpisode,
    ) -> Result<ProductionEpisode, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_series(&mut transaction, project_id, &episode.series_id).await?;
        let next = next_ordinal(
            &mut transaction,
            "production_episodes",
            "series_id",
            episode.series_id.as_str(),
        )
        .await?;
        sqlx::query(
            "INSERT INTO production_episodes
             (id, series_id, ordinal, name, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(episode.id.as_str())
        .bind(episode.series_id.as_str())
        .bind(next)
        .bind(&episode.name)
        .bind(&episode.description)
        .bind(format_datetime(episode.created_at))
        .bind(format_datetime(episode.updated_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(ProductionEpisode {
            ordinal: ordinal(&next, "production episode ordinal")?,
            ..episode.clone()
        })
    }

    async fn update_episode(
        &self,
        episode: &ProductionEpisode,
        project_id: &str,
    ) -> Result<ProductionEpisode, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_episodes
             SET name = ?, description = ?, updated_at = ?
             WHERE id = ?
               AND EXISTS (
                 SELECT 1 FROM production_series s
                 WHERE s.id = production_episodes.series_id AND s.project_id = ?
               )",
        )
        .bind(&episode.name)
        .bind(&episode.description)
        .bind(format_datetime(episode.updated_at))
        .bind(episode.id.as_str())
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::not_found(
                "production episode",
                episode.id.as_str(),
            ));
        }
        Ok(episode.clone())
    }

    async fn delete_episode(
        &self,
        project_id: &str,
        id: &ProductionEpisodeId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM production_episodes
             WHERE id = ? AND EXISTS (
               SELECT 1 FROM production_series s
               WHERE s.id = production_episodes.series_id AND s.project_id = ?
             )",
        )
        .bind(id.as_str())
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn reorder_episodes(
        &self,
        project_id: &str,
        series_id: &ProductionSeriesId,
        ordered_ids: &[ProductionEpisodeId],
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_series(&mut transaction, project_id, series_id).await?;
        reorder_table(
            &mut transaction,
            "production_episodes",
            "series_id",
            series_id.as_str(),
            "id",
            &ordered_ids
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>(),
            updated_at,
        )
        .await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn create_scene(
        &self,
        project_id: &str,
        scene: &ProductionScene,
    ) -> Result<ProductionScene, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_episode(&mut transaction, project_id, &scene.episode_id).await?;
        let next = next_ordinal(
            &mut transaction,
            "production_scenes",
            "episode_id",
            scene.episode_id.as_str(),
        )
        .await?;
        sqlx::query(
            "INSERT INTO production_scenes
             (id, episode_id, ordinal, name, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(scene.id.as_str())
        .bind(scene.episode_id.as_str())
        .bind(next)
        .bind(&scene.name)
        .bind(&scene.description)
        .bind(format_datetime(scene.created_at))
        .bind(format_datetime(scene.updated_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(ProductionScene {
            ordinal: ordinal(&next, "production scene ordinal")?,
            ..scene.clone()
        })
    }

    async fn update_scene(
        &self,
        scene: &ProductionScene,
        project_id: &str,
    ) -> Result<ProductionScene, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_scenes
             SET name = ?, description = ?, updated_at = ?
             WHERE id = ?
               AND EXISTS (
                 SELECT 1
                 FROM production_episodes e
                 INNER JOIN production_series s ON s.id = e.series_id
                 WHERE e.id = production_scenes.episode_id AND s.project_id = ?
               )",
        )
        .bind(&scene.name)
        .bind(&scene.description)
        .bind(format_datetime(scene.updated_at))
        .bind(scene.id.as_str())
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::not_found(
                "production scene",
                scene.id.as_str(),
            ));
        }
        Ok(scene.clone())
    }

    async fn delete_scene(
        &self,
        project_id: &str,
        id: &ProductionSceneId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM production_scenes
             WHERE id = ? AND EXISTS (
               SELECT 1
               FROM production_episodes e
               INNER JOIN production_series s ON s.id = e.series_id
               WHERE e.id = production_scenes.episode_id AND s.project_id = ?
             )",
        )
        .bind(id.as_str())
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn reorder_scenes(
        &self,
        project_id: &str,
        episode_id: &ProductionEpisodeId,
        ordered_ids: &[ProductionSceneId],
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_episode(&mut transaction, project_id, episode_id).await?;
        reorder_table(
            &mut transaction,
            "production_scenes",
            "episode_id",
            episode_id.as_str(),
            "id",
            &ordered_ids
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>(),
            updated_at,
        )
        .await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn assign_shots_atomic(
        &self,
        project_id: &str,
        scene_id: &ProductionSceneId,
        shot_ids: &[String],
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        validate_shot_ids(shot_ids)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_scene(&mut transaction, project_id, scene_id).await?;
        ensure_shots_in_project(&mut transaction, project_id, shot_ids).await?;

        let current = scene_shot_ids(&mut transaction, scene_id).await?;
        let moving = shot_ids.iter().collect::<HashSet<_>>();
        let mut final_ids = current
            .into_iter()
            .filter(|shot_id| !moving.contains(shot_id))
            .collect::<Vec<_>>();
        final_ids.extend(shot_ids.iter().cloned());

        delete_shot_ids(&mut transaction, shot_ids).await?;
        sqlx::query("DELETE FROM shot_scene_assignments WHERE scene_id = ?")
            .bind(scene_id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        insert_scene_shots(&mut transaction, scene_id, &final_ids, updated_at).await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn unassign_shots_atomic(
        &self,
        project_id: &str,
        shot_ids: &[String],
    ) -> Result<(), RepositoryError> {
        validate_shot_ids(shot_ids)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_shots_in_project(&mut transaction, project_id, shot_ids).await?;
        delete_shot_ids(&mut transaction, shot_ids).await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn reorder_scene_shots(
        &self,
        scene_id: &ProductionSceneId,
        ordered_shot_ids: &[String],
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        validate_ordered_shot_ids(ordered_shot_ids)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        if !scene_exists(&mut transaction, scene_id).await? {
            return Err(RepositoryError::not_found(
                "production scene",
                scene_id.as_str(),
            ));
        }
        let current = scene_shot_ids(&mut transaction, scene_id).await?;
        ensure_complete_ids("scene shot reorder", &current, ordered_shot_ids)?;
        reorder_scene_shots_in_transaction(
            &mut transaction,
            scene_id,
            ordered_shot_ids,
            updated_at,
        )
        .await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }
}

fn ordinal(value: &i64, context: &str) -> Result<u32, RepositoryError> {
    u32::try_from(*value).map_err(|_| RepositoryError::serialization(context, value.to_string()))
}

async fn next_ordinal(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    parent_column: &str,
    parent_id: &str,
) -> Result<i64, RepositoryError> {
    let query =
        format!("SELECT COALESCE(MAX(ordinal), -1) + 1 FROM {table} WHERE {parent_column} = ?");
    sqlx::query_scalar(&query)
        .bind(parent_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx_error)
}

async fn ensure_series(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    series_id: &ProductionSeriesId,
) -> Result<(), RepositoryError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM production_series WHERE id = ? AND project_id = ?",
    )
    .bind(series_id.as_str())
    .bind(project_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if count == 0 {
        return Err(RepositoryError::not_found(
            "production series",
            series_id.as_str(),
        ));
    }
    Ok(())
}

async fn ensure_episode(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    episode_id: &ProductionEpisodeId,
) -> Result<(), RepositoryError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM production_episodes e
         INNER JOIN production_series s ON s.id = e.series_id
         WHERE e.id = ? AND s.project_id = ?",
    )
    .bind(episode_id.as_str())
    .bind(project_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if count == 0 {
        return Err(RepositoryError::not_found(
            "production episode",
            episode_id.as_str(),
        ));
    }
    Ok(())
}

async fn ensure_scene(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    scene_id: &ProductionSceneId,
) -> Result<(), RepositoryError> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM production_scenes WHERE id = ?")
        .bind(scene_id.as_str())
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    if exists == 0 {
        return Err(RepositoryError::not_found(
            "production scene",
            scene_id.as_str(),
        ));
    }
    let in_project: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM production_scenes c
         INNER JOIN production_episodes e ON e.id = c.episode_id
         INNER JOIN production_series s ON s.id = e.series_id
         WHERE c.id = ? AND s.project_id = ?",
    )
    .bind(scene_id.as_str())
    .bind(project_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    if in_project == 0 {
        return Err(RepositoryError::integrity(
            "PRODUCTION_STRUCTURE_PROJECT_MISMATCH: scene does not belong to project",
        ));
    }
    Ok(())
}

async fn scene_exists(
    transaction: &mut Transaction<'_, Sqlite>,
    scene_id: &ProductionSceneId,
) -> Result<bool, RepositoryError> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_scenes WHERE id = ?")
            .bind(scene_id.as_str())
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?
            == 1,
    )
}

async fn ensure_shots_in_project(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    shot_ids: &[String],
) -> Result<(), RepositoryError> {
    let placeholders = std::iter::repeat("?")
        .take(shot_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let all_query = format!("SELECT COUNT(*) FROM shots WHERE id IN ({placeholders})");
    let mut all_query = sqlx::query_scalar::<_, i64>(&all_query);
    for id in shot_ids {
        all_query = all_query.bind(id);
    }
    let all: i64 = all_query
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    if all != i64::try_from(shot_ids.len()).unwrap_or(i64::MAX) {
        return Err(RepositoryError::not_found(
            "shot",
            "one or more requested ids",
        ));
    }
    let project_query =
        format!("SELECT COUNT(*) FROM shots WHERE project_id = ? AND id IN ({placeholders})");
    let mut query = sqlx::query_scalar::<_, i64>(&project_query).bind(project_id);
    for id in shot_ids {
        query = query.bind(id);
    }
    let in_project = query
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    if in_project != i64::try_from(shot_ids.len()).unwrap_or(i64::MAX) {
        return Err(RepositoryError::integrity(
            "PRODUCTION_STRUCTURE_PROJECT_MISMATCH: shot does not belong to project",
        ));
    }
    Ok(())
}

async fn delete_shot_ids(
    transaction: &mut Transaction<'_, Sqlite>,
    shot_ids: &[String],
) -> Result<(), RepositoryError> {
    let placeholders = std::iter::repeat("?")
        .take(shot_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!("DELETE FROM shot_scene_assignments WHERE shot_id IN ({placeholders})");
    let mut query = sqlx::query(&query);
    for id in shot_ids {
        query = query.bind(id);
    }
    query
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    Ok(())
}

async fn scene_shot_ids(
    transaction: &mut Transaction<'_, Sqlite>,
    scene_id: &ProductionSceneId,
) -> Result<Vec<String>, RepositoryError> {
    sqlx::query_scalar(
        "SELECT shot_id FROM shot_scene_assignments WHERE scene_id = ? ORDER BY ordinal ASC, shot_id ASC",
    )
    .bind(scene_id.as_str())
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)
}

async fn insert_scene_shots(
    transaction: &mut Transaction<'_, Sqlite>,
    scene_id: &ProductionSceneId,
    shot_ids: &[String],
    now: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    for (index, shot_id) in shot_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO shot_scene_assignments
             (shot_id, scene_id, ordinal, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(shot_id)
        .bind(scene_id.as_str())
        .bind(
            i64::try_from(index)
                .map_err(|_| RepositoryError::integrity("shot ordinal overflow"))?,
        )
        .bind(format_datetime(now))
        .bind(format_datetime(now))
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }
    Ok(())
}

async fn reorder_table(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    parent_column: &str,
    parent_id: &str,
    id_column: &str,
    ordered_ids: &[String],
    updated_at: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    let current_query = format!(
        "SELECT {id_column} FROM {table} WHERE {parent_column} = ? ORDER BY ordinal ASC, {id_column} ASC"
    );
    let current: Vec<String> = sqlx::query_scalar(&current_query)
        .bind(parent_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    ensure_complete_ids(&format!("{table} reorder"), &current, ordered_ids)?;

    if current.is_empty() {
        return Ok(());
    }
    let max_query = format!("SELECT MAX(ordinal) FROM {table} WHERE {parent_column} = ?");
    let max: Option<i64> = sqlx::query_scalar(&max_query)
        .bind(parent_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    let offset = max
        .unwrap_or(-1)
        .checked_add(1)
        .ok_or_else(|| RepositoryError::integrity("production structure ordinal overflow"))?;
    let temporary_query = format!(
        "UPDATE {table} SET ordinal = ordinal + ?, updated_at = ? WHERE {parent_column} = ?"
    );
    sqlx::query(&temporary_query)
        .bind(offset)
        .bind(format_datetime(updated_at))
        .bind(parent_id)
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    let update_query = format!(
        "UPDATE {table} SET ordinal = ?, updated_at = ? WHERE {parent_column} = ? AND {id_column} = ?"
    );
    for (index, id) in ordered_ids.iter().enumerate() {
        sqlx::query(&update_query)
            .bind(i64::try_from(index).map_err(|_| RepositoryError::integrity("ordinal overflow"))?)
            .bind(format_datetime(updated_at))
            .bind(parent_id)
            .bind(id)
            .execute(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
    }
    Ok(())
}

async fn reorder_scene_shots_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    scene_id: &ProductionSceneId,
    ordered_shot_ids: &[String],
    updated_at: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    let max: Option<i64> =
        sqlx::query_scalar("SELECT MAX(ordinal) FROM shot_scene_assignments WHERE scene_id = ?")
            .bind(scene_id.as_str())
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
    let offset = max
        .unwrap_or(-1)
        .checked_add(1)
        .ok_or_else(|| RepositoryError::integrity("shot scene ordinal overflow"))?;
    sqlx::query(
        "UPDATE shot_scene_assignments
         SET ordinal = ordinal + ?, updated_at = ?
         WHERE scene_id = ?",
    )
    .bind(offset)
    .bind(format_datetime(updated_at))
    .bind(scene_id.as_str())
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    for (index, shot_id) in ordered_shot_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE shot_scene_assignments
             SET ordinal = ?, updated_at = ?
             WHERE scene_id = ? AND shot_id = ?",
        )
        .bind(
            i64::try_from(index)
                .map_err(|_| RepositoryError::integrity("shot ordinal overflow"))?,
        )
        .bind(format_datetime(updated_at))
        .bind(scene_id.as_str())
        .bind(shot_id)
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }
    Ok(())
}

fn validate_shot_ids(shot_ids: &[String]) -> Result<(), RepositoryError> {
    if shot_ids.is_empty() || shot_ids.len() > 500 {
        return Err(RepositoryError::integrity(
            "production structure shot assignment must contain 1..500 shots",
        ));
    }
    if shot_ids.iter().collect::<HashSet<_>>().len() != shot_ids.len() {
        return Err(RepositoryError::integrity(
            "production structure shot assignment must not contain duplicate ids",
        ));
    }
    Ok(())
}

fn validate_ordered_shot_ids(shot_ids: &[String]) -> Result<(), RepositoryError> {
    if shot_ids.len() > 500 {
        return Err(RepositoryError::integrity(
            "production structure shot reorder must contain at most 500 shots",
        ));
    }
    if shot_ids.iter().collect::<HashSet<_>>().len() != shot_ids.len() {
        return Err(RepositoryError::integrity(
            "production structure shot reorder must not contain duplicate ids",
        ));
    }
    Ok(())
}

fn ensure_complete_ids<T: AsRef<str>>(
    context: &str,
    current: &[String],
    ordered: &[T],
) -> Result<(), RepositoryError> {
    let ordered_len = ordered.len();
    let ordered = ordered.iter().map(AsRef::as_ref).collect::<HashSet<_>>();
    let current_set = current.iter().map(String::as_str).collect::<HashSet<_>>();
    if ordered.len() != ordered_len || ordered.len() != current.len() || ordered != current_set {
        return Err(RepositoryError::integrity(format!(
            "{context} must contain every child exactly once"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SqliteProductionStructureRepository;
    use crate::application::ports::{Clock, ProductionStructureRepository, RepositoryError};
    use crate::domain::{
        ProductionEpisode, ProductionEpisodeId, ProductionScene, ProductionSceneId,
        ProductionSeries, ProductionSeriesId,
    };
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::{DateTime, TimeZone, Utc};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::{tempdir, TempDir};

    struct FixedClock(DateTime<Utc>);
    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    async fn setup() -> (TempDir, SqlitePool, SqliteProductionStructureRepository) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        for (id, ordinal) in [("shot_a", 0), ("shot_b", 1), ("shot_c", 2)] {
            sqlx::query(
                "INSERT INTO shots
                 (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
                 VALUES (?, 'project-1', ?, ?, '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .bind(id)
            .bind(ordinal)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        }
        (
            directory,
            pool.clone(),
            SqliteProductionStructureRepository::new(pool),
        )
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap()
    }

    fn series(id: &str) -> ProductionSeries {
        ProductionSeries {
            id: ProductionSeriesId::parse(id).unwrap(),
            project_id: "project-1".to_owned(),
            ordinal: 0,
            name: id.to_owned(),
            description: String::new(),
            created_at: now(),
            updated_at: now(),
        }
    }

    async fn create_tree(
        repository: &SqliteProductionStructureRepository,
    ) -> (ProductionSeries, ProductionEpisode, ProductionScene) {
        let first = repository.create_series(&series("ser_a")).await.unwrap();
        let episode = repository
            .create_episode(
                "project-1",
                &ProductionEpisode {
                    id: ProductionEpisodeId::parse("epi_a").unwrap(),
                    series_id: first.id.clone(),
                    ordinal: 0,
                    name: "Episode".to_owned(),
                    description: String::new(),
                    created_at: now(),
                    updated_at: now(),
                },
            )
            .await
            .unwrap();
        let scene = repository
            .create_scene(
                "project-1",
                &ProductionScene {
                    id: ProductionSceneId::parse("scn_a").unwrap(),
                    episode_id: episode.id.clone(),
                    ordinal: 0,
                    name: "Scene".to_owned(),
                    description: String::new(),
                    created_at: now(),
                    updated_at: now(),
                },
            )
            .await
            .unwrap();
        (first, episode, scene)
    }

    #[tokio::test]
    async fn migration_and_tree_are_set_based_and_keep_unassigned_shots() {
        let (_directory, _pool, repository) = setup().await;
        let (_series, _episode, scene) = create_tree(&repository).await;
        repository
            .assign_shots_atomic(
                "project-1",
                &scene.id,
                &["shot_b".to_owned(), "shot_a".to_owned()],
                now(),
            )
            .await
            .unwrap();
        let tree = repository.load_tree_data("project-1").await.unwrap();
        assert_eq!(tree.series.len(), 1);
        assert_eq!(tree.episodes.len(), 1);
        assert_eq!(tree.scenes.len(), 1);
        assert_eq!(tree.project_shot_ids.len(), 3);
        assert_eq!(
            tree.assignments
                .iter()
                .map(|a| a.shot_id.as_str())
                .collect::<Vec<_>>(),
            ["shot_b", "shot_a"]
        );
    }

    #[tokio::test]
    async fn append_reorders_and_scene_assignments_are_atomic() {
        let (_directory, pool, repository) = setup().await;
        let (series_a, episode, scene) = create_tree(&repository).await;
        let series_b = repository.create_series(&series("ser_b")).await.unwrap();
        repository
            .reorder_series(
                "project-1",
                &[series_b.id.clone(), series_a.id.clone()],
                now(),
            )
            .await
            .unwrap();
        let series_rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT id, ordinal FROM production_series ORDER BY ordinal")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            series_rows,
            vec![("ser_b".to_owned(), 0), ("ser_a".to_owned(), 1)]
        );

        repository
            .assign_shots_atomic(
                "project-1",
                &scene.id,
                &["shot_a".to_owned(), "shot_b".to_owned()],
                now(),
            )
            .await
            .unwrap();
        repository
            .assign_shots_atomic("project-1", &scene.id, &["shot_c".to_owned()], now())
            .await
            .unwrap();
        repository
            .reorder_scene_shots(
                &scene.id,
                &[
                    "shot_c".to_owned(),
                    "shot_a".to_owned(),
                    "shot_b".to_owned(),
                ],
                now(),
            )
            .await
            .unwrap();
        let local: Vec<(String, i64)> =
            sqlx::query_as("SELECT shot_id, ordinal FROM shot_scene_assignments ORDER BY ordinal")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            local,
            vec![
                ("shot_c".to_owned(), 0),
                ("shot_a".to_owned(), 1),
                ("shot_b".to_owned(), 2)
            ]
        );
        assert_eq!(episode.ordinal, 0);
    }

    #[tokio::test]
    async fn cross_project_and_invalid_reorders_fail_closed() {
        let (_directory, pool, repository) = setup().await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Other', 'C:/other', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let (_series, _episode, scene) = create_tree(&repository).await;
        let error = repository
            .assign_shots_atomic("project-2", &scene.id, &["shot_a".to_owned()], now())
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("PRODUCTION_STRUCTURE_PROJECT_MISMATCH"));
        let error = repository
            .reorder_scene_shots(&scene.id, &["shot_a".to_owned()], now())
            .await
            .unwrap_err();
        assert!(matches!(error, RepositoryError::Integrity { .. }));
    }

    #[tokio::test]
    async fn deleting_structure_cascades_assignments_but_preserves_shots() {
        let (_directory, pool, repository) = setup().await;
        let (series, episode, scene) = create_tree(&repository).await;
        repository
            .assign_shots_atomic("project-1", &scene.id, &["shot_a".to_owned()], now())
            .await
            .unwrap();
        assert!(repository
            .delete_series("project-1", &series.id)
            .await
            .unwrap());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shots WHERE project_id = 'project-1'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            3
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shot_scene_assignments")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(episode.series_id.as_str(), series.id.as_str());
    }

    #[test]
    fn fixed_clock_is_send_sync_compatible_with_service_boundaries() {
        let _clock: Arc<dyn Clock> = Arc::new(FixedClock(now()));
    }
}
