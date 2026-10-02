use super::*;
use crate::application::{
    asset_query_service::AssetQueryError, consistency_profile_service::ConsistencyProfileError,
    prompt_library_service::PromptLibraryError, reference_set_service::ReferenceSetError,
};
use crate::domain::consistency::ProfileType;

impl LibraryServices<'_> {
    pub async fn image_get(
        &self,
        project_id: &str,
        resource: &ResourceRef,
    ) -> Result<Vec<u8>, ProductError> {
        let LibraryDetail::Asset { asset } = self.get(project_id, resource).await? else {
            return Err(invalid_query());
        };
        if asset.asset_type != "image" || asset.file_size > 32 * 1024 * 1024 {
            return Err(ProductError::new(
                "ASSET_TYPE_MISMATCH",
                "该资源不能通过图片预览读取。",
                None,
            ));
        }
        self.asset_detail
            .read_image(project_id, &asset.id)
            .await
            .map(|v| v.bytes)
            .map_err(ProductError::internal)
    }
    pub async fn get(
        &self,
        project_id: &str,
        resource: &ResourceRef,
    ) -> Result<LibraryDetail, ProductError> {
        if resource.id().trim().is_empty() || project_id.trim().is_empty() {
            return Err(invalid_query());
        }
        match resource {
            ResourceRef::Asset { id } => self
                .asset_detail
                .get(project_id, id)
                .await
                .map(|asset| LibraryDetail::Asset { asset })
                .map_err(|error| match error {
                    AssetQueryError::NotFound(_) => missing(),
                    other => ProductError::internal(other),
                }),
            ResourceRef::Prompt { id } => self
                .prompts
                .get(project_id, id)
                .await
                .map(|prompt| LibraryDetail::Prompt { prompt })
                .map_err(|error| match error {
                    PromptLibraryError::NotFound(_) => missing(),
                    other => ProductError::internal(other),
                }),
            ResourceRef::Profile { id } => {
                // Type is not guessed from a name or an identifier prefix.
                for profile_type in [
                    ProfileType::Character,
                    ProfileType::Scene,
                    ProfileType::Prop,
                    ProfileType::Style,
                ] {
                    match self.profiles.get(project_id, profile_type, id).await {
                        Ok(profile) => return Ok(LibraryDetail::Profile { profile }),
                        Err(ConsistencyProfileError::NotFound(_)) => continue,
                        Err(error) => return Err(ProductError::internal(error)),
                    }
                }
                Err(missing())
            }
            ResourceRef::ReferenceSet { id } => {
                let view = self
                    .reference_sets
                    .get_detail(project_id, id)
                    .await
                    .map_err(|error| match error {
                        ReferenceSetError::NotFound(_) => missing(),
                        other => ProductError::internal(other),
                    })?;
                let set = view.reference_set;
                Ok(LibraryDetail::ReferenceSet {
                    reference_set: LibraryReferenceSet {
                        id: set.id,
                        name: set.name,
                        description: set.description,
                        purpose: set.purpose,
                        owner_profile_type: set.owner_profile_type,
                        owner_profile_id: set.owner_profile_id,
                        active_revision_id: set.active_revision_id,
                        items: view
                            .items
                            .into_iter()
                            .map(|item| LibraryReferenceMember {
                                asset_id: item.asset_id,
                                ordinal: item.ordinal,
                                role: item.role,
                                is_primary: item.is_primary,
                                asset_name: item.asset_name,
                                thumbnail_available: item.thumbnail_available,
                                width: item.width,
                                height: item.height,
                            })
                            .collect(),
                    },
                })
            }
        }
    }
}
