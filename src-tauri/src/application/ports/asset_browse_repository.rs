use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::RepositoryError;
use crate::domain::Asset;
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetCategoryFilter {
    All,
    SourceImage,
    GeneratedImage,
}

impl AssetCategoryFilter {
    pub fn category(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::SourceImage => Some("source_image"),
            Self::GeneratedImage => Some("generated_image"),
        }
    }
}

#[async_trait]
pub trait AssetBrowseRepository: Send + Sync {
    async fn list_page(
        &self,
        project_id: &str,
        category: AssetCategoryFilter,
        cursor: Option<PageCursor>,
        limit: u32,
    ) -> Result<PageResult<Asset>, RepositoryError>;
}
