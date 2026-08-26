use super::binding::{BindingRole, InheritanceMode};
use super::profile::ProfileType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConsistencyScopeType {
    Project,
    Series,
    Episode,
    Scene,
}

impl ConsistencyScopeType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "PROJECT",
            Self::Series => "SERIES",
            Self::Episode => "EPISODE",
            Self::Scene => "SCENE",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, ConsistencyScopeTypeError> {
        match value {
            "PROJECT" => Ok(Self::Project),
            "SERIES" => Ok(Self::Series),
            "EPISODE" => Ok(Self::Episode),
            "SCENE" => Ok(Self::Scene),
            other => Err(ConsistencyScopeTypeError::InvalidValue(other.to_owned())),
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, ConsistencyScopeTypeError> {
        Self::try_from_db(value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScopedProfileBinding {
    pub id: String,
    pub project_id: String,
    pub scope_type: ConsistencyScopeType,
    pub scope_id: String,
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
pub struct ScopedReferenceSetBinding {
    pub id: String,
    pub project_id: String,
    pub scope_type: ConsistencyScopeType,
    pub scope_id: String,
    pub role: BindingRole,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: InheritanceMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsistencyScopeTypeError {
    InvalidValue(String),
}

impl fmt::Display for ConsistencyScopeTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(value) => {
                write!(formatter, "invalid consistency scope type: {value}")
            }
        }
    }
}

impl Error for ConsistencyScopeTypeError {}

#[cfg(test)]
mod tests {
    use super::ConsistencyScopeType;

    #[test]
    fn scope_type_uses_stable_uppercase_database_values() {
        for (value, expected) in [
            ("PROJECT", ConsistencyScopeType::Project),
            ("SERIES", ConsistencyScopeType::Series),
            ("EPISODE", ConsistencyScopeType::Episode),
            ("SCENE", ConsistencyScopeType::Scene),
        ] {
            assert_eq!(ConsistencyScopeType::try_from_db(value).unwrap(), expected);
            assert_eq!(expected.as_str(), value);
        }
        assert!(ConsistencyScopeType::try_from_db("SHOT").is_err());
    }
}
