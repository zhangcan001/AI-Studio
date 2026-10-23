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
        let sections = match shot_id {
            Some(shot_id) => repository
                .prompt_sections_for_shot(project_id, shot_id)
                .await
                .map_err(invalid)?,
            None => Default::default(),
        };
        let has_audio = values.values().any(|v| match v {
            GenerationInputValue::AudioAsset(_) => true,
            GenerationInputValue::AudioAssets(ids) => !ids.is_empty(),
            _ => false,
        });
        let reordered = order_composition_references(&mut references, &entities, &usage);
        let compiled = compile_ref2va_prompt(
            prompt,
            project_id,
            &entities,
            &references,
            &usage,
            &sections,
            has_audio,
        )?;
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

/// Section headers of the official H3 Ref2VA full-reference prompt, in order.
const REF2VA_SECTIONS: [&str; 6] = [
    "subject_definitions:",
    "summary:",
    "retention_analysis:",
    "detailed_description:",
    "overall_soundscape:",
    "non_diegetic_music:",
];

/// Text already written in the six-section structure is passed through as-is
/// (after label substitution) instead of being wrapped a second time.
fn has_ref2va_sections(text: &str) -> bool {
    let mut from = 0;
    REF2VA_SECTIONS.iter().all(|section| {
        let found = text[from..]
            .lines()
            .scan(from, |offset, line| {
                let at = *offset;
                *offset += line.len() + 1;
                Some((at, line))
            })
            .find(|(_, line)| line.trim() == *section);
        match found {
            Some((at, _)) => {
                from = (at + section.len()).min(text.len());
                true
            }
            None => false,
        }
    })
}

/// True when the text is mostly English (Latin letters clearly outnumber Han
/// characters), so it can be injected into the English prompt as-is.
pub(crate) fn mostly_english(text: &str) -> bool {
    let han = text
        .chars()
        .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c) || ('\u{3400}'..='\u{4dbf}').contains(c))
        .count();
    let latin = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
    latin >= 3 && latin > han * 4
}

/// Lines starting with `[Shot N]` mark an already shot-structured body; a
/// `[Shot 1]` reference inside a sentence (e.g. "(appears in [Shot 1])") does not.
fn has_shot_markers(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_start();
        line.strip_prefix("[Shot ")
            .and_then(|rest| rest.split_once(']'))
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    })
}

/// Shot numbers (from line-leading `[Shot N]` markers) whose text mentions `label`.
fn shots_mentioning(body: &str, label: &str) -> Vec<String> {
    let mut current = "1".to_string();
    let mut found: Vec<String> = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some((n, _)) = trimmed
            .strip_prefix("[Shot ")
            .and_then(|rest| rest.split_once(']'))
        {
            if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
                current = n.to_string();
            }
        }
        if line.contains(label) && !found.contains(&current) {
            found.push(current.clone());
        }
    }
    if found.is_empty() {
        found.push("1".into());
    }
    found
}

fn compile_entity_prompt_with_usage(
    prompt: &str,
    project_id: &str,
    entities: &[crate::domain::story_entity::StoryEntity],
    references: &[AssetId],
    usage: &crate::domain::story_entity::ShotCompositionUsage,
) -> Result<String, ShotVideoInputError> {
    compile_ref2va_prompt(
        prompt,
        project_id,
        entities,
        references,
        usage,
        &Default::default(),
        false,
    )
}

struct SubjectRef<'a> {
    entity: &'a crate::domain::story_entity::StoryEntity,
    picture: usize,
}

struct MentionState<'a> {
    subjects: Vec<SubjectRef<'a>>,
    pictures: Vec<(&'a String, usize, Option<(&'static str, &'static str)>)>,
}

fn substitute_mentions<'a>(
    text: &str,
    project_id: &str,
    entities: &'a [crate::domain::story_entity::StoryEntity],
    references: &[AssetId],
    usage: &crate::domain::story_entity::ShotCompositionUsage,
    state: &mut MentionState<'a>,
) -> Result<String, ShotVideoInputError> {
    use crate::domain::story_entity::StoryEntityKind;
    let mut rest = text;
    let mut output = String::new();
    while let Some(at) = rest.find('@') {
        output.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let entity = match_entity_mention(rest, project_id, entities)?;
        let asset_id = entity_cover(entity).ok_or_else(|| {
            invalid(format!(
                "ENTITY_REFERENCE_MISSING_IMAGE: @{} 尚未关联图片",
                entity.name
            ))
        })?;
        let ordinal = references.iter().position(|id| id.as_str() == asset_id)
            .ok_or_else(|| invalid(format!("ENTITY_REFERENCE_NOT_APPLIED: @{} 的图片尚未加入当前 Recipe 的正式参考图，请点击应用角色与场景参考图", entity.name)))?;
        let picture = ordinal + 1;
        if entity.kind == StoryEntityKind::Composition {
            // Composition pictures are layout references only: a standalone
            // <Picture N>, never a <Subject N> and never a first/last keyframe.
            if !state.pictures.iter().any(|(id, _, _)| *id == &entity.id) {
                let opening = usage.opening_entity_id.as_deref() == Some(entity.id.as_str());
                let ending = usage.ending_entity_id.as_deref() == Some(entity.id.as_str());
                let moment = if opening {
                    Some(("opening", "at the start of the shot"))
                } else if ending {
                    Some(("ending", "near the end of the shot"))
                } else {
                    None
                };
                state.pictures.push((&entity.id, picture, moment));
            }
            output.push_str(&format!("<Picture {picture}>"));
        } else {
            let subject = match state.subjects.iter().position(|s| s.entity.id == entity.id) {
                Some(index) => index + 1,
                None => {
                    state.subjects.push(SubjectRef { entity, picture });
                    state.subjects.len()
                }
            };
            output.push_str(&format!("<Subject {subject}>"));
        }
        rest = &rest[entity.name.len()..];
    }
    output.push_str(rest);
    Ok(output)
}

/// Assembles the official six-section H3 Ref2VA prompt from the shot text
/// (detailed_description body), the writer sections and the entity
/// appearances. Missing writer sections fall back to neutral text; nothing
/// absent from the writer's input is invented.
pub(crate) fn compile_ref2va_prompt(
    prompt: &str,
    project_id: &str,
    entities: &[crate::domain::story_entity::StoryEntity],
    references: &[AssetId],
    usage: &crate::domain::story_entity::ShotCompositionUsage,
    sections: &crate::domain::story_entity::ShotPromptSections,
    has_audio_reference: bool,
) -> Result<String, ShotVideoInputError> {
    use crate::domain::story_entity::StoryEntityKind;
    let sections = sections.clone().normalized();
    let mut state = MentionState {
        subjects: Vec::new(),
        pictures: Vec::new(),
    };
    let output = substitute_mentions(prompt, project_id, entities, references, usage, &mut state)?;
    if has_ref2va_sections(&output) {
        return Ok(output);
    }
    let mut sub = |text: &Option<String>| -> Result<Option<String>, ShotVideoInputError> {
        text.as_deref()
            .map(|t| substitute_mentions(t, project_id, entities, references, usage, &mut state))
            .transpose()
    };
    let summary_text = sub(&sections.summary)?;
    let style_text = sub(&sections.style)?;
    let soundscape_text = sub(&sections.soundscape)?;
    let music_text = sub(&sections.music)?;
    if state.subjects.is_empty() && state.pictures.is_empty() {
        return Ok(output);
    }
    let body = if has_shot_markers(output.trim_start().lines().next().unwrap_or("")) {
        output.clone()
    } else {
        format!("[Shot 1] {output}")
    };
    let mut definitions = Vec::new();
    let mut retention = Vec::new();
    for (index, subject) in state.subjects.iter().enumerate() {
        let n = index + 1;
        let picture = subject.picture;
        let (role, features) = match subject.entity.kind {
            StoryEntityKind::Character => (
                "character",
                "facial features, hairstyle, clothing and overall appearance",
            ),
            StoryEntityKind::Scene => (
                "environment",
                "spatial layout, architecture, terrain and environmental structure",
            ),
            StoryEntityKind::Prop => ("object", "shape, materials and distinctive details"),
            StoryEntityKind::Composition => ("composition", "layout"),
        };
        let label = format!("<Subject {n}>");
        let appears = shots_mentioning(&body, &label)
            .iter()
            .map(|s| format!("[Shot {s}]"))
            .collect::<Vec<_>>()
            .join(", ");
        let appearance = subject.entity.description.trim();
        if !appearance.is_empty() && mostly_english(appearance) {
            let appearance = appearance.trim_end_matches(['.', ' ']);
            definitions.push(format!(
                "{label} is the {role} in <Picture {picture}>, {appearance}."
            ));
            retention.push(format!(
                "{label} (appears in {appears}): fully_preserved - the {features} from <Picture {picture}> are retained: {appearance}."
            ));
        } else {
            definitions.push(format!(
                "{label} is the {role} shown in <Picture {picture}>."
            ));
            retention.push(format!(
                "{label} (appears in {appears}): fully_preserved - retain {features} from <Picture {picture}>, except changes explicitly requested in detailed_description."
            ));
        }
    }
    let placed = if state.subjects.is_empty() {
        "the characters".to_string()
    } else {
        (1..=state.subjects.len())
            .map(|n| format!("<Subject {n}>"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (_, picture, moment) in &state.pictures {
        let (layout, when) = match moment {
            Some((name, when)) => (format!(" {name} layout"), format!(" {when}")),
            None => (String::new(), String::new()),
        };
        definitions.push(format!(
            "<Picture {picture}> is a shot-planning reference for [Shot 1], defining viewpoint and the placement of {placed} and props{when}."
        ));
        retention.push(format!(
            "<Picture {picture}> (shot-planning reference for [Shot 1]{layout}): partially_preserved - only viewpoint, subject placement, facing and prop positions are retained; the appearance and clothing of people in this picture are not used, identities follow {placed}."
        ));
    }
    let prefix = if has_audio_reference {
        "[reference generation + audio reference]"
    } else {
        "[reference generation]"
    };
    // Ref2VA section order follows the official H3 prompt-writing convention.
    // Do not invent camera, motion, dialogue or music absent from the user text.
    let summary = match summary_text {
        Some(text) => format!("{prefix} {text}"),
        None => {
            let count = |kind: StoryEntityKind| {
                state
                    .subjects
                    .iter()
                    .filter(|s| s.entity.kind == kind)
                    .count()
            };
            let mut parts = Vec::new();
            for (n, one, many) in [
                (
                    count(StoryEntityKind::Character),
                    "referenced character",
                    "referenced characters",
                ),
                (
                    count(StoryEntityKind::Scene),
                    "referenced environment",
                    "referenced environments",
                ),
                (
                    count(StoryEntityKind::Prop),
                    "referenced object",
                    "referenced objects",
                ),
            ] {
                if n > 0 {
                    parts.push(format!("{n} {}", if n == 1 { one } else { many }));
                }
            }
            let mut summary = format!("{prefix} The target video is a single shot");
            if !parts.is_empty() {
                summary.push_str(" featuring ");
                summary.push_str(&parts.join(", "));
            }
            summary.push('.');
            let pictures = state.pictures.len();
            if pictures > 0 {
                summary.push_str(&format!(
                    " {} shot-planning reference picture{} define{} only where the characters, environment and props are placed; appearance comes from the subject references.",
                    pictures,
                    if pictures == 1 { "" } else { "s" },
                    if pictures == 1 { "s" } else { "" },
                ));
            }
            summary.push_str(" Actions follow detailed_description.");
            summary
        }
    };
    let style = style_text.unwrap_or_else(|| {
        "A single continuous shot whose visual style follows the reference pictures.".into()
    });
    let soundscape = soundscape_text.unwrap_or_else(|| {
        "Quiet room tone matching the environment; follow any sound directions in detailed_description.".into()
    });
    let music = music_text.unwrap_or_else(|| "N/A".into());
    Ok(format!(
        "subject_definitions:\n{}\n\nsummary:\n{summary}\n\nretention_analysis:\n{}\nDo not replace the referenced environment with a generic setting or use incidental figures in an environment or layout reference as the character reference.\n\ndetailed_description:\n{style}\n{body}\n\noverall_soundscape:\n{soundscape}\n\nnon_diegetic_music:\n{music}",
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
        assert!(text.contains(
            "[Shot 1] <Subject 1>在<Subject 2>中飞翔，镜头推进；<Subject 1>说：\"向前！\"，5秒"
        ));
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
        assert!(compiled.contains("[Shot 1] <Subject 1>在<Subject 2>中飞翔，5秒，<Subject 1>"));
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
            assert!(compiled.contains("[Shot 1] <Subject 1>，按要求变为红色"));
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
        assert!(compiled.contains("[Shot 1] <Subject 1>走入<Subject 2>拿起<Subject 3>"));
        assert_eq!(
            super::compile_entity_prompt("镜头推进，5秒", "prj_owned", &entities, &refs).unwrap(),
            "镜头推进，5秒"
        );
        let compiled =
            super::compile_entity_prompt("按@剧情01构图", "prj_owned", &entities, &refs).unwrap();
        assert!(compiled.contains("<Picture 1> is a shot-planning reference for [Shot 1], defining viewpoint and the placement of the characters and props."));
        assert!(compiled.contains("<Picture 1> (shot-planning reference for [Shot 1]): partially_preserved - only viewpoint"));
        assert!(compiled.contains("[Shot 1] 按<Picture 1>构图"));
        assert!(!compiled.contains("<Subject 1"));
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
        assert!(compiled.contains("<Picture 1> is a shot-planning reference for [Shot 1], defining viewpoint and the placement of <Subject 1> and props at the start of the shot."));
        assert!(compiled.contains("<Picture 1> (shot-planning reference for [Shot 1] opening layout): partially_preserved - only viewpoint, subject placement, facing and prop positions are retained; the appearance and clothing of people in this picture are not used, identities follow <Subject 1>."));
        assert!(compiled.contains(
            "<Picture 4> (shot-planning reference for [Shot 1] ending layout): partially_preserved"
        ));
        assert!(compiled.contains(
            "<Picture 3> (shot-planning reference for [Shot 1]): partially_preserved - "
        ));
        assert!(compiled.contains("<Subject 1> is the character shown in <Picture 2>."));
        assert!(compiled.contains("[Shot 1] 开场按 <Picture 1> 的构图，<Subject 1> 走过 <Picture 3>，最后停在 <Picture 4> 的构图"));
        for banned in [
            "opening_composition",
            "ending_composition",
            "composition_reference",
            "<Subject 2>",
            "begins from",
            "ends on",
            "first frame",
            "last frame",
        ] {
            assert!(!compiled.contains(banned), "{banned}");
        }
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
        assert!(compiled
            .contains("<Picture 1> (shot-planning reference for [Shot 1]): partially_preserved"));
        assert!(compiled.contains(
            "<Picture 2> (shot-planning reference for [Shot 1] ending layout): partially_preserved"
        ));
        assert!(!compiled.contains("attribute_transfer"));
        assert!(!compiled.contains("opening layout"));
    }

    #[test]
    fn composition_pictures_are_layout_only_with_official_markers_and_summary() {
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
            entity("沈簿", StoryEntityKind::Character, "ast_a"),
            entity("林舟", StoryEntityKind::Character, "ast_b"),
            entity("开场构图", StoryEntityKind::Composition, "ast_open"),
            entity("结尾构图", StoryEntityKind::Composition, "ast_end"),
        ];
        let id = |v: &str| AssetId::parse(v).unwrap();
        let refs = [id("ast_a"), id("ast_b"), id("ast_open"), id("ast_end")];
        let both = ShotCompositionUsage {
            opening_entity_id: Some("开场构图".into()),
            ending_entity_id: Some("结尾构图".into()),
        };
        let text = "按@开场构图的站位，@沈簿站在门口，@林舟坐在桌后；@沈簿走到桌前递出信封，最后两人位置如@结尾构图，8秒";
        let compiled =
            super::compile_entity_prompt_with_usage(text, "prj_owned", &entities, &refs, &both)
                .unwrap();
        println!("{compiled}");
        assert!(compiled.contains("<Subject 1> is the character shown in <Picture 1>."));
        assert!(compiled.contains("<Subject 2> is the character shown in <Picture 2>."));
        assert!(compiled.contains("<Picture 3> is a shot-planning reference for [Shot 1], defining viewpoint and the placement of <Subject 1>, <Subject 2> and props at the start of the shot."));
        assert!(compiled.contains("detailed_description:\nA single continuous shot whose visual style follows the reference pictures.\n[Shot 1] "));
        assert!(compiled.contains("<Picture 3> (shot-planning reference for [Shot 1] opening layout): partially_preserved - only viewpoint, subject placement, facing and prop positions are retained; the appearance and clothing of people in this picture are not used, identities follow <Subject 1>, <Subject 2>."));
        assert!(compiled.contains(
            "<Picture 4> (shot-planning reference for [Shot 1] ending layout): partially_preserved"
        ));
        assert!(!compiled.contains("attribute_transfer"));
        assert!(compiled.contains("summary:\n[reference generation] The target video is a single shot featuring 2 referenced characters."));
        assert!(compiled.contains("[Shot 1] 按<Picture 3>的站位，<Subject 1>站在门口，<Subject 2>坐在桌后；<Subject 1>走到桌前递出信封，最后两人位置如<Picture 4>，8秒"));
        assert!(!compiled.contains("<Subject 3>"));
        let markers: Vec<&str> = compiled
            .lines()
            .filter_map(|l| l.split_once("): ").or_else(|| l.split_once(">: ")))
            .filter_map(|(_, r)| r.split(" - ").next())
            .collect();
        assert!(!markers.is_empty());
        for marker in markers {
            assert!(
                [
                    "fully_preserved",
                    "partially_preserved",
                    "attribute_transfer",
                    "weak_reference"
                ]
                .contains(&marker),
                "{marker}"
            );
        }
        for banned in [
            "opening_composition",
            "ending_composition",
            "composition_reference",
            "Use the referenced subjects for the requested video",
        ] {
            assert!(!compiled.contains(banned), "{banned}");
        }
        // Missing opening or ending composition still compiles.
        for usage in [
            ShotCompositionUsage {
                opening_entity_id: None,
                ending_entity_id: Some("结尾构图".into()),
            },
            ShotCompositionUsage {
                opening_entity_id: Some("开场构图".into()),
                ending_entity_id: None,
            },
            ShotCompositionUsage::default(),
        ] {
            let c = super::compile_entity_prompt_with_usage(
                "@沈簿和@林舟对话，6秒",
                "prj_owned",
                &entities,
                &refs,
                &usage,
            )
            .unwrap();
            assert!(c.starts_with("subject_definitions:\n<Subject 1>"));
            assert!(!c.contains("<Picture 3> is"));
            assert!(c.contains("[reference generation]"));
        }
        // Composition-only prompt still compiles as a layout picture.
        let c = super::compile_entity_prompt_with_usage(
            "按@结尾构图收尾",
            "prj_owned",
            &entities,
            &refs,
            &both,
        )
        .unwrap();
        assert!(c.contains("<Picture 4> is a shot-planning reference for [Shot 1], defining viewpoint and the placement of the characters and props near the end of the shot."));
        assert!(c.contains("summary:\n[reference generation] The target video is a single shot."));
        // Six-section text is passed through, never wrapped a second time.
        assert_eq!(
            super::compile_entity_prompt_with_usage(
                &compiled,
                "prj_owned",
                &entities,
                &refs,
                &both
            )
            .unwrap(),
            compiled
        );
        let user_sections = "subject_definitions:\n@沈簿 是主角\n\nsummary:\n[reference generation] 自写\n\nretention_analysis:\n略\n\ndetailed_description:\n@沈簿 推门，按@开场构图\n\noverall_soundscape:\n雨声\n\nnon_diegetic_music:\n无";
        let c = super::compile_entity_prompt_with_usage(
            user_sections,
            "prj_owned",
            &entities,
            &refs,
            &both,
        )
        .unwrap();
        assert_eq!(c.matches("subject_definitions:").count(), 1);
        assert!(c.starts_with("subject_definitions:\n<Subject 1> 是主角"));
        assert!(c.contains("<Subject 1> 推门，按<Picture 3>"));
    }

    #[test]
    fn writer_sections_and_appearance_assemble_the_six_official_sections() {
        use crate::domain::{
            story_entity::{
                ShotCompositionUsage, ShotPromptSections, StoryEntity, StoryEntityKind,
            },
            AssetId,
        };
        let entity = |id: &str, kind, cover: &str, description: &str| StoryEntity {
            id: id.into(),
            project_id: "prj_owned".into(),
            kind,
            name: id.into(),
            description: description.into(),
            cover_asset_id: Some(cover.into()),
            image_asset_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let entities = [
            entity("沈簿", StoryEntityKind::Character, "ast_a", "a man in his early 30s with short neat black hair, wearing a dark grey wool long coat."),
            entity("林舟", StoryEntityKind::Character, "ast_b", "三十岁女性，短发"),
            entity("书房", StoryEntityKind::Scene, "ast_s", "a dim study with tall dark-wood bookshelves along the left wall"),
            entity("开场构图", StoryEntityKind::Composition, "ast_open", ""),
            entity("结尾构图", StoryEntityKind::Composition, "ast_end", ""),
        ];
        let id = |v: &str| AssetId::parse(v).unwrap();
        let refs = [
            id("ast_a"),
            id("ast_b"),
            id("ast_s"),
            id("ast_open"),
            id("ast_end"),
        ];
        let usage = ShotCompositionUsage {
            opening_entity_id: Some("开场构图".into()),
            ending_entity_id: Some("结尾构图".into()),
        };
        let sections = ShotPromptSections {
            style: Some("Realistic period drama style with warm lamplight.".into()),
            summary: Some("@沈簿 enters @书房 and hands a letter to @林舟.".into()),
            soundscape: Some("Quiet indoor room tone.".into()),
            music: Some("  ".into()),
        };
        let body = "Positioned as in @开场构图, @沈簿 stands at the door of @书房. @林舟 (S1) says, <d>[Chinese] 你来了。</d>\n[Shot 2] At 00:04.000, the shot cuts to @林舟. By the end, positioned as in @结尾构图.";
        let c = super::compile_ref2va_prompt(
            body,
            "prj_owned",
            &entities,
            &refs,
            &usage,
            &sections,
            false,
        )
        .unwrap();
        println!("{c}");
        assert!(c.contains("<Subject 1> is the character in <Picture 1>, a man in his early 30s with short neat black hair, wearing a dark grey wool long coat."));
        assert!(c.contains("<Subject 1> (appears in [Shot 1]): fully_preserved - the facial features, hairstyle, clothing and overall appearance from <Picture 1> are retained: a man"));
        // Mostly Chinese description falls back to the generic line.
        assert!(c.contains("<Subject 3> is the character shown in <Picture 2>."));
        assert!(c.contains("<Subject 3> (appears in [Shot 1], [Shot 2]): fully_preserved - retain"));
        assert!(c.contains("<Subject 2> is the environment in <Picture 3>, a dim study"));
        assert!(c.contains("summary:\n[reference generation] <Subject 1> enters <Subject 2> and hands a letter to <Subject 3>."));
        assert!(c.contains("detailed_description:\nRealistic period drama style with warm lamplight.\n[Shot 1] Positioned as in <Picture 4>, <Subject 1>"));
        assert!(c.contains("\n[Shot 2] At 00:04.000, the shot cuts to <Subject 3>."));
        assert_eq!(c.matches("\n[Shot 1] ").count(), 1);
        assert!(c.contains("overall_soundscape:\nQuiet indoor room tone."));
        assert!(c.ends_with("non_diegetic_music:\nN/A"));
        assert!(!c.contains('@'));
        // Audio reference changes the task prefix; missing sections fall back.
        let c = super::compile_ref2va_prompt(
            "@沈簿 walks.",
            "prj_owned",
            &entities,
            &refs,
            &usage,
            &Default::default(),
            true,
        )
        .unwrap();
        assert!(c.contains("summary:\n[reference generation + audio reference] The target video is a single shot featuring 1 referenced character."));
        assert!(c.contains("detailed_description:\nA single continuous shot whose visual style follows the reference pictures.\n[Shot 1] <Subject 1> walks."));
        assert!(c.contains("overall_soundscape:\nQuiet room tone"));
        assert!(c.ends_with("non_diegetic_music:\nN/A"));
        // "[Shot 1]" inside a sentence does not count as a shot marker.
        let c = super::compile_ref2va_prompt(
            "@沈簿 (appears in [Shot 1]) walks.",
            "prj_owned",
            &entities,
            &refs,
            &usage,
            &Default::default(),
            false,
        )
        .unwrap();
        assert!(c.contains("\n[Shot 1] <Subject 1> (appears in [Shot 1]) walks."));
        // Unknown @name in a writer section is rejected like in the body.
        let bad = ShotPromptSections {
            summary: Some("@无名 walks".into()),
            ..Default::default()
        };
        assert!(super::compile_ref2va_prompt(
            "@沈簿 walks.",
            "prj_owned",
            &entities,
            &refs,
            &usage,
            &bad,
            false
        )
        .is_err());
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
