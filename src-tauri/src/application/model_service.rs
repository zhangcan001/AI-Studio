use crate::application::ports::{Clock, ModelRepository, RepositoryError};
use crate::domain::{Model, ModelDomainError, ModelId, ModelVersion, ModelVersionId};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::{error::Error, fmt, sync::Arc};

const MAX_IDENTITY_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 500;
const MAX_VERSION_CHARS: usize = 120;

#[derive(Clone, Debug)]
pub struct CreateModelRequest {
    pub name: String,
    pub provider: String,
    pub model_type: String,
    pub description: String,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct UpdateModelRequest {
    pub model_id: String,
    pub name: String,
    pub provider: String,
    pub model_type: String,
    pub description: String,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct CreateModelVersionRequest {
    pub model_id: String,
    pub version: String,
    pub capabilities: Value,
    pub parameter_schema: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelView {
    pub id: String,
    pub name: String,
    pub provider: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub description: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVersionView {
    pub id: String,
    pub model_id: String,
    pub version: String,
    pub capabilities: Value,
    pub parameter_schema: Value,
    pub created_at: DateTime<Utc>,
}

pub struct ModelService {
    repository: Arc<dyn ModelRepository>,
    clock: Arc<dyn Clock>,
}

impl ModelService {
    pub fn new(repository: Arc<dyn ModelRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn list(&self) -> Result<Vec<ModelView>, ModelServiceError> {
        self.repository
            .list_models()
            .await?
            .into_iter()
            .map(model_to_view)
            .collect()
    }

    pub async fn get(&self, model_id: &str) -> Result<ModelView, ModelServiceError> {
        let model_id = parse_model_id(model_id)?;
        let model = self
            .repository
            .find_model(&model_id)
            .await?
            .ok_or_else(|| ModelServiceError::NotFound(model_id.to_string()))?;
        model_to_view(model)
    }

    pub async fn create(
        &self,
        request: CreateModelRequest,
    ) -> Result<ModelView, ModelServiceError> {
        let model = Model::new(
            ModelId::new(),
            normalize_identity(&request.name, "name")?,
            normalize_identity(&request.provider, "provider")?,
            normalize_identity(&request.model_type, "type")?,
            normalize_description(&request.description)?,
            normalize_json(request.metadata, "metadata")?,
            self.clock.now(),
        )?;
        self.repository.create_model(&model).await?;
        model_to_view(model)
    }

    pub async fn update(
        &self,
        request: UpdateModelRequest,
    ) -> Result<ModelView, ModelServiceError> {
        let model_id = parse_model_id(&request.model_id)?;
        let current = self
            .repository
            .find_model(&model_id)
            .await?
            .ok_or_else(|| ModelServiceError::NotFound(model_id.to_string()))?;
        let model = Model::new(
            model_id,
            normalize_identity(&request.name, "name")?,
            normalize_identity(&request.provider, "provider")?,
            normalize_identity(&request.model_type, "type")?,
            normalize_description(&request.description)?,
            normalize_json(request.metadata, "metadata")?,
            current.created_at,
        )?;
        let model = self
            .repository
            .update_model(&model)
            .await?
            .ok_or_else(|| ModelServiceError::NotFound(model.id.to_string()))?;
        model_to_view(model)
    }

    pub async fn delete(&self, model_id: &str) -> Result<(), ModelServiceError> {
        let model_id = parse_model_id(model_id)?;
        if !self.repository.delete_model(&model_id).await? {
            return Err(ModelServiceError::NotFound(model_id.to_string()));
        }
        Ok(())
    }

    pub async fn list_versions(
        &self,
        model_id: &str,
    ) -> Result<Vec<ModelVersionView>, ModelServiceError> {
        let model_id = self.require_model(model_id).await?;
        self.repository
            .list_versions(&model_id)
            .await?
            .into_iter()
            .map(version_to_view)
            .collect()
    }

    pub async fn current_version(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelVersionView>, ModelServiceError> {
        let model_id = self.require_model(model_id).await?;
        self.repository
            .current_version(&model_id)
            .await?
            .map(version_to_view)
            .transpose()
    }

    pub async fn create_version(
        &self,
        request: CreateModelVersionRequest,
    ) -> Result<ModelVersionView, ModelServiceError> {
        let model_id = self.require_model(&request.model_id).await?;
        let version = ModelVersion::new(
            ModelVersionId::new(),
            model_id,
            normalize_version(&request.version)?,
            normalize_json(request.capabilities, "capabilities")?,
            normalize_json(request.parameter_schema, "parameter_schema")?,
            self.clock.now(),
        )?;
        self.repository.create_version(&version).await?;
        version_to_view(version)
    }

    pub async fn get_version(
        &self,
        version_id: &str,
    ) -> Result<ModelVersionView, ModelServiceError> {
        let version_id = parse_model_version_id(version_id)?;
        let version = self
            .repository
            .find_version_by_id(&version_id)
            .await?
            .ok_or_else(|| ModelServiceError::NotFound(version_id.to_string()))?;
        version_to_view(version)
    }

    async fn require_model(&self, model_id: &str) -> Result<ModelId, ModelServiceError> {
        let model_id = parse_model_id(model_id)?;
        if self.repository.find_model(&model_id).await?.is_none() {
            return Err(ModelServiceError::NotFound(model_id.to_string()));
        }
        Ok(model_id)
    }
}

fn model_to_view(model: Model) -> Result<ModelView, ModelServiceError> {
    model.validate().map_err(ModelServiceError::Domain)?;
    Ok(ModelView {
        id: model.id.to_string(),
        name: model.name,
        provider: model.provider,
        model_type: model.model_type,
        description: model.description,
        metadata: model.metadata_json,
        created_at: model.created_at,
    })
}

fn version_to_view(version: ModelVersion) -> Result<ModelVersionView, ModelServiceError> {
    version.validate().map_err(ModelServiceError::Domain)?;
    Ok(ModelVersionView {
        id: version.id.to_string(),
        model_id: version.model_id.to_string(),
        version: version.version,
        capabilities: version.capabilities_json,
        parameter_schema: version.parameter_schema_json,
        created_at: version.created_at,
    })
}

fn parse_model_id(value: &str) -> Result<ModelId, ModelServiceError> {
    ModelId::parse(value.trim().to_owned())
        .map_err(|error| ModelServiceError::InvalidInput(error.to_string()))
}

fn parse_model_version_id(value: &str) -> Result<ModelVersionId, ModelServiceError> {
    ModelVersionId::parse(value.trim().to_owned())
        .map_err(|error| ModelServiceError::InvalidInput(error.to_string()))
}

fn normalize_identity(value: &str, field: &str) -> Result<String, ModelServiceError> {
    let value = value.trim();
    if value.is_empty() || value.contains(['\r', '\n']) {
        return Err(ModelServiceError::InvalidInput(format!(
            "model {field} must be a non-empty single-line value"
        )));
    }
    if value.chars().count() > MAX_IDENTITY_CHARS {
        return Err(ModelServiceError::InvalidInput(format!(
            "model {field} must be at most {MAX_IDENTITY_CHARS} characters"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_description(value: &str) -> Result<String, ModelServiceError> {
    let value = value.trim();
    if value.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(ModelServiceError::InvalidInput(format!(
            "model description must be at most {MAX_DESCRIPTION_CHARS} characters"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_version(value: &str) -> Result<String, ModelServiceError> {
    let value = value.trim();
    if value.is_empty() || value.contains(['\r', '\n']) {
        return Err(ModelServiceError::InvalidInput(
            "model version must be a non-empty single-line value".to_owned(),
        ));
    }
    if value.chars().count() > MAX_VERSION_CHARS {
        return Err(ModelServiceError::InvalidInput(format!(
            "model version must be at most {MAX_VERSION_CHARS} characters"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_json(value: Value, field: &str) -> Result<Value, ModelServiceError> {
    if value.is_null() {
        return Err(ModelServiceError::InvalidInput(format!(
            "model {field} must not be null"
        )));
    }
    Ok(value)
}

#[derive(Debug)]
pub enum ModelServiceError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
    Domain(ModelDomainError),
}

impl fmt::Display for ModelServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) => formatter.write_str(message),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Domain(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ModelServiceError {}

impl From<RepositoryError> for ModelServiceError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

impl From<ModelDomainError> for ModelServiceError {
    fn from(value: ModelDomainError) -> Self {
        Self::Domain(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateModelRequest, CreateModelVersionRequest, ModelService, ModelServiceError,
        UpdateModelRequest,
    };
    use crate::application::ports::Clock;
    use crate::infrastructure::database::{initialize, SqliteModelRepository};
    use chrono::{DateTime, TimeZone, Utc};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    struct FixedClock(DateTime<Utc>);

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    async fn service() -> (tempfile::TempDir, ModelService) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("models.db"))
            .await
            .unwrap();
        (
            directory,
            ModelService::new(
                Arc::new(SqliteModelRepository::new(pool)),
                Arc::new(FixedClock(
                    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
                )),
            ),
        )
    }

    #[tokio::test]
    async fn service_crud_and_version_queries_are_validated() {
        let (_directory, service) = service().await;
        let created = service
            .create(CreateModelRequest {
                name: "  H3  ".to_owned(),
                provider: " MiniMax ".to_owned(),
                model_type: "video".to_owned(),
                description: " external ".to_owned(),
                metadata: json!({"tool": "H3"}),
            })
            .await
            .unwrap();
        assert_eq!(created.name, "H3");
        assert_eq!(service.list().await.unwrap().len(), 1);
        let version = service
            .create_version(CreateModelVersionRequest {
                model_id: created.id.clone(),
                version: "2026-01".to_owned(),
                capabilities: json!(["text_to_video"]),
                parameter_schema: json!({"fps": {"type": "integer"}}),
            })
            .await
            .unwrap();
        assert_eq!(
            service
                .current_version(&created.id)
                .await
                .unwrap()
                .unwrap()
                .id,
            version.id
        );
        assert_eq!(service.list_versions(&created.id).await.unwrap().len(), 1);
        let version_before_model_update = service.get_version(&version.id).await.unwrap();

        let updated = service
            .update(UpdateModelRequest {
                model_id: created.id.clone(),
                name: "H3 updated".to_owned(),
                provider: "MiniMax".to_owned(),
                model_type: "video".to_owned(),
                description: "updated".to_owned(),
                metadata: json!({"local": true}),
            })
            .await
            .unwrap();
        assert_eq!(updated.name, "H3 updated");
        assert_eq!(
            service.get_version(&version.id).await.unwrap(),
            version_before_model_update
        );
        assert!(matches!(
            service
                .create_version(CreateModelVersionRequest {
                    model_id: created.id.clone(),
                    version: "2026-01".to_owned(),
                    capabilities: json!([]),
                    parameter_schema: json!({}),
                })
                .await,
            Err(ModelServiceError::Repository(_))
        ));
        assert!(service.delete(&created.id).await.is_err());
    }
}
