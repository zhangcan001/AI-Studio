//! Deterministic, model-independent prompt context assembly.
use crate::domain::shot::ShotStage;
use crate::domain::shot_context::{
    ContextSourceScope, PromptContext, PromptSegment, PromptSegmentKind,
};
use std::collections::HashSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptFragmentInput {
    pub text: String,
    pub negative_prompt: Option<String>,
    pub source_scope: ContextSourceScope,
    pub source_entity_id: String,
    pub revision_id: Option<String>,
    pub ordinal: i64,
}

impl PromptFragmentInput {
    pub fn new(
        text: impl Into<String>,
        source_scope: ContextSourceScope,
        source_entity_id: impl Into<String>,
    ) -> Self {
        Self {
            text: text.into(),
            negative_prompt: None,
            source_scope,
            source_entity_id: source_entity_id.into(),
            revision_id: None,
            ordinal: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromptContextInput {
    pub global_style: Vec<PromptFragmentInput>,
    pub scene: Vec<PromptFragmentInput>,
    pub characters: Vec<PromptFragmentInput>,
    pub costumes: Vec<PromptFragmentInput>,
    pub props: Vec<PromptFragmentInput>,
    pub shot_action: Vec<PromptFragmentInput>,
    pub camera: Vec<PromptFragmentInput>,
    pub lighting: Vec<PromptFragmentInput>,
    pub output_specification: Vec<PromptFragmentInput>,
}

pub struct PromptContextBuilder;

impl PromptContextBuilder {
    pub fn build(input: &PromptContextInput) -> PromptContext {
        build_prompt_context(input)
    }
}

pub fn build_prompt_context(input: &PromptContextInput) -> PromptContext {
    let mut segments = Vec::new();
    let mut seen_segments = HashSet::<(ContextSourceScope, String, String)>::new();
    let mut negative_prompts = Vec::<String>::new();

    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::GlobalStyle,
        &input.global_style,
        true,
    );
    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::Scene,
        &input.scene,
        true,
    );
    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::Character,
        &input.characters,
        true,
    );
    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::Costume,
        &input.costumes,
        false,
    );
    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::Props,
        &input.props,
        false,
    );
    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::ShotAction,
        &input.shot_action,
        false,
    );
    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::Camera,
        &input.camera,
        false,
    );
    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::Lighting,
        &input.lighting,
        false,
    );
    append_section(
        &mut segments,
        &mut negative_prompts,
        &mut seen_segments,
        PromptSegmentKind::OutputSpecification,
        &input.output_specification,
        false,
    );

    let mut seen_negative = HashSet::new();
    let negative_prompt = negative_prompts
        .into_iter()
        .map(|value| normalize_prompt(&value))
        .filter(|value| !value.is_empty())
        .filter(|value| seen_negative.insert(value.clone()))
        .collect::<Vec<_>>()
        .join("\n");

    let rendered_text = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    PromptContext {
        segments,
        rendered_text,
        negative_prompt,
    }
}

fn append_section(
    segments: &mut Vec<PromptSegment>,
    negative_prompts: &mut Vec<String>,
    seen_segments: &mut HashSet<(ContextSourceScope, String, String)>,
    kind: PromptSegmentKind,
    inputs: &[PromptFragmentInput],
    collect_negative: bool,
) {
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|left, right| {
        (
            left.ordinal,
            left.source_scope.rank(),
            left.source_entity_id.as_str(),
            left.text.as_str(),
        )
            .cmp(&(
                right.ordinal,
                right.source_scope.rank(),
                right.source_entity_id.as_str(),
                right.text.as_str(),
            ))
    });

    for input in inputs {
        let text = input.text.trim().to_owned();
        if collect_negative {
            if let Some(negative) = input.negative_prompt.as_deref() {
                negative_prompts.push(negative.to_owned());
            }
        }
        if text.is_empty() {
            continue;
        }
        if !seen_segments.insert((
            input.source_scope,
            input.source_entity_id.clone(),
            text.clone(),
        )) {
            continue;
        }
        segments.push(PromptSegment {
            kind,
            text,
            source_scope: input.source_scope,
            source_entity_id: input.source_entity_id,
            revision_id: input.revision_id,
            omitted_reason: None,
        });
    }
}

pub fn normalize_prompt(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Selects only the prompt owned by the requested stage. A missing or empty
/// stage prompt falls back to the legacy shot prompt; the other stage is never
/// consulted.
pub fn select_stage_prompt(
    stage: ShotStage,
    image_prompt: Option<&str>,
    video_prompt: Option<&str>,
    legacy_prompt: &str,
) -> String {
    let selected = match stage {
        ShotStage::Image => image_prompt,
        ShotStage::Video => video_prompt,
    }
    .map(str::trim)
    .filter(|value| !value.is_empty());
    selected.unwrap_or_else(|| legacy_prompt.trim()).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(text: &str, scope: ContextSourceScope, id: &str) -> PromptFragmentInput {
        PromptFragmentInput::new(text, scope, id)
    }

    #[test]
    fn builder_keeps_nine_segment_order_and_omits_empty_sections() {
        let mut input = PromptContextInput {
            global_style: vec![source("style", ContextSourceScope::Project, "style")],
            scene: vec![source("scene", ContextSourceScope::Scene, "scene")],
            characters: vec![source("character", ContextSourceScope::Shot, "char")],
            costumes: vec![source("costume", ContextSourceScope::Shot, "costume")],
            props: vec![source("prop", ContextSourceScope::Shot, "prop")],
            shot_action: vec![source("action", ContextSourceScope::Shot, "shot")],
            camera: Vec::new(),
            lighting: vec![source("light", ContextSourceScope::Scene, "scene")],
            output_specification: vec![source("output", ContextSourceScope::Shot, "shot")],
        };
        input
            .global_style
            .push(source("", ContextSourceScope::Project, "empty"));

        let context = build_prompt_context(&input);
        assert_eq!(
            context
                .segments
                .iter()
                .map(|segment| segment.kind)
                .collect::<Vec<_>>(),
            vec![
                PromptSegmentKind::GlobalStyle,
                PromptSegmentKind::Scene,
                PromptSegmentKind::Character,
                PromptSegmentKind::Costume,
                PromptSegmentKind::Props,
                PromptSegmentKind::ShotAction,
                PromptSegmentKind::Lighting,
                PromptSegmentKind::OutputSpecification,
            ]
        );
        assert_eq!(
            context.rendered_text,
            "style\nscene\ncharacter\ncostume\nprop\naction\nlight\noutput"
        );
    }

    #[test]
    fn negative_prompts_are_ordered_and_deduplicated() {
        let mut style = source("style", ContextSourceScope::Project, "style");
        style.negative_prompt = Some("bad  anatomy".to_owned());
        let mut scene = source("scene", ContextSourceScope::Scene, "scene");
        scene.negative_prompt = Some("bad anatomy".to_owned());
        let mut character = source("char", ContextSourceScope::Shot, "char");
        character.negative_prompt = Some("low quality".to_owned());

        let context = build_prompt_context(&PromptContextInput {
            global_style: vec![style],
            scene: vec![scene],
            characters: vec![character],
            ..PromptContextInput::default()
        });
        assert_eq!(context.negative_prompt, "bad anatomy\nlow quality");
    }

    #[test]
    fn stage_prompt_never_leaks_the_other_stage() {
        assert_eq!(
            select_stage_prompt(ShotStage::Image, Some("image"), Some("video"), "legacy"),
            "image"
        );
        assert_eq!(
            select_stage_prompt(ShotStage::Video, Some("image"), Some("video"), "legacy"),
            "video"
        );
        assert_eq!(
            select_stage_prompt(ShotStage::Image, None, Some("video"), "legacy"),
            "legacy"
        );
    }
}
