use std::{error::Error, fmt};
use uuid::Uuid;

/// Stable prefixes for the consistency entities introduced in AI Studio 0.7.0.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConsistencyIdKind {
    CharacterProfile,
    SceneProfile,
    PropProfile,
    StyleProfile,
    CostumeVariant,
    ReferenceSet,
    ProfileRevision,
    ShotProfileBinding,
    ShotReferenceSetBinding,
}

impl ConsistencyIdKind {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::CharacterProfile => "cp_",
            Self::SceneProfile => "scp_",
            Self::PropProfile => "pp_",
            Self::StyleProfile => "stp_",
            Self::CostumeVariant => "cv_",
            Self::ReferenceSet => "rs_",
            Self::ProfileRevision => "prv_",
            Self::ShotProfileBinding => "spb_",
            Self::ShotReferenceSetBinding => "srb_",
        }
    }

    pub const fn entity_name(self) -> &'static str {
        match self {
            Self::CharacterProfile => "character profile",
            Self::SceneProfile => "scene profile",
            Self::PropProfile => "prop profile",
            Self::StyleProfile => "style profile",
            Self::CostumeVariant => "costume variant",
            Self::ReferenceSet => "reference set",
            Self::ProfileRevision => "profile revision",
            Self::ShotProfileBinding => "shot profile binding",
            Self::ShotReferenceSetBinding => "shot reference-set binding",
        }
    }
}

/// Generates a stable, application-owned consistency identifier.
pub fn generate_consistency_id(kind: ConsistencyIdKind) -> String {
    format!("{}{}", kind.prefix(), Uuid::new_v4())
}

/// Validates the exact `<entity prefix><hyphenated UUID>` contract.
pub fn validate_consistency_id(
    kind: ConsistencyIdKind,
    value: &str,
) -> Result<(), ConsistencyIdValidationError> {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return Err(ConsistencyIdValidationError::new(
            kind,
            value,
            "the value must not be empty or contain whitespace",
        ));
    }

    let Some(uuid_text) = value.strip_prefix(kind.prefix()) else {
        return Err(ConsistencyIdValidationError::new(
            kind,
            value,
            "the value has the wrong entity prefix",
        ));
    };

    if uuid_text.len() != 36 {
        return Err(ConsistencyIdValidationError::new(
            kind,
            value,
            "the UUID must use the 36-character hyphenated form",
        ));
    }

    let Ok(uuid) = Uuid::parse_str(uuid_text) else {
        return Err(ConsistencyIdValidationError::new(
            kind,
            value,
            "the suffix must be a UUID",
        ));
    };

    if !uuid
        .hyphenated()
        .to_string()
        .eq_ignore_ascii_case(uuid_text)
    {
        return Err(ConsistencyIdValidationError::new(
            kind,
            value,
            "the UUID must use hyphens in canonical positions",
        ));
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsistencyIdValidationError {
    kind: ConsistencyIdKind,
    value: String,
    reason: &'static str,
}

impl ConsistencyIdValidationError {
    fn new(kind: ConsistencyIdKind, value: &str, reason: &'static str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
            reason,
        }
    }

    pub fn kind(&self) -> ConsistencyIdKind {
        self.kind
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn reason(&self) -> &str {
        self.reason
    }
}

impl fmt::Display for ConsistencyIdValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "INVALID_CONSISTENCY_ID: {} {:?} {}",
            self.kind.entity_name(),
            self.value,
            self.reason
        )
    }
}

impl Error for ConsistencyIdValidationError {}
