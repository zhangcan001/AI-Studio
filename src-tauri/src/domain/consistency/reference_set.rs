use crate::domain::consistency::profile::ProfileType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferenceSetPurpose {
    Character,
    Costume,
    Scene,
    Prop,
    Style,
    Shot,
}

impl ReferenceSetPurpose {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Character => "CHARACTER",
            Self::Costume => "COSTUME",
            Self::Scene => "SCENE",
            Self::Prop => "PROP",
            Self::Style => "STYLE",
            Self::Shot => "SHOT",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, ReferenceSetPurposeError> {
        match value {
            "CHARACTER" => Ok(Self::Character),
            "COSTUME" => Ok(Self::Costume),
            "SCENE" => Ok(Self::Scene),
            "PROP" => Ok(Self::Prop),
            "STYLE" => Ok(Self::Style),
            "SHOT" => Ok(Self::Shot),
            other => Err(ReferenceSetPurposeError::InvalidValue(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReferenceSet {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub purpose: ReferenceSetPurpose,
    pub description: String,
    pub owner_profile_type: Option<ProfileType>,
    pub owner_profile_id: Option<String>,
    pub active_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReferenceSetItem {
    pub reference_set_id: String,
    pub asset_id: String,
    pub ordinal: i64,
    pub role: Option<String>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceSetPurposeError {
    InvalidValue(String),
}

impl fmt::Display for ReferenceSetPurposeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(value) => {
                write!(formatter, "invalid reference set purpose: {value}")
            }
        }
    }
}

impl Error for ReferenceSetPurposeError {}
