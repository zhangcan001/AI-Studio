use super::{error::ProductError, selection_ref::ExactGeneratorSelection};
use crate::application::{
    generation_catalog_service::GenerationCatalogService,
    project_workflow_binding_service::{ProjectWorkflowBindingService, ProjectWorkflowBindingView},
    shot_service::ShotService,
};
use serde::Serialize;
use std::sync::Arc;

mod context;
pub use context::*;

pub struct ProductCreationFacade {
    catalog: Arc<GenerationCatalogService>,
    bindings: Arc<ProjectWorkflowBindingService>,
    shots: Arc<ShotService>,
    registry: Arc<crate::application::workflow_registry_service::WorkflowRegistryService>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorOption {
    pub selection_ref: String,
    pub name: String,
    pub version: String,
    pub mode: String,
    pub media_kind: String,
    pub availability: bool,
    pub availability_reason: Option<String>,
    pub recommended: bool,
}

impl ProductCreationFacade {
    pub fn new(
        catalog: Arc<GenerationCatalogService>,
        bindings: Arc<ProjectWorkflowBindingService>,
        shots: Arc<ShotService>,
        registry: Arc<crate::application::workflow_registry_service::WorkflowRegistryService>,
    ) -> Self {
        Self {
            catalog,
            bindings,
            shots,
            registry,
        }
    }

    pub async fn generators_list(
        &self,
        project_id: &str,
        shot_id: Option<&str>,
        stage: &str,
    ) -> Result<Vec<GeneratorOption>, ProductError> {
        let stage = match stage {
            "image" => "IMAGE",
            "video" => "VIDEO",
            _ => {
                return Err(ProductError::new(
                    "INVALID_INPUT",
                    "请选择图片或视频阶段。",
                    None,
                ))
            }
        };
        let config = self
            .bindings
            .get(project_id)
            .await
            .map_err(ProductError::binding)?;
        let mut selected: Vec<ProjectWorkflowBindingView> = config
            .image_default
            .into_iter()
            .chain(config.video_default)
            .chain(config.video_mode_overrides)
            .filter(|b| b.stage == stage)
            .collect();
        let shot = match shot_id {
            Some(id) => Some(self.shots.get(project_id, id).await.map_err(
                |error| match error {
                    crate::application::shot_service::ShotServiceError::NotFound(_) => {
                        ProductError::new(
                            "PROJECT_SCOPE_VIOLATION",
                            "无法访问此项目中的镜头。",
                            None,
                        )
                    }
                    crate::application::shot_service::ShotServiceError::InvalidInput(_) => {
                        ProductError::new("INVALID_INPUT", "镜头标识无效。", None)
                    }
                    other => ProductError::internal(other),
                },
            )?),
            None => None,
        };
        let shot_pair = shot
            .as_ref()
            .and_then(|s| s.stage_configs.iter().find(|c| c.stage == stage))
            .map(|c| (&c.workflow_version_id, &c.recipe_id));
        let catalog = self.catalog.list().await.map_err(ProductError::internal)?;
        let mut options = Vec::new();
        for generator in catalog {
            let media_kind = if generator.output_types.iter().any(|t| t == "video") {
                "video"
            } else if generator.output_types.iter().any(|t| t == "image") {
                "image"
            } else {
                continue;
            };
            if media_kind != stage.to_ascii_lowercase() {
                continue;
            }
            let matching = selected.iter().position(|b| {
                b.workflow_version_id == generator.workflow_version_id
                    && b.recipe_id == generator.recipe_id
            });
            let recommended = if let Some((v, r)) = shot_pair {
                *v == generator.workflow_version_id && *r == generator.recipe_id
            } else {
                matching.is_some()
            };
            let available = self
                .bindings
                .is_workflow_available_for_recipe(
                    &generator.workflow_version_id,
                    &generator.recipe_id,
                )
                .await
                .map_err(ProductError::binding)?;
            selected.retain(|b| {
                b.workflow_version_id != generator.workflow_version_id
                    || b.recipe_id != generator.recipe_id
            });
            options.push(GeneratorOption {
                selection_ref: ExactGeneratorSelection {
                    workflow_version_id: generator.workflow_version_id,
                    recipe_id: generator.recipe_id,
                }
                .encode()?,
                name: generator.name,
                version: generator.recipe_version,
                mode: generator.mode,
                media_kind: media_kind.to_owned(),
                availability: available,
                availability_reason: (!available)
                    .then(|| "生成器当前不可用，请在高级工作流管理中检查。".to_owned()),
                recommended,
            });
        }
        // Existing bindings may refer to disabled historical generators absent
        // from the available catalog. Keep them visible without guessing identity.
        for binding in selected {
            let details = self
                .registry
                .get_saved_version_details(&binding.workflow_version_id)
                .await
                .map_err(ProductError::internal)?;
            if crate::application::product_runtime_scope::is_retired_h3_workflow(
                &details.workflow_id,
            ) {
                continue;
            }
            options.push(GeneratorOption {
                selection_ref: ExactGeneratorSelection {
                    workflow_version_id: binding.workflow_version_id,
                    recipe_id: binding.recipe_id,
                }
                .encode()?,
                name: details.name,
                version: details.workflow_version,
                mode: binding.mode,
                media_kind: stage.to_ascii_lowercase(),
                availability: false,
                availability_reason: Some("生成器已停用或不可用，请重新选择。".to_owned()),
                recommended: false,
            });
        }
        if let Some((version, recipe)) = shot_pair {
            let selection_ref = ExactGeneratorSelection {
                workflow_version_id: version.clone(),
                recipe_id: recipe.clone(),
            }
            .encode()?;
            if !options
                .iter()
                .any(|option| option.selection_ref == selection_ref)
            {
                let details = self
                    .registry
                    .get_saved_version_details(version)
                    .await
                    .map_err(ProductError::internal)?;
                if !crate::application::product_runtime_scope::is_retired_h3_workflow(
                    &details.workflow_id,
                ) {
                    options.push(GeneratorOption {
                        selection_ref,
                        name: details.name,
                        version: details.workflow_version,
                        mode: details.mode,
                        media_kind: stage.to_ascii_lowercase(),
                        availability: false,
                        availability_reason: Some(
                            "镜头所用的历史生成器当前不可用，请重新选择。".to_owned(),
                        ),
                        recommended: false,
                    });
                }
            }
        }
        Ok(options)
    }
}
