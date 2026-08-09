use crate::application::asset_query_service::{AssetSummaryView, AssetView};
use crate::application::pagination::PageCursor;
use crate::application::ports::{
    AssetBrowseRepository, AssetLibraryQuery, OrganizationRepository, RepositoryError,
};
use std::{error::Error, fmt, sync::Arc};

pub struct AssetLibraryService {
    repository: Arc<dyn AssetBrowseRepository>,
    organization_repository: Arc<dyn OrganizationRepository>,
}

impl AssetLibraryService {
    pub fn new(
        repository: Arc<dyn AssetBrowseRepository>,
        organization_repository: Arc<dyn OrganizationRepository>,
    ) -> Self {
        Self {
            repository,
            organization_repository,
        }
    }

    pub async fn list_page(
        &self,
        mut query: AssetLibraryQuery,
    ) -> Result<AssetLibraryPageView, AssetLibraryError> {
        if query.project_id.trim().is_empty() {
            return Err(AssetLibraryError::InvalidProjectId);
        }
        query.project_id = query.project_id.trim().to_owned();
        query.keyword = query.keyword.and_then(|keyword| {
            let keyword = keyword.trim().to_owned();
            (!keyword.is_empty()).then_some(keyword)
        });
        query.limit = query.limit.clamp(1, 100);
        let project_id = query.project_id.clone();
        let page = self.repository.list_page(query).await?;
        let asset_ids = page
            .items
            .iter()
            .map(|asset| asset.id.as_str().to_owned())
            .collect::<Vec<_>>();
        let mut organization = self
            .organization_repository
            .organization_for_assets(&project_id, &asset_ids)
            .await?;
        Ok(AssetLibraryPageView {
            items: page
                .items
                .into_iter()
                .map(|asset| {
                    let asset_id = asset.id.as_str().to_owned();
                    AssetView::from_asset_and_organization(
                        asset,
                        organization.remove(&asset_id).unwrap_or_default(),
                    )
                })
                .collect(),
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
