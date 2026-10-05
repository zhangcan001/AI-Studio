use super::*;
use crate::application::ports::{
    AssetCategoryFilter, AssetCreatedOrder, AssetLibraryQuery, AssetMediaTypeFilter,
    AssetSourceFilter,
};
use chrono::{DateTime, Utc};

impl LibraryServices<'_> {
    pub async fn list(
        &self,
        project_id: &str,
        query: LibraryQuery,
    ) -> Result<LibraryList, ProductError> {
        if project_id.trim().is_empty() {
            return Err(invalid_query());
        }
        let keyword = query
            .keyword
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        if keyword.as_ref().is_some_and(|v| v.chars().count() > 200) {
            return Err(invalid_query());
        }
        let favorite_only = query.favorite_only.unwrap_or(false);
        let tag_id = query
            .tag_id
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        if (favorite_only || tag_id.is_some())
            && !matches!(
                query.category,
                LibraryCategory::Media
                    | LibraryCategory::Images
                    | LibraryCategory::Videos
                    | LibraryCategory::Audio
            )
        {
            return Err(invalid_query());
        }
        let cursor = match query.cursor {
            Some(cursor)
                if cursor.project_id == project_id
                    && cursor.category == query.category
                    && cursor.keyword == keyword
                    && cursor.favorite_only == favorite_only
                    && cursor.tag_id == tag_id =>
            {
                Some(cursor.position)
            }
            Some(_) => return Err(invalid_query()),
            None => None,
        };
        let category = query.category;
        let limit = query.limit.unwrap_or(30).clamp(1, 100);
        if cursor.is_some()
            && matches!(
                category,
                LibraryCategory::All | LibraryCategory::Profiles | LibraryCategory::ReferenceSets
            )
        {
            return Err(invalid_query());
        }
        let (items, next, coverage) = match category {
            LibraryCategory::All => {
                // Bounded media/prompt pages + latest profile/set summaries are NOT
                // an exhaustive merged collection and expose no global cursor.
                let mut items = self
                    .media(
                        project_id,
                        LibraryCategory::Media,
                        keyword.clone(),
                        false,
                        None,
                        None,
                        12,
                    )
                    .await?
                    .0;
                items.extend(
                    self.prompt_page(project_id, keyword.as_deref(), None, 12)
                        .await?
                        .0,
                );
                let mut profiles = self.profile_list(project_id, keyword.as_deref()).await?;
                sort(&mut profiles);
                profiles.truncate(12);
                items.extend(profiles);
                let mut sets = self.set_list(project_id, keyword.as_deref()).await?;
                sort(&mut sets);
                sets.truncate(12);
                items.extend(sets);
                sort(&mut items);
                (items, None, LibraryCoverage::RecentSummary)
            }
            LibraryCategory::Prompts => {
                let (items, next) = self
                    .prompt_page(project_id, keyword.as_deref(), cursor, limit)
                    .await?;
                (items, next, LibraryCoverage::KeysetPage)
            }
            LibraryCategory::Profiles => (
                self.profile_list(project_id, keyword.as_deref()).await?,
                None,
                LibraryCoverage::CompleteCategory,
            ),
            LibraryCategory::ReferenceSets => (
                self.set_list(project_id, keyword.as_deref()).await?,
                None,
                LibraryCoverage::CompleteCategory,
            ),
            _ => {
                let (items, next) = self
                    .media(
                        project_id,
                        category,
                        keyword.clone(),
                        favorite_only,
                        tag_id.clone(),
                        cursor,
                        limit,
                    )
                    .await?;
                (items, next, LibraryCoverage::KeysetPage)
            }
        };
        Ok(LibraryList {
            items,
            next_cursor: next.map(|position| LibraryCursor {
                project_id: project_id.into(),
                category,
                keyword,
                favorite_only,
                tag_id,
                position,
            }),
            coverage,
            coverage_message: match coverage {
                LibraryCoverage::RecentSummary => "各类近期摘要；查看分类可浏览更多资源。",
                LibraryCoverage::KeysetPage => "当前分类数据库分页结果。",
                LibraryCoverage::CompleteCategory => {
                    "现有服务返回的完整分类列表；搜索在服务端筛选，不提供分页游标。"
                }
            },
        })
    }

    async fn media(
        &self,
        project: &str,
        category: LibraryCategory,
        keyword: Option<String>,
        favorite_only: bool,
        tag_id: Option<String>,
        cursor: Option<crate::application::pagination::PageCursor>,
        limit: u32,
    ) -> Result<
        (
            Vec<LibraryItem>,
            Option<crate::application::pagination::PageCursor>,
        ),
        ProductError,
    > {
        let page = self
            .assets
            .list_page(AssetLibraryQuery {
                project_id: project.into(),
                category: AssetCategoryFilter::All,
                keyword,
                media_type: match category {
                    LibraryCategory::Images => AssetMediaTypeFilter::Image,
                    LibraryCategory::Videos => AssetMediaTypeFilter::Video,
                    LibraryCategory::Audio => AssetMediaTypeFilter::Audio,
                    _ => AssetMediaTypeFilter::All,
                },
                source_kind: AssetSourceFilter::All,
                favorite_only,
                tag_id,
                created_order: AssetCreatedOrder::Newest,
                cursor,
                limit,
            })
            .await
            .map_err(ProductError::internal)?;
        Ok((
            page.items
                .into_iter()
                .map(|a| LibraryItem {
                    resource_ref: ResourceRef::Asset { id: a.id },
                    title: a.name,
                    subtype: a.asset_type,
                    created_at: a.created_at,
                    updated_at: a.updated_at,
                    thumbnail_available: a.thumbnail_available,
                })
                .collect(),
            page.next_cursor,
        ))
    }

    async fn prompt_page(
        &self,
        project: &str,
        keyword: Option<&str>,
        cursor: Option<crate::application::pagination::PageCursor>,
        limit: u32,
    ) -> Result<
        (
            Vec<LibraryItem>,
            Option<crate::application::pagination::PageCursor>,
        ),
        ProductError,
    > {
        let page = self
            .prompts
            .list(project, None, keyword, None, cursor, Some(limit))
            .await
            .map_err(ProductError::internal)?;
        let items = page
            .items
            .into_iter()
            .map(|p| {
                Ok(LibraryItem {
                    resource_ref: ResourceRef::Prompt { id: p.id },
                    title: p.name,
                    subtype: p.kind,
                    created_at: date(&p.created_at)?,
                    updated_at: date(&p.updated_at)?,
                    thumbnail_available: false,
                })
            })
            .collect::<Result<_, ProductError>>()?;
        Ok((items, page.next_cursor))
    }

    async fn profile_list(
        &self,
        project: &str,
        keyword: Option<&str>,
    ) -> Result<Vec<LibraryItem>, ProductError> {
        let mut items = self
            .profiles
            .list(project, None)
            .await
            .map_err(ProductError::internal)?
            .into_iter()
            .filter(|p| matches_keyword(p.name(), keyword))
            .map(|p| LibraryItem {
                resource_ref: ResourceRef::Profile { id: p.id().into() },
                title: p.name().into(),
                subtype: p.profile_type().as_str().into(),
                created_at: p.created_at(),
                updated_at: p.updated_at(),
                thumbnail_available: false,
            })
            .collect();
        sort(&mut items);
        Ok(items)
    }

    async fn set_list(
        &self,
        project: &str,
        keyword: Option<&str>,
    ) -> Result<Vec<LibraryItem>, ProductError> {
        let mut items = self
            .reference_sets
            .list(project, None)
            .await
            .map_err(ProductError::internal)?
            .into_iter()
            .filter(|s| matches_keyword(&s.name, keyword))
            .map(|s| LibraryItem {
                resource_ref: ResourceRef::ReferenceSet { id: s.id },
                title: s.name,
                subtype: s.purpose.as_str().into(),
                created_at: s.created_at,
                updated_at: s.updated_at,
                thumbnail_available: false,
            })
            .collect();
        sort(&mut items);
        Ok(items)
    }
}

fn matches_keyword(name: &str, keyword: Option<&str>) -> bool {
    keyword.is_none_or(|k| name.to_lowercase().contains(&k.to_lowercase()))
}
fn date(value: &str) -> Result<DateTime<Utc>, ProductError> {
    DateTime::parse_from_rfc3339(value)
        .map(|v| v.with_timezone(&Utc))
        .map_err(ProductError::internal)
}
fn sort(items: &mut Vec<LibraryItem>) {
    items.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.resource_ref.id().cmp(b.resource_ref.id()))
            .then_with(|| a.subtype.cmp(&b.subtype))
    });
}
