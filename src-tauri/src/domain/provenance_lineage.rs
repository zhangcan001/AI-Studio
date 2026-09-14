use crate::domain::{AssetVersionId, TaskId, ToolInstanceId, ToolVersionId};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

/// An explicit observation that an existing Task (the generation authority)
/// used a registered local tool instance.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GenerationToolUsageId(String);

impl GenerationToolUsageId {
    pub fn new() -> Self {
        Self(format!("gtu_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ProvenanceLineageDomainError> {
        let value = value.into();
        if value.starts_with("gtu_") && value.len() > "gtu_".len() {
            Ok(Self(value))
        } else {
            Err(ProvenanceLineageDomainError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for GenerationToolUsageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GenerationToolUsageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationToolUsage {
    pub id: GenerationToolUsageId,
    /// This is the existing Task identity. There is intentionally no new
    /// Generation entity in the provenance layer.
    pub generation_id: TaskId,
    pub tool_instance_id: ToolInstanceId,
    pub tool_version_id: Option<ToolVersionId>,
    pub metadata_json: Value,
    pub created_at: DateTime<Utc>,
}

impl GenerationToolUsage {
    pub fn new(
        id: GenerationToolUsageId,
        generation_id: TaskId,
        tool_instance_id: ToolInstanceId,
        tool_version_id: Option<ToolVersionId>,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, ProvenanceLineageDomainError> {
        let usage = Self {
            id,
            generation_id,
            tool_instance_id,
            tool_version_id,
            metadata_json,
            created_at,
        };
        usage.validate()?;
        Ok(usage)
    }

    pub fn validate(&self) -> Result<(), ProvenanceLineageDomainError> {
        if self.metadata_json.is_null() {
            return Err(ProvenanceLineageDomainError::InvalidField(
                "metadata must not be null".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GenerationAssetVersionId(String);

impl GenerationAssetVersionId {
    pub fn new() -> Self {
        Self(format!("gav_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ProvenanceLineageDomainError> {
        let value = value.into();
        if value.starts_with("gav_") && value.len() > "gav_".len() {
            Ok(Self(value))
        } else {
            Err(ProvenanceLineageDomainError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for GenerationAssetVersionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GenerationAssetVersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationAssetVersionRelationType {
    Output,
    Derived,
}

impl GenerationAssetVersionRelationType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Output => "OUTPUT",
            Self::Derived => "DERIVED",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, ProvenanceLineageDomainError> {
        match value {
            "OUTPUT" => Ok(Self::Output),
            "DERIVED" => Ok(Self::Derived),
            other => Err(ProvenanceLineageDomainError::InvalidRelationType(
                other.to_owned(),
            )),
        }
    }

    pub fn try_from_input(value: &str) -> Result<Self, ProvenanceLineageDomainError> {
        let value = value.trim().to_ascii_uppercase();
        Self::try_from_db(&value)
    }
}

impl fmt::Display for GenerationAssetVersionRelationType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationAssetVersion {
    pub id: GenerationAssetVersionId,
    /// This is the existing Task identity. The output key below keeps the
    /// relation attached to the existing Result authority.
    pub generation_id: TaskId,
    pub output_id: String,
    pub ordinal: u32,
    pub asset_version_id: AssetVersionId,
    pub relation_type: GenerationAssetVersionRelationType,
    pub created_at: DateTime<Utc>,
}

impl GenerationAssetVersion {
    pub fn new(
        id: GenerationAssetVersionId,
        generation_id: TaskId,
        output_id: impl Into<String>,
        ordinal: u32,
        asset_version_id: AssetVersionId,
        relation_type: GenerationAssetVersionRelationType,
        created_at: DateTime<Utc>,
    ) -> Result<Self, ProvenanceLineageDomainError> {
        let link = Self {
            id,
            generation_id,
            output_id: output_id.into(),
            ordinal,
            asset_version_id,
            relation_type,
            created_at,
        };
        link.validate()?;
        Ok(link)
    }

    pub fn validate(&self) -> Result<(), ProvenanceLineageDomainError> {
        if self.output_id.trim().is_empty() || self.output_id.contains(['\r', '\n']) {
            return Err(ProvenanceLineageDomainError::InvalidField(
                "output_id must be a non-empty single-line value".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProvenanceLineageDomainError {
    InvalidId(String),
    InvalidRelationType(String),
    InvalidField(String),
}

impl fmt::Display for ProvenanceLineageDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid provenance lineage id: {value}"),
            Self::InvalidRelationType(value) => {
                write!(
                    formatter,
                    "invalid generation asset version relation type: {value}"
                )
            }
            Self::InvalidField(message) => {
                write!(formatter, "invalid provenance lineage: {message}")
            }
        }
    }
}

impl Error for ProvenanceLineageDomainError {}

#[cfg(test)]
mod tests {
    use super::{
        GenerationAssetVersion, GenerationAssetVersionId, GenerationAssetVersionRelationType,
        GenerationToolUsage, GenerationToolUsageId,
    };
    use crate::domain::{AssetVersionId, TaskId, ToolInstanceId, ToolVersionId};
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    fn now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    #[test]
    fn usage_requires_metadata_and_keeps_existing_task_identity() {
        let usage = GenerationToolUsage::new(
            GenerationToolUsageId::parse("gtu_test").unwrap(),
            TaskId::parse("tsk_test").unwrap(),
            ToolInstanceId::parse("tins_test").unwrap(),
            Some(ToolVersionId::parse("tver_test").unwrap()),
            json!({"source": "explicit"}),
            now(),
        )
        .unwrap();
        assert_eq!(usage.generation_id.as_str(), "tsk_test");
        assert!(GenerationToolUsage::new(
            GenerationToolUsageId::parse("gtu_invalid").unwrap(),
            TaskId::parse("tsk_test").unwrap(),
            ToolInstanceId::parse("tins_test").unwrap(),
            None,
            serde_json::Value::Null,
            now(),
        )
        .is_err());
    }

    #[test]
    fn asset_version_link_accepts_only_typed_relation_values() {
        let link = GenerationAssetVersion::new(
            GenerationAssetVersionId::parse("gav_test").unwrap(),
            TaskId::parse("tsk_test").unwrap(),
            "output_0",
            0,
            AssetVersionId::parse("av_test").unwrap(),
            GenerationAssetVersionRelationType::Output,
            now(),
        )
        .unwrap();
        assert_eq!(link.relation_type.as_str(), "OUTPUT");
        assert_eq!(
            GenerationAssetVersionRelationType::try_from_input(" derived ").unwrap(),
            GenerationAssetVersionRelationType::Derived
        );
        assert!(GenerationAssetVersionRelationType::try_from_input("unknown").is_err());
    }
}
