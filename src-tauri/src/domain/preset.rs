use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PresetId(String);

impl PresetId {
    pub fn new() -> Self {
        Self(format!("pst_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, PresetDomainError> {
        let value = value.into();
        if value.starts_with("pst_") && value.len() > "pst_".len() {
            Ok(Self(value))
        } else {
            Err(PresetDomainError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for PresetId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PresetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub id: PresetId,
    pub project_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub name: String,
    pub values_json: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Preset {
    pub fn new(
        id: PresetId,
        project_id: impl Into<String>,
        workflow_version_id: impl Into<String>,
        recipe_id: impl Into<String>,
        name: impl Into<String>,
        values_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, PresetDomainError> {
        let preset = Self {
            id,
            project_id: project_id.into(),
            workflow_version_id: workflow_version_id.into(),
            recipe_id: recipe_id.into(),
            name: name.into(),
            values_json,
            created_at,
            updated_at: created_at,
        };
        preset.validate()?;
        Ok(preset)
    }

    pub fn validate(&self) -> Result<(), PresetDomainError> {
        for (field, value) in [
            ("project_id", self.project_id.as_str()),
            ("workflow_version_id", self.workflow_version_id.as_str()),
            ("recipe_id", self.recipe_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(PresetDomainError::InvalidField(field.to_owned()));
            }
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err(PresetDomainError::NameRequired);
        }
        if name.chars().count() > 80 {
            return Err(PresetDomainError::NameTooLong);
        }
        if !self.values_json.is_object() {
            return Err(PresetDomainError::InvalidField(
                "values_json must be an object".to_owned(),
            ));
        }
        if self.updated_at < self.created_at {
            return Err(PresetDomainError::InvalidField(
                "updated_at must not precede created_at".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresetDomainError {
    InvalidId(String),
    NameRequired,
    NameTooLong,
    InvalidField(String),
}

impl fmt::Display for PresetDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid preset id: {value}"),
            Self::NameRequired => {
                formatter.write_str("PRESET_NAME_REQUIRED: preset name is required")
            }
            Self::NameTooLong => formatter
                .write_str("PRESET_NAME_TOO_LONG: preset name must be 80 characters or fewer"),
            Self::InvalidField(message) => write!(formatter, "invalid preset: {message}"),
        }
    }
}

impl Error for PresetDomainError {}
