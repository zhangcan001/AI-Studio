use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::RepositoryError;
use crate::domain::Asset;
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetCategoryFilter {
    All,
    SourceImage,
    SourceVideo,
    SourceAudio,
    GeneratedImage,
    GeneratedVideo,
}

impl AssetCategoryFilter {
    pub fn category(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::SourceImage => Some("source_image"),
            Self::SourceVideo => Some("source_video"),
            Self::SourceAudio => Some("source_audio"),
            Self::GeneratedImage => Some("generated_image"),
            Self::GeneratedVideo => Some("generated_video"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AssetMediaTypeFilter {
    #[default]
    All,
    Image,
    Video,
    Audio,
}

impl AssetMediaTypeFilter {
    pub fn asset_type(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Image => Some("image"),
            Self::Video => Some("video"),
            Self::Audio => Some("audio"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AssetSourceFilter {
    #[default]
    All,
    Source,
    Generated,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AssetCreatedOrder {
    #[default]
    Newest,
    Oldest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetLibraryQuery {
    pub project_id: String,
    pub category: AssetCategoryFilter,
    pub keyword: Option<String>,
    pub media_type: AssetMediaTypeFilter,
    pub source_kind: AssetSourceFilter,
    pub created_order: AssetCreatedOrder,
    pub cursor: Option<PageCursor>,
    pub limit: u32,
}

#[async_trait]
pub trait AssetBrowseRepository: Send + Sync {
    async fn list_page(
        &self,
        query: AssetLibraryQuery,
    ) -> Result<PageResult<Asset>, RepositoryError>;
}
