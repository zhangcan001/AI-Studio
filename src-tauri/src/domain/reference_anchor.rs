use crate::domain::{AssetId, ProjectIdValidationError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReferenceAnchorId(String);

impl ReferenceAnchorId {
    pub fn new() -> Self {
        Self(format!("anc_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ReferenceAnchorDomainError> {
        let value = value.into();
        if value.starts_with("anc_") && value.len() > "anc_".len() {
            Ok(Self(value))
        } else {
            Err(ReferenceAnchorDomainError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ReferenceAnchorId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ReferenceAnchorId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferenceAnchorKind {
    Character,
    Scene,
    Prop,
    Style,
}

impl ReferenceAnchorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Character => "CHARACTER",
            Self::Scene => "SCENE",
            Self::Prop => "PROP",
            Self::Style => "STYLE",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, ReferenceAnchorDomainError> {
        match value {
            "CHARACTER" => Ok(Self::Character),
            "SCENE" => Ok(Self::Scene),
            "PROP" => Ok(Self::Prop),
            "STYLE" => Ok(Self::Style),
            other => Err(ReferenceAnchorDomainError::InvalidKind(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceAnchor {
    pub id: ReferenceAnchorId,
    pub project_id: String,
    pub kind: ReferenceAnchorKind,
    pub name: String,
    pub normalized_name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceAnchorAsset {
    pub anchor_id: ReferenceAnchorId,
    pub asset_id: AssetId,
    pub ordinal: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceAnchorDomainError {
    InvalidId(String),
    InvalidKind(String),
}

impl fmt::Display for ReferenceAnchorDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid reference anchor id: {value}"),
            Self::InvalidKind(value) => {
                write!(formatter, "invalid reference anchor kind: {value}")
            }
        }
    }
}

impl Error for ReferenceAnchorDomainError {}

impl From<ProjectIdValidationError> for ReferenceAnchorDomainError {
    fn from(error: ProjectIdValidationError) -> Self {
        Self::InvalidId(error.to_string())
    }
}
