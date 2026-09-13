use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelId(String);

impl ModelId {
    pub fn new() -> Self {
        Self(format!("mdl_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ModelDomainError> {
        let value = value.into();
        if value.starts_with("mdl_") && value.len() > "mdl_".len() {
            Ok(Self(value))
        } else {
            Err(ModelDomainError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ModelId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelVersionId(String);

impl ModelVersionId {
    pub fn new() -> Self {
        Self(format!("mdv_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ModelDomainError> {
        let value = value.into();
        if value.starts_with("mdv_") && value.len() > "mdv_".len() {
            Ok(Self(value))
        } else {
            Err(ModelDomainError::InvalidVersionId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ModelVersionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ModelVersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub id: ModelId,
    pub name: String,
    pub provider: String,
    pub model_type: String,
    pub description: String,
    pub metadata_json: Value,
    pub created_at: DateTime<Utc>,
}

impl Model {
    pub fn new(
        id: ModelId,
        name: impl Into<String>,
        provider: impl Into<String>,
        model_type: impl Into<String>,
        description: impl Into<String>,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, ModelDomainError> {
        let model = Self {
            id,
            name: name.into(),
            provider: provider.into(),
            model_type: model_type.into(),
            description: description.into(),
            metadata_json,
            created_at,
        };
        model.validate()?;
        Ok(model)
    }

    pub fn validate(&self) -> Result<(), ModelDomainError> {
        for (field, value) in [
            ("name", self.name.as_str()),
            ("provider", self.provider.as_str()),
            ("type", self.model_type.as_str()),
        ] {
            if value.trim().is_empty() || value.contains(['\r', '\n']) {
                return Err(ModelDomainError::InvalidField(field.to_owned()));
            }
        }
        if self.metadata_json.is_null() {
            return Err(ModelDomainError::InvalidField(
                "metadata must not be null".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelVersion {
    pub id: ModelVersionId,
    pub model_id: ModelId,
    pub version: String,
    pub capabilities_json: Value,
    pub parameter_schema_json: Value,
    pub created_at: DateTime<Utc>,
}

impl ModelVersion {
    pub fn new(
        id: ModelVersionId,
        model_id: ModelId,
        version: impl Into<String>,
        capabilities_json: Value,
        parameter_schema_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, ModelDomainError> {
        let version = Self {
            id,
            model_id,
            version: version.into(),
            capabilities_json,
            parameter_schema_json,
            created_at,
        };
        version.validate()?;
        Ok(version)
    }

    pub fn validate(&self) -> Result<(), ModelDomainError> {
        if self.version.trim().is_empty() || self.version.contains(['\r', '\n']) {
            return Err(ModelDomainError::InvalidField(
                "version must be a non-empty single-line value".to_owned(),
            ));
        }
        if self.capabilities_json.is_null() {
            return Err(ModelDomainError::InvalidField(
                "capabilities must not be null".to_owned(),
            ));
        }
        if self.parameter_schema_json.is_null() {
            return Err(ModelDomainError::InvalidField(
                "parameter_schema must not be null".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelDomainError {
    InvalidId(String),
    InvalidVersionId(String),
    InvalidField(String),
}

impl fmt::Display for ModelDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid model id: {value}"),
            Self::InvalidVersionId(value) => {
                write!(formatter, "invalid model version id: {value}")
            }
            Self::InvalidField(message) => write!(formatter, "invalid model: {message}"),
        }
    }
}

impl Error for ModelDomainError {}

#[cfg(test)]
mod tests {
    use super::{Model, ModelId, ModelVersion, ModelVersionId};
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    #[test]
    fn model_and_version_validate_external_metadata() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let model = Model::new(
            ModelId::parse("mdl_h3").unwrap(),
            "H3",
            "MiniMax",
            "video",
            "local reference",
            json!({"tool": "H3"}),
            now,
        )
        .unwrap();
        let version = ModelVersion::new(
            ModelVersionId::parse("mdv_h3_1").unwrap(),
            model.id,
            "2026-01",
            json!(["text_to_video"]),
            json!({"fps": {"type": "integer"}}),
            now,
        )
        .unwrap();
        assert_eq!(version.version, "2026-01");
    }

    #[test]
    fn model_rejects_empty_identity_or_null_metadata() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        assert!(Model::new(
            ModelId::parse("mdl_invalid").unwrap(),
            "",
            "provider",
            "image",
            "",
            json!({}),
            now,
        )
        .is_err());
        assert!(Model::new(
            ModelId::parse("mdl_invalid").unwrap(),
            "name",
            "provider",
            "image",
            "",
            serde_json::Value::Null,
            now,
        )
        .is_err());
    }
}
