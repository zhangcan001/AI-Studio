use crate::application::ports::{
    AssetRepository, AssetVideoPromptRecord, AssetVideoPromptRepository, Clock, RepositoryError,
};
use crate::domain::{validate_project_id, AssetId, AssetType};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{error::Error, fmt, sync::Arc};

pub const MAX_ASSET_VIDEO_PROMPT_BYTES: usize = 64 * 1024;
const MAX_BATCH_ASSET_IDS: usize = 100;

pub struct AssetVideoPromptService {
    repository: Arc<dyn AssetVideoPromptRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl AssetVideoPromptService {
    pub fn new(
        repository: Arc<dyn AssetVideoPromptRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            asset_repository,
            clock,
        }
    }

    pub async fn get(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<Option<AssetVideoPromptView>, AssetVideoPromptError> {
        validate_project_id(project_id)
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        Ok(self
            .repository
            .find(project_id, asset_id.as_str())
            .await?
            .map(AssetVideoPromptView::from))
    }

    pub async fn list(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<Vec<AssetVideoPromptView>, AssetVideoPromptError> {
        validate_project_id(project_id)
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        if asset_ids.len() > MAX_BATCH_ASSET_IDS {
            return Err(AssetVideoPromptError::InvalidInput(format!(
                "最多同时读取 {MAX_BATCH_ASSET_IDS} 个素材的视频提示词"
            )));
        }
        for asset_id in asset_ids {
            AssetId::parse(asset_id.clone())
                .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        }
        Ok(self
            .repository
            .list(project_id, asset_ids)
            .await?
            .into_iter()
            .map(AssetVideoPromptView::from)
            .collect())
    }

    pub async fn set(
        &self,
        project_id: &str,
        asset_id: &str,
        prompt_text: &str,
    ) -> Result<AssetVideoPromptView, AssetVideoPromptError> {
        validate_project_id(project_id)
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetVideoPromptError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id {
            return Err(AssetVideoPromptError::NotFound(
                asset_id.as_str().to_owned(),
            ));
        }
        if asset.asset_type != AssetType::Image {
            return Err(AssetVideoPromptError::InvalidInput(
                "只有图片素材可以配置视频提示词".to_owned(),
            ));
        }
        let prompt_text = prompt_text.trim().to_owned();
        if prompt_text.is_empty() {
            return Err(AssetVideoPromptError::InvalidInput(
                "视频提示词不能为空".to_owned(),
            ));
        }
        if prompt_text.len() > MAX_ASSET_VIDEO_PROMPT_BYTES {
            return Err(AssetVideoPromptError::InvalidInput(
                "视频提示词不能超过 64 KiB".to_owned(),
            ));
        }
        let record = AssetVideoPromptRecord {
            asset_id: asset_id.as_str().to_owned(),
            project_id: project_id.to_owned(),
            prompt_text,
            updated_at: self.clock.now(),
        };
        self.repository.upsert(&record).await?;
        Ok(record.into())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetVideoPromptView {
    pub asset_id: String,
    pub project_id: String,
    pub prompt_text: String,
    pub updated_at: DateTime<Utc>,
}

impl From<AssetVideoPromptRecord> for AssetVideoPromptView {
    fn from(record: AssetVideoPromptRecord) -> Self {
        Self {
            asset_id: record.asset_id,
            project_id: record.project_id,
            prompt_text: record.prompt_text,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug)]
pub enum AssetVideoPromptError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for AssetVideoPromptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(formatter, "ASSET_VIDEO_PROMPT_INVALID: {message}")
            }
            Self::NotFound(asset_id) => {
                write!(formatter, "ASSET_NOT_FOUND: asset {asset_id} was not found")
            }
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for AssetVideoPromptError {}

impl From<RepositoryError> for AssetVideoPromptError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}
