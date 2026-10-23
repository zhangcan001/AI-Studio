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
    story_entities: Option<Arc<dyn crate::application::ports::StoryEntityRepository>>,
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
            story_entities: None,
            assets,
            repository,
            shots,
            definitions,
            admission,
            preparer,
            clock,
        }
    }

    pub fn with_story_entities(
        mut self,
        repository: Arc<dyn crate::application::ports::StoryEntityRepository>,
    ) -> Self {
        self.story_entities = Some(repository);
        self
    }

    /// Resolve explicit @names against the exact Recipe's saved media, read-only.
    /// Associations never substitute first/last frames or create a second input store.
    /// The returned prompt and ordered media are frozen by the existing Queue together.
    pub async fn generation_values(
        &self,
        scope: &ShotVideoInputScope,
        recipe: &Recipe,
        prompt: Option<&str>,
    ) -> Result<BTreeMap<String, GenerationInputValue>, ShotVideoInputError> {
        let mut values = self.values(scope, recipe).await?;
        if let Some(prompt) = prompt {
            values.insert("prompt".into(), GenerationInputValue::Text(prompt.into()));
        }
        self.compile_prompt(&scope.project_id, Some(&scope.shot_id), recipe, &mut values)
            .await?;
        Ok(values)
    }

    /// Also used for formal multi-Shot preparation, against its already-resolved media.
    pub async fn compile_prompt(
        &self,
        project_id: &str,
        shot_id: Option<&str>,
        recipe: &Recipe,
        values: &mut BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), ShotVideoInputError> {
        let Some(GenerationInputValue::Text(prompt)) = values.get("prompt") else {
            return Ok(());
        };
        if !prompt.contains('@') {
            return Ok(());
        }
        if !matches!(
            recipe.inputs.get("reference_images"),
            Some(InputDefinition::Images { .. })
        ) {
            return Err(invalid("ENTITY_REFERENCE_MODE: @角色/场景需要混合参考 REF2VA；不会替换已有首尾帧，请明确选择兼容生成器"));
        }
        let repository = self.story_entities.as_ref().ok_or_else(|| {
            invalid("ENTITY_REFERENCE_UNAVAILABLE: 角色与场景解析不可用，请刷新后重试")
        })?;
        let entities = repository.list(project_id, None).await.map_err(invalid)?;
        let usage = match shot_id {
            Some(shot_id) => repository
                .composition_usage_for_shot(project_id, shot_id)
                .await
                .map_err(invalid)?,
            None => Default::default(),
        };
        let mut references = match values.get("reference_images") {
            Some(GenerationInputValue::ImageAssets(ids)) => ids.clone(),
            _ => Vec::new(),
        };
        let reordered = order_composition_references(&mut references, &entities, &usage);
        let compiled =
            compile_entity_prompt_with_usage(prompt, project_id, &entities, &references, &usage)?;
        values.insert("prompt".into(), GenerationInputValue::Text(compiled));
        if reordered {
            values.insert(
                "reference_images".into(),
                GenerationInputValue::ImageAssets(references),
            );
        }
        Ok(())
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
                    .map_err(invalid)?;
                // Missing assets keep the formal slot with empty sha so readiness
                // stays on the formal-input path instead of legacy keyframe semantics.
                match asset {
                    Some(asset) => inputs.push(ResolvedVideoInputAsset {
                        input_key: input.input_key.clone(),
                        ordinal: input.ordinal,
                        asset_id: input.asset_id.clone(),
                        media_type: asset.asset_type.as_str().into(),
                        sha256: asset.sha256,
                        duration_ms: asset.duration_ms,
                    }),
                    None => inputs.push(ResolvedVideoInputAsset {
                        input_key: input.input_key.clone(),
                        ordinal: input.ordinal,
                        asset_id: input.asset_id.clone(),
                        media_type: String::new(),
                        sha256: String::new(),
                        duration_ms: None,
                    }),
                }
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

/// Longest explicit name wins; duplicate names fail closed instead of guessing.
/// Keep all non-reference text unchanged and index only the actual reference_images.
/// Entity names are UI aliases, not visual descriptions; role-specific retention
/// instructions anchor the model to the pictures rather than the alias wording.
pub(crate) fn match_entity_mention<'a>(
    rest: &str,
    project_id: &str,
    entities: &'a [crate::domain::story_entity::StoryEntity],
) -> Result<&'a crate::domain::story_entity::StoryEntity, ShotVideoInputError> {
    let mut matches: Vec<_> = entities
        .iter()
        .filter(|entity| {
            entity.project_id == project_id
                && !entity.name.is_empty()
                && rest.starts_with(&entity.name)
                && !(entity
                    .name
                    .ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
                    && rest[entity.name.len()..]
                        .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_'))
        })
        .collect();
    matches.sort_by_key(|entity| std::cmp::Reverse(entity.name.len()));
    let entity = matches.first().ok_or_else(|| {
        invalid(
            "ENTITY_REFERENCE_UNKNOWN: 存在无法识别的 @引用，请使用当前项目唯一的角色或场景名称",
        )
    })?;
    if matches
        .iter()
        .filter(|item| item.name == entity.name)
        .count()
        != 1
    {
        return Err(invalid(format!(
            "ENTITY_REFERENCE_AMBIGUOUS: @{} 存在同名实体，请先改为唯一名称",
            entity.name
        )));
    }
    Ok(entity)
}

pub(crate) fn explicit_entity_mentions<'a>(
    prompt: &str,
    project_id: &str,
    entities: &'a [crate::domain::story_entity::StoryEntity],
) -> Result<Vec<&'a crate::domain::story_entity::StoryEntity>, ShotVideoInputError> {
    let mut rest = prompt;
    let mut found = Vec::new();
    while let Some(at) = rest.find('@') {
        rest = &rest[at + 1..];
        let entity = match_entity_mention(rest, project_id, entities)?;
        if !found
            .iter()
            .any(|e: &&crate::domain::story_entity::StoryEntity| e.id == entity.id)
        {
            found.push(entity);
        }
        rest = &rest[entity.name.len()..];
    }
    Ok(found)
}

fn entity_cover(entity: &crate::domain::story_entity::StoryEntity) -> Option<&String> {
    entity
        .cover_asset_id
        .as_ref()
        .or_else(|| entity.image_asset_ids.first())
}

/// 构图用途 ordering: among the composition pictures already in the formal
/// references, the opening composition takes the first composition slot and the
/// ending composition the last one. Other pictures keep their slots. Returns
/// whether anything moved; without 构图用途 nothing changes.
pub(crate) fn order_composition_references(
    references: &mut [AssetId],
    entities: &[crate::domain::story_entity::StoryEntity],
    usage: &crate::domain::story_entity::ShotCompositionUsage,
) -> bool {
    use crate::domain::story_entity::StoryEntityKind;
    if usage.opening_entity_id.is_none() && usage.ending_entity_id.is_none() {
        return false;
    }
    let cover_of = |id: &Option<String>| {
        id.as_ref()
            .and_then(|id| entities.iter().find(|e| &e.id == id))
            .and_then(entity_cover)
            .cloned()
    };
    let (opening, ending) = (
        cover_of(&usage.opening_entity_id),
        cover_of(&usage.ending_entity_id),
    );
    let composition_covers: Vec<&String> = entities
        .iter()
        .filter(|e| e.kind == StoryEntityKind::Composition)
        .filter_map(entity_cover)
        .collect();
    let slots: Vec<usize> = references
        .iter()
        .enumerate()
        .filter(|(_, id)| composition_covers.iter().any(|c| c.as_str() == id.as_str()))
        .map(|(index, _)| index)
        .collect();
    let mut ordered: Vec<AssetId> = slots.iter().map(|&i| references[i].clone()).collect();
    let before = ordered.clone();
    if let Some(cover) = &opening {
        if let Some(at) = ordered.iter().position(|id| id.as_str() == cover) {
            let item = ordered.remove(at);
            ordered.insert(0, item);
        }
    }
    if let Some(cover) = &ending {
        if let Some(at) = ordered.iter().position(|id| id.as_str() == cover) {
            let item = ordered.remove(at);
            ordered.push(item);
        }
    }
    if ordered == before {
        return false;
    }
    for (slot, id) in slots.into_iter().zip(ordered) {
        references[slot] = id;
    }
    true
}

#[cfg(test)]
fn compile_entity_prompt(
    prompt: &str,
    project_id: &str,
    entities: &[crate::domain::story_entity::StoryEntity],
    references: &[AssetId],
) -> Result<String, ShotVideoInputError> {
    compile_entity_prompt_with_usage(
        prompt,
        project_id,
        entities,
        references,
        &Default::default(),
    )
}

fn compile_entity_prompt_with_usage(
    prompt: &str,
    project_id: &str,
    entities: &[crate::domain::story_entity::StoryEntity],
    references: &[AssetId],
    usage: &crate::domain::story_entity::ShotCompositionUsage,
) -> Result<String, ShotVideoInputError> {
    use crate::domain::story_entity::StoryEntityKind;
    let mut has_usage = false;

    let mut rest = prompt;
    let mut output = String::new();
    let mut subjects = Vec::new();
    let mut definitions = Vec::new();
    let mut retention = Vec::new();
    while let Some(at) = rest.find('@') {
        output.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let entity = match_entity_mention(rest, project_id, entities)?;
        let asset_id = entity
            .cover_asset_id
            .as_ref()
            .or_else(|| entity.image_asset_ids.first())
            .ok_or_else(|| {
                invalid(format!(
                    "ENTITY_REFERENCE_MISSING_IMAGE: @{} 尚未关联图片",
                    entity.name
                ))
            })?;
        let ordinal = references.iter().position(|id| id.as_str() == asset_id)
            .ok_or_else(|| invalid(format!("ENTITY_REFERENCE_NOT_APPLIED: @{} 的图片尚未加入当前 Recipe 的正式参考图，请点击应用角色与场景参考图", entity.name)))?;
        let subject = match subjects.iter().position(|id| *id == &entity.id) {
            Some(index) => index + 1,
            None => {
                subjects.push(&entity.id);
                let subject = subjects.len();
                let (role, features) = match entity.kind {
                    StoryEntityKind::Character => (
                        "character",
                        "facial features, hairstyle, clothing and overall appearance",
                    ),
                    StoryEntityKind::Scene => (
                        "environment",
                        "spatial layout, architecture, terrain and environmental structure",
                    ),
                    StoryEntityKind::Prop => ("object", "shape, materials and distinctive details"),
                    StoryEntityKind::Composition => (
                        "composition",
                        "framing, camera angle, subject placement and overall layout",
                    ),
                };
                definitions.push(format!(
                    "<Subject {subject}> is the {role} shown in <Picture {}>.",
                    ordinal + 1
                ));
                let opening = usage.opening_entity_id.as_deref() == Some(entity.id.as_str());
                let ending = usage.ending_entity_id.as_deref() == Some(entity.id.as_str());
                has_usage |= entity.kind == StoryEntityKind::Composition && (opening || ending);
                retention.push(if entity.kind == StoryEntityKind::Composition && opening {
                    format!(
                        "<Subject {subject}>: opening_composition - <Picture {}> defines ONLY the opening frame of the shot: camera framing, angle, subject positions and object placement at the start; it does not govern the rest of the shot or the appearance of its people or setting.",
                        ordinal + 1
                    )
                } else if entity.kind == StoryEntityKind::Composition && ending {
                    format!(
                        "<Subject {subject}>: ending_composition - <Picture {}> defines ONLY the final frame of the shot: camera framing, angle, subject positions and object placement at the end; the middle of the shot transitions to it as described in detailed_description, not the appearance of its people or setting.",
                        ordinal + 1
                    )
                } else if entity.kind == StoryEntityKind::Composition {
                    format!(
                        "<Subject {subject}>: composition_reference - <Picture {}> defines the shot composition; follow its {features}, not the appearance of its people or setting, except changes explicitly requested in detailed_description.",
                        ordinal + 1
                    )
                } else {
                    format!(
                        "<Subject {subject}>: fully_preserved - retain {features} from <Picture {}>, except changes explicitly requested in detailed_description.",
                        ordinal + 1
                    )
                });
                subject
            }
        };
        output.push_str(&format!("<Subject {subject}>"));
        rest = &rest[entity.name.len()..];
    }
    output.push_str(rest);
    if subjects.is_empty() {
        return Ok(output);
    }
    // Ref2VA section order follows the official H3 prompt-writing convention.
    // Do not invent camera, motion, dialogue or music absent from the user text.
    let usage_summary = if has_usage {
        " The opening composition governs only the first frame and the ending composition only the last frame; the shot moves between them as described below."
    } else {
        ""
    };
    Ok(format!(
        "subject_definitions:\n{}\n\nsummary:\nUse the referenced subjects for the requested video. Pictures define appearance and environment; follow the actions and camera directions below.{usage_summary}\n\nretention_analysis:\n{}\nDo not replace the referenced environment with a generic setting or use incidental figures in an environment reference as the character reference.\n\ndetailed_description:\n{output}\n\noverall_soundscape:\nFollow any sound directions in detailed_description.\n\nnon_diegetic_music:\nFollow any music directions in detailed_description.",
        definitions.join("\n"), retention.join("\n")
    ))
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

    #[test]
    fn entity_prompt_retains_character_appearance_and_scene_structure_by_actual_picture() {
        use crate::domain::{
            story_entity::{StoryEntity, StoryEntityKind},
            AssetId,
        };
        let entity = |id: &str, name: &str, kind, image: &str| StoryEntity {
            id: id.into(),
            project_id: "prj_owned".into(),
            kind,
            name: name.into(),
            description: String::new(),
            cover_asset_id: Some(image.into()),
            image_asset_ids: vec![image.into()],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let entities = [
            entity("hero", "角色", StoryEntityKind::Character, "ast_hero"),
            entity("scene", "太空", StoryEntityKind::Scene, "ast_scene"),
            entity("unused", "未引用", StoryEntityKind::Character, "ast_unused"),
        ];
        let refs = ["ast_scene", "ast_hero"].map(|id| AssetId::parse(id).unwrap());
        let text = super::compile_entity_prompt(
            "@角色在@太空中飞翔，镜头推进；@角色说：\"向前！\"，5秒",
            "prj_owned",
            &entities,
            &refs,
        )
        .unwrap();
        assert!(text.contains("<Subject 1> is the character shown in <Picture 2>."));
        assert!(text.contains("<Subject 2> is the environment shown in <Picture 1>."));
        assert!(text.contains(
            "facial features, hairstyle, clothing and overall appearance from <Picture 2>"
        ));
        assert!(text.contains(
            "spatial layout, architecture, terrain and environmental structure from <Picture 1>"
        ));
        assert!(text.contains("detailed_description:\n<Subject 1>在<Subject 2>中飞翔，镜头推进；<Subject 1>说：\"向前！\"，5秒"));
        assert_eq!(text.matches("is the character shown").count(), 1);
        assert!(!text.contains("太空"));
        assert!(!text.contains("未引用"));
        assert!(!text.contains("<Picture 3>"));
        assert_eq!(
            super::compile_entity_prompt(&text, "prj_owned", &entities, &refs).unwrap(),
            text
        );
        let sections = [
            "subject_definitions:",
            "summary:",
            "retention_analysis:",
            "detailed_description:",
            "overall_soundscape:",
            "non_diegetic_music:",
        ];
        let positions: Vec<_> = sections
            .iter()
            .map(|section| text.find(section).unwrap())
            .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn entity_prompt_uses_actual_order_and_rejects_unknown_ambiguous_or_unapplied_images() {
        use crate::domain::{story_entity::StoryEntity, AssetId};
        let entity = |name: &str, image: Option<&str>| StoryEntity {
            id: name.into(),
            project_id: "prj_owned".into(),
            kind: crate::domain::story_entity::StoryEntityKind::Character,
            name: name.into(),
            description: String::new(),
            cover_asset_id: None,
            image_asset_ids: image.map(|id| vec![id.into()]).unwrap_or_default(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let hero = entity("小明", Some("ast_hero"));
        let mother = entity("小明妈妈", Some("ast_mother"));
        let mut scene = entity("太空", Some("ast_scene"));
        scene.kind = crate::domain::story_entity::StoryEntityKind::Scene;
        let refs = ["ast_scene", "ast_mother", "ast_hero"].map(|id| AssetId::parse(id).unwrap());
        let compile = |text: &str, all: &[StoryEntity]| {
            super::compile_entity_prompt(text, "prj_owned", all, &refs)
        };
        let compiled = compile(
            "@小明妈妈在@太空中飞翔，5秒，@小明妈妈",
            &[hero.clone(), mother, scene],
        )
        .unwrap();
        assert!(compiled.contains("<Subject 1> is the character shown in <Picture 2>."));
        assert!(compiled.contains("<Subject 2> is the environment shown in <Picture 1>."));
        assert!(compiled
            .contains("detailed_description:\n<Subject 1>在<Subject 2>中飞翔，5秒，<Subject 1>"));
        assert_eq!(
            compile("小明飞翔，0.98mp", &[hero.clone()]).unwrap(),
            "小明飞翔，0.98mp"
        );
        for (text, all, code) in [
            ("@未知", vec![hero.clone()], "UNKNOWN"),
            ("@小明", vec![hero.clone(), hero.clone()], "AMBIGUOUS"),
            ("@无图", vec![entity("无图", None)], "MISSING_IMAGE"),
            (
                "@已换图",
                vec![entity("已换图", Some("ast_new"))],
                "NOT_APPLIED",
            ),
            (
                "@AnnaBelle",
                vec![entity("Anna", Some("ast_hero"))],
                "UNKNOWN",
            ),
        ] {
            assert!(compile(text, &all).unwrap_err().to_string().contains(code));
        }
        let mut foreign = hero;
        foreign.project_id = "prj_foreign".into();
        assert!(compile("@小明", &[foreign])
            .unwrap_err()
            .to_string()
            .contains("UNKNOWN"));
    }

    #[test]
    fn entity_prompt_keeps_roles_independent_when_sharing_a_picture() {
        use crate::domain::{
            story_entity::{StoryEntity, StoryEntityKind},
            AssetId,
        };
        let entity = |id: &str, kind| StoryEntity {
            id: id.into(),
            project_id: "prj_owned".into(),
            kind,
            name: id.into(),
            description: String::new(),
            cover_asset_id: Some("ast_shared".into()),
            image_asset_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let entities = [
            entity("人物", StoryEntityKind::Character),
            entity("场景", StoryEntityKind::Scene),
            entity("道具", StoryEntityKind::Prop),
            entity("剧情01", StoryEntityKind::Composition),
        ];
        let refs = [AssetId::parse("ast_shared").unwrap()];
        for (name, role, features) in [
            ("人物", "character", "facial features, hairstyle, clothing"),
            (
                "场景",
                "environment",
                "spatial layout, architecture, terrain",
            ),
            ("道具", "object", "shape, materials and distinctive details"),
            (
                "剧情01",
                "composition",
                "framing, camera angle, subject placement and overall layout",
            ),
        ] {
            let compiled = super::compile_entity_prompt(
                &format!("@{name}，按要求变为红色"),
                "prj_owned",
                &entities,
                &refs,
            )
            .unwrap();
            assert!(compiled.contains(&format!("<Subject 1> is the {role} shown in <Picture 1>.")));
            assert!(compiled.contains(features));
            assert!(
                compiled.contains("except changes explicitly requested in detailed_description")
            );
            assert!(compiled.contains("detailed_description:\n<Subject 1>，按要求变为红色"));
            assert!(!compiled.contains("<Subject 2>"));
        }
        let compiled =
            super::compile_entity_prompt("@人物走入@场景拿起@道具", "prj_owned", &entities, &refs)
                .unwrap();
        for (subject, role) in [(1, "character"), (2, "environment"), (3, "object")] {
            assert!(compiled.contains(&format!(
                "<Subject {subject}> is the {role} shown in <Picture 1>."
            )));
        }
        assert!(
            compiled.contains("detailed_description:\n<Subject 1>走入<Subject 2>拿起<Subject 3>")
        );
        assert_eq!(
            super::compile_entity_prompt("镜头推进，5秒", "prj_owned", &entities, &refs).unwrap(),
            "镜头推进，5秒"
        );
        let compiled =
            super::compile_entity_prompt("按@剧情01构图", "prj_owned", &entities, &refs).unwrap();
        assert!(compiled.contains(
            "<Subject 1>: composition_reference - <Picture 1> defines the shot composition"
        ));
        assert!(!compiled.contains("fully_preserved"));
    }

    #[test]
    fn composition_usage_marks_opening_and_ending_and_orders_their_pictures() {
        use crate::domain::{
            story_entity::{ShotCompositionUsage, StoryEntity, StoryEntityKind},
            AssetId,
        };
        let entity = |id: &str, kind, cover: &str| StoryEntity {
            id: id.into(),
            project_id: "prj_owned".into(),
            kind,
            name: id.into(),
            description: String::new(),
            cover_asset_id: Some(cover.into()),
            image_asset_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let entities = [
            entity("开场", StoryEntityKind::Composition, "ast_open"),
            entity("沈簿", StoryEntityKind::Character, "ast_hero"),
            entity("结尾", StoryEntityKind::Composition, "ast_end"),
            entity("中段", StoryEntityKind::Composition, "ast_mid"),
        ];
        let id = |v: &str| AssetId::parse(v).unwrap();
        let text = "开场按 @开场 的构图，@沈簿 走过 @中段，最后停在 @结尾 的构图";
        let usage = ShotCompositionUsage {
            opening_entity_id: Some("开场".into()),
            ending_entity_id: Some("结尾".into()),
        };
        // Without 构图用途 nothing moves and the output is the historical one.
        let mut refs = vec![id("ast_end"), id("ast_hero"), id("ast_open"), id("ast_mid")];
        assert!(!super::order_composition_references(
            &mut refs,
            &entities,
            &Default::default()
        ));
        assert_eq!(
            super::compile_entity_prompt_with_usage(
                text,
                "prj_owned",
                &entities,
                &refs,
                &Default::default()
            )
            .unwrap(),
            super::compile_entity_prompt(text, "prj_owned", &entities, &refs).unwrap()
        );
        // Opening takes the first composition slot, ending the last; the character keeps its slot.
        assert!(super::order_composition_references(
            &mut refs, &entities, &usage
        ));
        assert_eq!(
            refs,
            vec![id("ast_open"), id("ast_hero"), id("ast_mid"), id("ast_end")]
        );
        let compiled =
            super::compile_entity_prompt_with_usage(text, "prj_owned", &entities, &refs, &usage)
                .unwrap();
        assert!(compiled.contains(
            "<Subject 1>: opening_composition - <Picture 1> defines ONLY the opening frame"
        ));
        assert!(compiled.contains(
            "<Subject 4>: ending_composition - <Picture 4> defines ONLY the final frame"
        ));
        assert!(compiled.contains(
            "<Subject 3>: composition_reference - <Picture 3> defines the shot composition"
        ));
        assert!(compiled.contains("the ending composition only the last frame"));
        // Only an ending: the opening slot is untouched.
        let mut refs = vec![id("ast_end"), id("ast_open")];
        let ending = ShotCompositionUsage {
            opening_entity_id: None,
            ending_entity_id: Some("结尾".into()),
        };
        assert!(super::order_composition_references(
            &mut refs, &entities, &ending
        ));
        assert_eq!(refs, vec![id("ast_open"), id("ast_end")]);
        let compiled = super::compile_entity_prompt_with_usage(
            "@开场 到 @结尾",
            "prj_owned",
            &entities,
            &refs,
            &ending,
        )
        .unwrap();
        assert!(compiled.contains("<Subject 1>: composition_reference"));
        assert!(compiled.contains("<Subject 2>: ending_composition - <Picture 2>"));
        assert!(!compiled.contains("opening_composition"));
    }

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
