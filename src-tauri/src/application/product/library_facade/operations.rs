use super::*;
use crate::application::{
    asset_data_service::AssetDataService,
    asset_deletion_service::{AssetDeletionError, AssetDeletionService},
    asset_usage_service::AssetUsageService,
    consistency_profile_service::*,
    reference_set_service::{ReferenceSetItemRequest, UpdateReferenceSetRequest},
};
use crate::domain::consistency::ConsistencyProfileRecord;

pub struct LibraryOperations<'a> {
    pub library: LibraryServices<'a>,
    pub usage: &'a AssetUsageService,
    pub deletion: &'a AssetDeletionService,
    pub data: &'a AssetDataService,
}

impl LibraryOperations<'_> {
    pub async fn versions_get(
        &self,
        project: &str,
        resource: &ResourceRef,
    ) -> Result<LibraryVersions, ProductError> {
        match self.library.get(project, resource).await? {
            LibraryDetail::Asset { asset } => Ok(LibraryVersions::Asset {
                versions: self
                    .data
                    .list_versions(project, &asset.id)
                    .await
                    .map_err(ProductError::internal)?
                    .into_iter()
                    .map(|v| LibraryAssetVersion {
                        id: v.id.as_str().into(),
                        version_number: v.version_number,
                        created_at: v.created_at,
                    })
                    .collect(),
            }),
            LibraryDetail::Prompt { mut prompt } => {
                prompt
                    .versions
                    .sort_by(|a, b| b.version.cmp(&a.version).then_with(|| b.id.cmp(&a.id)));
                Ok(LibraryVersions::Prompt {
                    versions: prompt.versions,
                })
            }
            _ => Ok(LibraryVersions::Unsupported {
                reason: "此类资源的高级修订历史仍由原有编辑器提供。",
            }),
        }
    }

    pub async fn use_in_creation(
        &self,
        project: &str,
        resource: &ResourceRef,
    ) -> Result<LibraryCreateIntent, ProductError> {
        match self.library.get(project, resource).await? {
            LibraryDetail::Asset { asset } => Ok(LibraryCreateIntent::Asset {
                project_id: project.into(),
                asset_id: asset.id,
                media_kind: asset.asset_type,
            }),
            LibraryDetail::Prompt { prompt } => {
                let v = prompt
                    .versions
                    .into_iter()
                    .max_by_key(|v| v.version)
                    .ok_or_else(missing)?;
                Ok(LibraryCreateIntent::Prompt {
                    project_id: project.into(),
                    prompt_version_id: v.id,
                    text: v.text,
                    model_version_id: v.model_version_id,
                })
            }
            _ => Ok(LibraryCreateIntent::Context {
                project_id: project.into(),
                resource: resource.clone(),
                message: "需要在创作页选择应用位置；当前只携带资源上下文，尚未应用设定。",
            }),
        }
    }

    pub async fn deletion_inspect(
        &self,
        project: &str,
        resource: &ResourceRef,
    ) -> Result<LibraryDeletionInspection, ProductError> {
        let detail = self.library.get(project, resource).await?;
        let relations = self.relations_get(project, resource).await?;
        let (blockers, warnings) = match detail {
            LibraryDetail::Asset { asset } => {
                let inspection = self
                    .deletion
                    .inspect(project, &[asset.id])
                    .await
                    .map_err(delete_error)?;
                let item = inspection.items.into_iter().next().ok_or_else(missing)?;
                (item.blocking_reasons, item.warnings)
            }
            LibraryDetail::Profile { profile } => (
                self.usage
                    .profile_delete_blockers(project, profile.profile_type(), profile.id())
                    .await
                    .map_err(ProductError::internal)?,
                Vec::new(),
            ),
            LibraryDetail::ReferenceSet { reference_set } => (
                self.usage
                    .reference_set_delete_blockers(project, &reference_set.id)
                    .await
                    .map_err(ProductError::internal)?,
                Vec::new(),
            ),
            // Existing prompt delete cascades immutable versions / SET NULL Shot
            // provenance without an official guard. Fail closed, do not bypass it.
            LibraryDetail::Prompt { .. } => (
                vec!["暂不支持提示词安全删除；不可变版本和引用关系将保留。".into()],
                Vec::new(),
            ),
        };
        Ok(LibraryDeletionInspection {
            allowed: blockers.is_empty(),
            blockers,
            warnings,
            relations,
            consequences: vec!["删除资源不可撤销；已有任务、运行和生成快照记录仍保留。".into()],
        })
    }

    pub async fn delete(
        &self,
        project: &str,
        resource: &ResourceRef,
        confirmed: bool,
    ) -> Result<(), ProductError> {
        if !confirmed {
            return Err(ProductError::new(
                "LIBRARY_DELETE_CONFIRMATION_REQUIRED",
                "请先查看删除影响并明确确认。",
                Some("INSPECT_DELETE"),
            ));
        }
        let inspection = self.deletion_inspect(project, resource).await?;
        if !inspection.allowed {
            return Err(blocked());
        }
        match self.library.get(project, resource).await? {
            LibraryDetail::Asset { asset } => {
                self.deletion
                    .delete(project, &[asset.id])
                    .await
                    .map_err(delete_error)?;
            }
            LibraryDetail::Profile { profile } => {
                self.usage
                    .ensure_profile_deletable(project, profile.profile_type(), profile.id())
                    .await
                    .map_err(usage_error)?;
                self.library
                    .profiles
                    .delete(project, profile.profile_type(), profile.id())
                    .await
                    .map_err(ProductError::internal)?;
            }
            LibraryDetail::ReferenceSet { reference_set } => {
                self.usage
                    .ensure_reference_set_deletable(project, &reference_set.id)
                    .await
                    .map_err(usage_error)?;
                self.library
                    .reference_sets
                    .delete(project, &reference_set.id)
                    .await
                    .map_err(ProductError::internal)?;
            }
            LibraryDetail::Prompt { .. } => return Err(blocked()),
        }
        Ok(())
    }

    pub async fn resource_edit(
        &self,
        project: &str,
        request: LibraryEditRequest,
    ) -> Result<LibraryDetail, ProductError> {
        let resource = match request {
            LibraryEditRequest::Prompt {
                id,
                text,
                model_version_id,
            } => {
                self.library
                    .prompts
                    .get(project, &id)
                    .await
                    .map_err(ProductError::internal)?;
                self.library
                    .prompts
                    .add_version_with_model_version(
                        project,
                        &id,
                        &text,
                        model_version_id.as_deref(),
                    )
                    .await
                    .map_err(edit_error)?;
                ResourceRef::Prompt { id }
            }
            LibraryEditRequest::Profile { id, name } => {
                let LibraryDetail::Profile { profile } = self
                    .library
                    .get(project, &ResourceRef::Profile { id: id.clone() })
                    .await?
                else {
                    unreachable!()
                };
                match profile {
                    ConsistencyProfileRecord::Character(p) => {
                        self.library
                            .profiles
                            .update_character(UpdateCharacterProfileRequest {
                                project_id: project.into(),
                                profile_id: id.clone(),
                                name,
                                description: p.description,
                                canonical_prompt: p.canonical_prompt,
                                negative_prompt: p.negative_prompt,
                                default_style_profile_id: p.default_style_profile_id,
                                default_reference_set_id: p.default_reference_set_id,
                                metadata_json: p.metadata_json,
                            })
                            .await
                            .map_err(edit_error)?;
                    }
                    ConsistencyProfileRecord::Scene(p) => {
                        self.library
                            .profiles
                            .update_scene(UpdateSceneProfileRequest {
                                project_id: project.into(),
                                profile_id: id.clone(),
                                name,
                                description: p.description,
                                environment_prompt: p.environment_prompt,
                                lighting_prompt: p.lighting_prompt,
                                negative_prompt: p.negative_prompt,
                                default_style_profile_id: p.default_style_profile_id,
                                default_reference_set_id: p.default_reference_set_id,
                            })
                            .await
                            .map_err(edit_error)?;
                    }
                    ConsistencyProfileRecord::Prop(p) => {
                        self.library
                            .profiles
                            .update_prop(UpdatePropProfileRequest {
                                project_id: project.into(),
                                profile_id: id.clone(),
                                name,
                                description: p.description,
                                canonical_prompt: p.canonical_prompt,
                                material_prompt: p.material_prompt,
                                scale_prompt: p.scale_prompt,
                                default_reference_set_id: p.default_reference_set_id,
                            })
                            .await
                            .map_err(edit_error)?;
                    }
                    ConsistencyProfileRecord::Style(p) => {
                        self.library
                            .profiles
                            .update_style(UpdateStyleProfileRequest {
                                project_id: project.into(),
                                profile_id: id.clone(),
                                name,
                                style_prompt: p.style_prompt,
                                color_prompt: p.color_prompt,
                                line_prompt: p.line_prompt,
                                negative_prompt: p.negative_prompt,
                                output_notes: p.output_notes,
                            })
                            .await
                            .map_err(edit_error)?;
                    }
                }
                ResourceRef::Profile { id }
            }
            LibraryEditRequest::ReferenceSet {
                id,
                name,
                description,
                items,
            } => {
                let old = self
                    .library
                    .reference_sets
                    .get(project, &id)
                    .await
                    .map_err(ProductError::internal)?;
                self.library
                    .reference_sets
                    .update(UpdateReferenceSetRequest {
                        project_id: project.into(),
                        reference_set_id: id.clone(),
                        name,
                        description,
                        purpose: old.purpose,
                        owner_profile_type: old.owner_profile_type,
                        owner_profile_id: old.owner_profile_id,
                        items: items
                            .into_iter()
                            .map(|v| ReferenceSetItemRequest {
                                asset_id: v.asset_id,
                                ordinal: v.ordinal,
                                role: v.role,
                                is_primary: v.is_primary,
                            })
                            .collect(),
                    })
                    .await
                    .map_err(edit_error)?;
                ResourceRef::ReferenceSet { id }
            }
        };
        self.library.get(project, &resource).await
    }
}
fn blocked() -> ProductError {
    ProductError::new(
        "LIBRARY_DELETE_BLOCKED",
        "资源仍有受保护的引用，不能删除。请刷新使用位置。",
        Some("INSPECT_DELETE"),
    )
}
fn usage_error(error: crate::application::asset_usage_service::AssetUsageError) -> ProductError {
    match error {
        crate::application::asset_usage_service::AssetUsageError::Blocked { .. } => blocked(),
        other => ProductError::internal(other),
    }
}
fn delete_error(error: AssetDeletionError) -> ProductError {
    match error {
        AssetDeletionError::NotFound(_) => missing(),
        AssetDeletionError::Blocked(_) => blocked(),
        other => ProductError::internal(other),
    }
}
fn edit_error(error: impl std::fmt::Display) -> ProductError {
    let mut e = ProductError::new(
        "LIBRARY_EDIT_INVALID",
        "修改未保存，请检查名称、正文或成员是否有效。",
        Some("EDIT_RESOURCE"),
    );
    e.details.technical_details = Some(error.to_string());
    e
}
