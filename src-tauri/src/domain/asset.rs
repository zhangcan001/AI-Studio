use crate::domain::TaskId;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

pub const GENERATED_IMAGE_CATEGORY: &str = "generated_image";
pub const GENERATED_VIDEO_CATEGORY: &str = "generated_video";
pub const SOURCE_IMAGE_CATEGORY: &str = "source_image";
pub const SOURCE_VIDEO_CATEGORY: &str = "source_video";
pub const SOURCE_AUDIO_CATEGORY: &str = "source_audio";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetId(String);

impl AssetId {
    pub fn new() -> Self {
        Self(format!("ast_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, AssetDomainError> {
        let value = value.into();
        if value.starts_with("ast_") && value.len() > "ast_".len() {
            Ok(Self(value))
        } else {
            Err(AssetDomainError::InvalidId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AssetId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetType {
    Image,
    Video,
    Audio,
}

impl AssetType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, AssetDomainError> {
        match value {
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            "audio" => Ok(Self::Audio),
            other => Err(AssetDomainError::InvalidType(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub id: AssetId,
    pub project_id: String,
    pub asset_type: AssetType,
    pub category: String,
    pub name: String,
    pub original_name: String,
    pub storage_path: String,
    pub thumbnail_path: Option<String>,
    pub sha256: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,
    pub file_size: u64,
    pub source_task_id: Option<TaskId>,
    pub metadata_json: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Asset {
    pub fn new_generated_image(
        id: AssetId,
        project_id: impl Into<String>,
        name: impl Into<String>,
        original_name: impl Into<String>,
        storage_path: impl Into<String>,
        sha256: impl Into<String>,
        mime_type: impl Into<String>,
        width: u32,
        height: u32,
        file_size: u64,
        source_task_id: TaskId,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AssetDomainError> {
        let asset = Self {
            id,
            project_id: project_id.into(),
            asset_type: AssetType::Image,
            category: GENERATED_IMAGE_CATEGORY.to_owned(),
            name: name.into(),
            original_name: original_name.into(),
            storage_path: storage_path.into(),
            thumbnail_path: None,
            sha256: sha256.into(),
            mime_type: mime_type.into(),
            width,
            height,
            duration_ms: None,
            file_size,
            source_task_id: Some(source_task_id),
            metadata_json,
            created_at,
            updated_at: created_at,
        };
        asset.validate()?;
        Ok(asset)
    }

    pub fn new_image(
        id: AssetId,
        project_id: impl Into<String>,
        name: impl Into<String>,
        original_name: impl Into<String>,
        storage_path: impl Into<String>,
        sha256: impl Into<String>,
        mime_type: impl Into<String>,
        width: u32,
        height: u32,
        file_size: u64,
        source_task_id: TaskId,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AssetDomainError> {
        Self::new_generated_image(
            id,
            project_id,
            name,
            original_name,
            storage_path,
            sha256,
            mime_type,
            width,
            height,
            file_size,
            source_task_id,
            metadata_json,
            created_at,
        )
    }

    pub fn new_source_image(
        id: AssetId,
        project_id: impl Into<String>,
        name: impl Into<String>,
        original_name: impl Into<String>,
        storage_path: impl Into<String>,
        sha256: impl Into<String>,
        mime_type: impl Into<String>,
        width: u32,
        height: u32,
        file_size: u64,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AssetDomainError> {
        let asset = Self {
            id,
            project_id: project_id.into(),
            asset_type: AssetType::Image,
            category: SOURCE_IMAGE_CATEGORY.to_owned(),
            name: name.into(),
            original_name: original_name.into(),
            storage_path: storage_path.into(),
            thumbnail_path: None,
            sha256: sha256.into(),
            mime_type: mime_type.into(),
            width,
            height,
            duration_ms: None,
            file_size,
            source_task_id: None,
            metadata_json,
            created_at,
            updated_at: created_at,
        };
        asset.validate()?;
        Ok(asset)
    }

    pub fn new_generated_video(
        id: AssetId,
        project_id: impl Into<String>,
        name: impl Into<String>,
        original_name: impl Into<String>,
        storage_path: impl Into<String>,
        sha256: impl Into<String>,
        mime_type: impl Into<String>,
        width: Option<u32>,
        height: Option<u32>,
        duration_ms: Option<u64>,
        file_size: u64,
        source_task_id: TaskId,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AssetDomainError> {
        let asset = Self {
            id,
            project_id: project_id.into(),
            asset_type: AssetType::Video,
            category: GENERATED_VIDEO_CATEGORY.to_owned(),
            name: name.into(),
            original_name: original_name.into(),
            storage_path: storage_path.into(),
            thumbnail_path: None,
            sha256: sha256.into(),
            mime_type: mime_type.into(),
            width: width.unwrap_or_default(),
            height: height.unwrap_or_default(),
            duration_ms,
            file_size,
            source_task_id: Some(source_task_id),
            metadata_json,
            created_at,
            updated_at: created_at,
        };
        asset.validate()?;
        Ok(asset)
    }

    pub fn new_source_video(
        id: AssetId,
        project_id: impl Into<String>,
        name: impl Into<String>,
        original_name: impl Into<String>,
        storage_path: impl Into<String>,
        sha256: impl Into<String>,
        mime_type: impl Into<String>,
        width: Option<u32>,
        height: Option<u32>,
        duration_ms: Option<u64>,
        file_size: u64,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AssetDomainError> {
        let asset = Self {
            id,
            project_id: project_id.into(),
            asset_type: AssetType::Video,
            category: SOURCE_VIDEO_CATEGORY.to_owned(),
            name: name.into(),
            original_name: original_name.into(),
            storage_path: storage_path.into(),
            thumbnail_path: None,
            sha256: sha256.into(),
            mime_type: mime_type.into(),
            width: width.unwrap_or_default(),
            height: height.unwrap_or_default(),
            duration_ms,
            file_size,
            source_task_id: None,
            metadata_json,
            created_at,
            updated_at: created_at,
        };
        asset.validate()?;
        Ok(asset)
    }

    pub fn new_source_audio(
        id: AssetId,
        project_id: impl Into<String>,
        name: impl Into<String>,
        original_name: impl Into<String>,
        storage_path: impl Into<String>,
        sha256: impl Into<String>,
        mime_type: impl Into<String>,
        duration_ms: Option<u64>,
        file_size: u64,
        metadata_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AssetDomainError> {
        let asset = Self {
            id,
            project_id: project_id.into(),
            asset_type: AssetType::Audio,
            category: SOURCE_AUDIO_CATEGORY.to_owned(),
            name: name.into(),
            original_name: original_name.into(),
            storage_path: storage_path.into(),
            thumbnail_path: None,
            sha256: sha256.into(),
            mime_type: mime_type.into(),
            width: 0,
            height: 0,
            duration_ms,
            file_size,
            source_task_id: None,
            metadata_json,
            created_at,
            updated_at: created_at,
        };
        asset.validate()?;
        Ok(asset)
    }

    pub fn validate(&self) -> Result<(), AssetDomainError> {
        for (field, value) in [
            ("project_id", self.project_id.as_str()),
            ("category", self.category.as_str()),
            ("name", self.name.as_str()),
            ("original_name", self.original_name.as_str()),
            ("storage_path", self.storage_path.as_str()),
            ("sha256", self.sha256.as_str()),
            ("mime_type", self.mime_type.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(AssetDomainError::InvalidField(field.to_owned()));
            }
        }
        if !matches!(
            self.category.as_str(),
            GENERATED_IMAGE_CATEGORY
                | GENERATED_VIDEO_CATEGORY
                | SOURCE_IMAGE_CATEGORY
                | SOURCE_VIDEO_CATEGORY
                | SOURCE_AUDIO_CATEGORY
        ) {
            return Err(AssetDomainError::InvalidField(
                "category must be a supported generated or source media category".to_owned(),
            ));
        }
        if matches!(
            self.category.as_str(),
            GENERATED_IMAGE_CATEGORY | GENERATED_VIDEO_CATEGORY
        ) && self.source_task_id.is_none()
        {
            return Err(AssetDomainError::InvalidField(
                "generated assets must have a source task".to_owned(),
            ));
        }
        if matches!(
            self.category.as_str(),
            SOURCE_IMAGE_CATEGORY | SOURCE_VIDEO_CATEGORY | SOURCE_AUDIO_CATEGORY
        ) && self.source_task_id.is_some()
        {
            return Err(AssetDomainError::InvalidField(
                "source assets must not have a source task".to_owned(),
            ));
        }
        if self.category == GENERATED_VIDEO_CATEGORY && self.asset_type != AssetType::Video {
            return Err(AssetDomainError::InvalidField(
                "generated_video must have video asset_type".to_owned(),
            ));
        }
        if matches!(
            self.category.as_str(),
            GENERATED_IMAGE_CATEGORY | SOURCE_IMAGE_CATEGORY
        ) && self.asset_type != AssetType::Image
        {
            return Err(AssetDomainError::InvalidField(
                "image categories must have image asset_type".to_owned(),
            ));
        }
        if self.category == SOURCE_VIDEO_CATEGORY && self.asset_type != AssetType::Video {
            return Err(AssetDomainError::InvalidField(
                "source_video must have video asset_type".to_owned(),
            ));
        }
        if self.category == SOURCE_AUDIO_CATEGORY && self.asset_type != AssetType::Audio {
            return Err(AssetDomainError::InvalidField(
                "source_audio must have audio asset_type".to_owned(),
            ));
        }
        if self.asset_type == AssetType::Image
            && (self.width == 0 || self.height == 0 || self.file_size == 0)
        {
            return Err(AssetDomainError::InvalidField(
                "image dimensions and file_size must be positive".to_owned(),
            ));
        }
        if matches!(self.asset_type, AssetType::Video | AssetType::Audio) && self.file_size == 0 {
            return Err(AssetDomainError::InvalidField(
                "media file_size must be positive".to_owned(),
            ));
        }
        if self.updated_at < self.created_at {
            return Err(AssetDomainError::InvalidField(
                "updated_at must not precede created_at".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetVersionId(String);

impl AssetVersionId {
    pub fn new() -> Self {
        Self(format!("av_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, AssetDomainError> {
        let value = value.into();
        if value.starts_with("av_") && value.len() > "av_".len() {
            Ok(Self(value))
        } else {
            Err(AssetDomainError::InvalidVersionId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AssetVersionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssetVersion {
    pub id: AssetVersionId,
    pub project_id: String,
    pub asset_id: AssetId,
    pub version_number: u32,
    pub metadata_snapshot: Value,
    pub location: String,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
}

impl AssetVersion {
    pub fn new(
        id: AssetVersionId,
        project_id: impl Into<String>,
        asset_id: AssetId,
        version_number: u32,
        metadata_snapshot: Value,
        location: impl Into<String>,
        checksum: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AssetDomainError> {
        let version = Self {
            id,
            project_id: project_id.into(),
            asset_id,
            version_number,
            metadata_snapshot,
            location: location.into(),
            checksum: checksum.into(),
            created_at,
        };
        version.validate()?;
        Ok(version)
    }

    pub fn validate(&self) -> Result<(), AssetDomainError> {
        for (field, value) in [
            ("project_id", self.project_id.as_str()),
            ("location", self.location.as_str()),
            ("checksum", self.checksum.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(AssetDomainError::InvalidField(field.to_owned()));
            }
        }
        if self.version_number == 0 {
            return Err(AssetDomainError::InvalidField(
                "version_number must be positive".to_owned(),
            ));
        }
        if self.metadata_snapshot.is_null() {
            return Err(AssetDomainError::InvalidField(
                "metadata_snapshot must not be null".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetRelationType {
    SourceOf,
    DerivedFrom,
    VariantOf,
    Reference,
    Replacement,
    Related,
}

impl AssetRelationType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceOf => "SOURCE_OF",
            Self::DerivedFrom => "DERIVED_FROM",
            Self::VariantOf => "VARIANT_OF",
            Self::Reference => "REFERENCE",
            Self::Replacement => "REPLACEMENT",
            Self::Related => "RELATED",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, AssetDomainError> {
        match value {
            "SOURCE_OF" => Ok(Self::SourceOf),
            "DERIVED_FROM" => Ok(Self::DerivedFrom),
            "VARIANT_OF" => Ok(Self::VariantOf),
            "REFERENCE" => Ok(Self::Reference),
            "REPLACEMENT" => Ok(Self::Replacement),
            "RELATED" => Ok(Self::Related),
            other => Err(AssetDomainError::InvalidRelationType(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetRelationId(String);

impl AssetRelationId {
    pub fn new() -> Self {
        Self(format!("rel_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, AssetDomainError> {
        let value = value.into();
        if value.starts_with("rel_") && value.len() > "rel_".len() {
            Ok(Self(value))
        } else {
            Err(AssetDomainError::InvalidRelationId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AssetRelationId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetRelation {
    pub id: AssetRelationId,
    pub project_id: String,
    pub source_asset_id: AssetId,
    pub target_asset_id: AssetId,
    pub relation_type: AssetRelationType,
    pub created_at: DateTime<Utc>,
}

impl AssetRelation {
    pub fn new(
        id: AssetRelationId,
        project_id: impl Into<String>,
        source_asset_id: AssetId,
        target_asset_id: AssetId,
        relation_type: AssetRelationType,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AssetDomainError> {
        let relation = Self {
            id,
            project_id: project_id.into(),
            source_asset_id,
            target_asset_id,
            relation_type,
            created_at,
        };
        relation.validate()?;
        Ok(relation)
    }

    pub fn validate(&self) -> Result<(), AssetDomainError> {
        if self.project_id.trim().is_empty() {
            return Err(AssetDomainError::InvalidField("project_id".to_owned()));
        }
        if self.source_asset_id == self.target_asset_id {
            return Err(AssetDomainError::InvalidField(
                "source and target assets must differ".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetDomainError {
    InvalidId(String),
    InvalidVersionId(String),
    InvalidRelationId(String),
    InvalidType(String),
    InvalidRelationType(String),
    InvalidField(String),
}

impl fmt::Display for AssetDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid asset id: {value}"),
            Self::InvalidVersionId(value) => write!(formatter, "invalid asset version id: {value}"),
            Self::InvalidRelationId(value) => {
                write!(formatter, "invalid asset relation id: {value}")
            }
            Self::InvalidType(value) => write!(formatter, "invalid asset type: {value}"),
            Self::InvalidRelationType(value) => {
                write!(formatter, "invalid asset relation type: {value}")
            }
            Self::InvalidField(message) => write!(formatter, "invalid asset: {message}"),
        }
    }
}

impl Error for AssetDomainError {}
