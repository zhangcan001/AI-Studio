use crate::application::asset_query_service::{AssetSummaryView, AssetView};
use crate::application::pagination::PageCursor;
use crate::application::ports::{AssetBrowseRepository, AssetCategoryFilter, RepositoryError};
use std::{error::Error, fmt, sync::Arc};

pub struct AssetLibraryService {
    repository: Arc<dyn AssetBrowseRepository>,
}

impl AssetLibraryService {
    pub fn new(repository: Arc<dyn AssetBrowseRepository>) -> Self {
        Self { repository }
    }

    pub async fn list_page(
        &self,
        project_id: &str,
        category: AssetCategoryFilter,
        cursor: Option<PageCursor>,
        limit: u32,
    ) -> Result<AssetLibraryPageView, AssetLibraryError> {
        if project_id.trim().is_empty() {
            return Err(AssetLibraryError::InvalidProjectId);
        }
        let page = self
            .repository
            .list_page(project_id, category, cursor, limit.clamp(1, 100))
            .await?;
        Ok(AssetLibraryPageView {
            items: page.items.into_iter().map(AssetView::from).collect(),
            next_cursor: page.next_cursor,
        })
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetLibraryPageView {
    pub items: Vec<AssetSummaryView>,
    pub next_cursor: Option<PageCursor>,
}

#[derive(Debug)]
pub enum AssetLibraryError {
    InvalidProjectId,
    Repository(RepositoryError),
}

impl fmt::Display for AssetLibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectId => {
                formatter.write_str("INVALID_PROJECT_ID: project id must not be empty")
            }
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for AssetLibraryError {}

impl From<RepositoryError> for AssetLibraryError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}
