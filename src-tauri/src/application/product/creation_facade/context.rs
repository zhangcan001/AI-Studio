//! Product projections and mutations delegate to Shot/Asset authorities. No persistence here.
use super::ProductCreationFacade;
use crate::application::{
    asset_query_service::{AssetQueryError, AssetQueryService, AssetSummaryView},
    product::{
        error::ProductError,
        run_facade::{RunRef, RunSource},
        selection_ref::ExactGeneratorSelection,
    },
    project_command_center_service::ProjectCommandCenterService,
    shot_service::{ShotServiceError, ShotUpdateRequest, ShotView},
};
use crate::domain::ShotStage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationShotSummary {
    pub id: String,
    pub name: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationShot {
    pub summary: CreationShotSummary,
    pub prompt: String,
    pub selection_ref: Option<String>,
    pub values: Value,
    pub reference_asset_ids: Vec<String>,
    pub selected_result_id: Option<String>,
    pub recent_run: Option<RunRef>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationAsset {
    pub id: String,
    pub name: String,
    pub media_kind: String,
    pub selected: bool,
    pub thumbnail_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationContext {
    pub project_id: String,
    pub project_name: String,
    pub stage: String,
    pub shots: Vec<CreationShotSummary>,
    // No implicit selection: the consumer must navigate to a canonical Shot route.
    pub selected_shot: Option<CreationShot>,
    pub candidates: Vec<CreationAsset>,
    // Picker inputs are deliberately separate from linked generation candidates.
    pub media_inputs: Vec<CreationAsset>,
    pub prompt_choices: Vec<CreationPromptChoice>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationPromptChoice {
    pub name: String,
    pub version: i64,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreationShotUpdate {
    pub shot_id: String,
    pub name: String,
}

fn stage(value: &str) -> Result<ShotStage, ProductError> {
    match value {
        "image" => Ok(ShotStage::Image),
        "video" => Ok(ShotStage::Video),
        _ => Err(ProductError::new(
            "INVALID_INPUT",
            "请选择图片或视频阶段。",
            Some("EDIT_INPUT"),
        )),
    }
}

fn shot_error(error: ShotServiceError) -> ProductError {
    match error {
        ShotServiceError::NotFound(_) => ProductError::new(
            "PROJECT_SCOPE_VIOLATION",
            "镜头不存在或不属于当前项目。",
            None,
        ),
        ShotServiceError::InvalidInput(detail) => {
            let mut error = ProductError::new(
                "INVALID_INPUT",
                "请检查镜头、参考素材和所选结果。",
                Some("EDIT_INPUT"),
            );
            error.details.technical_details = Some(detail);
            error
        }
        other => ProductError::internal(other),
    }
}

fn summary(shot: &ShotView) -> CreationShotSummary {
    CreationShotSummary {
        id: shot.id.clone(),
        name: shot.name.clone(),
        ordinal: shot.ordinal,
    }
}

fn project_shot(shot: &ShotView, stage: ShotStage) -> Result<CreationShot, ProductError> {
    let config = shot
        .stage_configs
        .iter()
        .find(|config| config.stage == stage.as_str());
    let prompt = shot
        .stage_prompts
        .iter()
        .find(|prompt| prompt.stage == stage.as_str())
        .map(|prompt| prompt.prompt_text.clone())
        .unwrap_or_else(|| shot.prompt_text.clone());
    let selected_result_id = match stage {
        ShotStage::Image => shot.selected_image_asset_id.clone(),
        ShotStage::Video => shot.selected_video_asset_id.clone(),
    };
    let recent_run = shot
        .generation_links
        .iter()
        .filter(|link| link.stage == stage.as_str())
        .filter_map(|link| link.task_id.as_ref().map(|id| (link.created_at, id)))
        .max_by_key(|(created_at, _)| *created_at)
        .map(|(_, id)| RunRef {
            source: RunSource::Task,
            id: id.clone(),
        });
    Ok(CreationShot {
        summary: summary(shot),
        prompt,
        selection_ref: config
            .map(|config| {
                ExactGeneratorSelection {
                    workflow_version_id: config.workflow_version_id.clone(),
                    recipe_id: config.recipe_id.clone(),
                }
                .encode()
            })
            .transpose()?,
        values: config
            .map(|config| config.scalar_values.clone())
            .unwrap_or_else(|| serde_json::json!({})),
        reference_asset_ids: {
            let mut references: Vec<_> = shot
                .reference_assets
                .iter()
                .filter(|asset| asset.stage == stage.as_str())
                .collect();
            references.sort_by_key(|asset| asset.ordinal);
            references
                .into_iter()
                .map(|asset| asset.asset_id.clone())
                .collect()
        },
        selected_result_id,
        recent_run,
    })
}

fn candidate(asset: AssetSummaryView, selected_result_id: Option<&str>) -> CreationAsset {
    CreationAsset {
        selected: selected_result_id == Some(asset.id.as_str()),
        id: asset.id,
        name: asset.name,
        media_kind: asset.asset_type,
        thumbnail_bytes: None,
    }
}

impl ProductCreationFacade {
    pub async fn get(
        &self,
        projects: &ProjectCommandCenterService,
        assets: &AssetQueryService,
        project_id: &str,
        shot_id: Option<&str>,
        requested_stage: &str,
    ) -> Result<CreationContext, ProductError> {
        let stage = stage(requested_stage)?;
        // Existing binding service validates the formal project before any Shot read/write.
        self.bindings
            .get(project_id)
            .await
            .map_err(ProductError::binding)?;
        let project = projects
            .get(project_id)
            .await
            .map_err(ProductError::internal)?
            .project;
        let shots = self.shots.list(project_id).await.map_err(shot_error)?;
        let selected = match shot_id {
            Some(id) => Some(shots.iter().find(|shot| shot.id == id).ok_or_else(|| {
                ProductError::new(
                    "PROJECT_SCOPE_VIOLATION",
                    "镜头不存在或不属于当前项目。",
                    None,
                )
            })?),
            None => None,
        };
        let selected_shot = selected.map(|shot| project_shot(shot, stage)).transpose()?;
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        if let Some(shot) = selected {
            // Linked tasks, not the global recent asset list, define candidate membership.
            for task_id in shot
                .generation_links
                .iter()
                .filter(|link| link.stage == stage.as_str())
                .filter_map(|link| link.task_id.as_deref())
            {
                for asset in assets
                    .list_by_task(project_id, task_id)
                    .await
                    .map_err(ProductError::internal)?
                {
                    if asset.asset_type == requested_stage && seen.insert(asset.id.clone()) {
                        candidates.push(candidate(
                            asset,
                            selected_shot
                                .as_ref()
                                .and_then(|shot| shot.selected_result_id.as_deref()),
                        ));
                    }
                }
            }
        }
        // Bounded thumbnails only; failures never erase valid candidate metadata.
        for item in candidates
            .iter_mut()
            .filter(|item| item.media_kind == "image")
            .take(12)
        {
            if let Ok(binary) = assets.read_thumbnail(project_id, &item.id).await {
                if binary.bytes.len() <= 256 * 1024 {
                    item.thumbnail_bytes = Some(binary.bytes);
                }
            }
        }
        let mut media_inputs: Vec<_> = assets
            .list_recent(project_id, 100)
            .await
            .map_err(ProductError::internal)?
            .into_iter()
            .map(|asset| candidate(asset, None))
            .collect();
        if let Some(shot) = selected {
            let ids = shot
                .reference_assets
                .iter()
                .map(|item| item.asset_id.as_str())
                .chain(shot.selected_image_asset_id.as_deref())
                .chain(shot.selected_video_asset_id.as_deref());
            for id in ids {
                if !media_inputs.iter().any(|item| item.id == id) {
                    if let Ok(asset) = assets.get(project_id, id).await {
                        media_inputs.push(candidate(asset, None));
                    }
                }
            }
        }
        Ok(CreationContext {
            project_id: project.id,
            project_name: project.name,
            stage: requested_stage.to_owned(),
            shots: shots.iter().map(summary).collect(),
            selected_shot,
            candidates,
            media_inputs,
            prompt_choices: Vec::new(),
        })
    }

    pub async fn project_prompt_choices(
        &self,
        context: &mut CreationContext,
        prompts: &crate::application::prompt_library_service::PromptLibraryService,
    ) -> Result<(), ProductError> {
        let page = prompts
            .list(&context.project_id, None, None, None, None, Some(20))
            .await
            .map_err(ProductError::internal)?;
        for entry in page.items {
            let entry = prompts
                .get(&context.project_id, &entry.id)
                .await
                .map_err(ProductError::internal)?;
            if let Some(version) = entry.versions.iter().max_by_key(|version| version.version) {
                context.prompt_choices.push(CreationPromptChoice {
                    name: entry.name,
                    version: version.version,
                    text: version.text.clone(),
                });
            }
        }
        Ok(())
    }

    pub async fn create_shot(&self, project_id: &str) -> Result<CreationShotSummary, ProductError> {
        self.bindings
            .get(project_id)
            .await
            .map_err(ProductError::binding)?;
        self.shots
            .create(project_id)
            .await
            .map(|shot| summary(&shot))
            .map_err(shot_error)
    }

    pub async fn update_shot(
        &self,
        project_id: &str,
        request: CreationShotUpdate,
    ) -> Result<CreationShotSummary, ProductError> {
        self.bindings
            .get(project_id)
            .await
            .map_err(ProductError::binding)?;
        let previous = self
            .shots
            .get(project_id, &request.shot_id)
            .await
            .map_err(shot_error)?;
        // Metadata edits never overwrite stage-owned prompts/configs or their provenance.
        self.shots
            .update(ShotUpdateRequest {
                project_id: project_id.to_owned(),
                shot_id: request.shot_id,
                name: request.name,
                prompt_text: previous.prompt_text,
                prompt_entry_id: previous.prompt_entry_id,
                prompt_version_id: previous.prompt_version_id,
            })
            .await
            .map(|shot| summary(&shot))
            .map_err(shot_error)
    }

    pub async fn delete_shot(&self, project_id: &str, shot_id: &str) -> Result<(), ProductError> {
        self.bindings
            .get(project_id)
            .await
            .map_err(ProductError::binding)?;
        self.shots
            .delete(project_id, shot_id)
            .await
            .map_err(shot_error)
    }

    pub async fn references_set(
        &self,
        project_id: &str,
        shot_id: &str,
        requested_stage: &str,
        asset_ids: Vec<String>,
    ) -> Result<CreationShot, ProductError> {
        let stage = stage(requested_stage)?;
        self.bindings
            .get(project_id)
            .await
            .map_err(ProductError::binding)?;
        // Validate scope before delegating: no write-before-lookup on a foreign Shot.
        self.shots
            .get(project_id, shot_id)
            .await
            .map_err(shot_error)?;
        let shot = self
            .shots
            .replace_references(project_id, shot_id, stage, asset_ids)
            .await
            .map_err(shot_error)?;
        project_shot(&shot, stage)
    }

    pub async fn select_result(
        &self,
        assets: &AssetQueryService,
        project_id: &str,
        shot_id: &str,
        requested_stage: &str,
        asset_id: &str,
    ) -> Result<CreationShot, ProductError> {
        let stage = stage(requested_stage)?;
        self.bindings
            .get(project_id)
            .await
            .map_err(ProductError::binding)?;
        self.shots
            .get(project_id, shot_id)
            .await
            .map_err(shot_error)?;
        assets
            .get(project_id, asset_id)
            .await
            .map_err(|error| match error {
                AssetQueryError::NotFound(_) => ProductError::new(
                    "PROJECT_SCOPE_VIOLATION",
                    "素材不存在或不属于当前项目。",
                    None,
                ),
                other => ProductError::internal(other),
            })?;
        // Ordinary candidates MUST be outputs of this Shot/stage; no arbitrary asset adoption.
        let shot = self
            .shots
            .select_result(project_id, shot_id, stage, asset_id, true)
            .await
            .map_err(shot_error)?;
        project_shot(&shot, stage)
    }
}
