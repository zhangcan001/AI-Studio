use crate::domain::TaskId;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

pub const GENERATED_IMAGE_CATEGORY: &str = "generated_image";
pub const SOURCE_IMAGE_CATEGORY: &str = "source_image";

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
}

impl AssetType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, AssetDomainError> {
        match value {
            "image" => Ok(Self::Image),
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
            GENERATED_IMAGE_CATEGORY | SOURCE_IMAGE_CATEGORY
        ) {
            return Err(AssetDomainError::InvalidField(
                "category must be generated_image or source_image".to_owned(),
            ));
        }
        if self.category == GENERATED_IMAGE_CATEGORY && self.source_task_id.is_none() {
            return Err(AssetDomainError::InvalidField(
                "generated_image must have a source task".to_owned(),
            ));
        }
        if self.category == SOURCE_IMAGE_CATEGORY && self.source_task_id.is_some() {
            return Err(AssetDomainError::InvalidField(
                "source_image must not have a source task".to_owned(),
            ));
        }
        if self.width == 0 || self.height == 0 || self.file_size == 0 {
            return Err(AssetDomainError::InvalidField(
                "image dimensions and file_size must be positive".to_owned(),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetDomainError {
    InvalidId(String),
    InvalidType(String),
    InvalidField(String),
}

impl fmt::Display for AssetDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid asset id: {value}"),
            Self::InvalidType(value) => write!(formatter, "invalid asset type: {value}"),
            Self::InvalidField(message) => write!(formatter, "invalid asset: {message}"),
        }
    }
}

impl Error for AssetDomainError {}
