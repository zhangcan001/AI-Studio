use crate::application::generation_input_preparer::{
    GenerationInputPreparer, GenerationInputValue,
};
use crate::application::generation_service::NewGenerationAdmission;
use crate::application::ports::{
    Clock, GenerationDefinitionRepository, ShotRepository, ShotVideoInputAsset,
    ShotVideoInputRepository, ShotVideoInputScope, ShotVideoInputSet, ShotVideoInputToken,
};
use crate::compiler::RecipeParser;
use crate::domain::{AssetId, InputDefinition, OutputType, Recipe};
use std::{collections::BTreeMap, fmt, sync::Arc};

#[derive(Debug)]
pub enum ShotVideoInputError {
    Invalid(String),
    Conflict,
    Combination,
}
impl fmt::Display for ShotVideoInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "{message}"),
            Self::Conflict => write!(f, "SHOT_VIDEO_INPUT_CONFLICT: 输入已修改，请刷新后重新保存"),
            Self::Combination => write!(
                f,
                "SHOT_VIDEO_INPUT_COMBINATION_INVALID: 参考图最多 9 个、参考视频最多 3 个、参考音频最多 3 个，合计不超过 12 个，且不能只有音频"
            ),
        }
    }
}
impl std::error::Error for ShotVideoInputError {}
fn invalid(error: impl fmt::Display) -> ShotVideoInputError {
    ShotVideoInputError::Invalid(error.to_string())
}

/// Coordinates the existing repositories and preflight. The persistent input
/// set is the media authority; result selections/legacy references are not inputs.
pub struct ShotVideoInputService {
    assets: Arc<dyn crate::application::ports::AssetRepository>,
    repository: Arc<dyn ShotVideoInputRepository>,
    shots: Arc<dyn ShotRepository>,
    definitions: Arc<dyn GenerationDefinitionRepository>,
    admission: Arc<dyn NewGenerationAdmission>,
    preparer: Arc<GenerationInputPreparer>,
    clock: Arc<dyn Clock>,
}

impl ShotVideoInputService {
    pub fn new(
        assets: Arc<dyn crate::application::ports::AssetRepository>,
        repository: Arc<dyn ShotVideoInputRepository>,
        shots: Arc<dyn ShotRepository>,
        definitions: Arc<dyn GenerationDefinitionRepository>,
        admission: Arc<dyn NewGenerationAdmission>,
        preparer: Arc<GenerationInputPreparer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            assets,
            repository,
            shots,
            definitions,
            admission,
            preparer,
            clock,
        }
    }

    pub async fn get(
        &self,
        scope: &ShotVideoInputScope,
    ) -> Result<Option<ShotVideoInputSet>, ShotVideoInputError> {
        self.ensure_shot(scope).await?;
        // History/stale recipe selections remain readable, not product admission.
        self.repository.find(scope).await.map_err(invalid)
    }

    pub async fn available_assets(
        &self,
        project_id: &str,
        inputs: &[ShotVideoInputAsset],
    ) -> Result<Vec<crate::domain::Asset>, ShotVideoInputError> {
        let mut assets = self
            .assets
            .list_recent(project_id, 100)
            .await
            .map_err(invalid)?;
        for input in inputs {
            let id = AssetId::parse(&input.asset_id).map_err(invalid)?;
            if !assets.iter().any(|a| a.id == id) {
                if let Some(asset) = self.assets.find_by_id(&id).await.map_err(invalid)? {
                    assets.push(asset);
                }
            }
        }
        let mut eligible = Vec::new();
        for asset in assets {
            if asset.project_id != project_id {
                continue;
            }
            if asset.asset_type == crate::domain::AssetType::Image {
                if asset.source_task_id.is_some()
                    || asset.category != crate::domain::SOURCE_IMAGE_CATEGORY
                    || self
                        .assets
                        .find_external_import(project_id, &asset.id)
                        .await
                        .map_err(invalid)?
                        .is_none()
                {
                    continue;
                }
            } else if !matches!(
                asset.asset_type,
                crate::domain::AssetType::Video | crate::domain::AssetType::Audio
            ) {
                continue;
            }
            eligible.push(asset);
        }
        Ok(eligible)
    }

    pub async fn save(
        &self,
        scope: &ShotVideoInputScope,
        expected: Option<&ShotVideoInputToken>,
        inputs: &[ShotVideoInputAsset],
    ) -> Result<ShotVideoInputSet, ShotVideoInputError> {
        self.ensure_shot(scope).await?;
        if !self
            .admission
            .is_available_for_new_generation(&scope.workflow_version_id, &scope.recipe_id)
            .await
            .map_err(invalid)?
        {
            return Err(invalid(
                crate::application::minimax_video_product_policy::REQUIRED,
            ));
        }
        let definition = self
            .definitions
            .find(&scope.workflow_version_id, &scope.recipe_id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("生成器不可用"))?;
        let recipe = RecipeParser::parse(&definition.recipe_yaml).map_err(invalid)?;
        if !recipe
            .outputs
            .iter()
            .any(|o| o.output_type == OutputType::Video)
        {
            return Err(invalid("正式输入需要视频 Recipe"));
        }
        let values = media_values(&recipe, inputs)?;
        // Incomplete drafts (missing first_frame) may save. Combinations that
        // cannot generate — over 9/3/3/12 or audio-only — may not.
        reject_ungeneratable_combination(&values)?;
        self.preparer
            .validate_product_assets(
                &scope.project_id,
                definition.workflow_json.clone(),
                &recipe,
                &values,
            )
            .await
            .map_err(invalid)?;
        // Save may be incomplete (e.g. first frame before last). Readiness/compile
        // owns required slots, overall reference count and audio-only denial.
        self.repository
            .replace(scope, expected, inputs, self.clock.now())
            .await
            .map_err(invalid)?
            .ok_or(ShotVideoInputError::Conflict)
    }

    pub async fn values(
        &self,
        scope: &ShotVideoInputScope,
        recipe: &Recipe,
    ) -> Result<BTreeMap<String, GenerationInputValue>, ShotVideoInputError> {
        Ok(self.load_validated(scope, recipe).await?.1)
    }

    async fn load_validated(
        &self,
        scope: &ShotVideoInputScope,
        recipe: &Recipe,
    ) -> Result<
        (
            Option<ShotVideoInputSet>,
            BTreeMap<String, GenerationInputValue>,
        ),
        ShotVideoInputError,
    > {
        let set = self.get(scope).await?;
        let values = match &set {
            Some(set) => media_values(recipe, &set.inputs)?,
            None => BTreeMap::new(),
        };
        if !self
            .admission
            .is_available_for_new_generation(&scope.workflow_version_id, &scope.recipe_id)
            .await
            .map_err(invalid)?
        {
            return Err(invalid(
                crate::application::minimax_video_product_policy::REQUIRED,
            ));
        }
        let definition = self
            .definitions
            .find(&scope.workflow_version_id, &scope.recipe_id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("生成器不可用"))?;
        self.preparer
            .validate_product_assets(&scope.project_id, definition.workflow_json, recipe, &values)
            .await
            .map_err(invalid)?;
        Ok((set, values))
    }

    pub async fn validate_generation_assets(
        &self,
        project_id: &str,
        workflow: serde_json::Value,
        recipe: &Recipe,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), ShotVideoInputError> {
        self.preparer
            .validate_product_assets(project_id, workflow, recipe, values)
            .await
            .map_err(invalid)
    }

    pub async fn context_input(
        &self,
        scope: &ShotVideoInputScope,
    ) -> Result<crate::domain::shot_context::ResolvedVideoInputSet, ShotVideoInputError> {
        use crate::domain::shot_context::{ResolvedVideoInputAsset, ResolvedVideoInputSet};
        let definition = self
            .definitions
            .find(&scope.workflow_version_id, &scope.recipe_id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("生成器不可用"))?;
        let recipe = RecipeParser::parse(&definition.recipe_yaml).map_err(invalid)?;
        let (set, _) = self.load_validated(scope, &recipe).await?;
        let mut inputs = Vec::new();
        if let Some(set) = &set {
            for input in &set.inputs {
                let asset = self
                    .assets
                    .find_by_id(&AssetId::parse(&input.asset_id).map_err(invalid)?)
                    .await
                    .map_err(invalid)?
                    .ok_or_else(|| invalid("正式输入素材不存在"))?;
                inputs.push(ResolvedVideoInputAsset {
                    input_key: input.input_key.clone(),
                    ordinal: input.ordinal,
                    asset_id: input.asset_id.clone(),
                    media_type: asset.asset_type.as_str().into(),
                    sha256: asset.sha256,
                    duration_ms: asset.duration_ms,
                });
            }
        }
        Ok(ResolvedVideoInputSet {
            workflow_version_id: scope.workflow_version_id.clone(),
            recipe_id: scope.recipe_id.clone(),
            instance_id: set.as_ref().map(|s| s.token.instance_id.clone()),
            revision: set.as_ref().map(|s| s.token.revision),
            inputs,
        })
    }

    async fn ensure_shot(&self, scope: &ShotVideoInputScope) -> Result<(), ShotVideoInputError> {
        if [
            &scope.project_id,
            &scope.shot_id,
            &scope.workflow_version_id,
            &scope.recipe_id,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        {
            return Err(invalid("正式输入作用域不能为空"));
        }
        self.shots
            .find(&scope.project_id, &scope.shot_id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("镜头不存在或不属于当前项目"))?;
        Ok(())
    }
}

/// One exact-recipe conversion for both single-shot and batch preparation.
/// Never zip first/last by position, or merge references across media types.
pub(crate) fn media_values(
    recipe: &Recipe,
    inputs: &[ShotVideoInputAsset],
) -> Result<BTreeMap<String, GenerationInputValue>, ShotVideoInputError> {
    let mut slots = BTreeMap::<&str, Vec<&ShotVideoInputAsset>>::new();
    for input in inputs {
        slots.entry(&input.input_key).or_default().push(input);
    }
    let mut values = BTreeMap::new();
    for (key, mut inputs) in slots {
        inputs.sort_by_key(|input| input.ordinal);
        if inputs
            .iter()
            .enumerate()
            .any(|(i, input)| input.ordinal != i as i64)
        {
            return Err(invalid("正式输入槽顺序无效"));
        }
        let ids = inputs
            .iter()
            .map(|i| AssetId::parse(&i.asset_id).map_err(invalid))
            .collect::<Result<Vec<_>, _>>()?;
        let value = match (key, recipe.inputs.get(key)) {
            ("first_frame" | "last_frame", Some(InputDefinition::Image { .. }))
                if ids.len() == 1 =>
            {
                GenerationInputValue::ImageAsset(ids[0].clone())
            }
            ("reference_images", Some(InputDefinition::Images { max_items, .. }))
                if ids.len() <= *max_items =>
            {
                GenerationInputValue::ImageAssets(ids)
            }
            ("reference_videos", Some(InputDefinition::Videos { max_items, .. }))
                if ids.len() <= *max_items =>
            {
                GenerationInputValue::VideoAssets(ids)
            }
            ("reference_audios", Some(InputDefinition::Audios { max_items, .. }))
                if ids.len() <= *max_items =>
            {
                GenerationInputValue::AudioAssets(ids)
            }
            _ => {
                return Err(invalid(format!(
                    "正式输入 {key} 与当前 Recipe 不匹配；需要重新选择素材"
                )))
            }
        };
        values.insert(key.into(), value);
    }
    Ok(values)
}

pub(crate) fn merge_media_inputs(
    values: &mut BTreeMap<String, GenerationInputValue>,
    media: BTreeMap<String, GenerationInputValue>,
) {
    values.retain(|_, value| {
        matches!(
            value,
            GenerationInputValue::Text(_)
                | GenerationInputValue::Integer(_)
                | GenerationInputValue::Number(_)
                | GenerationInputValue::Seed(_)
        )
    });
    values.extend(media);
}

/// Save allows a partial draft. It rejects only combinations prepare would
/// refuse even after the missing required slots were filled: per-slot caps,
/// the combined cap of 12, and audio-only REF2VA.
pub(crate) fn reject_ungeneratable_combination(
    values: &BTreeMap<String, GenerationInputValue>,
) -> Result<(), ShotVideoInputError> {
    let count = |key: &str| match values.get(key) {
        Some(
            GenerationInputValue::ImageAssets(ids)
            | GenerationInputValue::VideoAssets(ids)
            | GenerationInputValue::AudioAssets(ids),
        ) => ids.len(),
        Some(
            GenerationInputValue::ImageAsset(_)
            | GenerationInputValue::VideoAsset(_)
            | GenerationInputValue::AudioAsset(_),
        ) => 1,
        _ => 0,
    };
    let (images, videos, audios) = (
        count("reference_images"),
        count("reference_videos"),
        count("reference_audios"),
    );
    if images > 9
        || videos > 3
        || audios > 3
        || images + videos + audios > 12
        || (audios > 0 && images + videos == 0)
    {
        return Err(ShotVideoInputError::Combination);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{media_values, reject_ungeneratable_combination};
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::application::ports::ShotVideoInputAsset;
    use crate::domain::{InputDefinition, Recipe};
    use std::collections::BTreeMap;

    fn recipe() -> Recipe {
        Recipe {
            schema_version: 1,
            id: "rcp_ref".into(),
            name: "REF".into(),
            workflow: crate::domain::WorkflowRef {
                file: "workflow_api.json".into(),
            },
            inputs: BTreeMap::from([
                (
                    "first_frame".into(),
                    InputDefinition::Image {
                        label: "First".into(),
                        required: false,
                    },
                ),
                (
                    "reference_images".into(),
                    InputDefinition::Images {
                        label: "Images".into(),
                        required: false,
                        min_items: 0,
                        max_items: 9,
                    },
                ),
                (
                    "reference_videos".into(),
                    InputDefinition::Videos {
                        label: "Videos".into(),
                        required: false,
                        min_items: 0,
                        max_items: 3,
                    },
                ),
                (
                    "reference_audios".into(),
                    InputDefinition::Audios {
                        label: "Audios".into(),
                        required: false,
                        min_items: 0,
                        max_items: 3,
                    },
                ),
            ]),
            bindings: Vec::new(),
            outputs: Vec::new(),
        }
    }

    fn asset(key: &str, ordinal: i64, id: &str) -> ShotVideoInputAsset {
        ShotVideoInputAsset {
            input_key: key.into(),
            ordinal,
            asset_id: id.into(),
        }
    }

    #[test]
    fn save_allows_incomplete_draft_but_rejects_ungeneratable_combinations() {
        let recipe = recipe();
        let empty = media_values(&recipe, &[]).unwrap();
        assert!(reject_ungeneratable_combination(&empty).is_ok());
        let one_image = media_values(&recipe, &[asset("reference_images", 0, "ast_one")]).unwrap();
        assert!(reject_ungeneratable_combination(&one_image).is_ok());
        let audio_only =
            media_values(&recipe, &[asset("reference_audios", 0, "ast_audio")]).unwrap();
        assert!(matches!(
            reject_ungeneratable_combination(&audio_only),
            Err(super::ShotVideoInputError::Combination)
        ));
        let mut too_many = Vec::new();
        for ordinal in 0..9 {
            too_many.push(asset(
                "reference_images",
                ordinal,
                &format!("ast_img{ordinal}"),
            ));
        }
        for ordinal in 0..3 {
            too_many.push(asset(
                "reference_videos",
                ordinal,
                &format!("ast_vid{ordinal}"),
            ));
        }
        too_many.push(asset("reference_audios", 0, "ast_aud0"));
        let values = media_values(&recipe, &too_many).unwrap();
        assert!(matches!(
            reject_ungeneratable_combination(&values),
            Err(super::ShotVideoInputError::Combination)
        ));
        let mut images = Vec::new();
        for ordinal in 0..10 {
            images.push(asset(
                "reference_images",
                ordinal,
                &format!("ast_over{ordinal}"),
            ));
        }
        assert!(media_values(&recipe, &images).is_err());
    }
}
