use super::*;
use crate::application::ports::{prompt_library_repository::PromptUsageKind, AssetUsageItem};
use crate::application::product::run_facade::{RunRef, RunSource};

impl LibraryOperations<'_> {
    pub async fn relations_get(
        &self,
        project: &str,
        resource: &ResourceRef,
    ) -> Result<Vec<LibraryRelation>, ProductError> {
        let detail = self.library.get(project, resource).await?;
        match detail {
            LibraryDetail::Asset { asset } => {
                let mut items: Vec<_> = self
                    .usage
                    .asset_usage(project, &asset.id)
                    .await
                    .map_err(ProductError::internal)?
                    .items
                    .into_iter()
                    .map(usage_relation)
                    .collect();
                let refs = self
                    .deletion
                    .references(project, &[asset.id])
                    .await
                    .map_err(ProductError::internal)?;
                for reference in refs {
                    for _ in reference.meaningful_artifact_review_ids {
                        items.push(LibraryRelation {
                            kind: LibraryRelationKind::ArtifactReviewMeaningful,
                            title: "素材审核历史".into(),
                            description: "已经形成审核记录。为避免删除审核历史，请保留该素材。"
                                .into(),
                            blocking: true,
                            location: None,
                        });
                    }
                    for _ in reference.placeholder_artifact_review_ids {
                        items.push(LibraryRelation {
                            kind: LibraryRelationKind::ArtifactReviewPlaceholder,
                            title: "待审核".into(),
                            description: "系统自动建立的待审核占位，不属于已形成的审核历史。"
                                .into(),
                            blocking: false,
                            location: None,
                        });
                    }
                    for _ in reference.generation_asset_version_ids {
                        items.push(LibraryRelation {
                            kind: LibraryRelationKind::GenerationAssetVersionLineage,
                            title: "生成版本溯源".into(),
                            description: "素材版本被不可变生成溯源使用。".into(),
                            blocking: true,
                            location: None,
                        });
                    }
                }
                Ok(items)
            }
            LibraryDetail::Profile { profile } => Ok(self
                .usage
                .profile_usage(project, profile.profile_type(), profile.id())
                .await
                .map_err(ProductError::internal)?
                .items
                .into_iter()
                .map(usage_relation)
                .collect()),
            LibraryDetail::ReferenceSet { reference_set } => Ok(self
                .usage
                .reference_set_usage(project, &reference_set.id)
                .await
                .map_err(ProductError::internal)?
                .items
                .into_iter()
                .map(usage_relation)
                .collect()),
            LibraryDetail::Prompt { prompt } => Ok(self
                .library
                .prompts
                .usage(project, &prompt.id)
                .await
                .map_err(ProductError::internal)?
                .into_iter()
                .map(|u| LibraryRelation {
                    kind: if u.kind == PromptUsageKind::Snapshot {
                        LibraryRelationKind::GenerationSnapshotInput
                    } else {
                        LibraryRelationKind::PromptVersionUsage
                    },
                    title: u.display_name,
                    description: if u.kind == PromptUsageKind::Snapshot {
                        "不可变快照保存了此提示词版本。".into()
                    } else {
                        "镜头正式提示词引用。".into()
                    },
                    blocking: true,
                    location: Some(if u.kind == PromptUsageKind::Snapshot {
                        LibraryRelationLocation::Run {
                            run_ref: RunRef {
                                source: RunSource::Task,
                                id: u.entity_id,
                            },
                        }
                    } else {
                        LibraryRelationLocation::Shot {
                            id: u.entity_id,
                            stage: u.stage,
                        }
                    }),
                })
                .collect()),
        }
    }
}

fn usage_relation(u: AssetUsageItem) -> LibraryRelation {
    use LibraryRelationKind as K;
    let kind = match u.relation_type.as_str() {
        "SELECTED_IMAGE_ASSET" | "SELECTED_VIDEO_ASSET" => K::ShotSelectedResult,
        "LEGACY_SHOT_REFERENCE" | "REFERENCE_ANCHOR_ASSET" => K::ShotReference,
        "GENERATION_SNAPSHOT_ASSET" => K::GenerationSnapshotInput,
        "ASSET_SOURCE_TASK" => {
            if u.blocking {
                K::ActiveTask
            } else {
                K::GenerationOutput
            }
        }
        "TASK_OUTPUT_ASSET" => K::RunResult,
        "PRODUCTION_STAGE_ASSET" | "PRODUCTION_QUEUE_ASSET" => {
            if u.blocking {
                K::ActiveProduction
            } else {
                K::RunResult
            }
        }
        "PRODUCTION_REVIEW_ASSET" => K::LegacyReviewHistory,
        "REFERENCE_SET_ITEM" => K::ReferenceSetMember,
        _ => {
            if u.entity_type == "REFERENCE_SET" {
                K::ReferenceSetUsage
            } else {
                K::ProfileRelation
            }
        }
    };
    let location = if u.entity_type == "SHOT" {
        Some(LibraryRelationLocation::Shot {
            id: u.entity_id.clone(),
            stage: match u.relation_type.as_str() {
                "SELECTED_VIDEO_ASSET" => Some("video".into()),
                "SELECTED_IMAGE_ASSET" => Some("image".into()),
                _ => None,
            },
        })
    } else if u.entity_type == "TASK" {
        Some(LibraryRelationLocation::Run {
            run_ref: RunRef {
                source: RunSource::Task,
                id: u.entity_id.clone(),
            },
        })
    } else if u.entity_type == "ASSET" {
        Some(LibraryRelationLocation::Resource {
            resource: ResourceRef::Asset {
                id: u.entity_id.clone(),
            },
        })
    } else if u.entity_type == "REFERENCE_SET" {
        Some(LibraryRelationLocation::Resource {
            resource: ResourceRef::ReferenceSet {
                id: u.entity_id.clone(),
            },
        })
    } else if u.entity_type == "PROFILE" {
        Some(LibraryRelationLocation::Resource {
            resource: ResourceRef::Profile {
                id: u.entity_id.clone(),
            },
        })
    } else {
        None
    };
    let title = if u.display_name == u.entity_id || u.display_name.is_empty() {
        match u.entity_type.as_str() {
            "TASK" => "生成运行",
            "SHOT" => "镜头",
            "PRODUCTION_ITEM" | "PRODUCTION_STAGE_ITEM" => "生产运行",
            "REVIEW" => "审核历史",
            "PROFILE_REVISION" => "设定修订历史",
            _ => "使用关系",
        }
        .into()
    } else {
        u.display_name
    };
    LibraryRelation {
        kind,
        title,
        description: u.detail,
        blocking: u.blocking,
        location,
    }
}
