use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};
use std::{error::Error, fmt};
use uuid::Uuid;

/// The identifiers owned by the Script/Draft domain. They must never be
/// confused with IDs from the formal production structure.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ScriptDraftIdKind {
    Source,
    Draft,
    DraftRevision,
    DraftNode,
    Diagnostic,
}

impl ScriptDraftIdKind {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Source => "scr_",
            Self::Draft => "drf_",
            Self::DraftRevision => "drev_",
            Self::DraftNode => "dnode_",
            Self::Diagnostic => "diag_",
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Source => "INVALID_SOURCE_ID",
            Self::Draft => "INVALID_DRAFT_ID",
            Self::DraftRevision => "INVALID_DRAFT_REVISION_ID",
            Self::DraftNode => "INVALID_DRAFT_NODE_ID",
            Self::Diagnostic => "INVALID_DIAGNOSTIC_ID",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptDraftIdError {
    kind: ScriptDraftIdKind,
}

impl ScriptDraftIdError {
    fn new(kind: ScriptDraftIdKind) -> Self {
        Self { kind }
    }

    pub const fn kind(&self) -> ScriptDraftIdKind {
        self.kind
    }

    pub const fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl fmt::Display for ScriptDraftIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: invalid script/draft identifier",
            self.code()
        )
    }
}

impl Error for ScriptDraftIdError {}

macro_rules! script_draft_id {
    ($name:ident, $kind:expr) => {
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $kind.prefix(), Uuid::new_v4()))
            }

            pub fn parse(value: impl AsRef<str>) -> Result<Self, ScriptDraftIdError> {
                let value = value.as_ref();
                let suffix = value
                    .strip_prefix($kind.prefix())
                    .ok_or_else(|| ScriptDraftIdError::new($kind))?;
                if suffix.len() != 36 || Uuid::parse_str(suffix).is_err() {
                    return Err(ScriptDraftIdError::new($kind));
                }
                if Uuid::parse_str(suffix)
                    .expect("validated above")
                    .hyphenated()
                    .to_string()
                    != suffix.to_ascii_lowercase()
                {
                    return Err(ScriptDraftIdError::new($kind));
                }
                Ok(Self(value.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(D::Error::custom)
            }
        }
    };
}

script_draft_id!(SourceId, ScriptDraftIdKind::Source);
script_draft_id!(DraftId, ScriptDraftIdKind::Draft);
script_draft_id!(DraftRevisionId, ScriptDraftIdKind::DraftRevision);
script_draft_id!(DraftNodeId, ScriptDraftIdKind::DraftNode);
script_draft_id!(DiagnosticId, ScriptDraftIdKind::Diagnostic);

pub fn validate_source_id(value: &str) -> Result<(), ScriptDraftIdError> {
    SourceId::parse(value).map(|_| ())
}

pub fn validate_draft_id(value: &str) -> Result<(), ScriptDraftIdError> {
    DraftId::parse(value).map(|_| ())
}

pub fn validate_draft_revision_id(value: &str) -> Result<(), ScriptDraftIdError> {
    DraftRevisionId::parse(value).map(|_| ())
}

pub fn validate_draft_node_id(value: &str) -> Result<(), ScriptDraftIdError> {
    DraftNodeId::parse(value).map(|_| ())
}

pub fn validate_diagnostic_id(value: &str) -> Result<(), ScriptDraftIdError> {
    DiagnosticId::parse(value).map(|_| ())
}
