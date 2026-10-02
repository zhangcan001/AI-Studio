//! Library is a projection, not a resource repository or lifecycle authority.
mod detail;
mod list;
mod operations;
mod relations;
mod types;

pub use operations::LibraryOperations;
pub use types::*;

use super::error::ProductError;
use crate::application::{
    asset_library_service::AssetLibraryService, asset_query_service::AssetQueryService,
    consistency_profile_service::ConsistencyProfileService,
    prompt_library_service::PromptLibraryService, reference_set_service::ReferenceSetService,
};

/// Borrow established authorities; no new persistence or store is introduced.
pub struct LibraryServices<'a> {
    pub assets: &'a AssetLibraryService,
    pub asset_detail: &'a AssetQueryService,
    pub prompts: &'a PromptLibraryService,
    pub profiles: &'a ConsistencyProfileService,
    pub reference_sets: &'a ReferenceSetService,
}

pub(super) fn missing() -> ProductError {
    ProductError::new(
        "LIBRARY_RESOURCE_NOT_FOUND",
        "资源已删除或不属于当前项目，请返回资源库。",
        Some("OPEN_LIBRARY"),
    )
}

pub(super) fn invalid_query() -> ProductError {
    ProductError::new(
        "LIBRARY_QUERY_INVALID",
        "资源库查询已变化，请重新加载当前分类。",
        Some("REFRESH_LIBRARY"),
    )
}
