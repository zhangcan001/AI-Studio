use super::{
    format_datetime, map_domain_error, map_sqlx_error, parse_datetime, parse_json, serialize_json,
};
use crate::application::ports::{ModelRepository, RepositoryError};
use crate::domain::{Model, ModelId, ModelVersion, ModelVersionId};
use async_trait::async_trait;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone)]
pub struct SqliteModelRepository {
    pool: SqlitePool,
}

impl SqliteModelRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ModelRepository for SqliteModelRepository {
    async fn list_models(&self) -> Result<Vec<Model>, RepositoryError> {
        let rows = sqlx::query_as::<_, ModelRow>(
            "SELECT id, name, provider, type AS model_type, description,
                    metadata_json, created_at
             FROM models
             ORDER BY provider COLLATE NOCASE ASC, name COLLATE NOCASE ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(ModelRow::try_into_domain).collect()
    }

    async fn find_model(&self, model_id: &ModelId) -> Result<Option<Model>, RepositoryError> {
        let row = sqlx::query_as::<_, ModelRow>(
            "SELECT id, name, provider, type AS model_type, description,
                    metadata_json, created_at
             FROM models WHERE id = ?",
        )
        .bind(model_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(ModelRow::try_into_domain).transpose()
    }

    async fn create_model(&self, model: &Model) -> Result<(), RepositoryError> {
        model
            .validate()
            .map_err(|error| map_domain_error("model validation", error))?;
        let metadata_json = serialize_json("model metadata", Some(&model.metadata_json))?
            .ok_or_else(|| RepositoryError::serialization("model metadata", "missing value"))?;
        sqlx::query(
            "INSERT INTO models
             (id, name, provider, type, description, metadata_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(model.id.as_str())
        .bind(&model.name)
        .bind(&model.provider)
        .bind(&model.model_type)
        .bind(&model.description)
        .bind(metadata_json)
        .bind(format_datetime(model.created_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn update_model(&self, model: &Model) -> Result<Option<Model>, RepositoryError> {
        model
            .validate()
            .map_err(|error| map_domain_error("model validation", error))?;
        let metadata_json = serialize_json("model metadata", Some(&model.metadata_json))?
            .ok_or_else(|| RepositoryError::serialization("model metadata", "missing value"))?;
        let result = sqlx::query(
            "UPDATE models
             SET name = ?, provider = ?, type = ?, description = ?, metadata_json = ?
             WHERE id = ?",
        )
        .bind(&model.name)
        .bind(&model.provider)
        .bind(&model.model_type)
        .bind(&model.description)
        .bind(metadata_json)
        .bind(model.id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_model(&model.id).await
    }

    async fn delete_model(&self, model_id: &ModelId) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(model_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn list_versions(
        &self,
        model_id: &ModelId,
    ) -> Result<Vec<ModelVersion>, RepositoryError> {
        let rows = sqlx::query_as::<_, ModelVersionRow>(
            "SELECT id, model_id, version, capabilities_json, parameter_schema_json, created_at
             FROM model_versions
             WHERE model_id = ?
             ORDER BY created_at ASC, id ASC",
        )
        .bind(model_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ModelVersionRow::try_into_domain)
            .collect()
    }

    async fn current_version(
        &self,
        model_id: &ModelId,
    ) -> Result<Option<ModelVersion>, RepositoryError> {
        let row = sqlx::query_as::<_, ModelVersionRow>(
            "SELECT id, model_id, version, capabilities_json, parameter_schema_json, created_at
             FROM model_versions
             WHERE model_id = ?
             ORDER BY created_at DESC, id DESC
             LIMIT 1",
        )
        .bind(model_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(ModelVersionRow::try_into_domain).transpose()
    }

    async fn find_version_by_id(
        &self,
        version_id: &ModelVersionId,
    ) -> Result<Option<ModelVersion>, RepositoryError> {
        let row = sqlx::query_as::<_, ModelVersionRow>(
            "SELECT id, model_id, version, capabilities_json, parameter_schema_json, created_at
             FROM model_versions WHERE id = ?",
        )
        .bind(version_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(ModelVersionRow::try_into_domain).transpose()
    }

    async fn create_version(&self, version: &ModelVersion) -> Result<(), RepositoryError> {
        version
            .validate()
            .map_err(|error| map_domain_error("model version validation", error))?;
        let capabilities_json = serialize_json(
            "model version capabilities",
            Some(&version.capabilities_json),
        )?
        .ok_or_else(|| {
            RepositoryError::serialization("model version capabilities", "missing value")
        })?;
        let parameter_schema_json = serialize_json(
            "model version parameter schema",
            Some(&version.parameter_schema_json),
        )?
        .ok_or_else(|| {
            RepositoryError::serialization("model version parameter schema", "missing value")
        })?;
        sqlx::query(
            "INSERT INTO model_versions
             (id, model_id, version, capabilities_json, parameter_schema_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(version.id.as_str())
        .bind(version.model_id.as_str())
        .bind(&version.version)
        .bind(capabilities_json)
        .bind(parameter_schema_json)
        .bind(format_datetime(version.created_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }
}

#[derive(Debug, FromRow)]
struct ModelRow {
    id: String,
    name: String,
    provider: String,
    model_type: String,
    description: String,
    metadata_json: String,
    created_at: String,
}

impl ModelRow {
    fn try_into_domain(self) -> Result<Model, RepositoryError> {
        let model = Model {
            id: ModelId::parse(self.id).map_err(|error| map_domain_error("model id", error))?,
            name: self.name,
            provider: self.provider,
            model_type: self.model_type,
            description: self.description,
            metadata_json: parse_json("model metadata", Some(&self.metadata_json))?
                .ok_or_else(|| RepositoryError::serialization("model metadata", "missing value"))?,
            created_at: parse_datetime("model created_at", &self.created_at)?,
        };
        model
            .validate()
            .map_err(|error| map_domain_error("model integrity", error))?;
        Ok(model)
    }
}

#[derive(Debug, FromRow)]
struct ModelVersionRow {
    id: String,
    model_id: String,
    version: String,
    capabilities_json: String,
    parameter_schema_json: String,
    created_at: String,
}

impl ModelVersionRow {
    fn try_into_domain(self) -> Result<ModelVersion, RepositoryError> {
        let version = ModelVersion {
            id: ModelVersionId::parse(self.id)
                .map_err(|error| map_domain_error("model version id", error))?,
            model_id: ModelId::parse(self.model_id)
                .map_err(|error| map_domain_error("model version model_id", error))?,
            version: self.version,
            capabilities_json: parse_json(
                "model version capabilities",
                Some(&self.capabilities_json),
            )?
            .ok_or_else(|| {
                RepositoryError::serialization("model version capabilities", "missing value")
            })?,
            parameter_schema_json: parse_json(
                "model version parameter schema",
                Some(&self.parameter_schema_json),
            )?
            .ok_or_else(|| {
                RepositoryError::serialization("model version parameter schema", "missing value")
            })?,
            created_at: parse_datetime("model version created_at", &self.created_at)?,
        };
        version
            .validate()
            .map_err(|error| map_domain_error("model version integrity", error))?;
        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteModelRepository;
    use crate::application::ports::ModelRepository;
    use crate::domain::{Model, ModelId, ModelVersion, ModelVersionId};
    use crate::infrastructure::database::initialize;
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use tempfile::tempdir;

    fn model(id: &str) -> Model {
        Model::new(
            ModelId::parse(id).unwrap(),
            "H3",
            "MiniMax",
            "video",
            "external model",
            json!({"tool": "H3"}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        )
        .unwrap()
    }

    fn version(id: &str, model_id: &str, value: &str, second: u32) -> ModelVersion {
        ModelVersion::new(
            ModelVersionId::parse(id).unwrap(),
            ModelId::parse(model_id).unwrap(),
            value,
            json!(["text_to_video"]),
            json!({"duration": {"type": "number"}}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, second).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn model_crud_and_version_query_round_trip() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("models.db"))
            .await
            .unwrap();
        let repository = SqliteModelRepository::new(pool);
        let first = model("mdl_first");
        repository.create_model(&first).await.unwrap();
        assert_eq!(repository.list_models().await.unwrap(), vec![first.clone()]);
        assert_eq!(
            repository.find_model(&first.id).await.unwrap(),
            Some(first.clone())
        );

        let updated = Model {
            name: "H3 updated".to_owned(),
            description: "updated".to_owned(),
            metadata_json: json!({"tool": "H3", "local": true}),
            ..first.clone()
        };
        assert_eq!(
            repository.update_model(&updated).await.unwrap(),
            Some(updated.clone())
        );

        let first_version = version("mdv_first_1", "mdl_first", "2026-01", 1);
        let second_version = version("mdv_first_2", "mdl_first", "2026-02", 2);
        repository.create_version(&first_version).await.unwrap();
        repository.create_version(&second_version).await.unwrap();
        assert_eq!(
            repository.list_versions(&first.id).await.unwrap(),
            vec![first_version.clone(), second_version.clone()]
        );
        assert_eq!(
            repository.current_version(&first.id).await.unwrap(),
            Some(second_version)
        );
        assert_eq!(
            repository
                .find_version_by_id(&first_version.id)
                .await
                .unwrap(),
            Some(first_version)
        );

        let second = model("mdl_second");
        repository.create_model(&second).await.unwrap();
        assert!(repository.delete_model(&second.id).await.unwrap());
        assert!(!repository.delete_model(&second.id).await.unwrap());
        assert!(repository.delete_model(&updated.id).await.is_err());
        assert!(repository.find_model(&updated.id).await.unwrap().is_some());
    }
}
