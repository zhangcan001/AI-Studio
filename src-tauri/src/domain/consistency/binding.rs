use crate::domain::consistency::profile::ProfileType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BindingRole {
    Character,
    Scene,
    Prop,
    Style,
    ShotReference,
}

impl BindingRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Character => "CHARACTER",
            Self::Scene => "SCENE",
            Self::Prop => "PROP",
            Self::Style => "STYLE",
            Self::ShotReference => "SHOT_REFERENCE",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, BindingDomainError> {
        match value {
            "CHARACTER" => Ok(Self::Character),
            "SCENE" => Ok(Self::Scene),
            "PROP" => Ok(Self::Prop),
            "STYLE" => Ok(Self::Style),
            "SHOT_REFERENCE" => Ok(Self::ShotReference),
            other => Err(BindingDomainError::InvalidRole(other.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InheritanceMode {
    Explicit,
    Inherited,
    Replace,
    Remove,
}

impl InheritanceMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "EXPLICIT",
            Self::Inherited => "INHERITED",
            Self::Replace => "REPLACE",
            Self::Remove => "REMOVE",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, BindingDomainError> {
        match value {
            "EXPLICIT" => Ok(Self::Explicit),
            "INHERITED" => Ok(Self::Inherited),
            "REPLACE" => Ok(Self::Replace),
            "REMOVE" => Ok(Self::Remove),
            other => Err(BindingDomainError::InvalidInheritanceMode(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShotProfileBinding {
    pub id: String,
    pub shot_id: String,
    pub role: BindingRole,
    pub profile_type: ProfileType,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: InheritanceMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShotReferenceSetBinding {
    pub id: String,
    pub shot_id: String,
    pub role: BindingRole,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: InheritanceMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingDomainError {
    InvalidRole(String),
    InvalidInheritanceMode(String),
}

impl fmt::Display for BindingDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRole(value) => write!(formatter, "invalid binding role: {value}"),
            Self::InvalidInheritanceMode(value) => {
                write!(formatter, "invalid inheritance mode: {value}")
            }
        }
    }
}

impl Error for BindingDomainError {}
