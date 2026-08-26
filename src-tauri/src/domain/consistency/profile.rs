//! Consistency profile data contracts.
//!
//! Profiles describe reusable semantic entities. They intentionally contain
//! text and relation identifiers only; asset storage, persistence, and
//! resolution belong to later application layers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// The four profile kinds persisted by the consistency asset system.
///
/// `CostumeVariant` is a child of `CharacterProfile`, not a profile kind of
/// its own, so it is deliberately absent from this enum.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProfileType {
    Character,
    Scene,
    Prop,
    Style,
}

impl ProfileType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Character => "CHARACTER",
            Self::Scene => "SCENE",
            Self::Prop => "PROP",
            Self::Style => "STYLE",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, ProfileDomainError> {
        Self::try_from_str(value)
    }

    pub fn try_from_str(value: &str) -> Result<Self, ProfileDomainError> {
        match value {
            "CHARACTER" => Ok(Self::Character),
            "SCENE" => Ok(Self::Scene),
            "PROP" => Ok(Self::Prop),
            "STYLE" => Ok(Self::Style),
            other => Err(ProfileDomainError::InvalidProfileType(other.to_owned())),
        }
    }
}

/// A revision is either the current usable revision or a historical one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProfileRevisionStatus {
    Active,
    Archived,
}

impl ProfileRevisionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Archived => "ARCHIVED",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, ProfileDomainError> {
        Self::try_from_str(value)
    }

    pub fn try_from_str(value: &str) -> Result<Self, ProfileDomainError> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "ARCHIVED" => Ok(Self::Archived),
            other => Err(ProfileDomainError::InvalidRevisionStatus(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterProfile {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: String,
    pub negative_prompt: String,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub active_revision_id: Option<String>,
    pub metadata_json: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CostumeVariant {
    pub id: String,
    pub character_profile_id: String,
    pub name: String,
    pub prompt_fragment: String,
    pub reference_set_id: Option<String>,
    pub is_default: bool,
    pub ordinal: i64,
    pub active_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SceneProfile {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub environment_prompt: String,
    pub lighting_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub active_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PropProfile {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: String,
    pub material_prompt: Option<String>,
    pub scale_prompt: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub active_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StyleProfile {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub style_prompt: String,
    pub color_prompt: Option<String>,
    pub line_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub output_notes: Option<String>,
    pub active_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProfileRevision {
    pub id: String,
    pub profile_type: ProfileType,
    pub profile_id: String,
    pub revision_number: i64,
    pub content_json: String,
    pub content_sha256: String,
    pub status: ProfileRevisionStatus,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

/// A project-scoped profile record. Costume variants remain separate child
/// records so callers cannot accidentally treat a costume as a top-level
/// profile or resolve it without its character owner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConsistencyProfileRecord {
    Character(CharacterProfile),
    Scene(SceneProfile),
    Prop(PropProfile),
    Style(StyleProfile),
}

impl ConsistencyProfileRecord {
    pub const fn profile_type(&self) -> ProfileType {
        match self {
            Self::Character(_) => ProfileType::Character,
            Self::Scene(_) => ProfileType::Scene,
            Self::Prop(_) => ProfileType::Prop,
            Self::Style(_) => ProfileType::Style,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Character(profile) => &profile.id,
            Self::Scene(profile) => &profile.id,
            Self::Prop(profile) => &profile.id,
            Self::Style(profile) => &profile.id,
        }
    }

    pub fn project_id(&self) -> &str {
        match self {
            Self::Character(profile) => &profile.project_id,
            Self::Scene(profile) => &profile.project_id,
            Self::Prop(profile) => &profile.project_id,
            Self::Style(profile) => &profile.project_id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Character(profile) => &profile.name,
            Self::Scene(profile) => &profile.name,
            Self::Prop(profile) => &profile.name,
            Self::Style(profile) => &profile.name,
        }
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            Self::Character(profile) => profile.created_at,
            Self::Scene(profile) => profile.created_at,
            Self::Prop(profile) => profile.created_at,
            Self::Style(profile) => profile.created_at,
        }
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        match self {
            Self::Character(profile) => profile.updated_at,
            Self::Scene(profile) => profile.updated_at,
            Self::Prop(profile) => profile.updated_at,
            Self::Style(profile) => profile.updated_at,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileDomainError {
    InvalidProfileType(String),
    InvalidRevisionStatus(String),
}

impl fmt::Display for ProfileDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfileType(value) => {
                write!(formatter, "invalid profile type: {value}")
            }
            Self::InvalidRevisionStatus(value) => {
                write!(formatter, "invalid profile revision status: {value}")
            }
        }
    }
}

impl Error for ProfileDomainError {}
