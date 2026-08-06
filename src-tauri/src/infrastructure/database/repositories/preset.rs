use super::{
    format_datetime, map_domain_error, map_sqlx_error, parse_datetime, parse_json, serialize_json,
};
use crate::application::ports::{PresetRepository, RepositoryError};
use crate::domain::{Preset, PresetId};
use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqlitePresetRepository {
    pool: SqlitePool,
}

impl SqlitePresetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PresetRepository for SqlitePresetRepository {
    async fn list(
        &self,
        project_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Vec<Preset>, RepositoryError> {
        let rows = sqlx::query_as::<_, PresetRow>(
            "SELECT id, project_id, workflow_version_id, recipe_id, name, values_json, created_at, updated_at
             FROM presets
             WHERE project_id = ? AND workflow_version_id = ? AND recipe_id = ?
             ORDER BY updated_at DESC, id ASC",
        )
        .bind(project_id)
        .bind(workflow_version_id)
        .bind(recipe_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(PresetRow::try_into_domain).collect()
    }

    async fn find_by_id(
        &self,
        project_id: &str,
        preset_id: &PresetId,
    ) -> Result<Option<Preset>, RepositoryError> {
        let row = sqlx::query_as::<_, PresetRow>(
            "SELECT id, project_id, workflow_version_id, recipe_id, name, values_json, created_at, updated_at
             FROM presets WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(preset_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(PresetRow::try_into_domain).transpose()
    }

    async fn find_by_name(
        &self,
        project_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
        name: &str,
    ) -> Result<Option<Preset>, RepositoryError> {
        let row = sqlx::query_as::<_, PresetRow>(
            "SELECT id, project_id, workflow_version_id, recipe_id, name, values_json, created_at, updated_at
             FROM presets
             WHERE project_id = ? AND workflow_version_id = ? AND recipe_id = ? AND name = ?",
        )
        .bind(project_id)
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(PresetRow::try_into_domain).transpose()
    }

    async fn insert(&self, preset: &Preset) -> Result<(), RepositoryError> {
        preset
            .validate()
            .map_err(|error| map_domain_error("preset validation", error))?;
        let values_json = serialize_json("preset values_json", Some(&preset.values_json))?
            .ok_or_else(|| RepositoryError::serialization("preset values_json", "missing value"))?;
        sqlx::query(
            "INSERT INTO presets (id, project_id, workflow_version_id, recipe_id, name, values_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(preset.id.as_str())
        .bind(&preset.project_id)
        .bind(&preset.workflow_version_id)
        .bind(&preset.recipe_id)
        .bind(&preset.name)
        .bind(values_json)
        .bind(format_datetime(preset.created_at))
        .bind(format_datetime(preset.updated_at))
        .execute(&self.pool)
        .await
        .map_err(|error| map_preset_sqlx_error(error, preset))?;
        Ok(())
    }

    async fn update(&self, preset: &Preset) -> Result<Option<Preset>, RepositoryError> {
        preset
            .validate()
            .map_err(|error| map_domain_error("preset validation", error))?;
        let values_json = serialize_json("preset values_json", Some(&preset.values_json))?
            .ok_or_else(|| RepositoryError::serialization("preset values_json", "missing value"))?;
        let result = sqlx::query(
            "UPDATE presets
             SET name = ?, values_json = ?, updated_at = ?
             WHERE project_id = ? AND id = ?",
        )
        .bind(&preset.name)
        .bind(values_json)
        .bind(format_datetime(preset.updated_at))
        .bind(&preset.project_id)
        .bind(preset.id.as_str())
        .execute(&self.pool)
        .await
        .map_err(|error| map_preset_sqlx_error(error, preset))?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_by_id(&preset.project_id, &preset.id).await
    }

    async fn delete(
        &self,
        project_id: &str,
        preset_id: &PresetId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM presets WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(preset_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }
}

fn map_preset_sqlx_error(error: sqlx::Error, preset: &Preset) -> RepositoryError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error
            .message()
            .to_ascii_lowercase()
            .contains("unique constraint failed: presets")
        {
            return RepositoryError::preset_name_conflict(
                &preset.project_id,
                &preset.workflow_version_id,
                &preset.recipe_id,
                &preset.name,
            );
        }
    }
    map_sqlx_error(error)
}

#[derive(sqlx::FromRow)]
struct PresetRow {
    id: String,
    project_id: String,
    workflow_version_id: String,
    recipe_id: String,
    name: String,
    values_json: String,
    created_at: String,
    updated_at: String,
}

impl PresetRow {
    fn try_into_domain(self) -> Result<Preset, RepositoryError> {
        let id = PresetId::parse(self.id).map_err(|error| map_domain_error("preset id", error))?;
        let values_json = parse_json("preset values_json", Some(&self.values_json))?
            .ok_or_else(|| RepositoryError::serialization("preset values_json", "missing value"))?;
        let created_at = parse_datetime("preset created_at", &self.created_at)?;
        let updated_at = parse_datetime("preset updated_at", &self.updated_at)?;
        let preset = Preset {
            id,
            project_id: self.project_id,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            name: self.name,
            values_json,
            created_at,
            updated_at,
        };
        preset
            .validate()
            .map_err(|error| map_domain_error("preset integrity", error))?;
        Ok(preset)
    }
}
