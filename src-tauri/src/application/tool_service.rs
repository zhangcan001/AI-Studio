use crate::application::ports::{Clock, RepositoryError, ToolRepository};
use crate::domain::{
    Capability, Tool, ToolDomainError, ToolHealthStatus, ToolId, ToolInstance, ToolInstanceId,
    ToolVersion, ToolVersionId,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::{error::Error, fmt, sync::Arc};

const MAX_IDENTITY_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 500;
const MAX_LOCATION_CHARS: usize = 4_096;
const MAX_VERSION_CHARS: usize = 120;

#[derive(Clone, Debug)]
pub struct CreateToolRequest {
    pub name: String,
    pub tool_type: String,
    pub description: String,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct UpdateToolRequest {
    pub tool_id: String,
    pub name: String,
    pub tool_type: String,
    pub description: String,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct CreateToolInstanceRequest {
    pub tool_id: String,
    pub path: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateToolVersionRequest {
    pub tool_id: String,
    pub version: String,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct CreateCapabilityRequest {
    pub tool_id: String,
    pub capability_name: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolView {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub description: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInstanceView {
    pub id: String,
    pub tool_id: String,
    pub path: Option<String>,
    pub endpoint: Option<String>,
    pub status: ToolHealthStatus,
    pub last_checked: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersionView {
    pub id: String,
    pub tool_id: String,
    pub version: String,
    pub observed_at: DateTime<Utc>,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityView {
    pub tool_id: String,
    pub capability_name: String,
    pub metadata: Value,
}

pub type ToolCapabilityView = CapabilityView;

pub struct ToolService {
    repository: Arc<dyn ToolRepository>,
    clock: Arc<dyn Clock>,
}

impl ToolService {
    pub fn new(repository: Arc<dyn ToolRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn list(&self) -> Result<Vec<ToolView>, ToolServiceError> {
        self.repository
            .list_tools()
            .await?
            .into_iter()
            .map(tool_to_view)
            .collect()
    }

    pub async fn get(&self, tool_id: &str) -> Result<ToolView, ToolServiceError> {
        let tool_id = parse_tool_id(tool_id)?;
        let tool = self
            .repository
            .find_tool(&tool_id)
            .await?
            .ok_or_else(|| ToolServiceError::NotFound(tool_id.to_string()))?;
        tool_to_view(tool)
    }

    pub async fn create(&self, request: CreateToolRequest) -> Result<ToolView, ToolServiceError> {
        let tool = Tool::new(
            ToolId::new(),
            normalize_identity(&request.name, "name")?,
            normalize_identity(&request.tool_type, "type")?,
            normalize_description(&request.description)?,
            normalize_json(request.metadata, "metadata")?,
            self.clock.now(),
        )?;
        self.repository.create_tool(&tool).await?;
        tool_to_view(tool)
    }

    pub async fn update(&self, request: UpdateToolRequest) -> Result<ToolView, ToolServiceError> {
        let tool_id = parse_tool_id(&request.tool_id)?;
        let current = self
            .repository
            .find_tool(&tool_id)
            .await?
            .ok_or_else(|| ToolServiceError::NotFound(tool_id.to_string()))?;
        let tool = Tool::new(
            tool_id,
            normalize_identity(&request.name, "name")?,
            normalize_identity(&request.tool_type, "type")?,
            normalize_description(&request.description)?,
            normalize_json(request.metadata, "metadata")?,
            current.created_at,
        )?;
        let tool = self
            .repository
            .update_tool(&tool)
            .await?
            .ok_or_else(|| ToolServiceError::NotFound(tool.id.to_string()))?;
        tool_to_view(tool)
    }

    pub async fn delete(&self, tool_id: &str) -> Result<(), ToolServiceError> {
        let tool_id = parse_tool_id(tool_id)?;
        if !self.repository.delete_tool(&tool_id).await? {
            return Err(ToolServiceError::NotFound(tool_id.to_string()));
        }
        Ok(())
    }

    pub async fn list_instances(
        &self,
        tool_id: &str,
    ) -> Result<Vec<ToolInstanceView>, ToolServiceError> {
        let tool_id = self.require_tool(tool_id).await?;
        self.repository
            .list_instances(&tool_id)
            .await?
            .into_iter()
            .map(instance_to_view)
            .collect()
    }

    pub async fn get_instance(
        &self,
        instance_id: &str,
    ) -> Result<ToolInstanceView, ToolServiceError> {
        let instance_id = parse_instance_id(instance_id)?;
        let instance = self
            .repository
            .find_instance(&instance_id)
            .await?
            .ok_or_else(|| ToolServiceError::NotFound(instance_id.to_string()))?;
        instance_to_view(instance)
    }

    pub async fn create_instance(
        &self,
        request: CreateToolInstanceRequest,
    ) -> Result<ToolInstanceView, ToolServiceError> {
        let tool_id = self.require_tool(&request.tool_id).await?;
        let instance = ToolInstance::new(
            ToolInstanceId::new(),
            tool_id,
            normalize_location(request.path, "path")?,
            normalize_location(request.endpoint, "endpoint")?,
            ToolHealthStatus::Unknown,
            None,
        )?;
        self.repository.create_instance(&instance).await?;
        instance_to_view(instance)
    }

    /// Records an explicitly supplied observation; it never probes or starts a tool.
    pub async fn record_health(
        &self,
        instance_id: &str,
        status: ToolHealthStatus,
    ) -> Result<ToolInstanceView, ToolServiceError> {
        let instance_id = parse_instance_id(instance_id)?;
        let current = self
            .repository
            .find_instance(&instance_id)
            .await?
            .ok_or_else(|| ToolServiceError::NotFound(instance_id.to_string()))?;
        let instance = ToolInstance::new(
            current.id,
            current.tool_id,
            current.path,
            current.endpoint,
            status,
            Some(self.clock.now()),
        )?;
        let instance = self
            .repository
            .update_instance(&instance)
            .await?
            .ok_or_else(|| ToolServiceError::NotFound(instance.id.to_string()))?;
        instance_to_view(instance)
    }

    pub async fn list_versions(
        &self,
        tool_id: &str,
    ) -> Result<Vec<ToolVersionView>, ToolServiceError> {
        let tool_id = self.require_tool(tool_id).await?;
        self.repository
            .list_versions(&tool_id)
            .await?
            .into_iter()
            .map(version_to_view)
            .collect()
    }

    pub async fn get_version(&self, version_id: &str) -> Result<ToolVersionView, ToolServiceError> {
        let version_id = parse_version_id(version_id)?;
        let version = self
            .repository
            .find_version(&version_id)
            .await?
            .ok_or_else(|| ToolServiceError::NotFound(version_id.to_string()))?;
        version_to_view(version)
    }

    pub async fn create_version(
        &self,
        request: CreateToolVersionRequest,
    ) -> Result<ToolVersionView, ToolServiceError> {
        let tool_id = self.require_tool(&request.tool_id).await?;
        let version = ToolVersion::new(
            ToolVersionId::new(),
            tool_id,
            normalize_version(&request.version)?,
            self.clock.now(),
            normalize_json(request.metadata, "metadata")?,
        )?;
        self.repository.create_version(&version).await?;
        version_to_view(version)
    }

    pub async fn list_capabilities(
        &self,
        tool_id: &str,
    ) -> Result<Vec<CapabilityView>, ToolServiceError> {
        let tool_id = self.require_tool(tool_id).await?;
        self.repository
            .list_capabilities(&tool_id)
            .await?
            .into_iter()
            .map(capability_to_view)
            .collect()
    }

    pub async fn create_capability(
        &self,
        request: CreateCapabilityRequest,
    ) -> Result<CapabilityView, ToolServiceError> {
        let tool_id = self.require_tool(&request.tool_id).await?;
        let capability = Capability::new(
            tool_id,
            normalize_identity(&request.capability_name, "capability_name")?,
            normalize_json(request.metadata, "metadata")?,
        )?;
        self.repository.create_capability(&capability).await?;
        capability_to_view(capability)
    }

    async fn require_tool(&self, tool_id: &str) -> Result<ToolId, ToolServiceError> {
        let tool_id = parse_tool_id(tool_id)?;
        if self.repository.find_tool(&tool_id).await?.is_none() {
            return Err(ToolServiceError::NotFound(tool_id.to_string()));
        }
        Ok(tool_id)
    }
}

fn tool_to_view(tool: Tool) -> Result<ToolView, ToolServiceError> {
    tool.validate().map_err(ToolServiceError::Domain)?;
    Ok(ToolView {
        id: tool.id.to_string(),
        name: tool.name,
        tool_type: tool.tool_type,
        description: tool.description,
        metadata: tool.metadata_json,
        created_at: tool.created_at,
    })
}

fn instance_to_view(instance: ToolInstance) -> Result<ToolInstanceView, ToolServiceError> {
    instance.validate().map_err(ToolServiceError::Domain)?;
    Ok(ToolInstanceView {
        id: instance.id.to_string(),
        tool_id: instance.tool_id.to_string(),
        path: instance.path,
        endpoint: instance.endpoint,
        status: instance.status,
        last_checked: instance.last_checked,
    })
}

fn version_to_view(version: ToolVersion) -> Result<ToolVersionView, ToolServiceError> {
    version.validate().map_err(ToolServiceError::Domain)?;
    Ok(ToolVersionView {
        id: version.id.to_string(),
        tool_id: version.tool_id.to_string(),
        version: version.version,
        observed_at: version.observed_at,
        metadata: version.metadata_json,
    })
}

fn capability_to_view(capability: Capability) -> Result<CapabilityView, ToolServiceError> {
    capability.validate().map_err(ToolServiceError::Domain)?;
    Ok(CapabilityView {
        tool_id: capability.tool_id.to_string(),
        capability_name: capability.capability_name,
        metadata: capability.metadata_json,
    })
}

fn parse_tool_id(value: &str) -> Result<ToolId, ToolServiceError> {
    ToolId::parse(value.trim().to_owned())
        .map_err(|error| ToolServiceError::InvalidInput(error.to_string()))
}

fn parse_instance_id(value: &str) -> Result<ToolInstanceId, ToolServiceError> {
    ToolInstanceId::parse(value.trim().to_owned())
        .map_err(|error| ToolServiceError::InvalidInput(error.to_string()))
}

fn parse_version_id(value: &str) -> Result<ToolVersionId, ToolServiceError> {
    ToolVersionId::parse(value.trim().to_owned())
        .map_err(|error| ToolServiceError::InvalidInput(error.to_string()))
}

fn normalize_identity(value: &str, field: &str) -> Result<String, ToolServiceError> {
    let value = value.trim();
    if value.is_empty() || value.contains(['\r', '\n']) {
        return Err(ToolServiceError::InvalidInput(format!(
            "tool {field} must be a non-empty single-line value"
        )));
    }
    if value.chars().count() > MAX_IDENTITY_CHARS {
        return Err(ToolServiceError::InvalidInput(format!(
            "tool {field} must be at most {MAX_IDENTITY_CHARS} characters"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_description(value: &str) -> Result<String, ToolServiceError> {
    let value = value.trim();
    if value.contains(['\r', '\n']) {
        return Err(ToolServiceError::InvalidInput(
            "tool description must be a single-line value".to_owned(),
        ));
    }
    if value.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(ToolServiceError::InvalidInput(format!(
            "tool description must be at most {MAX_DESCRIPTION_CHARS} characters"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_location(
    value: Option<String>,
    field: &str,
) -> Result<Option<String>, ToolServiceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.contains(['\r', '\n']) {
        return Err(ToolServiceError::InvalidInput(format!(
            "tool {field} must be a single-line value"
        )));
    }
    if value.chars().count() > MAX_LOCATION_CHARS {
        return Err(ToolServiceError::InvalidInput(format!(
            "tool {field} must be at most {MAX_LOCATION_CHARS} characters"
        )));
    }
    Ok(Some(value.to_owned()))
}

fn normalize_version(value: &str) -> Result<String, ToolServiceError> {
    let value = value.trim();
    if value.is_empty() || value.contains(['\r', '\n']) {
        return Err(ToolServiceError::InvalidInput(
            "tool version must be a non-empty single-line value".to_owned(),
        ));
    }
    if value.chars().count() > MAX_VERSION_CHARS {
        return Err(ToolServiceError::InvalidInput(format!(
            "tool version must be at most {MAX_VERSION_CHARS} characters"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_json(value: Value, field: &str) -> Result<Value, ToolServiceError> {
    if value.is_null() {
        return Err(ToolServiceError::InvalidInput(format!(
            "tool {field} must not be null"
        )));
    }
    Ok(value)
}

#[derive(Debug)]
pub enum ToolServiceError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
    Domain(ToolDomainError),
}

impl fmt::Display for ToolServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) => formatter.write_str(message),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Domain(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ToolServiceError {}

impl From<RepositoryError> for ToolServiceError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

impl From<ToolDomainError> for ToolServiceError {
    fn from(value: ToolDomainError) -> Self {
        Self::Domain(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateCapabilityRequest, CreateToolInstanceRequest, CreateToolRequest,
        CreateToolVersionRequest, ToolService, UpdateToolRequest,
    };
    use crate::application::ports::Clock;
    use crate::domain::ToolHealthStatus;
    use crate::infrastructure::database::{initialize, SqliteToolRepository};
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

    async fn service() -> (tempfile::TempDir, ToolService) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("tools.db"))
            .await
            .unwrap();
        (
            directory,
            ToolService::new(
                Arc::new(SqliteToolRepository::new(pool)),
                Arc::new(FixedClock(
                    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
                )),
            ),
        )
    }

    #[tokio::test]
    async fn service_crud_queries_children_and_records_health_without_probing() {
        let (_directory, service) = service().await;
        let created = service
            .create(CreateToolRequest {
                name: "  ComfyUI  ".to_owned(),
                tool_type: "image".to_owned(),
                description: " local runtime ".to_owned(),
                metadata: json!({"managed": false}),
            })
            .await
            .unwrap();
        assert_eq!(created.name, "ComfyUI");
        assert_eq!(service.list().await.unwrap().len(), 1);

        let instance = service
            .create_instance(CreateToolInstanceRequest {
                tool_id: created.id.clone(),
                path: Some(" C:/ComfyUI ".to_owned()),
                endpoint: Some("http://127.0.0.1:8188".to_owned()),
            })
            .await
            .unwrap();
        assert_eq!(instance.status, ToolHealthStatus::Unknown);
        let observed = service
            .record_health(&instance.id, ToolHealthStatus::Available)
            .await
            .unwrap();
        assert_eq!(observed.status, ToolHealthStatus::Available);
        assert!(observed.last_checked.is_some());

        let version = service
            .create_version(CreateToolVersionRequest {
                tool_id: created.id.clone(),
                version: "0.3.0".to_owned(),
                metadata: json!({"source": "manual"}),
            })
            .await
            .unwrap();
        assert_eq!(
            service.list_versions(&created.id).await.unwrap(),
            vec![version]
        );

        let capability = service
            .create_capability(CreateCapabilityRequest {
                tool_id: created.id.clone(),
                capability_name: "image_generation".to_owned(),
                metadata: json!({"available": true}),
            })
            .await
            .unwrap();
        assert_eq!(
            service.list_capabilities(&created.id).await.unwrap(),
            vec![capability]
        );

        let updated = service
            .update(UpdateToolRequest {
                tool_id: created.id.clone(),
                name: "ComfyUI updated".to_owned(),
                tool_type: "image".to_owned(),
                description: "updated".to_owned(),
                metadata: json!({"managed": true}),
            })
            .await
            .unwrap();
        assert_eq!(updated.name, "ComfyUI updated");
        assert!(service.delete(&created.id).await.is_err());
    }
}
