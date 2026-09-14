use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolId(String);

impl ToolId {
    pub fn new() -> Self {
        Self(format!("tool_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ToolDomainError> {
        let value = value.into();
        if value.starts_with("tool_") && value.len() > "tool_".len() {
            Ok(Self(value))
        } else {
            Err(ToolDomainError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ToolId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolInstanceId(String);

impl ToolInstanceId {
    pub fn new() -> Self {
        Self(format!("tins_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ToolDomainError> {
        let value = value.into();
        if value.starts_with("tins_") && value.len() > "tins_".len() {
            Ok(Self(value))
        } else {
            Err(ToolDomainError::InvalidInstanceId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ToolInstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ToolInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolVersionId(String);

impl ToolVersionId {
    pub fn new() -> Self {
        Self(format!("tver_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ToolDomainError> {
        let value = value.into();
        if value.starts_with("tver_") && value.len() > "tver_".len() {
            Ok(Self(value))
        } else {
            Err(ToolDomainError::InvalidVersionId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ToolVersionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ToolVersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolHealthStatus {
    Available,
    Missing,
    Unknown,
}

impl ToolHealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "AVAILABLE",
            Self::Missing => "MISSING",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, ToolDomainError> {
        match value {
            "AVAILABLE" => Ok(Self::Available),
            "MISSING" => Ok(Self::Missing),
            "UNKNOWN" => Ok(Self::Unknown),
            other => Err(ToolDomainError::InvalidField(format!(
                "invalid health status {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub id: ToolId,
    pub name: String,
    pub tool_type: String,
    pub description: String,
    pub metadata_json: Value,
    pub created_at: DateTime<Utc>,
}

impl Tool {
    pub fn new(
        id: ToolId,
        name: impl Into<String>,
        tool_type: impl Into<String>,
        description: impl Into<String>,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, ToolDomainError> {
        let tool = Self {
            id,
            name: name.into(),
            tool_type: tool_type.into(),
            description: description.into(),
            metadata_json,
            created_at,
        };
        tool.validate()?;
        Ok(tool)
    }

    pub fn validate(&self) -> Result<(), ToolDomainError> {
        validate_single_line_required(&self.name, "name")?;
        validate_single_line_required(&self.tool_type, "type")?;
        validate_single_line_optional(&self.description, "description")?;
        validate_metadata(&self.metadata_json, "metadata")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolInstance {
    pub id: ToolInstanceId,
    pub tool_id: ToolId,
    pub path: Option<String>,
    pub endpoint: Option<String>,
    pub status: ToolHealthStatus,
    pub last_checked: Option<DateTime<Utc>>,
}

impl ToolInstance {
    pub fn new(
        id: ToolInstanceId,
        tool_id: ToolId,
        path: Option<String>,
        endpoint: Option<String>,
        status: ToolHealthStatus,
        last_checked: Option<DateTime<Utc>>,
    ) -> Result<Self, ToolDomainError> {
        let instance = Self {
            id,
            tool_id,
            path,
            endpoint,
            status,
            last_checked,
        };
        instance.validate()?;
        Ok(instance)
    }

    pub fn validate(&self) -> Result<(), ToolDomainError> {
        validate_optional_location(&self.path, "path")?;
        validate_optional_location(&self.endpoint, "endpoint")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolVersion {
    pub id: ToolVersionId,
    pub tool_id: ToolId,
    pub version: String,
    pub observed_at: DateTime<Utc>,
    pub metadata_json: Value,
}

impl ToolVersion {
    pub fn new(
        id: ToolVersionId,
        tool_id: ToolId,
        version: impl Into<String>,
        observed_at: DateTime<Utc>,
        metadata_json: Value,
    ) -> Result<Self, ToolDomainError> {
        let version = Self {
            id,
            tool_id,
            version: version.into(),
            observed_at,
            metadata_json,
        };
        version.validate()?;
        Ok(version)
    }

    pub fn validate(&self) -> Result<(), ToolDomainError> {
        validate_single_line_required(&self.version, "version")?;
        validate_metadata(&self.metadata_json, "metadata")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Capability {
    pub tool_id: ToolId,
    pub capability_name: String,
    pub metadata_json: Value,
}

impl Capability {
    pub fn new(
        tool_id: ToolId,
        capability_name: impl Into<String>,
        metadata_json: Value,
    ) -> Result<Self, ToolDomainError> {
        let capability = Self {
            tool_id,
            capability_name: capability_name.into(),
            metadata_json,
        };
        capability.validate()?;
        Ok(capability)
    }

    pub fn validate(&self) -> Result<(), ToolDomainError> {
        validate_single_line_required(&self.capability_name, "capability_name")?;
        validate_metadata(&self.metadata_json, "metadata")
    }
}

pub type ToolCapability = Capability;

fn validate_single_line_required(value: &str, field: &str) -> Result<(), ToolDomainError> {
    if value.trim().is_empty() || value.contains(['\r', '\n']) {
        return Err(ToolDomainError::InvalidField(field.to_owned()));
    }
    Ok(())
}

fn validate_single_line_optional(value: &str, field: &str) -> Result<(), ToolDomainError> {
    if value.contains(['\r', '\n']) {
        return Err(ToolDomainError::InvalidField(field.to_owned()));
    }
    Ok(())
}

fn validate_optional_location(value: &Option<String>, field: &str) -> Result<(), ToolDomainError> {
    if let Some(value) = value {
        validate_single_line_required(value, field)?;
    }
    Ok(())
}

fn validate_metadata(value: &Value, field: &str) -> Result<(), ToolDomainError> {
    if value.is_null() {
        return Err(ToolDomainError::InvalidField(format!(
            "{field} must not be null"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolDomainError {
    InvalidId(String),
    InvalidInstanceId(String),
    InvalidVersionId(String),
    InvalidField(String),
}

impl fmt::Display for ToolDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid tool id: {value}"),
            Self::InvalidInstanceId(value) => {
                write!(formatter, "invalid tool instance id: {value}")
            }
            Self::InvalidVersionId(value) => {
                write!(formatter, "invalid tool version id: {value}")
            }
            Self::InvalidField(field) => write!(formatter, "invalid tool field: {field}"),
        }
    }
}

impl Error for ToolDomainError {}

#[cfg(test)]
mod tests {
    use super::{Capability, Tool, ToolHealthStatus, ToolId, ToolInstance, ToolInstanceId};
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    #[test]
    fn tool_entities_validate_metadata_and_health_states() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let tool = Tool::new(
            ToolId::parse("tool_comfy").unwrap(),
            "ComfyUI",
            "image",
            "local runtime",
            json!({"managed": false}),
            now,
        )
        .unwrap();
        let instance = ToolInstance::new(
            ToolInstanceId::parse("tins_comfy").unwrap(),
            tool.id.clone(),
            Some("C:/ComfyUI".to_owned()),
            None,
            ToolHealthStatus::Unknown,
            None,
        )
        .unwrap();
        let capability =
            Capability::new(tool.id, "image_generation", json!({"source": "manual"})).unwrap();

        assert_eq!(instance.status.as_str(), "UNKNOWN");
        assert_eq!(capability.capability_name, "image_generation");
    }

    #[test]
    fn tool_instance_rejects_blank_locations() {
        assert!(ToolInstance::new(
            ToolInstanceId::parse("tins_invalid").unwrap(),
            ToolId::parse("tool_invalid").unwrap(),
            Some("  ".to_owned()),
            None,
            ToolHealthStatus::Unknown,
            None,
        )
        .is_err());
    }
}
