//! Phase 1 semantic graph boundary for generic workflow capability inference.

use crate::{
    application::{
        workflow_analysis_service::{OutputRoot, WorkflowAnalysisReport},
        workflow_graph_analysis::{WorkflowGraph, WorkflowGraphError, WorkflowLink},
        workflow_recognition_schema::{
            MediaKind, RecognitionDeclaredType, RecognitionMatchTypeTemplate,
            RecognitionNodeSchema, RecognitionSchemaContext,
        },
    },
    domain::WorkflowDocument,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Semantic types are intentionally provider-neutral.  They describe the
/// data crossing a node boundary, not the node implementation that produced
/// it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SemanticType {
    Text,
    Conditioning,
    Image,
    ImageList,
    Video,
    VideoList,
    Audio,
    AudioList,
    Latent,
    Model,
    VideoModel,
    Mask,
    Unknown,
}

/// The schema contract is intentionally kept separate from the provider-neutral
/// semantic vocabulary.  A declared custom or polymorphic socket can be
/// structurally understood even when its business meaning is not known.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SchemaTypeResolution {
    KnownSemantic {
        raw_type: String,
        semantic_type: SemanticType,
    },
    DynamicResolved {
        raw_type: String,
        semantic_type: SemanticType,
    },
    DynamicUnresolved {
        raw_type: String,
    },
    OpaqueCustom {
        raw_type: String,
    },
    Unresolved,
    Conflict {
        raw_type: String,
        message: String,
    },
}

impl SchemaTypeResolution {
    fn semantic_type(&self) -> SemanticType {
        match self {
            Self::KnownSemantic { semantic_type, .. }
            | Self::DynamicResolved { semantic_type, .. } => *semantic_type,
            Self::DynamicUnresolved { .. }
            | Self::OpaqueCustom { .. }
            | Self::Unresolved
            | Self::Conflict { .. } => SemanticType::Unknown,
        }
    }

    fn raw_type(&self) -> Option<&str> {
        match self {
            Self::KnownSemantic { raw_type, .. }
            | Self::DynamicResolved { raw_type, .. }
            | Self::DynamicUnresolved { raw_type }
            | Self::OpaqueCustom { raw_type }
            | Self::Conflict { raw_type, .. } => Some(raw_type.as_str()),
            Self::Unresolved => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SemanticNodeRole {
    TextSource,
    TextConditioning,
    ImageSource,
    VideoSource,
    AudioSource,
    ModelLoader,
    ImageGenerator,
    VideoGenerator,
    AudioGenerator,
    ImageEncoder,
    VideoEncoder,
    LatentEncoder,
    ImageDecoder,
    VideoDecoder,
    ImageOutput,
    VideoOutput,
    AudioOutput,
    GenericTransform,
    Unknown,
}

/// The provider-neutral vocabulary shared by analysis, onboarding inspection,
/// suggestions, and capability evaluation.  Specificity such as
/// `ReferenceImage` is carried as usage semantics while its socket remains an
/// `Image` at the type-compatibility layer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CanonicalSemantic {
    PromptText,
    PositivePrompt,
    NegativePrompt,
    Image,
    ImageList,
    SourceImage,
    ReferenceImage,
    ReferenceImageList,
    Video,
    VideoList,
    ReferenceVideo,
    ReferenceVideoList,
    Audio,
    AudioList,
    ReferenceAudio,
    ReferenceAudioList,
    FirstFrame,
    LastFrame,
    Mask,
    Conditioning,
    Seed,
    Width,
    Height,
    Frames,
    Duration,
    Fps,
    Model,
    ImageModel,
    VideoModel,
    Vae,
    Latent,
    Steps,
    Cfg,
    Guidance,
    Denoise,
    Strength,
    Shift,
    Scale,
    Weight,
    Unknown,
}

impl CanonicalSemantic {
    pub const fn semantic_key(self) -> &'static str {
        match self {
            Self::PromptText | Self::PositivePrompt => "prompt",
            Self::NegativePrompt => "negative_prompt",
            Self::Image | Self::SourceImage => "image",
            Self::ImageList => "images",
            Self::ReferenceImage => "reference_image",
            Self::ReferenceImageList => "reference_images",
            Self::Video => "video",
            Self::VideoList => "videos",
            Self::ReferenceVideo => "reference_video",
            Self::ReferenceVideoList => "reference_videos",
            Self::Audio => "audio",
            Self::AudioList => "audios",
            Self::ReferenceAudio => "reference_audio",
            Self::ReferenceAudioList => "reference_audios",
            Self::FirstFrame => "first_frame",
            Self::LastFrame => "last_frame",
            Self::Mask => "mask",
            Self::Conditioning => "conditioning",
            Self::Seed => "seed",
            Self::Width => "width",
            Self::Height => "height",
            Self::Frames | Self::Duration => "duration_seconds",
            Self::Fps => "fps",
            Self::Model | Self::ImageModel => "model",
            Self::VideoModel => "video_model",
            Self::Vae => "vae",
            Self::Latent => "latent",
            Self::Steps => "steps",
            Self::Cfg => "cfg",
            Self::Guidance => "guidance",
            Self::Denoise => "denoise",
            Self::Strength => "strength",
            Self::Shift => "shift",
            Self::Scale => "scale",
            Self::Weight => "weight",
            Self::Unknown => "unknown",
        }
    }

    pub const fn field_type(self) -> &'static str {
        match self {
            Self::PromptText | Self::PositivePrompt | Self::NegativePrompt => "textarea",
            Self::Image
            | Self::SourceImage
            | Self::ReferenceImage
            | Self::FirstFrame
            | Self::LastFrame => "image",
            Self::ImageList | Self::ReferenceImageList => "images",
            Self::Video | Self::ReferenceVideo => "video",
            Self::VideoList | Self::ReferenceVideoList => "videos",
            Self::Audio | Self::ReferenceAudio => "audio",
            Self::AudioList | Self::ReferenceAudioList => "audios",
            Self::Seed => "seed",
            Self::Width | Self::Height | Self::Frames | Self::Duration | Self::Steps => "integer",
            Self::Fps
            | Self::Cfg
            | Self::Guidance
            | Self::Denoise
            | Self::Strength
            | Self::Shift
            | Self::Scale
            | Self::Weight => "number",
            _ => "text",
        }
    }

    pub const fn semantic_type(self) -> SemanticType {
        match self {
            Self::PromptText | Self::PositivePrompt | Self::NegativePrompt => SemanticType::Text,
            Self::Image
            | Self::SourceImage
            | Self::ReferenceImage
            | Self::FirstFrame
            | Self::LastFrame => SemanticType::Image,
            Self::ImageList | Self::ReferenceImageList => SemanticType::ImageList,
            Self::Video | Self::ReferenceVideo => SemanticType::Video,
            Self::VideoList | Self::ReferenceVideoList => SemanticType::VideoList,
            Self::Audio | Self::ReferenceAudio => SemanticType::Audio,
            Self::AudioList | Self::ReferenceAudioList => SemanticType::AudioList,
            Self::Mask => SemanticType::Mask,
            Self::Conditioning => SemanticType::Conditioning,
            Self::Model | Self::ImageModel | Self::Vae => SemanticType::Model,
            Self::VideoModel => SemanticType::VideoModel,
            Self::Latent => SemanticType::Latent,
            _ => SemanticType::Unknown,
        }
    }

    pub const fn media_family(self) -> Option<&'static str> {
        match self {
            Self::Image
            | Self::ImageList
            | Self::SourceImage
            | Self::ReferenceImage
            | Self::ReferenceImageList
            | Self::FirstFrame
            | Self::LastFrame => Some("image"),
            Self::Video | Self::VideoList | Self::ReferenceVideo | Self::ReferenceVideoList => {
                Some("video")
            }
            Self::Audio | Self::AudioList | Self::ReferenceAudio | Self::ReferenceAudioList => {
                Some("audio")
            }
            _ => None,
        }
    }

    pub const fn is_reference(self) -> bool {
        matches!(
            self,
            Self::ReferenceImage
                | Self::ReferenceImageList
                | Self::ReferenceVideo
                | Self::ReferenceVideoList
                | Self::ReferenceAudio
                | Self::ReferenceAudioList
        )
    }

    pub const fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::Seed
                | Self::Width
                | Self::Height
                | Self::Frames
                | Self::Duration
                | Self::Fps
                | Self::Steps
                | Self::Cfg
                | Self::Guidance
                | Self::Denoise
                | Self::Strength
                | Self::Shift
                | Self::Scale
                | Self::Weight
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SemanticEvidenceSource {
    SchemaDeclared,
    StandardSocketType,
    GraphContext,
    CanonicalNameHint,
    ClassTitleHint,
    ManualExplicit,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SemanticHintConfidence {
    High,
    Medium,
    Low,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalSemanticEvidence {
    pub semantic: CanonicalSemantic,
    pub source: SemanticEvidenceSource,
    pub confidence: SemanticHintConfidence,
}

/// The single semantic alias table shared by analysis, onboarding inspection,
/// and capability evaluation. `explicit_reference` distinguishes a generic
/// `image` socket from a socket whose name explicitly promises reference media.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticInputHint {
    pub semantic_key: &'static str,
    pub semantic: CanonicalSemantic,
    pub item_index: Option<usize>,
    pub force_media_source: bool,
    pub explicit_reference: bool,
    pub evidence: CanonicalSemanticEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticInputResolution {
    pub semantic_type: SemanticType,
    pub semantic: CanonicalSemantic,
    pub semantic_key: Option<String>,
    pub explicit_reference: bool,
    pub type_resolution: SchemaTypeResolution,
    pub evidence: Vec<CanonicalSemanticEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSuggestion {
    pub semantic: CanonicalSemantic,
    pub semantic_key: Option<String>,
    pub field_type: String,
    pub evidence: CanonicalSemanticEvidence,
}

pub fn linked_target_semantic(input_name: &str) -> Option<SemanticInputHint> {
    canonical_semantic_hint(input_name)
}

/// Resolve an input name through the one canonical alias table. Generic media
/// names remain ordinary media; only explicit reference aliases receive
/// reference specificity. Indexed reference slots are parsed numerically so
/// diagnostics remain deterministic (`0, 1, 2, 10`).
pub fn canonical_semantic_hint(input_name: &str) -> Option<SemanticInputHint> {
    let name = normalize_input_name(input_name);
    let semantic = match name.as_str() {
        "prompt" | "text" => CanonicalSemantic::PromptText,
        "positive" | "positive_prompt" => CanonicalSemantic::PositivePrompt,
        "negative" | "negative_prompt" => CanonicalSemantic::NegativePrompt,
        "description" => CanonicalSemantic::PromptText,
        "width" => CanonicalSemantic::Width,
        "height" => CanonicalSemantic::Height,
        "seed" | "noise_seed" | "random_seed" => CanonicalSemantic::Seed,
        "length" | "duration" | "duration_seconds" | "seconds" => CanonicalSemantic::Duration,
        "frames" | "num_frames" | "frame_count" | "duration_frames" => CanonicalSemantic::Frames,
        "fps" | "frame_rate" | "framerate" => CanonicalSemantic::Fps,
        "steps" | "num_steps" | "sampling_steps" => CanonicalSemantic::Steps,
        "cfg" | "cfg_scale" => CanonicalSemantic::Cfg,
        "guidance" => CanonicalSemantic::Guidance,
        "denoise" => CanonicalSemantic::Denoise,
        "strength" => CanonicalSemantic::Strength,
        "shift" => CanonicalSemantic::Shift,
        "scale" => CanonicalSemantic::Scale,
        "weight" => CanonicalSemantic::Weight,
        "first_frame" | "start_frame" | "first_image" | "start_image" => {
            CanonicalSemantic::FirstFrame
        }
        "last_frame" | "end_frame" | "last_image" | "end_image" => CanonicalSemantic::LastFrame,
        "image" | "input_image" => CanonicalSemantic::Image,
        "images" => CanonicalSemantic::ImageList,
        "reference_image" | "ref_image" | "reference" => CanonicalSemantic::ReferenceImage,
        "reference_images" | "ref_images" | "references" => CanonicalSemantic::ReferenceImageList,
        "video" | "input_video" => CanonicalSemantic::Video,
        "videos" => CanonicalSemantic::VideoList,
        "reference_video" | "ref_video" => CanonicalSemantic::ReferenceVideo,
        "reference_videos" | "ref_videos" => CanonicalSemantic::ReferenceVideoList,
        "audio" | "input_audio" => CanonicalSemantic::Audio,
        "audios" => CanonicalSemantic::AudioList,
        "reference_audio" | "ref_audio" => CanonicalSemantic::ReferenceAudio,
        "reference_audios" | "ref_audios" => CanonicalSemantic::ReferenceAudioList,
        "mask" => CanonicalSemantic::Mask,
        "conditioning" => CanonicalSemantic::Conditioning,
        "model" | "checkpoint" => CanonicalSemantic::Model,
        "image_model" => CanonicalSemantic::ImageModel,
        "video_model" => CanonicalSemantic::VideoModel,
        "vae" => CanonicalSemantic::Vae,
        "latent" | "latent_image" => CanonicalSemantic::Latent,
        _ if indexed_slot(&name, IMAGE_SLOT_PREFIXES).is_some() => {
            CanonicalSemantic::ReferenceImageList
        }
        _ if indexed_slot(&name, VIDEO_SLOT_PREFIXES).is_some() => {
            CanonicalSemantic::ReferenceVideoList
        }
        _ if indexed_slot(&name, AUDIO_SLOT_PREFIXES).is_some() => {
            CanonicalSemantic::ReferenceAudioList
        }
        _ if name.starts_with("prompt_") || name.ends_with("_prompt") => {
            CanonicalSemantic::PromptText
        }
        _ => return None,
    };
    let semantic_key = semantic.semantic_key();
    let item_index = indexed_slot_for_semantic(&name, semantic);
    let explicit_reference = semantic.is_reference();
    Some(SemanticInputHint {
        semantic_key,
        semantic,
        item_index,
        force_media_source: item_index.is_some()
            || matches!(
                semantic,
                CanonicalSemantic::FirstFrame | CanonicalSemantic::LastFrame
            )
            || explicit_reference,
        explicit_reference,
        evidence: CanonicalSemanticEvidence {
            semantic,
            source: SemanticEvidenceSource::CanonicalNameHint,
            confidence: SemanticHintConfidence::Low,
        },
    })
}

pub fn normalize_input_name(input_name: &str) -> String {
    input_name
        .to_ascii_lowercase()
        .replace(['-', ' ', '.'], "_")
}

const IMAGE_SLOT_PREFIXES: &[&str] = &[
    "ref_images_ref_image_",
    "ref_images_image_",
    "ref_images_",
    "reference_images_image_",
    "ref_image_",
    "reference_images_",
    "reference_image_",
];
const VIDEO_SLOT_PREFIXES: &[&str] = &[
    "ref_videos_ref_video_",
    "ref_videos_video_",
    "ref_videos_",
    "reference_videos_video_",
    "ref_video_",
    "reference_videos_",
    "reference_video_",
];
const AUDIO_SLOT_PREFIXES: &[&str] = &[
    "ref_video_audios_ref_video_audio_",
    "ref_audios_ref_audio_",
    "ref_audios_audio_",
    "ref_audios_",
    "reference_audios_audio_",
    "ref_audio_",
    "reference_audios_",
    "reference_audio_",
];

fn indexed_slot_for_semantic(name: &str, semantic: CanonicalSemantic) -> Option<usize> {
    let prefixes = match semantic {
        CanonicalSemantic::ReferenceImageList => IMAGE_SLOT_PREFIXES,
        CanonicalSemantic::ReferenceVideoList => VIDEO_SLOT_PREFIXES,
        CanonicalSemantic::ReferenceAudioList => AUDIO_SLOT_PREFIXES,
        _ => &[],
    };
    indexed_slot(name, prefixes)
}

fn indexed_slot(name: &str, prefixes: &[&str]) -> Option<usize> {
    prefixes.iter().find_map(|prefix| {
        name.strip_prefix(prefix)
            .filter(|suffix| !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|suffix| suffix.parse::<usize>().ok())
    })
}

pub fn is_indexed_media_slot(name: &str, media: &str) -> bool {
    let prefixes = match media {
        "image" => IMAGE_SLOT_PREFIXES,
        "video" => VIDEO_SLOT_PREFIXES,
        "audio" => AUDIO_SLOT_PREFIXES,
        _ => return false,
    };
    indexed_slot(name, prefixes).is_some()
}

pub fn semantic_media_family(semantic_key: &str) -> Option<&'static str> {
    canonical_semantic_hint(semantic_key).and_then(|hint| hint.semantic.media_family())
}

pub fn semantic_field_type(semantic_key: &str) -> &'static str {
    canonical_semantic_hint(semantic_key)
        .map(|hint| hint.semantic.field_type())
        .unwrap_or("image")
}

fn declared_semantic_type(declared: RecognitionDeclaredType) -> Option<SemanticType> {
    match declared {
        RecognitionDeclaredType::String => Some(SemanticType::Text),
        RecognitionDeclaredType::Conditioning => Some(SemanticType::Conditioning),
        RecognitionDeclaredType::Image => Some(SemanticType::Image),
        RecognitionDeclaredType::Video => Some(SemanticType::Video),
        RecognitionDeclaredType::Audio => Some(SemanticType::Audio),
        RecognitionDeclaredType::Latent => Some(SemanticType::Latent),
        RecognitionDeclaredType::Model => Some(SemanticType::Model),
        RecognitionDeclaredType::VideoModel => Some(SemanticType::VideoModel),
        RecognitionDeclaredType::Mask => Some(SemanticType::Mask),
        _ => None,
    }
}

fn canonical_matches_declared(
    semantic: CanonicalSemantic,
    declared: RecognitionDeclaredType,
) -> bool {
    match declared {
        RecognitionDeclaredType::String | RecognitionDeclaredType::Enum => matches!(
            semantic,
            CanonicalSemantic::PromptText
                | CanonicalSemantic::PositivePrompt
                | CanonicalSemantic::NegativePrompt
        ),
        RecognitionDeclaredType::Integer | RecognitionDeclaredType::Float => semantic.is_numeric(),
        RecognitionDeclaredType::Conditioning => matches!(
            semantic,
            CanonicalSemantic::Conditioning
                | CanonicalSemantic::PromptText
                | CanonicalSemantic::PositivePrompt
                | CanonicalSemantic::NegativePrompt
        ),
        RecognitionDeclaredType::Image => matches!(semantic.media_family(), Some("image")),
        RecognitionDeclaredType::Video => matches!(semantic.media_family(), Some("video")),
        RecognitionDeclaredType::Audio => matches!(semantic.media_family(), Some("audio")),
        RecognitionDeclaredType::Mask => semantic == CanonicalSemantic::Mask,
        RecognitionDeclaredType::Latent => semantic == CanonicalSemantic::Latent,
        RecognitionDeclaredType::Model => matches!(
            semantic,
            CanonicalSemantic::Model | CanonicalSemantic::ImageModel | CanonicalSemantic::Vae
        ),
        RecognitionDeclaredType::VideoModel => semantic == CanonicalSemantic::VideoModel,
        RecognitionDeclaredType::Unknown => true,
        RecognitionDeclaredType::Standard | RecognitionDeclaredType::DynamicCombo => false,
        RecognitionDeclaredType::Boolean => false,
    }
}

fn canonical_for_declared(declared: RecognitionDeclaredType) -> CanonicalSemantic {
    match declared {
        RecognitionDeclaredType::String => CanonicalSemantic::PromptText,
        RecognitionDeclaredType::Conditioning => CanonicalSemantic::Conditioning,
        RecognitionDeclaredType::Image => CanonicalSemantic::Image,
        RecognitionDeclaredType::Video => CanonicalSemantic::Video,
        RecognitionDeclaredType::Audio => CanonicalSemantic::Audio,
        RecognitionDeclaredType::Mask => CanonicalSemantic::Mask,
        RecognitionDeclaredType::Latent => CanonicalSemantic::Latent,
        RecognitionDeclaredType::Model => CanonicalSemantic::Model,
        RecognitionDeclaredType::VideoModel => CanonicalSemantic::VideoModel,
        _ => CanonicalSemantic::Unknown,
    }
}

fn canonical_for_semantic_type(semantic_type: SemanticType) -> CanonicalSemantic {
    match semantic_type {
        SemanticType::Text => CanonicalSemantic::PromptText,
        SemanticType::Conditioning => CanonicalSemantic::Conditioning,
        SemanticType::Image => CanonicalSemantic::Image,
        SemanticType::ImageList => CanonicalSemantic::ImageList,
        SemanticType::Video => CanonicalSemantic::Video,
        SemanticType::VideoList => CanonicalSemantic::VideoList,
        SemanticType::Audio => CanonicalSemantic::Audio,
        SemanticType::AudioList => CanonicalSemantic::AudioList,
        SemanticType::Latent => CanonicalSemantic::Latent,
        SemanticType::Model => CanonicalSemantic::Model,
        SemanticType::VideoModel => CanonicalSemantic::VideoModel,
        SemanticType::Mask => CanonicalSemantic::Mask,
        SemanticType::Unknown => CanonicalSemantic::Unknown,
    }
}

fn normalized_raw_type(raw_type: &str) -> String {
    raw_type.trim().to_ascii_uppercase()
}

fn is_dynamic_raw_type(
    raw_type: &str,
    match_template: Option<&RecognitionMatchTypeTemplate>,
) -> bool {
    match_template.is_some()
        || matches!(
            normalized_raw_type(raw_type).as_str(),
            "*" | "COMFY_MATCHTYPE_V3" | "COMFY_AUTOGROW_V3" | "COMFY_DYNAMICCOMBO_V3"
        )
}

fn type_resolution_for_declared(
    declared: RecognitionDeclaredType,
    raw_type: &str,
    list: bool,
    match_template: Option<&RecognitionMatchTypeTemplate>,
) -> SchemaTypeResolution {
    let raw_type = raw_type.trim().to_owned();
    if matches!(
        declared,
        RecognitionDeclaredType::Unknown | RecognitionDeclaredType::DynamicCombo
    ) && is_dynamic_raw_type(&raw_type, match_template)
    {
        return SchemaTypeResolution::DynamicUnresolved { raw_type };
    }
    if declared == RecognitionDeclaredType::Unknown {
        return if raw_type.is_empty() {
            SchemaTypeResolution::Unresolved
        } else {
            SchemaTypeResolution::OpaqueCustom { raw_type }
        };
    }
    SchemaTypeResolution::KnownSemantic {
        raw_type,
        semantic_type: semantic_type_for_declared(declared, list),
    }
}

fn resolution_for_public_declared(declared: RecognitionDeclaredType) -> SchemaTypeResolution {
    type_resolution_for_declared(declared, &format!("{declared:?}"), false, None)
}

fn resolve_schema_input(
    input_schema: &crate::application::workflow_recognition_schema::RecognitionInputSchema,
    hint: Option<SemanticInputHint>,
) -> SemanticInputResolution {
    let declared = input_schema.declared_type;
    let type_resolution = type_resolution_for_declared(
        declared,
        &input_schema.raw_type,
        false,
        input_schema.match_template.as_ref(),
    );
    let schema_type = declared_semantic_type(declared);
    let hint_is_compatible = declared != RecognitionDeclaredType::Unknown
        && hint.is_some_and(|hint| canonical_matches_declared(hint.semantic, declared));
    let semantic_type = type_resolution.semantic_type();
    let semantic = if hint_is_compatible {
        hint.map(|hint| hint.semantic)
            .unwrap_or(CanonicalSemantic::Unknown)
    } else if declared != RecognitionDeclaredType::Unknown {
        canonical_for_declared(declared)
    } else {
        CanonicalSemantic::Unknown
    };
    let mut evidence = if declared == RecognitionDeclaredType::Unknown {
        Vec::new()
    } else {
        vec![CanonicalSemanticEvidence {
            semantic,
            source: if declared == RecognitionDeclaredType::Standard {
                SemanticEvidenceSource::StandardSocketType
            } else {
                SemanticEvidenceSource::SchemaDeclared
            },
            confidence: SemanticHintConfidence::High,
        }]
    };
    if hint_is_compatible {
        if let Some(hint) = hint {
            evidence.push(hint.evidence);
        }
    }
    let semantic_key = hint
        .filter(|_| hint_is_compatible)
        .map(|hint| hint.semantic_key.to_owned())
        .or_else(|| {
            schema_type
                .filter(|_| {
                    matches!(
                        declared,
                        RecognitionDeclaredType::Image
                            | RecognitionDeclaredType::Video
                            | RecognitionDeclaredType::Audio
                    )
                })
                .map(|_| semantic.semantic_key().to_owned())
        });
    SemanticInputResolution {
        semantic_type,
        semantic,
        semantic_key,
        explicit_reference: hint_is_compatible && hint.is_some_and(|hint| hint.explicit_reference),
        type_resolution,
        evidence,
    }
}

pub fn resolve_semantic_input(
    declared: Option<RecognitionDeclaredType>,
    input_name: &str,
    hint: Option<SemanticInputHint>,
) -> SemanticInputResolution {
    if let Some(declared) = declared {
        let hint = hint.or_else(|| canonical_semantic_hint(input_name));
        let raw_type = format!("{declared:?}");
        let mut resolution = resolve_schema_input(
            &crate::application::workflow_recognition_schema::RecognitionInputSchema {
                name: input_name.to_owned(),
                raw_type,
                declared_type: declared,
                match_template: None,
                upload_media_kind: None,
                required: false,
                enum_options: Vec::new(),
                enum_values: Vec::new(),
                numeric_min: None,
                numeric_max: None,
                numeric_step: None,
                multiline: false,
                default_value: None,
                socketless: false,
                dynamic_prefix: None,
                dynamic_names: Vec::new(),
                dynamic_value_type: None,
            },
            hint,
        );
        if declared == RecognitionDeclaredType::Unknown {
            resolution.type_resolution = resolution_for_public_declared(declared);
        }
        return resolution;
    }

    let hint = hint.or_else(|| canonical_semantic_hint(input_name));
    let semantic = hint
        .map(|hint| hint.semantic)
        .unwrap_or(CanonicalSemantic::Unknown);
    let semantic_type = semantic.semantic_type();
    let semantic_key =
        (semantic != CanonicalSemantic::Unknown).then(|| semantic.semantic_key().to_owned());
    let evidence = hint.map(|hint| vec![hint.evidence]).unwrap_or_default();
    SemanticInputResolution {
        semantic_type,
        semantic,
        semantic_key,
        explicit_reference: hint.is_some_and(|hint| hint.explicit_reference),
        type_resolution: if semantic_type == SemanticType::Unknown {
            SchemaTypeResolution::Unresolved
        } else {
            SchemaTypeResolution::KnownSemantic {
                raw_type: "INFERRED".to_owned(),
                semantic_type,
            }
        },
        evidence,
    }
}

pub fn canonical_suggestion_for_input(
    input_name: &str,
    value: &Value,
    linked: bool,
) -> Option<CanonicalSuggestion> {
    if value.is_object() || value.is_boolean() || value.is_null() {
        return None;
    }
    let hint = canonical_semantic_hint(input_name)?;
    let semantic = hint.semantic;
    if linked {
        return Some(CanonicalSuggestion {
            semantic,
            semantic_key: Some(hint.semantic_key.to_owned()),
            field_type: semantic.field_type().to_owned(),
            evidence: hint.evidence,
        });
    }
    let supported = if semantic.is_numeric() {
        value.is_number()
    } else if semantic.media_family().is_some() {
        value.is_string() || value.is_array()
    } else {
        value.is_string()
    };
    if !supported {
        return None;
    }
    let field_type = if value.is_array() {
        match semantic.media_family() {
            Some("image") => "images",
            Some("video") => "videos",
            Some("audio") => "audios",
            _ => semantic.field_type(),
        }
    } else if semantic == CanonicalSemantic::Seed && !is_integer_json_number(value) {
        return None;
    } else if semantic.is_numeric() && semantic != CanonicalSemantic::Seed {
        if is_integer_json_number(value) {
            "integer"
        } else {
            "number"
        }
    } else {
        semantic.field_type()
    };
    let semantic_key = if value.is_array() && semantic.media_family().is_some() {
        Some(
            match semantic.media_family() {
                Some("image") => {
                    if semantic.is_reference() {
                        "reference_images"
                    } else {
                        "images"
                    }
                }
                Some("video") => {
                    if semantic.is_reference() {
                        "reference_videos"
                    } else {
                        "videos"
                    }
                }
                Some("audio") => {
                    if semantic.is_reference() {
                        "reference_audios"
                    } else {
                        "audios"
                    }
                }
                _ => semantic.semantic_key(),
            }
            .to_owned(),
        )
    } else {
        Some(semantic.semantic_key().to_owned())
    };
    Some(CanonicalSuggestion {
        semantic,
        semantic_key,
        field_type: field_type.to_owned(),
        evidence: hint.evidence,
    })
}

fn is_integer_json_number(value: &Value) -> bool {
    value.as_i64().is_some() || value.as_u64().is_some()
}

fn canonical_for_declared_output(
    declared: RecognitionDeclaredType,
    list: bool,
) -> CanonicalSemantic {
    match declared {
        RecognitionDeclaredType::Image => {
            if list {
                CanonicalSemantic::ImageList
            } else {
                CanonicalSemantic::Image
            }
        }
        RecognitionDeclaredType::Video => {
            if list {
                CanonicalSemantic::VideoList
            } else {
                CanonicalSemantic::Video
            }
        }
        RecognitionDeclaredType::Audio => {
            if list {
                CanonicalSemantic::AudioList
            } else {
                CanonicalSemantic::Audio
            }
        }
        _ => canonical_for_declared(declared),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveDependencyGraph {
    pub roots: Vec<String>,
    pub active_nodes: BTreeSet<String>,
    pub active_edges: Vec<WorkflowLink>,
    pub disconnected_nodes: BTreeSet<String>,
}

impl ActiveDependencyGraph {
    pub fn from_graph(graph: &WorkflowGraph, roots: &[String]) -> Result<Self, WorkflowGraphError> {
        let mut normalized_roots = roots
            .iter()
            .filter(|node_id| graph.nodes.contains(*node_id))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        normalized_roots.sort();
        if normalized_roots.is_empty() {
            return Err(WorkflowGraphError::NoOutputRoots);
        }
        let active_nodes: BTreeSet<String> = normalized_roots
            .iter()
            .flat_map(|root| graph.upstream_closure(root))
            .collect();
        let active_edges = graph
            .upstream
            .values()
            .flatten()
            .filter(|link| {
                active_nodes.contains(&link.source_node_id)
                    && active_nodes.contains(&link.target_node_id)
            })
            .cloned()
            .collect();
        let disconnected_nodes = graph.nodes.difference(&active_nodes).cloned().collect();

        Ok(Self {
            roots: normalized_roots,
            active_nodes,
            active_edges,
            disconnected_nodes,
        })
    }

    pub fn from_workflow(
        workflow: &WorkflowDocument,
        roots: &[String],
    ) -> Result<Self, WorkflowGraphError> {
        let graph = WorkflowGraph::from_document(workflow)?;
        Self::from_graph(&graph, roots)
    }
}

/// The upstream closure belonging to one resolved output root.  A shared
/// upstream node may intentionally appear in more than one closure; the
/// closures are not a partition of the workflow graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootDependencyClosure {
    pub root_id: String,
    pub root_node_id: String,
    pub active_nodes: BTreeSet<String>,
    pub active_edges: Vec<WorkflowLink>,
}

impl RootDependencyClosure {
    pub fn from_graph(
        graph: &WorkflowGraph,
        root_id: impl Into<String>,
        root_node_id: &str,
    ) -> Result<Self, WorkflowGraphError> {
        if !graph.nodes.contains(root_node_id) {
            return Err(WorkflowGraphError::NoOutputRoots);
        }
        let active_nodes = graph.upstream_closure(root_node_id);
        let active_edges = graph
            .upstream
            .values()
            .flatten()
            .filter(|link| {
                active_nodes.contains(&link.source_node_id)
                    && active_nodes.contains(&link.target_node_id)
            })
            .cloned()
            .collect();
        Ok(Self {
            root_id: root_id.into(),
            root_node_id: root_node_id.to_owned(),
            active_nodes,
            active_edges,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticNodeInput {
    pub name: String,
    pub semantic_type: SemanticType,
    pub canonical_semantic: CanonicalSemantic,
    pub type_resolution: SchemaTypeResolution,
    pub required: bool,
    pub connected: bool,
    pub match_template: Option<RecognitionMatchTypeTemplate>,
    pub semantic_key: Option<String>,
    pub raw_type: Option<String>,
    pub explicit_reference: bool,
    pub semantic_evidence: Vec<CanonicalSemanticEvidence>,
    #[serde(skip)]
    dynamic_parent_semantic: Option<CanonicalSemantic>,
    #[serde(skip)]
    dynamic_member_expected_type: Option<RecognitionDeclaredType>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticNodeOutput {
    pub index: usize,
    pub semantic_type: SemanticType,
    pub canonical_semantic: CanonicalSemantic,
    pub type_resolution: SchemaTypeResolution,
    pub raw_type: Option<String>,
    pub semantic_evidence: Vec<CanonicalSemanticEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticEvidence {
    pub level: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticNode {
    pub node_id: String,
    pub class_type: String,
    pub role: SemanticNodeRole,
    pub inputs: Vec<SemanticNodeInput>,
    pub outputs: Vec<SemanticNodeOutput>,
    pub output_match_types: Vec<Option<String>>,
    pub evidence: Vec<SemanticEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticDependencyIssue {
    pub code: String,
    pub node_id: String,
    pub input_name: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticGraph {
    pub active: ActiveDependencyGraph,
    pub roots: Vec<String>,
    pub nodes: BTreeMap<String, SemanticNode>,
    pub edges: Vec<WorkflowLink>,
    pub unknown_active_dependencies: Vec<SemanticDependencyIssue>,
    pub noncritical_unknown_nodes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityOutput {
    pub node_id: String,
    pub output_type: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RootCapabilityReadiness {
    Ready,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowCapabilityReadiness {
    Ready,
    PartiallySupported,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootCapabilityProfile {
    pub root_id: String,
    pub root_node_id: String,
    #[serde(rename = "outputType")]
    pub output_type: String,
    pub primary_capability: Option<String>,
    pub secondary_capabilities: Vec<String>,
    pub required_external_inputs: Vec<String>,
    pub optional_external_inputs: Vec<String>,
    pub outputs: Vec<CapabilityOutput>,
    pub unknown_dependencies: Vec<SemanticDependencyIssue>,
    pub warnings: Vec<String>,
    pub readiness: RootCapabilityReadiness,
    pub usable: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProfile {
    pub roots: Vec<RootCapabilityProfile>,
    pub aggregate_capabilities: Vec<String>,
    pub readiness: WorkflowCapabilityReadiness,
    pub selected_root_id: Option<String>,
    pub primary_capability: Option<String>,
    pub secondary_capabilities: Vec<String>,
    pub required_external_inputs: Vec<String>,
    pub optional_external_inputs: Vec<String>,
    pub outputs: Vec<CapabilityOutput>,
    pub unknown_dependencies: Vec<SemanticDependencyIssue>,
    pub warnings: Vec<String>,
    pub noncritical_unknown_nodes: Vec<String>,
    pub usable: bool,
    pub reason: Option<String>,
}

pub fn resolve_semantic_graph(
    workflow: &WorkflowDocument,
    schema: &RecognitionSchemaContext,
    active: ActiveDependencyGraph,
) -> SemanticGraph {
    let mut nodes = BTreeMap::new();
    let mut unknown_active_dependencies = Vec::new();
    let mut noncritical_unknown_nodes = Vec::new();
    let all_nodes = workflow.value().as_object();

    for node_id in &active.active_nodes {
        let Some(_node) = all_nodes.and_then(|nodes| nodes.get(node_id)) else {
            unknown_active_dependencies.push(SemanticDependencyIssue {
                code: "unknown_semantic_dependency".to_owned(),
                node_id: node_id.clone(),
                input_name: None,
                message: "active workflow node is missing from the parsed document".to_owned(),
            });
            continue;
        };
        let class_type = workflow.class_type(node_id).unwrap_or_default().to_owned();
        let Some(node_schema) = schema.node(&class_type) else {
            unknown_active_dependencies.push(unknown_node_issue(node_id, &class_type, true));
            continue;
        };
        let semantic_node = resolve_node(workflow, node_id, node_schema);
        nodes.insert(node_id.clone(), semantic_node);
    }

    resolve_dynamic_types(schema, &active.active_edges, &mut nodes);
    for (node_id, node) in &mut nodes {
        if let Some(node_schema) = schema.node(&node.class_type) {
            apply_frame_index_roles(workflow.inputs(node_id), node_schema, &mut node.inputs);
        }
    }
    collect_active_dependency_issues(workflow, &active, &nodes, &mut unknown_active_dependencies);

    for node_id in &active.disconnected_nodes {
        let class_type = workflow.class_type(node_id).unwrap_or_default();
        if schema.node(class_type).is_none() {
            noncritical_unknown_nodes.push(node_id.clone());
        }
    }
    noncritical_unknown_nodes.sort();

    SemanticGraph {
        roots: active.roots.clone(),
        edges: active.active_edges.clone(),
        active,
        nodes,
        unknown_active_dependencies,
        noncritical_unknown_nodes,
    }
}

fn unknown_node_issue(node_id: &str, class_type: &str, active: bool) -> SemanticDependencyIssue {
    SemanticDependencyIssue {
        code: if active {
            "unknown_semantic_dependency".to_owned()
        } else {
            "noncritical_unknown_node".to_owned()
        },
        node_id: node_id.to_owned(),
        input_name: None,
        message: format!("workflow node class {class_type} has no schema evidence"),
    }
}

fn resolve_node(
    workflow: &WorkflowDocument,
    node_id: &str,
    schema: &RecognitionNodeSchema,
) -> SemanticNode {
    let class_type = workflow.class_type(node_id).unwrap_or_default().to_owned();
    let mut inputs = Vec::new();
    let workflow_inputs = workflow.inputs(node_id);
    let mut input_names = schema.inputs.keys().cloned().collect::<BTreeSet<_>>();
    if let Some(workflow_inputs) = workflow_inputs {
        input_names.extend(workflow_inputs.keys().cloned());
    }

    for input_name in input_names {
        let input_schema = resolve_node_input_schema(schema, workflow_inputs, &input_name);
        let dynamic_parent_semantic = input_schema
            .as_ref()
            .and_then(|_| dynamic_parent_semantic(schema, &input_name));
        let value = workflow_inputs.and_then(|inputs| inputs.get(&input_name));
        if value.is_none()
            && schema
                .input(&input_name)
                .is_some_and(|input| has_dynamic_instance(schema, workflow_inputs, input))
        {
            continue;
        }
        let connected = value.is_some_and(|value| is_link(value));
        let hint = linked_target_semantic(&input_name);
        let resolution = input_schema
            .as_ref()
            .map(|input| resolve_schema_input(input, hint))
            .unwrap_or_else(|| resolve_semantic_input(None, &input_name, hint));
        let mut semantic_evidence = resolution.evidence.clone();
        let (semantic_key, explicit_reference, dynamic_member_expected_type) =
            if let (Some(parent_semantic), Some(input_schema)) =
                (dynamic_parent_semantic, input_schema.as_ref())
            {
                if input_schema.dynamic_value_type == Some(RecognitionDeclaredType::Image) {
                    if let Some(role) = dynamic_image_member_role(parent_semantic) {
                        semantic_evidence.push(CanonicalSemanticEvidence {
                            semantic: role,
                            source: SemanticEvidenceSource::GraphContext,
                            confidence: SemanticHintConfidence::Medium,
                        });
                        (
                            Some(role.semantic_key().to_owned()),
                            role.is_reference(),
                            input_schema.dynamic_value_type,
                        )
                    } else {
                        (
                            resolution.semantic_key.clone(),
                            resolution.explicit_reference,
                            None,
                        )
                    }
                } else {
                    (
                        resolution.semantic_key.clone(),
                        resolution.explicit_reference,
                        None,
                    )
                }
            } else {
                (
                    resolution.semantic_key.clone(),
                    resolution.explicit_reference,
                    None,
                )
            };
        if connected && resolution.semantic != CanonicalSemantic::Unknown {
            semantic_evidence.push(CanonicalSemanticEvidence {
                semantic: resolution.semantic,
                source: SemanticEvidenceSource::GraphContext,
                confidence: SemanticHintConfidence::Medium,
            });
        }
        let semantic_type = resolution.semantic_type;
        let required = input_schema.as_ref().is_some_and(|input| input.required);
        let raw_type = input_schema
            .as_ref()
            .map(|input| input.raw_type.clone())
            .or_else(|| resolution.type_resolution.raw_type().map(str::to_owned));
        inputs.push(SemanticNodeInput {
            name: input_name,
            semantic_type,
            canonical_semantic: resolution.semantic,
            type_resolution: resolution.type_resolution,
            required,
            connected,
            match_template: input_schema
                .as_ref()
                .and_then(|input| input.match_template.clone()),
            semantic_key,
            raw_type,
            explicit_reference,
            semantic_evidence,
            dynamic_parent_semantic,
            dynamic_member_expected_type,
        });
    }

    let output_count = schema
        .declared_output_types
        .len()
        .max(schema.raw_output_types.len());
    let outputs = (0..output_count)
        .map(|index| {
            let declared = schema
                .declared_output_types
                .get(index)
                .copied()
                .unwrap_or(RecognitionDeclaredType::Unknown);
            let raw_type = schema
                .raw_output_types
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("{declared:?}"));
            let type_resolution = type_resolution_for_declared(
                declared,
                &raw_type,
                schema.output_is_list.get(index).copied().unwrap_or(false),
                None,
            );
            let semantic_type = type_resolution.semantic_type();
            let canonical_semantic = if semantic_type == SemanticType::Unknown {
                canonical_for_declared_output(
                    declared,
                    schema.output_is_list.get(index).copied().unwrap_or(false),
                )
            } else {
                canonical_for_semantic_type(semantic_type)
            };
            SemanticNodeOutput {
                index,
                semantic_type,
                canonical_semantic,
                type_resolution,
                raw_type: Some(raw_type),
                semantic_evidence: vec![CanonicalSemanticEvidence {
                    semantic: canonical_semantic,
                    source: SemanticEvidenceSource::SchemaDeclared,
                    confidence: SemanticHintConfidence::High,
                }],
            }
        })
        .collect::<Vec<_>>();
    let role = infer_role(&class_type, schema, &inputs, &outputs);
    let evidence = evidence_for_node(schema, &inputs, &outputs, &role);

    SemanticNode {
        node_id: node_id.to_owned(),
        class_type,
        role,
        inputs,
        outputs,
        output_match_types: schema.output_match_types.clone(),
        evidence,
    }
}

fn dynamic_parent_semantic(
    schema: &RecognitionNodeSchema,
    input_name: &str,
) -> Option<CanonicalSemantic> {
    schema.inputs.values().find_map(|parent| {
        if parent.name == input_name || !dynamic_input_name_matches(schema, parent, input_name) {
            return None;
        }
        canonical_semantic_hint(&parent.name).map(|hint| hint.semantic)
    })
}

fn dynamic_image_member_role(parent: CanonicalSemantic) -> Option<CanonicalSemantic> {
    match parent {
        CanonicalSemantic::ReferenceImageList => Some(CanonicalSemantic::ReferenceImage),
        CanonicalSemantic::ImageList => Some(CanonicalSemantic::Image),
        _ => None,
    }
}

fn apply_frame_index_roles(
    workflow_inputs: Option<&serde_json::Map<String, Value>>,
    schema: &RecognitionNodeSchema,
    inputs: &mut [SemanticNodeInput],
) {
    let Some(workflow_inputs) = workflow_inputs else {
        return;
    };
    let frame_roles = workflow_inputs
        .iter()
        .filter_map(|(name, value)| {
            let normalized = normalize_input_name(name);
            if !matches!(normalized.as_str(), "frame_idx" | "frame_index")
                || schema.input(name).map_or(true, |input| {
                    input.declared_type != RecognitionDeclaredType::Integer
                })
            {
                return None;
            }
            match value.as_i64()? {
                0 => Some(CanonicalSemantic::FirstFrame),
                -1 => Some(CanonicalSemantic::LastFrame),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    if frame_roles.len() != 1 {
        return;
    }
    let role = frame_roles[0];
    let mut image_inputs = inputs.iter_mut().filter(|input| {
        input.connected
            && input.semantic_type == SemanticType::Image
            && matches!(
                input.canonical_semantic,
                CanonicalSemantic::Image
                    | CanonicalSemantic::SourceImage
                    | CanonicalSemantic::ReferenceImage
            )
    });
    let Some(image_input) = image_inputs.next() else {
        return;
    };
    if image_inputs.next().is_some() {
        return;
    }
    image_input
        .semantic_evidence
        .push(CanonicalSemanticEvidence {
            semantic: role,
            source: SemanticEvidenceSource::GraphContext,
            confidence: SemanticHintConfidence::Medium,
        });
    if !image_input.explicit_reference {
        image_input.canonical_semantic = role;
        image_input.semantic_key = Some(role.semantic_key().to_owned());
    }
}

fn has_dynamic_instance(
    schema: &RecognitionNodeSchema,
    workflow_inputs: Option<&serde_json::Map<String, Value>>,
    base: &crate::application::workflow_recognition_schema::RecognitionInputSchema,
) -> bool {
    workflow_inputs.is_some_and(|inputs| {
        inputs.keys().any(|input_name| {
            input_name != &base.name && dynamic_input_name_matches(schema, base, input_name)
        })
    })
}

fn dynamic_input_name_matches(
    _schema: &RecognitionNodeSchema,
    base: &crate::application::workflow_recognition_schema::RecognitionInputSchema,
    input_name: &str,
) -> bool {
    let base_prefix = format!("{}.", base.name);
    let (suffix, includes_base_prefix) = if let Some(suffix) = input_name.strip_prefix(&base_prefix)
    {
        (suffix, true)
    } else if let Some(prefix) = base.dynamic_prefix.as_deref() {
        let Some(suffix) = input_name.strip_prefix(prefix) else {
            return false;
        };
        (suffix, false)
    } else {
        return false;
    };

    if let Some(prefix) = base.dynamic_prefix.as_deref() {
        if includes_base_prefix {
            return suffix
                .strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty());
        }
        return !suffix.is_empty();
    }
    !base.dynamic_names.is_empty() && base.dynamic_names.iter().any(|name| name == suffix)
}

fn resolve_node_input_schema(
    schema: &RecognitionNodeSchema,
    workflow_inputs: Option<&serde_json::Map<String, Value>>,
    input_name: &str,
) -> Option<crate::application::workflow_recognition_schema::RecognitionInputSchema> {
    if let Some(input) = schema.input(input_name) {
        return Some(input.clone());
    }

    if let Some((selector_name, conditional_name)) = input_name.split_once('.') {
        let selected_value = workflow_inputs
            .and_then(|inputs| inputs.get(selector_name))
            .and_then(Value::as_str);
        if let Some(input) = selected_value
            .and_then(|selected_value| {
                schema
                    .conditional_inputs
                    .get(selector_name)
                    .and_then(|by_value| by_value.get(selected_value))
            })
            .and_then(|inputs| inputs.get(conditional_name))
        {
            return Some(input.clone());
        }
    }

    schema.inputs.values().find_map(|base| {
        if !dynamic_input_name_matches(schema, base, input_name) {
            return None;
        }
        let mut resolved = base.clone();
        resolved.name = input_name.to_owned();
        Some(resolved)
    })
}

fn resolve_dynamic_types(
    schema: &RecognitionSchemaContext,
    edges: &[WorkflowLink],
    nodes: &mut BTreeMap<String, SemanticNode>,
) {
    let max_passes = nodes.len().saturating_mul(3).max(1);
    for _ in 0..max_passes {
        let mut changed = false;

        for edge in edges {
            let Some(source_output) = nodes
                .get(&edge.source_node_id)
                .and_then(|node| node.outputs.get(edge.source_output_index as usize))
                .cloned()
            else {
                continue;
            };
            let Some(target_input) = nodes
                .get(&edge.target_node_id)
                .and_then(|node| {
                    node.inputs
                        .iter()
                        .find(|input| input.name == edge.target_input)
                })
                .cloned()
            else {
                continue;
            };

            if let Some(target_node) = nodes.get_mut(&edge.target_node_id) {
                if let Some(input) = target_node
                    .inputs
                    .iter_mut()
                    .find(|input| input.name == edge.target_input)
                {
                    changed |= resolve_dynamic_input_from_source(input, &source_output);
                }
            }

            if let Some(source_node) = nodes.get_mut(&edge.source_node_id) {
                if let Some(output) = source_node
                    .outputs
                    .get_mut(edge.source_output_index as usize)
                {
                    changed |= resolve_dynamic_output_from_target(output, &target_input);
                }
            }
        }

        let node_ids = nodes.keys().cloned().collect::<Vec<_>>();
        for node_id in node_ids {
            let Some(node) = nodes.get_mut(&node_id) else {
                continue;
            };
            changed |= resolve_match_outputs(node);
            if let Some(node_schema) = schema.node(&node.class_type) {
                let role = infer_role(&node.class_type, node_schema, &node.inputs, &node.outputs);
                if node.role != role {
                    node.role = role;
                    changed = true;
                }
                node.evidence = evidence_for_node(node_schema, &node.inputs, &node.outputs, &role);
            }
        }

        if !changed {
            break;
        }
    }
}

fn resolve_dynamic_input_from_source(
    input: &mut SemanticNodeInput,
    source: &SemanticNodeOutput,
) -> bool {
    if !matches!(
        input.type_resolution,
        SchemaTypeResolution::DynamicUnresolved { .. }
    ) {
        return false;
    }
    if input
        .dynamic_member_expected_type
        .is_some_and(|expected| !source_matches_declared_type(expected, source))
    {
        let raw_type = input
            .type_resolution
            .raw_type()
            .unwrap_or_default()
            .to_owned();
        input.type_resolution = SchemaTypeResolution::Conflict {
            raw_type,
            message: "connected source type conflicts with the dynamic member value type"
                .to_owned(),
        };
        input.semantic_type = SemanticType::Unknown;
        input.canonical_semantic = CanonicalSemantic::Unknown;
        input.semantic_key = None;
        input.explicit_reference = false;
        return true;
    }
    match &source.type_resolution {
        SchemaTypeResolution::KnownSemantic { .. }
        | SchemaTypeResolution::DynamicResolved { .. } => {
            if !match_allows_source(
                input.match_template.as_ref(),
                source.raw_type.as_deref(),
                source.semantic_type,
            ) {
                let raw_type = input
                    .type_resolution
                    .raw_type()
                    .unwrap_or_default()
                    .to_owned();
                input.type_resolution = SchemaTypeResolution::Conflict {
                    raw_type,
                    message: "connected source type is outside the declared dynamic constraint"
                        .to_owned(),
                };
                return true;
            }
            set_input_semantic_type(input, source.semantic_type, true)
        }
        SchemaTypeResolution::OpaqueCustom { raw_type } => {
            if !match_allows_source(
                input.match_template.as_ref(),
                Some(raw_type),
                SemanticType::Unknown,
            ) {
                let input_raw_type = input
                    .type_resolution
                    .raw_type()
                    .unwrap_or_default()
                    .to_owned();
                input.type_resolution = SchemaTypeResolution::Conflict {
                    raw_type: input_raw_type,
                    message:
                        "connected opaque source type is outside the declared dynamic constraint"
                            .to_owned(),
                };
                return true;
            }
            if input.type_resolution
                == (SchemaTypeResolution::OpaqueCustom {
                    raw_type: raw_type.clone(),
                })
            {
                false
            } else {
                input.type_resolution = SchemaTypeResolution::OpaqueCustom {
                    raw_type: raw_type.clone(),
                };
                true
            }
        }
        SchemaTypeResolution::DynamicUnresolved { .. }
        | SchemaTypeResolution::Unresolved
        | SchemaTypeResolution::Conflict { .. } => false,
    }
}

fn source_matches_declared_type(
    declared: RecognitionDeclaredType,
    source: &SemanticNodeOutput,
) -> bool {
    match declared {
        RecognitionDeclaredType::Image => source.semantic_type == SemanticType::Image,
        RecognitionDeclaredType::Video => source.semantic_type == SemanticType::Video,
        RecognitionDeclaredType::Audio => source.semantic_type == SemanticType::Audio,
        RecognitionDeclaredType::Mask => source.semantic_type == SemanticType::Mask,
        RecognitionDeclaredType::Latent => source.semantic_type == SemanticType::Latent,
        RecognitionDeclaredType::Model => source.semantic_type == SemanticType::Model,
        RecognitionDeclaredType::VideoModel => source.semantic_type == SemanticType::VideoModel,
        RecognitionDeclaredType::Conditioning => source.semantic_type == SemanticType::Conditioning,
        RecognitionDeclaredType::String => source.semantic_type == SemanticType::Text,
        _ => true,
    }
}

fn resolve_dynamic_output_from_target(
    output: &mut SemanticNodeOutput,
    target: &SemanticNodeInput,
) -> bool {
    if !matches!(
        output.type_resolution,
        SchemaTypeResolution::DynamicUnresolved { .. }
    ) || target.semantic_type == SemanticType::Unknown
    {
        return false;
    }
    let raw_type = output
        .type_resolution
        .raw_type()
        .unwrap_or_default()
        .to_owned();
    output.type_resolution = SchemaTypeResolution::DynamicResolved {
        raw_type,
        semantic_type: target.semantic_type,
    };
    output.semantic_type = target.semantic_type;
    output.canonical_semantic = canonical_for_semantic_type(target.semantic_type);
    true
}

fn resolve_match_outputs(node: &mut SemanticNode) -> bool {
    let mut changed = false;
    for index in 0..node.outputs.len() {
        let Some(match_type) = node
            .output_match_types
            .get(index)
            .and_then(|match_type| match_type.as_deref())
        else {
            continue;
        };
        let candidates = node
            .inputs
            .iter()
            .filter(|input| input.connected)
            .filter(|input| {
                input
                    .match_template
                    .as_ref()
                    .is_some_and(|template| template.template_id == match_type)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            continue;
        }
        let known_types = candidates
            .iter()
            .filter_map(|input| match input.type_resolution {
                SchemaTypeResolution::KnownSemantic { semantic_type, .. }
                | SchemaTypeResolution::DynamicResolved { semantic_type, .. }
                    if semantic_type != SemanticType::Unknown =>
                {
                    Some(semantic_type)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let opaque_types = candidates
            .iter()
            .filter_map(|input| match &input.type_resolution {
                SchemaTypeResolution::OpaqueCustom { raw_type } => Some(raw_type.clone()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let output = &mut node.outputs[index];
        if known_types.len() == 1 && opaque_types.is_empty() {
            changed |= set_output_semantic_type(output, *known_types.first().unwrap(), true);
        } else if known_types.is_empty() && opaque_types.len() == 1 {
            let raw_type = opaque_types.into_iter().next().unwrap();
            let next = SchemaTypeResolution::OpaqueCustom { raw_type };
            if output.type_resolution != next {
                output.type_resolution = next;
                output.semantic_type = SemanticType::Unknown;
                output.canonical_semantic = CanonicalSemantic::Unknown;
                changed = true;
            }
        } else if known_types.len() > 1 {
            let raw_type = output
                .type_resolution
                .raw_type()
                .unwrap_or_default()
                .to_owned();
            let next = SchemaTypeResolution::Conflict {
                raw_type,
                message: "matching inputs resolve to incompatible semantic types".to_owned(),
            };
            if output.type_resolution != next {
                output.type_resolution = next;
                output.semantic_type = SemanticType::Unknown;
                output.canonical_semantic = CanonicalSemantic::Unknown;
                changed = true;
            }
        }
    }
    changed
}

fn match_allows_source(
    template: Option<&RecognitionMatchTypeTemplate>,
    source_raw_type: Option<&str>,
    source_semantic_type: SemanticType,
) -> bool {
    let Some(template) = template else {
        return true;
    };
    if template.allowed_types.is_empty() || template.allowed_types.iter().any(|raw| raw == "*") {
        return true;
    }
    let source_raw_type = source_raw_type.map(normalized_raw_type);
    template.allowed_types.iter().any(|allowed| {
        let allowed = normalized_raw_type(allowed);
        source_raw_type.as_deref() == Some(allowed.as_str())
            || semantic_type_matches_raw(source_semantic_type, &allowed)
    })
}

fn semantic_type_matches_raw(semantic_type: SemanticType, raw_type: &str) -> bool {
    match semantic_type {
        SemanticType::Image | SemanticType::ImageList => raw_type == "IMAGE",
        SemanticType::Video | SemanticType::VideoList => raw_type == "VIDEO",
        SemanticType::Audio | SemanticType::AudioList => raw_type == "AUDIO",
        SemanticType::Mask => raw_type == "MASK",
        SemanticType::Latent => raw_type == "LATENT",
        SemanticType::Model => raw_type == "MODEL",
        SemanticType::VideoModel => raw_type == "VIDEO_MODEL" || raw_type == "VIDEOMODEL",
        SemanticType::Conditioning => raw_type == "CONDITIONING",
        SemanticType::Text => matches!(raw_type, "STRING" | "TEXT"),
        SemanticType::Unknown => false,
    }
}

fn set_input_semantic_type(
    input: &mut SemanticNodeInput,
    semantic_type: SemanticType,
    dynamic: bool,
) -> bool {
    let raw_type = input
        .type_resolution
        .raw_type()
        .unwrap_or_default()
        .to_owned();
    let next = if dynamic {
        SchemaTypeResolution::DynamicResolved {
            raw_type,
            semantic_type,
        }
    } else {
        SchemaTypeResolution::KnownSemantic {
            raw_type,
            semantic_type,
        }
    };
    if input.type_resolution == next && input.semantic_type == semantic_type {
        return false;
    }
    input.type_resolution = next;
    input.semantic_type = semantic_type;
    let hinted = input
        .semantic_key
        .as_deref()
        .and_then(canonical_semantic_hint)
        .filter(|hint| hint.semantic.semantic_type() == semantic_type);
    input.canonical_semantic = hinted
        .map(|hint| hint.semantic)
        .unwrap_or_else(|| canonical_for_semantic_type(semantic_type));
    if input.semantic_key.is_none() && input.canonical_semantic != CanonicalSemantic::Unknown {
        input.semantic_key = Some(input.canonical_semantic.semantic_key().to_owned());
    }
    true
}

fn set_output_semantic_type(
    output: &mut SemanticNodeOutput,
    semantic_type: SemanticType,
    dynamic: bool,
) -> bool {
    let raw_type = output
        .type_resolution
        .raw_type()
        .unwrap_or_default()
        .to_owned();
    let next = if dynamic {
        SchemaTypeResolution::DynamicResolved {
            raw_type,
            semantic_type,
        }
    } else {
        SchemaTypeResolution::KnownSemantic {
            raw_type,
            semantic_type,
        }
    };
    if output.type_resolution == next && output.semantic_type == semantic_type {
        return false;
    }
    output.type_resolution = next;
    output.semantic_type = semantic_type;
    output.canonical_semantic = canonical_for_semantic_type(semantic_type);
    true
}

fn collect_active_dependency_issues(
    workflow: &WorkflowDocument,
    active: &ActiveDependencyGraph,
    nodes: &BTreeMap<String, SemanticNode>,
    issues: &mut Vec<SemanticDependencyIssue>,
) {
    for node_id in &active.active_nodes {
        let Some(node) = nodes.get(node_id) else {
            continue;
        };
        let workflow_inputs = workflow.inputs(node_id);
        for input in &node.inputs {
            let value = workflow_inputs.and_then(|inputs| inputs.get(&input.name));
            let blocks = match &input.type_resolution {
                SchemaTypeResolution::Unresolved => {
                    input.connected && opaque_input_is_capability_critical(node, input)
                }
                SchemaTypeResolution::DynamicUnresolved { .. } => {
                    if input.connected {
                        opaque_input_is_capability_critical(node, input)
                    } else {
                        input.required && value.is_none()
                    }
                }
                SchemaTypeResolution::Conflict { .. } => input.connected || input.required,
                SchemaTypeResolution::OpaqueCustom { .. } => {
                    input.connected && opaque_input_is_capability_critical(node, input)
                }
                SchemaTypeResolution::KnownSemantic { .. }
                | SchemaTypeResolution::DynamicResolved { .. } => false,
            };
            if blocks {
                let raw_type = input
                    .type_resolution
                    .raw_type()
                    .unwrap_or("unresolved")
                    .to_owned();
                issues.push(SemanticDependencyIssue {
                    code: "unknown_semantic_dependency".to_owned(),
                    node_id: node_id.clone(),
                    input_name: Some(input.name.clone()),
                    message: format!(
                        "active input {} has unresolved capability-relevant schema type {}",
                        input.name, raw_type
                    ),
                });
            }
            if input.required && value.is_none() && is_external_semantic_type(input.semantic_type) {
                issues.push(SemanticDependencyIssue {
                    code: "invalid_required_input".to_owned(),
                    node_id: node_id.clone(),
                    input_name: Some(input.name.clone()),
                    message: format!("required semantic input {} is not present", input.name),
                });
            }
        }

        let has_known_media_input = node
            .inputs
            .iter()
            .any(|input| is_media_semantic_type(input.semantic_type));
        let opaque_media_sink = has_known_media_input
            && !node.outputs.is_empty()
            && node.outputs.iter().all(|output| {
                matches!(
                    output.type_resolution,
                    SchemaTypeResolution::OpaqueCustom { .. }
                )
            });
        if active.roots.contains(node_id)
            && ((!node.outputs.is_empty()
                && node.outputs.iter().any(|output| {
                    output.semantic_type == SemanticType::Unknown
                        || !matches!(
                            output.type_resolution,
                            SchemaTypeResolution::KnownSemantic { .. }
                                | SchemaTypeResolution::DynamicResolved { .. }
                        )
                })
                && !opaque_media_sink)
                || (node.outputs.is_empty() && !has_known_media_input))
        {
            issues.push(SemanticDependencyIssue {
                code: "unknown_output".to_owned(),
                node_id: node_id.clone(),
                input_name: None,
                message: "active output root has no safely resolved media type".to_owned(),
            });
        }
    }
}

fn opaque_input_is_capability_critical(
    node: &SemanticNode,
    opaque_input: &SemanticNodeInput,
) -> bool {
    let has_known_media_output = node
        .outputs
        .iter()
        .any(|output| is_media_semantic_type(output.semantic_type));
    let has_known_capability_input = node.inputs.iter().any(|input| {
        input.name != opaque_input.name
            && matches!(
                input.semantic_type,
                SemanticType::Text
                    | SemanticType::Conditioning
                    | SemanticType::Image
                    | SemanticType::ImageList
                    | SemanticType::Video
                    | SemanticType::VideoList
                    | SemanticType::Audio
                    | SemanticType::AudioList
            )
    });
    has_known_media_output && !has_known_capability_input
}

fn is_media_semantic_type(semantic_type: SemanticType) -> bool {
    matches!(
        semantic_type,
        SemanticType::Image
            | SemanticType::ImageList
            | SemanticType::Video
            | SemanticType::VideoList
            | SemanticType::Audio
            | SemanticType::AudioList
    )
}

fn evidence_for_node(
    schema: &RecognitionNodeSchema,
    inputs: &[SemanticNodeInput],
    outputs: &[SemanticNodeOutput],
    role: &SemanticNodeRole,
) -> Vec<SemanticEvidence> {
    let mut evidence = Vec::new();
    if !outputs.is_empty() {
        evidence.push(SemanticEvidence {
            level: "schema".to_owned(),
            kind: "declared_output_type".to_owned(),
            detail: format!("{} declared output(s)", outputs.len()),
        });
    }
    if inputs
        .iter()
        .any(|input| input.semantic_type != SemanticType::Unknown)
    {
        evidence.push(SemanticEvidence {
            level: "schema".to_owned(),
            kind: "declared_input_type".to_owned(),
            detail: "at least one input has a recognized semantic type".to_owned(),
        });
    }
    if schema.output_node {
        evidence.push(SemanticEvidence {
            level: "graph_context".to_owned(),
            kind: "output_node".to_owned(),
            detail: "schema marks this node as an output".to_owned(),
        });
    }
    if *role != SemanticNodeRole::Unknown {
        evidence.push(SemanticEvidence {
            level: "class_title".to_owned(),
            kind: "generic_role".to_owned(),
            detail: format!("resolved role {role:?}"),
        });
    }
    evidence
}

fn infer_role(
    class_type: &str,
    schema: &RecognitionNodeSchema,
    inputs: &[SemanticNodeInput],
    outputs: &[SemanticNodeOutput],
) -> SemanticNodeRole {
    let output_types = outputs
        .iter()
        .map(|output| output.semantic_type)
        .collect::<BTreeSet<_>>();
    let input_types = inputs
        .iter()
        .map(|input| input.semantic_type)
        .collect::<BTreeSet<_>>();
    let lower = format!(
        "{} {}",
        class_type,
        schema.display_name.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase();
    if schema.output_node {
        if output_types.contains(&SemanticType::Video)
            || output_types.contains(&SemanticType::VideoList)
        {
            return SemanticNodeRole::VideoOutput;
        }
        if output_types.contains(&SemanticType::Audio)
            || output_types.contains(&SemanticType::AudioList)
        {
            return SemanticNodeRole::AudioOutput;
        }
        if output_types.contains(&SemanticType::Image)
            || output_types.contains(&SemanticType::ImageList)
        {
            return SemanticNodeRole::ImageOutput;
        }
        if input_types.contains(&SemanticType::Video)
            || input_types.contains(&SemanticType::VideoList)
        {
            return SemanticNodeRole::VideoOutput;
        }
        if input_types.contains(&SemanticType::Audio)
            || input_types.contains(&SemanticType::AudioList)
        {
            return SemanticNodeRole::AudioOutput;
        }
        if input_types.contains(&SemanticType::Image)
            || input_types.contains(&SemanticType::ImageList)
        {
            return SemanticNodeRole::ImageOutput;
        }
    }
    if output_types.contains(&SemanticType::Video)
        || output_types.contains(&SemanticType::VideoList)
    {
        if schema_has_upload(schema, MediaKind::Video) {
            return SemanticNodeRole::VideoSource;
        }
        if input_types.contains(&SemanticType::Conditioning)
            || input_types.contains(&SemanticType::Text)
            || lower.contains("generate")
            || lower.contains("sample")
        {
            return SemanticNodeRole::VideoGenerator;
        }
        if lower.contains("decode") {
            return SemanticNodeRole::VideoDecoder;
        }
        if lower.contains("load") || lower.contains("input") {
            return SemanticNodeRole::VideoSource;
        }
    }
    if output_types.contains(&SemanticType::Image)
        || output_types.contains(&SemanticType::ImageList)
    {
        if schema_has_upload(schema, MediaKind::Image) {
            return SemanticNodeRole::ImageSource;
        }
        if input_types.contains(&SemanticType::Conditioning)
            || input_types.contains(&SemanticType::Text)
            || lower.contains("generate")
            || lower.contains("sample")
        {
            return SemanticNodeRole::ImageGenerator;
        }
        if lower.contains("encode") {
            return SemanticNodeRole::ImageEncoder;
        }
        if lower.contains("decode") {
            return SemanticNodeRole::ImageDecoder;
        }
        if lower.contains("load") || lower.contains("input") {
            return SemanticNodeRole::ImageSource;
        }
    }
    if output_types.contains(&SemanticType::Audio)
        || output_types.contains(&SemanticType::AudioList)
    {
        if schema_has_upload(schema, MediaKind::Audio) {
            return SemanticNodeRole::AudioSource;
        }
        if lower.contains("load") || lower.contains("input") {
            return SemanticNodeRole::AudioSource;
        }
        return SemanticNodeRole::AudioGenerator;
    }
    if output_types.contains(&SemanticType::Model)
        || output_types.contains(&SemanticType::VideoModel)
    {
        return SemanticNodeRole::ModelLoader;
    }
    if output_types.contains(&SemanticType::Conditioning)
        && (input_types.contains(&SemanticType::Text) || lower.contains("text"))
    {
        return SemanticNodeRole::TextConditioning;
    }
    if output_types.contains(&SemanticType::Latent) && input_types.contains(&SemanticType::Image) {
        return SemanticNodeRole::LatentEncoder;
    }
    if output_types
        .iter()
        .any(|output| *output != SemanticType::Unknown)
    {
        SemanticNodeRole::GenericTransform
    } else {
        SemanticNodeRole::Unknown
    }
}

fn schema_has_upload(schema: &RecognitionNodeSchema, media: MediaKind) -> bool {
    schema
        .inputs
        .values()
        .any(|input| input.upload_media_kind == Some(media))
}

fn semantic_type_for_declared(declared: RecognitionDeclaredType, list: bool) -> SemanticType {
    match declared {
        RecognitionDeclaredType::String => SemanticType::Text,
        RecognitionDeclaredType::Conditioning => SemanticType::Conditioning,
        RecognitionDeclaredType::Image => {
            if list {
                SemanticType::ImageList
            } else {
                SemanticType::Image
            }
        }
        RecognitionDeclaredType::Video => {
            if list {
                SemanticType::VideoList
            } else {
                SemanticType::Video
            }
        }
        RecognitionDeclaredType::Audio => {
            if list {
                SemanticType::AudioList
            } else {
                SemanticType::Audio
            }
        }
        RecognitionDeclaredType::Latent => SemanticType::Latent,
        RecognitionDeclaredType::Model => SemanticType::Model,
        RecognitionDeclaredType::VideoModel => SemanticType::VideoModel,
        RecognitionDeclaredType::Mask => SemanticType::Mask,
        _ => SemanticType::Unknown,
    }
}

fn is_link(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|values| values.len() >= 2 && values[0].is_string() && values[1].is_u64())
}

fn is_external_semantic_type(semantic_type: SemanticType) -> bool {
    matches!(
        semantic_type,
        SemanticType::Text
            | SemanticType::Image
            | SemanticType::ImageList
            | SemanticType::Video
            | SemanticType::VideoList
            | SemanticType::Audio
            | SemanticType::AudioList
            | SemanticType::Conditioning
            | SemanticType::Latent
            | SemanticType::Mask
            | SemanticType::Model
            | SemanticType::VideoModel
    )
}

fn is_user_external_semantic_type(semantic_type: SemanticType) -> bool {
    matches!(
        semantic_type,
        SemanticType::Text
            | SemanticType::Image
            | SemanticType::ImageList
            | SemanticType::Video
            | SemanticType::VideoList
            | SemanticType::Audio
            | SemanticType::AudioList
    )
}

fn is_external_media_source(role: SemanticNodeRole) -> bool {
    matches!(
        role,
        SemanticNodeRole::ImageSource
            | SemanticNodeRole::VideoSource
            | SemanticNodeRole::AudioSource
    )
}

fn frame_input_has_external_image_ancestor(
    closure: &RootDependencyClosure,
    semantic: &SemanticGraph,
    target_node_id: &str,
    target_input: &str,
) -> bool {
    let mut pending = closure
        .active_edges
        .iter()
        .filter(|edge| edge.target_node_id == target_node_id && edge.target_input == target_input)
        .map(|edge| edge.source_node_id.clone())
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();

    while let Some(node_id) = pending.pop() {
        if !visited.insert(node_id.clone()) {
            continue;
        }
        let Some(node) = semantic.nodes.get(&node_id) else {
            continue;
        };
        if node.role == SemanticNodeRole::ImageSource {
            return true;
        }
        if node.role != SemanticNodeRole::GenericTransform {
            continue;
        }
        for input in node.inputs.iter().filter(|input| {
            input.connected
                && matches!(
                    input.semantic_type,
                    SemanticType::Image | SemanticType::ImageList
                )
        }) {
            pending.extend(
                closure
                    .active_edges
                    .iter()
                    .filter(|edge| {
                        edge.target_node_id == node_id && edge.target_input == input.name
                    })
                    .map(|edge| edge.source_node_id.clone()),
            );
        }
    }
    false
}

pub fn build_root_capability_profile(
    analysis: &WorkflowAnalysisReport,
    semantic: &SemanticGraph,
    closure: &RootDependencyClosure,
    root: &OutputRoot,
) -> RootCapabilityProfile {
    let mut required_external_inputs = Vec::new();
    let mut optional_external_inputs = Vec::new();
    let mut has_image = false;
    let mut has_explicit_reference = false;
    let mut has_video = false;
    let mut has_audio = false;
    let mut has_first_frame = false;
    let mut has_last_frame = false;

    for node_id in &closure.active_nodes {
        let Some(node) = semantic.nodes.get(node_id) else {
            continue;
        };
        for input in &node.inputs {
            if input.connected {
                if matches!(
                    node.role,
                    SemanticNodeRole::ImageOutput
                        | SemanticNodeRole::VideoOutput
                        | SemanticNodeRole::AudioOutput
                ) {
                    continue;
                }
                let source_is_external = closure.active_edges.iter().any(|link| {
                    link.target_node_id == node.node_id
                        && link.target_input == input.name
                        && semantic
                            .nodes
                            .get(&link.source_node_id)
                            .is_some_and(|source| is_external_media_source(source.role))
                });
                let has_first_role = input.canonical_semantic == CanonicalSemantic::FirstFrame
                    || input
                        .semantic_evidence
                        .iter()
                        .any(|evidence| evidence.semantic == CanonicalSemantic::FirstFrame);
                let has_last_role = input.canonical_semantic == CanonicalSemantic::LastFrame
                    || input
                        .semantic_evidence
                        .iter()
                        .any(|evidence| evidence.semantic == CanonicalSemantic::LastFrame);
                let frame_has_external_ancestor = (has_first_role || has_last_role)
                    && input.semantic_type == SemanticType::Image
                    && frame_input_has_external_image_ancestor(
                        closure,
                        semantic,
                        &node.node_id,
                        &input.name,
                    );
                if !source_is_external {
                    if frame_has_external_ancestor {
                        has_explicit_reference |= input.explicit_reference;
                        has_first_frame |= has_first_role;
                        has_last_frame |= has_last_role;
                    }
                    continue;
                }
                match input.semantic_type {
                    SemanticType::Image | SemanticType::ImageList => {
                        has_image = true;
                        has_explicit_reference |= input.explicit_reference;
                        has_first_frame |= has_first_role;
                        has_last_frame |= has_last_role;
                    }
                    SemanticType::Video | SemanticType::VideoList => {
                        has_video = true;
                        has_explicit_reference |= input.explicit_reference;
                    }
                    SemanticType::Audio | SemanticType::AudioList => {
                        has_audio = true;
                        has_explicit_reference |= input.explicit_reference;
                    }
                    _ => {}
                }
            } else if is_user_external_semantic_type(input.semantic_type)
                && !matches!(
                    node.role,
                    SemanticNodeRole::ImageOutput
                        | SemanticNodeRole::VideoOutput
                        | SemanticNodeRole::AudioOutput
                )
            {
                let target = if input.required {
                    &mut required_external_inputs
                } else {
                    &mut optional_external_inputs
                };
                target.push(format!("{}.{}", node.node_id, input.name));
            }
        }
    }
    required_external_inputs.sort();
    required_external_inputs.dedup();
    optional_external_inputs.sort();
    optional_external_inputs.dedup();

    let outputs = analysis
        .outputs
        .iter()
        .filter(|output| output.node_id == root.node_id)
        .map(|output| CapabilityOutput {
            node_id: output.node_id.clone(),
            output_type: output.output_type.clone(),
        })
        .collect::<Vec<_>>();
    let frame_reference_conflict = (has_first_frame || has_last_frame) && has_explicit_reference;
    let primary_capability = match root.output_type.as_str() {
        "video" if frame_reference_conflict => None,
        "video" if has_first_frame && has_last_frame => {
            Some("first_last_frame_to_video".to_owned())
        }
        "video" if has_first_frame || has_last_frame => Some("image_to_video".to_owned()),
        "video" if has_video || has_audio || has_explicit_reference => {
            Some("reference_to_video".to_owned())
        }
        "video" if has_image => Some("image_to_video".to_owned()),
        "video" => Some("text_to_video".to_owned()),
        "image" if has_image || has_video || has_audio => Some("image_to_image".to_owned()),
        "image" => Some("text_to_image".to_owned()),
        _ => None,
    };

    let mut secondary_capabilities = Vec::new();
    if has_image {
        secondary_capabilities.push("image_input".to_owned());
    }
    if has_video {
        secondary_capabilities.push("video_input".to_owned());
    }
    if has_audio {
        secondary_capabilities.push("audio_input".to_owned());
    }

    let mut unknown_dependencies = semantic
        .unknown_active_dependencies
        .iter()
        .filter(|issue| issue.node_id.is_empty() || closure.active_nodes.contains(&issue.node_id))
        .cloned()
        .collect::<Vec<_>>();
    if !outputs.is_empty() {
        // Output-root analysis is an independent, already-selected capability
        // boundary.  It can identify a terminal output whose live object_info
        // entry is present but omits output socket metadata (for example a
        // sparse compatibility response).  Keep the semantic graph strict
        // when used directly, but do not let that missing socket detail
        // override an established per-root output contract here.
        unknown_dependencies
            .retain(|issue| !(issue.code == "unknown_output" && issue.node_id == root.node_id));
    }
    if outputs.is_empty() {
        unknown_dependencies.push(SemanticDependencyIssue {
            code: "unknown_output".to_owned(),
            node_id: String::new(),
            input_name: None,
            message: "output root has no recognized output".to_owned(),
        });
    }
    let usable = unknown_dependencies.is_empty() && primary_capability.is_some();
    let reason = if usable {
        None
    } else if !unknown_dependencies.is_empty() {
        Some("output root contains unknown semantic dependencies".to_owned())
    } else if outputs.is_empty() {
        Some("output root has no recognized output".to_owned())
    } else {
        Some("output root capability could not be resolved".to_owned())
    };
    let mut warnings = semantic
        .noncritical_unknown_nodes
        .iter()
        .filter(|node_id| closure.active_nodes.contains(*node_id))
        .cloned()
        .collect::<Vec<_>>();
    if root.confidence
        == crate::application::workflow_recognition_service::RecognitionConfidence::Low
    {
        warnings.push("output root recognition confidence is low".to_owned());
    }
    warnings.sort();
    warnings.dedup();

    RootCapabilityProfile {
        root_id: closure.root_id.clone(),
        root_node_id: closure.root_node_id.clone(),
        output_type: root.output_type.clone(),
        primary_capability,
        secondary_capabilities,
        required_external_inputs,
        optional_external_inputs,
        outputs,
        unknown_dependencies,
        warnings,
        readiness: if usable {
            RootCapabilityReadiness::Ready
        } else {
            RootCapabilityReadiness::Unsupported
        },
        usable,
        reason,
    }
}

pub fn build_capability_profile_for_roots(
    analysis: &WorkflowAnalysisReport,
    semantic: &SemanticGraph,
    closures: &[RootDependencyClosure],
    roots: &[OutputRoot],
    selected_root_id: Option<&str>,
) -> CapabilityProfile {
    let mut roots = closures
        .iter()
        .filter_map(|closure| {
            roots
                .iter()
                .find(|root| root.node_id == closure.root_node_id)
                .map(|root| build_root_capability_profile(analysis, semantic, closure, root))
        })
        .collect::<Vec<_>>();
    roots.sort_by(|left, right| {
        left.root_id
            .cmp(&right.root_id)
            .then(left.root_node_id.cmp(&right.root_node_id))
    });

    let aggregate_capabilities = roots
        .iter()
        .filter_map(|profile| profile.primary_capability.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let primary_capability = if let Some(selected) = selected_root_id {
        roots
            .iter()
            .find(|profile| profile.root_id == selected || profile.root_node_id == selected)
            .and_then(|profile| profile.primary_capability.clone())
    } else if !roots.is_empty()
        && roots
            .iter()
            .all(|profile| profile.primary_capability.is_some())
        && aggregate_capabilities.len() == 1
    {
        aggregate_capabilities.first().cloned()
    } else {
        None
    };

    let secondary_capabilities = roots
        .iter()
        .filter_map(|profile| profile.primary_capability.as_deref())
        .filter(|capability| Some(*capability) != primary_capability.as_deref())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let readiness = if roots.is_empty()
        || roots
            .iter()
            .all(|profile| profile.readiness == RootCapabilityReadiness::Unsupported)
    {
        WorkflowCapabilityReadiness::Unsupported
    } else if roots
        .iter()
        .all(|profile| profile.readiness == RootCapabilityReadiness::Ready)
    {
        WorkflowCapabilityReadiness::Ready
    } else {
        WorkflowCapabilityReadiness::PartiallySupported
    };

    let required_external_inputs = roots
        .iter()
        .flat_map(|profile| profile.required_external_inputs.iter().cloned())
        .collect::<BTreeSet<_>>();
    let required_external_inputs = required_external_inputs.into_iter().collect::<Vec<_>>();
    let optional_external_inputs = roots
        .iter()
        .flat_map(|profile| profile.optional_external_inputs.iter().cloned())
        .collect::<BTreeSet<_>>();
    let optional_external_inputs = optional_external_inputs.into_iter().collect::<Vec<_>>();

    let mut outputs = roots
        .iter()
        .flat_map(|profile| profile.outputs.iter().cloned())
        .collect::<Vec<_>>();
    outputs.sort_by(|left, right| {
        left.node_id
            .cmp(&right.node_id)
            .then(left.output_type.cmp(&right.output_type))
    });
    outputs.dedup();

    let mut unknown_dependencies = roots
        .iter()
        .flat_map(|profile| profile.unknown_dependencies.iter().cloned())
        .collect::<Vec<_>>();
    unknown_dependencies.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.node_id.cmp(&right.node_id))
            .then(left.input_name.cmp(&right.input_name))
            .then(left.message.cmp(&right.message))
    });
    unknown_dependencies.dedup();

    let warnings = roots
        .iter()
        .flat_map(|profile| {
            profile
                .warnings
                .iter()
                .map(|warning| format!("{}: {warning}", profile.root_id))
        })
        .collect::<Vec<_>>();

    let usable = readiness == WorkflowCapabilityReadiness::Ready
        && roots.iter().all(|profile| profile.usable);
    let reason = match readiness {
        WorkflowCapabilityReadiness::Ready => None,
        WorkflowCapabilityReadiness::PartiallySupported => {
            Some("one or more output roots are unsupported".to_owned())
        }
        WorkflowCapabilityReadiness::Unsupported => {
            Some("no output root has a usable capability".to_owned())
        }
    };

    CapabilityProfile {
        roots,
        aggregate_capabilities,
        readiness,
        selected_root_id: selected_root_id.map(str::to_owned),
        primary_capability,
        secondary_capabilities,
        required_external_inputs,
        optional_external_inputs,
        outputs,
        unknown_dependencies,
        warnings,
        noncritical_unknown_nodes: semantic.noncritical_unknown_nodes.clone(),
        usable,
        reason,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        build_capability_profile_for_roots, canonical_semantic_hint,
        canonical_suggestion_for_input, resolve_semantic_graph, resolve_semantic_input,
        ActiveDependencyGraph, CanonicalSemantic, RootCapabilityReadiness, RootDependencyClosure,
        SchemaTypeResolution, SemanticNodeRole, SemanticType, WorkflowCapabilityReadiness,
    };
    use crate::{
        application::{
            workflow_analysis_service::{
                OutputRootSelection, WorkflowAnalysisReport, WorkflowAnalysisService,
            },
            workflow_graph_analysis::WorkflowGraph,
            workflow_recognition_schema::{RecognitionDeclaredType, RecognitionSchemaContext},
            workflow_recognition_service::WorkflowIdentity,
        },
        domain::WorkflowDocument,
    };
    use serde_json::{json, Value};

    pub(crate) fn run_replay_test(name: &str) {
        match name {
            "active_dependency_graph_excludes_disconnected_nodes_and_keeps_multiple_roots" => {
                active_dependency_graph_excludes_disconnected_nodes_and_keeps_multiple_roots()
            }
            "semantic_resolver_prefers_schema_types_for_an_unknown_generator" => {
                semantic_resolver_prefers_schema_types_for_an_unknown_generator()
            }
            "disconnected_missing_schema_is_noncritical_but_active_missing_schema_blocks" => {
                disconnected_missing_schema_is_noncritical_but_active_missing_schema_blocks()
            }
            "capability_uses_active_image_connection_not_optional_schema_presence" => {
                capability_uses_active_image_connection_not_optional_schema_presence()
            }
            "generic_reference_media_is_distinct_from_generic_image_to_video" => {
                generic_reference_media_is_distinct_from_generic_image_to_video()
            }
            "active_unknown_critical_socket_fails_closed_without_provider_rules" => {
                active_unknown_critical_socket_fails_closed_without_provider_rules()
            }
            "PER_ROOT_CLOSURE_DOES_NOT_UNION_INPUTS" => PER_ROOT_CLOSURE_DOES_NOT_UNION_INPUTS(),
            "TWO_COMPATIBLE_VIDEO_ROOTS" => TWO_COMPATIBLE_VIDEO_ROOTS(),
            "TWO_DIFFERENT_CAPABILITY_ROOTS" => TWO_DIFFERENT_CAPABILITY_ROOTS(),
            "ROOT_A_READY_ROOT_B_UNSUPPORTED" => ROOT_A_READY_ROOT_B_UNSUPPORTED(),
            "IMAGE_INPUT_DOES_NOT_LEAK_ACROSS_ROOTS" => IMAGE_INPUT_DOES_NOT_LEAK_ACROSS_ROOTS(),
            "REFERENCE_SEMANTIC_DOES_NOT_LEAK_ACROSS_ROOTS" => {
                REFERENCE_SEMANTIC_DOES_NOT_LEAK_ACROSS_ROOTS()
            }
            "SHARED_UPSTREAM_CAN_BELONG_TO_MULTIPLE_ROOTS" => {
                SHARED_UPSTREAM_CAN_BELONG_TO_MULTIPLE_ROOTS()
            }
            "INTERNAL_GENERATED_MEDIA_IS_NOT_EXTERNAL_INPUT" => {
                INTERNAL_GENERATED_MEDIA_IS_NOT_EXTERNAL_INPUT()
            }
            "ROOT_UNKNOWN_DEPENDENCY_IS_SCOPED" => ROOT_UNKNOWN_DEPENDENCY_IS_SCOPED(),
            "EXPLICIT_SELECTED_ROOT_CONTROLS_PRIMARY_WITHOUT_DROPPING_OTHER_ROOTS" => {
                EXPLICIT_SELECTED_ROOT_CONTROLS_PRIMARY_WITHOUT_DROPPING_OTHER_ROOTS()
            }
            "PER_ROOT_OUTPUT_TYPES_ARE_PRESERVED" => PER_ROOT_OUTPUT_TYPES_ARE_PRESERVED(),
            "PER_ROOT_CAPABILITY_IS_ORDER_INDEPENDENT" => {
                PER_ROOT_CAPABILITY_IS_ORDER_INDEPENDENT()
            }
            "SCHEMA_TYPE_OVERRIDES_ALIAS_NAME" => SCHEMA_TYPE_OVERRIDES_ALIAS_NAME(),
            "CLASS_TITLE_CANNOT_OVERRIDE_SCHEMA_TYPE" => CLASS_TITLE_CANNOT_OVERRIDE_SCHEMA_TYPE(),
            "SPARSE_SCHEMA_T2V" => SPARSE_SCHEMA_T2V(),
            "SPARSE_SCHEMA_I2V" => SPARSE_SCHEMA_I2V(),
            "UNKNOWN_ROLE_KNOWN_TYPES_NONBLOCKING" => UNKNOWN_ROLE_KNOWN_TYPES_NONBLOCKING(),
            "UNKNOWN_TRANSFORM_WITH_KNOWN_MEDIA_TYPES" => {
                UNKNOWN_TRANSFORM_WITH_KNOWN_MEDIA_TYPES()
            }
            "UNKNOWN_SOURCE_WITH_KNOWN_IMAGE_OUTPUT" => UNKNOWN_SOURCE_WITH_KNOWN_IMAGE_OUTPUT(),
            "UNKNOWN_SINK_WITH_KNOWN_VIDEO_INPUT" => UNKNOWN_SINK_WITH_KNOWN_VIDEO_INPUT(),
            "UNKNOWN_REQUIRED_CUSTOM_TYPE_FAILS_CLOSED" => {
                UNKNOWN_REQUIRED_CUSTOM_TYPE_FAILS_CLOSED()
            }
            "REFERENCE_LIST_SCHEMA_IS_REFERENCE" => REFERENCE_LIST_SCHEMA_IS_REFERENCE(),
            "REFERENCE_INDEXED_SLOTS_ARE_REFERENCE" => REFERENCE_INDEXED_SLOTS_ARE_REFERENCE(),
            "MULTIPLE_IMAGE_INPUTS_NOT_REFERENCE" => MULTIPLE_IMAGE_INPUTS_NOT_REFERENCE(),
            "REFERENCE_ALIAS_CANNOT_OVERRIDE_NON_IMAGE_SCHEMA" => {
                REFERENCE_ALIAS_CANNOT_OVERRIDE_NON_IMAGE_SCHEMA()
            }
            "CANONICAL_PROMPT_HINT_IS_SHARED" => CANONICAL_PROMPT_HINT_IS_SHARED(),
            "CANONICAL_MEDIA_HINT_IS_SHARED" => CANONICAL_MEDIA_HINT_IS_SHARED(),
            "INTERNAL_GENERATED_MEDIA_REMAINS_INTERNAL" => {
                INTERNAL_GENERATED_MEDIA_REMAINS_INTERNAL()
            }
            "REFERENCE_SEMANTIC_REMAINS_ROOT_SCOPED" => REFERENCE_SEMANTIC_REMAINS_ROOT_SCOPED(),
            "CANONICAL_RESOLUTION_IS_ORDER_INDEPENDENT" => {
                CANONICAL_RESOLUTION_IS_ORDER_INDEPENDENT()
            }
            "SCHEMA_TYPE_LAYERING_PRESERVES_RAW_CUSTOM_TYPES" => {
                SCHEMA_TYPE_LAYERING_PRESERVES_RAW_CUSTOM_TYPES()
            }
            "OPAQUE_CUSTOM_CONTROL_NONBLOCKING" => OPAQUE_CUSTOM_CONTROL_NONBLOCKING(),
            "ARBITRARY_OPAQUE_TYPES_DO_NOT_REQUIRE_ADAPTER" => {
                ARBITRARY_OPAQUE_TYPES_DO_NOT_REQUIRE_ADAPTER()
            }
            "OPAQUE_EXTERNAL_MEDIA_BOUNDARY_FAILS_CLOSED" => {
                OPAQUE_EXTERNAL_MEDIA_BOUNDARY_FAILS_CLOSED()
            }
            "OPAQUE_OUTPUT_BOUNDARY_FAILS_CLOSED" => OPAQUE_OUTPUT_BOUNDARY_FAILS_CLOSED(),
            "OPAQUE_MEDIA_SINK_OUTPUT_IS_NONBLOCKING" => OPAQUE_MEDIA_SINK_OUTPUT_IS_NONBLOCKING(),
            "WILDCARD_RESOLVES_FROM_KNOWN_SOURCE" => WILDCARD_RESOLVES_FROM_KNOWN_SOURCE(),
            "WILDCARD_RESOLVES_FROM_KNOWN_TARGET" => WILDCARD_RESOLVES_FROM_KNOWN_TARGET(),
            "UNCONSTRAINED_WILDCARD_CAPABILITY_BOUNDARY_FAILS_CLOSED" => {
                UNCONSTRAINED_WILDCARD_CAPABILITY_BOUNDARY_FAILS_CLOSED()
            }
            "UNCONSTRAINED_WILDCARD_AUXILIARY_NONBLOCKING" => {
                UNCONSTRAINED_WILDCARD_AUXILIARY_NONBLOCKING()
            }
            "MATCHTYPE_PROPAGATES_KNOWN_MEDIA" => MATCHTYPE_PROPAGATES_KNOWN_MEDIA(),
            "MATCHTYPE_UNRESOLVED_CRITICAL_FAILS_CLOSED" => {
                MATCHTYPE_UNRESOLVED_CRITICAL_FAILS_CLOSED()
            }
            "AUTOGROW_INPUT_RESOLVES_FROM_KNOWN_SOURCE" => {
                AUTOGROW_INPUT_RESOLVES_FROM_KNOWN_SOURCE()
            }
            "CONDITIONAL_INPUT_RESOLVES_SELECTED_OPTION" => {
                CONDITIONAL_INPUT_RESOLVES_SELECTED_OPTION()
            }
            "AUTOGROW_PREFIX_INPUT_RESOLVES_FROM_KNOWN_SOURCE" => {
                AUTOGROW_PREFIX_INPUT_RESOLVES_FROM_KNOWN_SOURCE()
            }
            "CUSTOM_TYPE_NAME_DOES_NOT_CHANGE_CAPABILITY" => {
                CUSTOM_TYPE_NAME_DOES_NOT_CHANGE_CAPABILITY()
            }
            "OPAQUE_TYPE_ORDER_INDEPENDENT" => OPAQUE_TYPE_ORDER_INDEPENDENT(),
            _ => panic!("unregistered semantic-graph replay test: {name}"),
        }
    }

    fn schema() -> RecognitionSchemaContext {
        RecognitionSchemaContext::parse(&json!({
            "TextSource": {
                "output": ["CONDITIONING"],
                "input": {"required": {"text": ["STRING", {}]}}
            },
            "NeutralVideoGenerator": {
                "output": ["VIDEO"],
                "input": {
                    "required": {"conditioning": ["CONDITIONING", {}]},
                    "optional": {"image": ["IMAGE", {}]}
                }
            },
            "VideoOutput": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"video": ["VIDEO", {}]}}
            },
            "ImageOutput": {
                "output": ["IMAGE"],
                "output_node": true,
                "input": {"required": {"image": ["IMAGE", {}]}}
            },
            "LoadImage": {
                "output": ["IMAGE"],
                "input": {"required": {"image": [["available.png"], {}]}}
            }
        }))
    }

    fn workflow(image_connected: bool) -> WorkflowDocument {
        let image_input = if image_connected {
            json!(["4", 0])
        } else {
            json!(null)
        };
        WorkflowDocument::parse(json!({
            "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
            "2": {"class_type": "NeutralVideoGenerator", "inputs": {
                "conditioning": ["1", 0],
                "image": image_input
            }},
            "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}},
            "4": {"class_type": "LoadImage", "inputs": {"image": "available.png"}},
            "99": {"class_type": "DisconnectedMissingNode", "inputs": {}}
        }))
        .expect("generic workflow should parse")
    }

    fn per_root_schema() -> RecognitionSchemaContext {
        RecognitionSchemaContext::parse(&json!({
            "TextSource": {
                "output": ["CONDITIONING"],
                "input": {"required": {"text": ["STRING", {}]}}
            },
            "LoadImage": {
                "output": ["IMAGE"],
                "input": {"required": {"image": [["available.png"], {}]}}
            },
            "ImageGenerator": {
                "output": ["IMAGE"],
                "input": {
                    "required": {"conditioning": ["CONDITIONING", {}]},
                    "optional": {"image": ["IMAGE", {}]}
                }
            },
            "VideoGenerator": {
                "output": ["VIDEO"],
                "input": {
                    "required": {"conditioning": ["CONDITIONING", {}]},
                    "optional": {"image": ["IMAGE", {}]}
                }
            },
            "ReferenceVideoGenerator": {
                "output": ["VIDEO"],
                "input": {
                    "required": {"conditioning": ["CONDITIONING", {}]},
                    "optional": {"reference_images": ["IMAGE", {}]}
                }
            },
            "ImageOutput": {
                "output": ["IMAGE"],
                "output_node": true,
                "input": {"required": {"image": ["IMAGE", {}]}}
            },
            "VideoOutput": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"video": ["VIDEO", {}]}}
            }
        }))
    }

    fn dynamic_image_collection_schema() -> RecognitionSchemaContext {
        RecognitionSchemaContext::parse(&dynamic_image_collection_schema_value())
    }

    fn dynamic_image_collection_schema_value() -> Value {
        json!({
            "TextSource": {
                "output": ["CONDITIONING"],
                "input": {"required": {"text": ["STRING", {}]}}
            },
            "ImageSource": {
                "output": ["IMAGE"],
                "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
            },
            "VideoSource": {
                "output": ["VIDEO"],
                "input": {"required": {"file": ["COMBO", {"video_upload": true}]}}
            },
            "NeutralDynamicMediaConsumer": {
                "output": ["VIDEO"],
                "input": {
                    "required": {"prompt": ["STRING", {}]},
                    "optional": {
                        "reference_images": ["COMFY_AUTOGROW_V3", {
                            "template": {
                                "input": {"required": {"ref_image": ["IMAGE", {}]}},
                                "prefix": "ref_image_",
                                "min": 0,
                                "max": 9
                            }
                        }],
                        "images": ["COMFY_AUTOGROW_V3", {
                            "template": {
                                "input": {"required": {"image": ["IMAGE", {}]}},
                                "prefix": "image_",
                                "min": 0,
                                "max": 9
                            }
                        }]
                    }
                }
            },
            "VideoGenerator": {
                "output": ["VIDEO"],
                "input": {"required": {"conditioning": ["CONDITIONING", {}]}}
            },
            "VideoOutput": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"video": ["VIDEO", {}]}}
            }
        })
    }

    fn root_selection(node_id: &str, output_type: &str) -> OutputRootSelection {
        OutputRootSelection {
            node_id: node_id.to_owned(),
            output_type: output_type.to_owned(),
        }
    }

    fn per_root_profile_fixture(
        value: Value,
        explicit_roots: &[OutputRootSelection],
    ) -> (WorkflowAnalysisReport, super::CapabilityProfile) {
        let workflow = WorkflowDocument::parse(value).expect("per-root workflow should parse");
        let schema = per_root_schema();
        let bytes = serde_json::to_vec(workflow.value()).expect("workflow should serialize");
        let analysis = WorkflowAnalysisService::analyze_workflow_with_schema_and_output_roots(
            &workflow,
            &bytes,
            Some(&schema),
            explicit_roots,
        );
        let roots = analysis.output_root_resolution.roots();
        let graph = WorkflowGraph::from_document(&workflow).expect("graph should build");
        let root_nodes = roots
            .iter()
            .map(|root| root.node_id.clone())
            .collect::<Vec<_>>();
        let active = ActiveDependencyGraph::from_graph(&graph, &root_nodes)
            .expect("union graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema, active);
        let closures = roots
            .iter()
            .map(|root| {
                RootDependencyClosure::from_graph(&graph, root.output_id.clone(), &root.node_id)
                    .expect("root closure should build")
            })
            .collect::<Vec<_>>();
        let profile = build_capability_profile_for_roots(
            &analysis,
            &semantic,
            &closures,
            roots,
            analysis.selected_root_id.as_deref(),
        );
        (analysis, profile)
    }

    fn legacy_profile(
        analysis: &WorkflowAnalysisReport,
        semantic: &super::SemanticGraph,
    ) -> super::CapabilityProfile {
        let roots = analysis.output_root_resolution.roots();
        let closures = roots
            .iter()
            .map(|root| RootDependencyClosure {
                root_id: root.output_id.clone(),
                root_node_id: root.node_id.clone(),
                active_nodes: semantic.active.active_nodes.clone(),
                active_edges: semantic.active.active_edges.clone(),
            })
            .collect::<Vec<_>>();
        build_capability_profile_for_roots(analysis, semantic, &closures, roots, None)
    }

    fn profile_with_schema(
        workflow_value: Value,
        schema_value: Value,
    ) -> (
        WorkflowAnalysisReport,
        super::SemanticGraph,
        super::CapabilityProfile,
    ) {
        let workflow = WorkflowDocument::parse(workflow_value).expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&schema_value);
        let bytes = serde_json::to_vec(workflow.value()).expect("workflow should serialize");
        let analysis =
            WorkflowAnalysisService::analyze_workflow_with_schema(&workflow, &bytes, Some(&schema));
        let roots = analysis.output_root_resolution.roots();
        assert!(
            !roots.is_empty(),
            "fixture must resolve at least one output root"
        );
        let graph = WorkflowGraph::from_document(&workflow).expect("graph should build");
        let root_nodes = roots
            .iter()
            .map(|root| root.node_id.clone())
            .collect::<Vec<_>>();
        let active = ActiveDependencyGraph::from_graph(&graph, &root_nodes)
            .expect("active graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema, active);
        let closures = roots
            .iter()
            .map(|root| {
                RootDependencyClosure::from_graph(&graph, root.output_id.clone(), &root.node_id)
                    .expect("root closure should build")
            })
            .collect::<Vec<_>>();
        let profile = build_capability_profile_for_roots(
            &analysis,
            &semantic,
            &closures,
            roots,
            analysis.selected_root_id.as_deref(),
        );
        (analysis, semantic, profile)
    }

    fn sparse_schema() -> Value {
        json!({
            "NodeText": {
                "output": ["CONDITIONING"],
                "input": {"required": {"text": ["STRING", {}]}}
            },
            "NodeAlpha": {
                "output": ["VIDEO"],
                "input": {"required": {"conditioning": ["CONDITIONING", {}]}}
            },
            "NodeBeta": {
                "output": ["VIDEO"],
                "input": {"required": {
                    "conditioning": ["CONDITIONING", {}],
                    "image": ["IMAGE", {}]
                }}
            },
            "ImageInput": {
                "output": ["IMAGE"],
                "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
            },
            "NodeGamma": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"video": ["VIDEO", {}]}}
            },
            "ImageOutput": {
                "output": ["IMAGE"],
                "output_node": true,
                "input": {"required": {"image": ["IMAGE", {}]}}
            }
        })
    }

    #[test]
    #[allow(non_snake_case)]
    fn SCHEMA_TYPE_OVERRIDES_ALIAS_NAME() {
        let image = resolve_semantic_input(Some(RecognitionDeclaredType::Float), "image", None);
        assert_eq!(image.semantic_type, SemanticType::Unknown);
        assert_eq!(image.semantic, CanonicalSemantic::Unknown);
        assert!(image.semantic_key.is_none());

        let reference = resolve_semantic_input(
            Some(RecognitionDeclaredType::String),
            "reference_image",
            None,
        );
        assert_eq!(reference.semantic_type, SemanticType::Text);
        assert_eq!(reference.semantic, CanonicalSemantic::PromptText);
        assert!(!reference.explicit_reference);
        assert!(reference.semantic_key.is_none());
    }

    #[test]
    #[allow(non_snake_case)]
    fn CLASS_TITLE_CANNOT_OVERRIDE_SCHEMA_TYPE() {
        let schema = RecognitionSchemaContext::parse(&json!({
            "NodeText": {
                "output": ["CONDITIONING"],
                "input": {"required": {"text": ["STRING", {}]}}
            },
            "NodeAlpha": {
                "display_name": "Video Generator",
                "output": ["IMAGE"],
                "input": {"required": {"conditioning": ["CONDITIONING", {}]}}
            },
            "ImageOutput": {
                "output": ["IMAGE"],
                "output_node": true,
                "input": {"required": {"image": ["IMAGE", {}]}}
            }
        }));
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "NodeText", "inputs": {"text": "hello"}},
            "2": {"class_type": "NodeAlpha", "inputs": {"conditioning": ["1", 0]}},
            "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}}
        }))
        .expect("schema-priority workflow should parse");
        let active = ActiveDependencyGraph::from_workflow(&workflow, &["3".to_owned()])
            .expect("active graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema, active);
        let node = semantic.nodes.get("2").expect("transform should resolve");

        assert_eq!(node.role, SemanticNodeRole::ImageGenerator);
        assert!(node.outputs.iter().all(|output| {
            output.semantic_type == SemanticType::Image
                && output.canonical_semantic == CanonicalSemantic::Image
        }));
        assert!(semantic.unknown_active_dependencies.is_empty());
    }

    #[test]
    #[allow(non_snake_case)]
    fn SPARSE_SCHEMA_T2V() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "NodeText", "inputs": {"text": "hello"}},
                "2": {"class_type": "NodeAlpha", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "NodeGamma", "inputs": {"video": ["2", 0]}}
            }),
            sparse_schema(),
        );
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_video"));
        assert!(profile.usable);
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert_eq!(
            semantic.nodes.get("2").unwrap().role,
            SemanticNodeRole::VideoGenerator
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn SPARSE_SCHEMA_I2V() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "NodeText", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageInput", "inputs": {"file": "input.png"}},
                "3": {"class_type": "NodeBeta", "inputs": {
                    "conditioning": ["1", 0],
                    "image": ["2", 0]
                }},
                "4": {"class_type": "NodeGamma", "inputs": {"video": ["3", 0]}}
            }),
            sparse_schema(),
        );
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("image_to_video")
        );
        assert!(profile.usable);
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert_eq!(
            semantic.nodes.get("3").unwrap().role,
            SemanticNodeRole::VideoGenerator
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn UNKNOWN_ROLE_KNOWN_TYPES_NONBLOCKING() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "NodeSource", "inputs": {}},
                "2": {"class_type": "NodeAlpha", "inputs": {"image": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}}
            }),
            json!({
                "NodeSource": {"output": ["IMAGE"]},
                "NodeAlpha": {
                    "output": ["IMAGE"],
                    "input": {"required": {"image": ["IMAGE", {}]}}
                },
                "ImageOutput": {
                    "output": ["IMAGE"],
                    "output_node": true,
                    "input": {"required": {"image": ["IMAGE", {}]}}
                }
            }),
        );
        let node = semantic
            .nodes
            .get("2")
            .expect("typed transform should resolve");
        assert_eq!(node.role, SemanticNodeRole::GenericTransform);
        assert_eq!(node.inputs[0].semantic_type, SemanticType::Image);
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_image"));
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn UNKNOWN_TRANSFORM_WITH_KNOWN_MEDIA_TYPES() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageInput", "inputs": {"file": "input.png"}},
                "2": {"class_type": "NodeAlpha", "inputs": {"image": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}}
            }),
            json!({
                "ImageInput": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "NodeAlpha": {
                    "output": ["IMAGE"],
                    "input": {"required": {"image": ["IMAGE", {}]}}
                },
                "ImageOutput": {
                    "output": ["IMAGE"],
                    "output_node": true,
                    "input": {"required": {"image": ["IMAGE", {}]}}
                }
            }),
        );
        assert_eq!(
            semantic.nodes.get("2").unwrap().role,
            SemanticNodeRole::GenericTransform
        );
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("image_to_image")
        );
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn UNKNOWN_SOURCE_WITH_KNOWN_IMAGE_OUTPUT() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "NodeAlpha", "inputs": {}},
                "2": {"class_type": "ImageOutput", "inputs": {"image": ["1", 0]}}
            }),
            json!({
                "NodeAlpha": {"output": ["IMAGE"]},
                "ImageOutput": {
                    "output": ["IMAGE"],
                    "output_node": true,
                    "input": {"required": {"image": ["IMAGE", {}]}}
                }
            }),
        );
        let source = semantic.nodes.get("1").expect("source should resolve");
        assert_eq!(source.role, SemanticNodeRole::GenericTransform);
        assert_eq!(source.outputs[0].semantic_type, SemanticType::Image);
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_image"));
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn UNKNOWN_SINK_WITH_KNOWN_VIDEO_INPUT() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "NodeAlpha", "inputs": {}},
                "2": {"class_type": "NodeGamma", "inputs": {"video": ["1", 0]}}
            }),
            json!({
                "NodeAlpha": {"output": ["VIDEO"]},
                "NodeGamma": {
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        assert_eq!(
            semantic.nodes.get("2").unwrap().role,
            SemanticNodeRole::VideoOutput
        );
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_video"));
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn UNKNOWN_REQUIRED_CUSTOM_TYPE_FAILS_CLOSED() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "NodeSource", "inputs": {}},
                "2": {"class_type": "NodeAlpha", "inputs": {"mystery": ["1", 0]}},
                "3": {"class_type": "NodeGamma", "inputs": {"video": ["2", 0]}}
            }),
            json!({
                "NodeSource": {"output": ["CUSTOM_MAGIC_OBJECT"]},
                "NodeAlpha": {
                    "output": ["VIDEO"],
                    "input": {"required": {"mystery": ["CUSTOM_MAGIC_OBJECT", {}]}}
                },
                "NodeGamma": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        assert!(semantic.unknown_active_dependencies.iter().any(|issue| {
            issue.code == "unknown_semantic_dependency"
                && issue.node_id == "2"
                && issue.input_name.as_deref() == Some("mystery")
        }));
        assert!(!profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn REFERENCE_LIST_SCHEMA_IS_REFERENCE() {
        let (analysis, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "NodeText", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageInput", "inputs": {"file": "reference.png"}},
                "3": {"class_type": "NodeBeta", "inputs": {
                    "conditioning": ["1", 0],
                    "reference_images": ["2", 0]
                }},
                "4": {"class_type": "NodeGamma", "inputs": {"video": ["3", 0]}}
            }),
            json!({
                "NodeText": {
                    "output": ["CONDITIONING"],
                    "input": {"required": {"text": ["STRING", {}]}}
                },
                "ImageInput": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "NodeBeta": {
                    "output": ["VIDEO"],
                    "input": {"required": {
                        "conditioning": ["CONDITIONING", {}],
                        "reference_images": ["IMAGE", {}]
                    }}
                },
                "NodeGamma": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        let reference_input = semantic
            .nodes
            .get("3")
            .unwrap()
            .inputs
            .iter()
            .find(|input| input.name == "reference_images")
            .unwrap();
        assert_eq!(
            reference_input.canonical_semantic,
            CanonicalSemantic::ReferenceImageList
        );
        assert!(reference_input.explicit_reference);
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("reference_to_video")
        );
        assert!(profile.usable);
        assert_eq!(analysis.category, "video");
    }

    #[test]
    #[allow(non_snake_case)]
    fn REFERENCE_INDEXED_SLOTS_ARE_REFERENCE() {
        for (name, index) in [("ref_image_0", 0), ("ref_image_1", 1), ("ref_image_10", 10)] {
            let hint = canonical_semantic_hint(name).expect("reference slot should resolve");
            assert_eq!(hint.semantic, CanonicalSemantic::ReferenceImageList);
            assert!(hint.explicit_reference);
            assert_eq!(hint.item_index, Some(index));
        }
        assert!(canonical_semantic_hint("image_0").is_none());
        assert!(canonical_semantic_hint("image_1").is_none());
    }

    #[test]
    #[allow(non_snake_case)]
    fn MULTIPLE_IMAGE_INPUTS_NOT_REFERENCE() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "NodeText", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageInputA", "inputs": {"file": "a.png"}},
                "3": {"class_type": "ImageInputB", "inputs": {"file": "b.png"}},
                "4": {"class_type": "NodeBeta", "inputs": {
                    "conditioning": ["1", 0],
                    "image_a": ["2", 0],
                    "image_b": ["3", 0]
                }},
                "5": {"class_type": "NodeGamma", "inputs": {"video": ["4", 0]}}
            }),
            json!({
                "NodeText": {
                    "output": ["CONDITIONING"],
                    "input": {"required": {"text": ["STRING", {}]}}
                },
                "ImageInputA": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "ImageInputB": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "NodeBeta": {
                    "output": ["VIDEO"],
                    "input": {"required": {
                        "conditioning": ["CONDITIONING", {}],
                        "image_a": ["IMAGE", {}],
                        "image_b": ["IMAGE", {}]
                    }}
                },
                "NodeGamma": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        let node = semantic.nodes.get("4").unwrap();
        assert!(node
            .inputs
            .iter()
            .filter(|input| matches!(input.name.as_str(), "image_a" | "image_b"))
            .all(|input| !input.explicit_reference));
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("image_to_video")
        );
        assert_ne!(
            profile.primary_capability.as_deref(),
            Some("reference_to_video")
        );
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn REFERENCE_ALIAS_CANNOT_OVERRIDE_NON_IMAGE_SCHEMA() {
        let resolution = resolve_semantic_input(
            Some(RecognitionDeclaredType::String),
            "reference_image",
            None,
        );
        assert_eq!(resolution.semantic_type, SemanticType::Text);
        assert_eq!(resolution.semantic, CanonicalSemantic::PromptText);
        assert!(!resolution.explicit_reference);
        assert!(resolution.semantic_key.is_none());
    }

    #[test]
    #[allow(non_snake_case)]
    fn CANONICAL_PROMPT_HINT_IS_SHARED() {
        let hint = canonical_semantic_hint("positive_prompt").expect("prompt hint");
        let suggestion = canonical_suggestion_for_input("positive_prompt", &json!("hello"), false)
            .expect("prompt suggestion");
        assert_eq!(hint.semantic, CanonicalSemantic::PositivePrompt);
        assert_eq!(suggestion.semantic, hint.semantic);
        assert_eq!(suggestion.semantic_key.as_deref(), Some("prompt"));

        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "NodeAlpha", "inputs": {"positive_prompt": "hello"}},
            "2": {"class_type": "SaveImage", "inputs": {"image": ["1", 0]}}
        }))
        .expect("prompt workflow should parse");
        let bytes = serde_json::to_vec(workflow.value()).unwrap();
        let report = WorkflowAnalysisService::analyze_workflow(&workflow, &bytes);
        assert_eq!(
            report
                .inputs
                .iter()
                .find(|input| input.input_name == "positive_prompt")
                .map(|input| input.semantic_key.as_str()),
            Some("prompt")
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn CANONICAL_MEDIA_HINT_IS_SHARED() {
        let generic = canonical_semantic_hint("image").expect("generic image hint");
        let generic_suggestion =
            canonical_suggestion_for_input("image", &json!("input.png"), false)
                .expect("generic image suggestion");
        assert_eq!(generic.semantic, CanonicalSemantic::Image);
        assert!(!generic.explicit_reference);
        assert_eq!(generic_suggestion.semantic, CanonicalSemantic::Image);
        assert_eq!(generic_suggestion.semantic_key.as_deref(), Some("image"));

        let reference = canonical_semantic_hint("reference_image").expect("reference hint");
        let reference_suggestion =
            canonical_suggestion_for_input("reference_image", &json!("input.png"), false)
                .expect("reference image suggestion");
        assert_eq!(reference.semantic, CanonicalSemantic::ReferenceImage);
        assert!(reference.explicit_reference);
        assert_eq!(
            reference_suggestion.semantic_key.as_deref(),
            Some("reference_image")
        );
    }

    #[test]
    fn active_dependency_graph_excludes_disconnected_nodes_and_keeps_multiple_roots() {
        let workflow = workflow(false);
        let graph =
            ActiveDependencyGraph::from_workflow(&workflow, &["3".to_owned(), "4".to_owned()])
                .expect("active graph should build");

        assert_eq!(graph.roots, vec!["3".to_owned(), "4".to_owned()]);
        assert!(graph.active_nodes.contains("1"));
        assert!(graph.active_nodes.contains("3"));
        assert!(graph.active_nodes.contains("4"));
        assert!(graph.disconnected_nodes.contains("99"));
    }

    #[test]
    fn semantic_resolver_prefers_schema_types_for_an_unknown_generator() {
        let workflow = workflow(false);
        let active = ActiveDependencyGraph::from_workflow(&workflow, &["3".to_owned()])
            .expect("active graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema(), active);
        let generator = semantic.nodes.get("2").expect("generator should resolve");

        assert_eq!(generator.role, SemanticNodeRole::VideoGenerator);
        assert!(generator
            .outputs
            .iter()
            .any(|output| output.semantic_type == SemanticType::Video));
        assert!(semantic.unknown_active_dependencies.is_empty());
    }

    #[test]
    fn disconnected_missing_schema_is_noncritical_but_active_missing_schema_blocks() {
        let workflow = workflow(false);
        let active = ActiveDependencyGraph::from_workflow(&workflow, &["3".to_owned()])
            .expect("active graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema(), active);
        let analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            b"generic-unknown",
            Some(&schema()),
        );
        let profile = legacy_profile(&analysis, &semantic);

        assert!(profile.usable);
        assert!(profile
            .noncritical_unknown_nodes
            .iter()
            .any(|node| node == "99"));

        let active_missing = ActiveDependencyGraph::from_workflow(&workflow, &["99".to_owned()])
            .expect("active graph should build");
        let active_semantic = resolve_semantic_graph(&workflow, &schema(), active_missing);
        assert!(active_semantic
            .unknown_active_dependencies
            .iter()
            .any(|issue| issue.code == "unknown_semantic_dependency"));
    }

    #[test]
    fn capability_uses_active_image_connection_not_optional_schema_presence() {
        let schema = schema();
        let no_image = workflow(false);
        let no_image_active = ActiveDependencyGraph::from_workflow(&no_image, &["3".to_owned()])
            .expect("active graph should build");
        let no_image_semantic = resolve_semantic_graph(&no_image, &schema, no_image_active);
        let no_image_analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &no_image,
            b"text-only",
            Some(&schema),
        );
        assert_eq!(
            legacy_profile(&no_image_analysis, &no_image_semantic)
                .primary_capability
                .as_deref(),
            Some("text_to_video")
        );
        assert!(legacy_profile(&no_image_analysis, &no_image_semantic).usable);
        assert_eq!(no_image_analysis.identity, WorkflowIdentity::New);

        let with_image = workflow(true);
        let with_image_active =
            ActiveDependencyGraph::from_workflow(&with_image, &["3".to_owned()])
                .expect("active graph should build");
        let with_image_semantic = resolve_semantic_graph(&with_image, &schema, with_image_active);
        let with_image_analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &with_image,
            b"image-input",
            Some(&schema),
        );
        assert_eq!(
            legacy_profile(&with_image_analysis, &with_image_semantic)
                .primary_capability
                .as_deref(),
            Some("image_to_video")
        );
    }

    #[test]
    fn generic_reference_media_is_distinct_from_generic_image_to_video() {
        let schema = RecognitionSchemaContext::parse(&json!({
            "TextSource": {
                "output": ["CONDITIONING"],
                "input": {"required": {"text": ["STRING", {}]}}
            },
            "LoadImage": {
                "output": ["IMAGE"],
                "input": {"required": {"image": [["available.png"], {}]}}
            },
            "NeutralMultimodalVideoGenerator": {
                "output": ["VIDEO"],
                "input": {
                    "required": {"conditioning": ["CONDITIONING", {}]},
                    "optional": {"reference_images": ["IMAGE", {}]}
                }
            },
            "VideoOutput": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"video": ["VIDEO", {}]}}
            }
        }));
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
            "2": {"class_type": "LoadImage", "inputs": {"image": "available.png"}},
            "3": {"class_type": "NeutralMultimodalVideoGenerator", "inputs": {
                "conditioning": ["1", 0], "reference_images": ["2", 0]
            }},
            "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
        }))
        .expect("reference workflow should parse");
        let active = ActiveDependencyGraph::from_workflow(&workflow, &["4".to_owned()])
            .expect("active graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema, active);
        let analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            b"reference-workflow",
            Some(&schema),
        );
        let profile = legacy_profile(&analysis, &semantic);

        assert_eq!(analysis.identity, WorkflowIdentity::New);
        assert!(profile.usable);
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("reference_to_video")
        );

        let disconnected_reference_workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
            "2": {"class_type": "LoadImage", "inputs": {"image": "available.png"}},
            "3": {"class_type": "NeutralMultimodalVideoGenerator", "inputs": {
                "conditioning": ["1", 0], "reference_images": null
            }},
            "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
        }))
        .expect("disconnected reference workflow should parse");
        let disconnected_reference_active = ActiveDependencyGraph::from_workflow(
            &disconnected_reference_workflow,
            &["4".to_owned()],
        )
        .expect("disconnected reference graph should build");
        assert!(!disconnected_reference_active.active_nodes.contains("2"));
        let disconnected_reference_semantic = resolve_semantic_graph(
            &disconnected_reference_workflow,
            &schema,
            disconnected_reference_active,
        );
        let disconnected_reference_analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &disconnected_reference_workflow,
            b"disconnected-reference-workflow",
            Some(&schema),
        );
        let disconnected_reference_profile = legacy_profile(
            &disconnected_reference_analysis,
            &disconnected_reference_semantic,
        );
        assert_eq!(
            disconnected_reference_profile.primary_capability.as_deref(),
            Some("text_to_video")
        );
        assert!(disconnected_reference_profile.usable);
    }

    #[test]
    fn ordinary_multiple_image_inputs_do_not_imply_reference_capability() {
        let schema = RecognitionSchemaContext::parse(&json!({
            "TextSource": {
                "output": ["CONDITIONING"],
                "input": {"required": {"text": ["STRING", {}]}}
            },
            "LoadImage": {
                "output": ["IMAGE"],
                "input": {"required": {"image": [["available.png"], {}]}}
            },
            "NeutralTwoImageVideoGenerator": {
                "output": ["VIDEO"],
                "input": {
                    "required": {"conditioning": ["CONDITIONING", {}]},
                    "optional": {
                        "image_a": ["IMAGE", {}],
                        "image_b": ["IMAGE", {}]
                    }
                }
            },
            "VideoOutput": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"video": ["VIDEO", {}]}}
            }
        }));
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
            "2": {"class_type": "LoadImage", "inputs": {"image": "a.png"}},
            "3": {"class_type": "LoadImage", "inputs": {"image": "b.png"}},
            "4": {"class_type": "NeutralTwoImageVideoGenerator", "inputs": {
                "conditioning": ["1", 0], "image_a": ["2", 0], "image_b": ["3", 0]
            }},
            "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
        }))
        .expect("ordinary multi-image workflow should parse");
        let active = ActiveDependencyGraph::from_workflow(&workflow, &["5".to_owned()])
            .expect("ordinary multi-image graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema, active);
        let analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            b"ordinary-multi-image-workflow",
            Some(&schema),
        );

        let profile = legacy_profile(&analysis, &semantic);

        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("image_to_video")
        );
        assert!(profile.usable);
    }

    #[test]
    fn active_unknown_critical_socket_fails_closed_without_provider_rules() {
        let schema = RecognitionSchemaContext::parse(&json!({
            "KnownValue": {"output": ["TEXT"]},
            "UnknownSocketVideoGenerator": {
                "output": ["VIDEO"],
                "input": {"required": {"mystery": ["CUSTOM_SOCKET", {}]}}
            },
            "VideoOutput": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"video": ["VIDEO", {}]}}
            }
        }));
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "KnownValue", "inputs": {}},
            "2": {"class_type": "UnknownSocketVideoGenerator", "inputs": {
                "mystery": ["1", 0]
            }},
            "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
        }))
        .expect("unknown socket workflow should parse");
        let active = ActiveDependencyGraph::from_workflow(&workflow, &["3".to_owned()])
            .expect("active graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema, active);
        assert!(semantic
            .unknown_active_dependencies
            .iter()
            .any(|issue| issue.code == "unknown_semantic_dependency"));
        let analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            b"unknown-critical",
            Some(&schema),
        );
        let profile = legacy_profile(&analysis, &semantic);
        assert!(!profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn PER_ROOT_CLOSURE_DOES_NOT_UNION_INPUTS() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}},
                "4": {"class_type": "LoadImage", "inputs": {"image": "ref.png"}},
                "5": {"class_type": "ReferenceVideoGenerator", "inputs": {
                    "conditioning": ["1", 0], "reference_images": ["4", 0]
                }},
                "6": {"class_type": "VideoOutput", "inputs": {"video": ["5", 0]}}
            }),
            &[root_selection("3", "image"), root_selection("6", "video")],
        );
        let image_root = profile
            .roots
            .iter()
            .find(|root| root.root_node_id == "3")
            .expect("image root profile");
        let video_root = profile
            .roots
            .iter()
            .find(|root| root.root_node_id == "6")
            .expect("video root profile");
        assert!(!image_root
            .required_external_inputs
            .iter()
            .any(|input| input.contains("reference")));
        assert_eq!(
            video_root.primary_capability.as_deref(),
            Some("reference_to_video")
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn TWO_COMPATIBLE_VIDEO_ROOTS() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}},
                "4": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            &[],
        );
        assert_eq!(profile.roots.len(), 2);
        assert_eq!(profile.aggregate_capabilities, vec!["text_to_video"]);
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_video"));
        assert_eq!(profile.readiness, WorkflowCapabilityReadiness::Ready);
    }

    #[test]
    #[allow(non_snake_case)]
    fn TWO_DIFFERENT_CAPABILITY_ROOTS() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}},
                "4": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            &[root_selection("3", "image"), root_selection("5", "video")],
        );
        assert_eq!(
            profile.aggregate_capabilities,
            vec!["text_to_image", "text_to_video"]
        );
        assert_eq!(profile.primary_capability, None);
        assert_eq!(profile.readiness, WorkflowCapabilityReadiness::Ready);
    }

    #[test]
    #[allow(non_snake_case)]
    fn ROOT_A_READY_ROOT_B_UNSUPPORTED() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}},
                "4": {"class_type": "UnknownVideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            &[root_selection("3", "image"), root_selection("5", "video")],
        );
        assert_eq!(
            profile.readiness,
            WorkflowCapabilityReadiness::PartiallySupported
        );
        assert_eq!(profile.primary_capability, None);
        assert_eq!(
            profile
                .roots
                .iter()
                .find(|root| root.root_node_id == "3")
                .unwrap()
                .readiness,
            RootCapabilityReadiness::Ready
        );
        assert_eq!(
            profile
                .roots
                .iter()
                .find(|root| root.root_node_id == "5")
                .unwrap()
                .readiness,
            RootCapabilityReadiness::Unsupported
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn IMAGE_INPUT_DOES_NOT_LEAK_ACROSS_ROOTS() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "LoadImage", "inputs": {"image": "ref.png"}},
                "3": {"class_type": "VideoGenerator", "inputs": {
                    "conditioning": ["1", 0], "image": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}},
                "5": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "6": {"class_type": "VideoOutput", "inputs": {"video": ["5", 0]}}
            }),
            &[root_selection("4", "video"), root_selection("6", "video")],
        );
        assert_eq!(
            profile
                .roots
                .iter()
                .find(|root| root.root_node_id == "4")
                .unwrap()
                .primary_capability
                .as_deref(),
            Some("image_to_video")
        );
        assert_eq!(
            profile
                .roots
                .iter()
                .find(|root| root.root_node_id == "6")
                .unwrap()
                .primary_capability
                .as_deref(),
            Some("text_to_video")
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn REFERENCE_SEMANTIC_DOES_NOT_LEAK_ACROSS_ROOTS() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "LoadImage", "inputs": {"image": "ref.png"}},
                "3": {"class_type": "ReferenceVideoGenerator", "inputs": {
                    "conditioning": ["1", 0], "reference_images": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}},
                "5": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "6": {"class_type": "VideoOutput", "inputs": {"video": ["5", 0]}}
            }),
            &[root_selection("4", "video"), root_selection("6", "video")],
        );
        assert_eq!(
            profile
                .roots
                .iter()
                .find(|root| root.root_node_id == "4")
                .unwrap()
                .primary_capability
                .as_deref(),
            Some("reference_to_video")
        );
        assert_eq!(
            profile
                .roots
                .iter()
                .find(|root| root.root_node_id == "6")
                .unwrap()
                .primary_capability
                .as_deref(),
            Some("text_to_video")
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn SHARED_UPSTREAM_CAN_BELONG_TO_MULTIPLE_ROOTS() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}},
                "4": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            &[],
        );
        assert_eq!(profile.roots.len(), 2);
        assert!(profile
            .roots
            .iter()
            .all(|root| root.root_node_id == "3" || root.root_node_id == "5"));
    }

    #[test]
    #[allow(non_snake_case)]
    fn INTERNAL_GENERATED_MEDIA_IS_NOT_EXTERNAL_INPUT() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "VideoGenerator", "inputs": {
                    "conditioning": ["1", 0], "image": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
            }),
            &[],
        );
        let root = &profile.roots[0];
        assert_eq!(root.primary_capability.as_deref(), Some("text_to_video"));
        assert!(!root
            .secondary_capabilities
            .iter()
            .any(|capability| capability == "image_input"));
        assert!(!root
            .required_external_inputs
            .iter()
            .any(|input| input.ends_with(".image")));
    }

    #[test]
    #[allow(non_snake_case)]
    fn ROOT_UNKNOWN_DEPENDENCY_IS_SCOPED() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}},
                "4": {"class_type": "UnknownVideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            &[root_selection("3", "image"), root_selection("5", "video")],
        );
        let image_root = profile
            .roots
            .iter()
            .find(|root| root.root_node_id == "3")
            .unwrap();
        let video_root = profile
            .roots
            .iter()
            .find(|root| root.root_node_id == "5")
            .unwrap();
        assert!(image_root.unknown_dependencies.is_empty());
        assert!(!video_root.unknown_dependencies.is_empty());
    }

    #[test]
    #[allow(non_snake_case)]
    fn EXPLICIT_SELECTED_ROOT_CONTROLS_PRIMARY_WITHOUT_DROPPING_OTHER_ROOTS() {
        let (analysis, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}},
                "4": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            &[root_selection("5", "video")],
        );
        assert_eq!(profile.roots.len(), 2);
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_video"));
        assert_eq!(profile.secondary_capabilities, vec!["text_to_image"]);
        assert_eq!(
            profile.selected_root_id.as_deref(),
            analysis.selected_root_id.as_deref()
        );
        assert!(profile.roots.iter().any(|root| root.root_node_id == "3"));
    }

    #[test]
    #[allow(non_snake_case)]
    fn PER_ROOT_OUTPUT_TYPES_ARE_PRESERVED() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}},
                "4": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            &[root_selection("3", "image"), root_selection("5", "video")],
        );
        assert_eq!(
            profile
                .roots
                .iter()
                .map(|root| (root.root_node_id.as_str(), root.output_type.as_str()))
                .collect::<Vec<_>>(),
            vec![("3", "image"), ("5", "video")]
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn PER_ROOT_CAPABILITY_IS_ORDER_INDEPENDENT() {
        let first = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}},
                "4": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            &[root_selection("3", "image"), root_selection("5", "video")],
        )
        .1;
        let second = per_root_profile_fixture(
            json!({
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}},
                "4": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "ImageOutput", "inputs": {"image": ["2", 0]}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}}
            }),
            &[root_selection("5", "video"), root_selection("3", "image")],
        )
        .1;
        assert_eq!(first, second);
    }

    #[test]
    fn multiple_effective_output_roots_keep_each_upstream_component_active() {
        let workflow = workflow(false);
        let active =
            ActiveDependencyGraph::from_workflow(&workflow, &["3".to_owned(), "4".to_owned()])
                .expect("multi-output graph should build");

        assert_eq!(active.roots, vec!["3".to_owned(), "4".to_owned()]);
        assert!(active.active_nodes.contains("1"));
        assert!(active.active_nodes.contains("2"));
        assert!(active.active_nodes.contains("3"));
        assert!(active.active_nodes.contains("4"));
        assert!(!active.active_nodes.contains("99"));
        assert!(active.disconnected_nodes.contains("99"));
    }

    #[test]
    #[allow(non_snake_case)]
    fn INTERNAL_GENERATED_MEDIA_REMAINS_INTERNAL() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ImageGenerator", "inputs": {"conditioning": ["1", 0]}},
                "3": {"class_type": "VideoGenerator", "inputs": {
                    "conditioning": ["1", 0], "image": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
            }),
            &[],
        );
        let root = &profile.roots[0];
        assert_eq!(root.primary_capability.as_deref(), Some("text_to_video"));
        assert!(!root
            .secondary_capabilities
            .iter()
            .any(|capability| capability == "image_input"));
        assert!(!root
            .required_external_inputs
            .iter()
            .any(|input| input.ends_with(".image")));
    }

    #[test]
    #[allow(non_snake_case)]
    fn REFERENCE_SEMANTIC_REMAINS_ROOT_SCOPED() {
        let (_, profile) = per_root_profile_fixture(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "LoadImage", "inputs": {"image": "ref.png"}},
                "3": {"class_type": "ReferenceVideoGenerator", "inputs": {
                    "conditioning": ["1", 0], "reference_images": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}},
                "5": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["1", 0]}},
                "6": {"class_type": "VideoOutput", "inputs": {"video": ["5", 0]}}
            }),
            &[root_selection("4", "video"), root_selection("6", "video")],
        );
        assert_eq!(
            profile
                .roots
                .iter()
                .find(|root| root.root_node_id == "4")
                .unwrap()
                .primary_capability
                .as_deref(),
            Some("reference_to_video")
        );
        assert_eq!(
            profile
                .roots
                .iter()
                .find(|root| root.root_node_id == "6")
                .unwrap()
                .primary_capability
                .as_deref(),
            Some("text_to_video")
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn CANONICAL_RESOLUTION_IS_ORDER_INDEPENDENT() {
        let names = [
            "positive_prompt",
            "reference_image",
            "ref_image_10",
            "image",
            "ref_image_2",
            "width",
        ];
        let mut forward = names
            .iter()
            .map(|name| {
                let hint = canonical_semantic_hint(name).expect("canonical hint");
                ((*name).to_owned(), hint.semantic, hint.item_index)
            })
            .collect::<Vec<_>>();
        let mut reverse = names
            .iter()
            .rev()
            .map(|name| {
                let hint = canonical_semantic_hint(name).expect("canonical hint");
                ((*name).to_owned(), hint.semantic, hint.item_index)
            })
            .collect::<Vec<_>>();
        forward.sort_by(|left, right| left.0.cmp(&right.0));
        reverse.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(forward, reverse);
        assert_eq!(
            forward
                .iter()
                .find(|entry| entry.0 == "ref_image_10")
                .map(|entry| entry.2),
            Some(Some(10))
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn SCHEMA_TYPE_LAYERING_PRESERVES_RAW_CUSTOM_TYPES() {
        let schema = RecognitionSchemaContext::parse(&json!({
            "TypedNode": {
                "input": {"required": {
                    "box": ["BOX", {}],
                    "any": ["*", {}]
                }},
                "output": ["BOX", "*"]
            }
        }));
        let node = schema.node("TypedNode").expect("schema node");

        assert_eq!(node.input("box").unwrap().raw_type, "BOX");
        assert_eq!(node.input("any").unwrap().raw_type, "*");
        assert_eq!(node.raw_output_types, vec!["BOX", "*"]);
    }

    #[test]
    #[allow(non_snake_case)]
    fn OPAQUE_CUSTOM_CONTROL_NONBLOCKING() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ControlSource", "inputs": {}},
                "3": {"class_type": "VideoGenerator", "inputs": {
                    "prompt": ["1", 0],
                    "control": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
            }),
            json!({
                "TextSource": {
                    "output": ["STRING"],
                    "input": {"required": {"text": ["STRING", {}]}}
                },
                "ControlSource": {"output": ["CUSTOM_CONTROL_X"]},
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {
                        "prompt": ["STRING", {}],
                        "control": ["CUSTOM_CONTROL_X", {}]
                    }}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );

        let control = semantic
            .nodes
            .get("3")
            .unwrap()
            .inputs
            .iter()
            .find(|input| input.name == "control")
            .unwrap();
        assert!(matches!(
            control.type_resolution,
            SchemaTypeResolution::OpaqueCustom { ref raw_type } if raw_type == "CUSTOM_CONTROL_X"
        ));
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_video"));
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn ARBITRARY_OPAQUE_TYPES_DO_NOT_REQUIRE_ADAPTER() {
        for raw_type in ["CUSTOM_ALPHA", "CUSTOM_BETA", "CUSTOM_GAMMA"] {
            let schema_value = json!({
                "TextSource": {
                    "output": ["STRING"],
                    "input": {"required": {"text": ["STRING", {}]}}
                },
                "ControlSource": {"output": [raw_type]},
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {
                        "prompt": ["STRING", {}],
                        "control": [raw_type, {}]
                    }}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            });
            let (_, semantic, profile) = profile_with_schema(
                json!({
                    "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                    "2": {"class_type": "ControlSource", "inputs": {}},
                    "3": {"class_type": "VideoGenerator", "inputs": {
                        "prompt": ["1", 0],
                        "control": ["2", 0]
                    }},
                    "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
                }),
                schema_value,
            );
            assert!(
                semantic.unknown_active_dependencies.is_empty(),
                "{raw_type}"
            );
            assert!(profile.usable, "{raw_type}");
        }
    }

    #[test]
    #[allow(non_snake_case)]
    fn OPAQUE_EXTERNAL_MEDIA_BOUNDARY_FAILS_CLOSED() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "CustomMediaSource", "inputs": {}},
                "2": {"class_type": "VideoGenerator", "inputs": {
                    "media": ["1", 0]
                }},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            json!({
                "CustomMediaSource": {"output": ["CUSTOM_MEDIA_X"]},
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {"media": ["CUSTOM_MEDIA_X", {}]}}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );

        assert!(semantic.unknown_active_dependencies.iter().any(|issue| {
            issue.code == "unknown_semantic_dependency"
                && issue.node_id == "2"
                && issue.input_name.as_deref() == Some("media")
        }));
        assert!(!profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn OPAQUE_OUTPUT_BOUNDARY_FAILS_CLOSED() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "OpaqueOutput", "inputs": {}}
        }))
        .expect("opaque output workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "OpaqueOutput": {"output": ["CUSTOM_OUTPUT_X"], "output_node": true}
        }));
        let graph = WorkflowGraph::from_document(&workflow).expect("graph should build");
        let active = ActiveDependencyGraph::from_graph(&graph, &["1".to_owned()])
            .expect("active graph should build");
        let semantic = resolve_semantic_graph(&workflow, &schema, active);

        let output = &semantic.nodes.get("1").unwrap().outputs[0];
        assert!(matches!(
            output.type_resolution,
            SchemaTypeResolution::OpaqueCustom { ref raw_type } if raw_type == "CUSTOM_OUTPUT_X"
        ));
        assert!(semantic
            .unknown_active_dependencies
            .iter()
            .any(|issue| issue.code == "unknown_output" && issue.node_id == "1"));
    }

    #[test]
    #[allow(non_snake_case)]
    fn OPAQUE_MEDIA_SINK_OUTPUT_IS_NONBLOCKING() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "input.png"}},
                "2": {"class_type": "VideoFileSink", "inputs": {"images": ["1", 0]}}
            }),
            json!({
                "ImageSource": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "VideoFileSink": {
                    "output": ["VHS_FILENAMES"],
                    "output_node": true,
                    "input": {"required": {"images": ["IMAGE", {}]}}
                }
            }),
        );

        assert!(semantic.unknown_active_dependencies.is_empty());
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn WILDCARD_RESOLVES_FROM_KNOWN_SOURCE() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "input.png"}},
                "2": {"class_type": "VideoGenerator", "inputs": {"image": ["1", 0]}},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            json!({
                "ImageSource": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {"image": ["*", {}]}}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        let input = semantic.nodes.get("2").unwrap().inputs.first().unwrap();
        assert!(matches!(
            input.type_resolution,
            SchemaTypeResolution::DynamicResolved {
                ref raw_type,
                semantic_type: SemanticType::Image
            } if raw_type == "*"
        ));
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("image_to_video")
        );
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn WILDCARD_RESOLVES_FROM_KNOWN_TARGET() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "DynamicSource", "inputs": {}},
                "2": {"class_type": "VideoConsumer", "inputs": {"video": ["1", 0]}},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            json!({
                "DynamicSource": {"output": ["*"]},
                "VideoConsumer": {
                    "output": ["VIDEO"],
                    "input": {"required": {"video": ["VIDEO", {}]}}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        let output = &semantic.nodes.get("1").unwrap().outputs[0];
        assert!(matches!(
            output.type_resolution,
            SchemaTypeResolution::DynamicResolved {
                ref raw_type,
                semantic_type: SemanticType::Video
            } if raw_type == "*"
        ));
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_video"));
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn UNCONSTRAINED_WILDCARD_CAPABILITY_BOUNDARY_FAILS_CLOSED() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "AnySource", "inputs": {}},
                "2": {"class_type": "VideoGenerator", "inputs": {"media": ["1", 0]}},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            json!({
                "AnySource": {"output": ["*"]},
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {"media": ["*", {}]}}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        assert!(semantic.unknown_active_dependencies.iter().any(|issue| {
            issue.code == "unknown_semantic_dependency"
                && issue.node_id == "2"
                && issue.input_name.as_deref() == Some("media")
        }));
        assert!(!profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn UNCONSTRAINED_WILDCARD_AUXILIARY_NONBLOCKING() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "AnySource", "inputs": {}},
                "3": {"class_type": "VideoGenerator", "inputs": {
                    "prompt": ["1", 0],
                    "auxiliary": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
            }),
            json!({
                "TextSource": {
                    "output": ["STRING"],
                    "input": {"required": {"text": ["STRING", {}]}}
                },
                "AnySource": {"output": ["*"]},
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {
                        "prompt": ["STRING", {}],
                        "auxiliary": ["*", {}]
                    }}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_video"));
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn MATCHTYPE_PROPAGATES_KNOWN_MEDIA() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "input.png"}},
                "2": {"class_type": "MatchTransform", "inputs": {"input": ["1", 0]}},
                "3": {"class_type": "VideoGenerator", "inputs": {"image": ["2", 0]}},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
            }),
            json!({
                "ImageSource": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "MatchTransform": {
                    "input": {"required": {"input": ["COMFY_MATCHTYPE_V3", {
                        "template": {"template_id": "input_type", "allowed_types": "IMAGE,MASK"}
                    }]}},
                    "output": ["COMFY_MATCHTYPE_V3"],
                    "output_matchtypes": ["input_type"]
                },
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {"image": ["IMAGE", {}]}}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        let output = &semantic.nodes.get("2").unwrap().outputs[0];
        assert!(matches!(
            output.type_resolution,
            SchemaTypeResolution::DynamicResolved {
                ref raw_type,
                semantic_type: SemanticType::Image
            } if raw_type == "COMFY_MATCHTYPE_V3"
        ));
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("image_to_video")
        );
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn MATCHTYPE_UNRESOLVED_CRITICAL_FAILS_CLOSED() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "MatchSource", "inputs": {}},
                "2": {"class_type": "VideoGenerator", "inputs": {"media": ["1", 0]}},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            json!({
                "MatchSource": {
                    "output": ["COMFY_MATCHTYPE_V3"],
                    "output_matchtypes": ["source"]
                },
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {"media": ["COMFY_MATCHTYPE_V3", {
                        "template": {"template_id": "source", "allowed_types": "*"}
                    }]}}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        assert!(semantic.unknown_active_dependencies.iter().any(|issue| {
            issue.code == "unknown_semantic_dependency"
                && issue.node_id == "2"
                && issue.input_name.as_deref() == Some("media")
        }));
        assert!(!profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn AUTOGROW_INPUT_RESOLVES_FROM_KNOWN_SOURCE() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "IntegerSource", "inputs": {}},
                "3": {"class_type": "MathNode", "inputs": {
                    "expression": "a + 1",
                    "values.a": ["2", 0]
                }},
                "4": {"class_type": "VideoGenerator", "inputs": {
                    "prompt": ["1", 0],
                    "steps": ["3", 1]
                }},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            json!({
                "TextSource": {
                    "output": ["STRING"],
                    "input": {"required": {"text": ["STRING", {}]}}
                },
                "IntegerSource": {"output": ["INT"]},
                "MathNode": {
                    "output": ["FLOAT", "INT", "BOOLEAN"],
                    "input": {"required": {
                        "expression": ["STRING", {}],
                        "values": ["COMFY_AUTOGROW_V3", {
                            "template": {
                                "input": {"required": {"value": ["FLOAT,INT,BOOLEAN", {}]}},
                                "names": ["a", "b"]
                            }
                        }]
                    }}
                },
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {
                        "prompt": ["STRING", {}],
                        "steps": ["INT", {}]
                    }}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );

        let input = semantic
            .nodes
            .get("3")
            .unwrap()
            .inputs
            .iter()
            .find(|input| input.name == "values.a")
            .unwrap();
        assert!(matches!(
            input.type_resolution,
            SchemaTypeResolution::DynamicResolved { ref raw_type, .. }
                if raw_type == "COMFY_AUTOGROW_V3"
        ));
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn CONDITIONAL_INPUT_RESOLVES_SELECTED_OPTION() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "input.png"}},
                "2": {"class_type": "ResizeNode", "inputs": {
                    "resize_type": "scale dimensions",
                    "resize_type.width": ["3", 0],
                    "input": ["1", 0]
                }},
                "3": {"class_type": "IntegerSource", "inputs": {}},
                "4": {"class_type": "VideoGenerator", "inputs": {"image": ["2", 0]}},
                "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
            }),
            json!({
                "ImageSource": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "IntegerSource": {"output": ["INT"]},
                "ResizeNode": {
                    "output": ["IMAGE"],
                    "input": {"required": {
                        "resize_type": ["COMFY_DYNAMICCOMBO_V3", {
                            "options": [{"key": "scale dimensions", "inputs": {
                                "required": {"width": ["INT", {}]}
                            }}]
                        }],
                        "input": ["COMFY_MATCHTYPE_V3", {
                            "template": {"template_id": "input_type", "allowed_types": "IMAGE,MASK"}
                        }]
                    }}
                },
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {"image": ["IMAGE", {}]}}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );

        let input = semantic
            .nodes
            .get("2")
            .unwrap()
            .inputs
            .iter()
            .find(|input| input.name == "resize_type.width")
            .unwrap();
        assert_eq!(input.semantic_type, SemanticType::Unknown);
        assert!(matches!(
            input.type_resolution,
            SchemaTypeResolution::KnownSemantic { ref raw_type, .. }
                if raw_type == "INT"
        ));
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn AUTOGROW_PREFIX_INPUT_RESOLVES_FROM_KNOWN_SOURCE() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "input.png"}},
                "2": {"class_type": "ReferenceNode", "inputs": {
                    "ref_images.ref_image_0": ["1", 0]
                }},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            json!({
                "ImageSource": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "ReferenceNode": {
                    "output": ["VIDEO"],
                    "input": {"required": {"ref_images": ["COMFY_AUTOGROW_V3", {
                        "template": {
                            "input": {"required": {"ref_image": ["IMAGE", {}]}},
                            "prefix": "ref_image_",
                            "min": 0,
                            "max": 9
                        }
                    }]}}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );

        let input = semantic
            .nodes
            .get("2")
            .unwrap()
            .inputs
            .iter()
            .find(|input| input.name == "ref_images.ref_image_0")
            .unwrap();
        assert!(matches!(
            input.type_resolution,
            SchemaTypeResolution::DynamicResolved {
                semantic_type: SemanticType::Image,
                ..
            }
        ));
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn AUTOGROW_REFERENCE_PARENT_PROPAGATES_REFERENCE_TO_MEMBER() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "ref.png"}},
                "2": {"class_type": "NeutralDynamicMediaConsumer", "inputs": {
                    "prompt": "make a video",
                    "reference_images.ref_image_0": ["1", 0]
                }},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            json!({
                "ImageSource": {
                    "output": ["IMAGE"],
                    "input": {"required": {"file": ["COMBO", {"image_upload": true}]}}
                },
                "NeutralDynamicMediaConsumer": {
                    "output": ["VIDEO"],
                    "input": {
                        "required": {"prompt": ["STRING", {}]},
                        "optional": {"reference_images": ["COMFY_AUTOGROW_V3", {
                            "template": {
                                "input": {"required": {"ref_image": ["IMAGE", {}]}},
                                "prefix": "ref_image_", "min": 0, "max": 9
                            }
                        }]}
                    }
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        let member = semantic.nodes["2"]
            .inputs
            .iter()
            .find(|input| input.name == "reference_images.ref_image_0")
            .unwrap();

        assert_eq!(member.canonical_semantic, CanonicalSemantic::ReferenceImage);
        assert_eq!(member.semantic_key.as_deref(), Some("reference_image"));
        assert!(member.explicit_reference);
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("reference_to_video")
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn NESTED_DYNAMIC_REFERENCE_MEMBER_REMAINS_REFERENCE() {
        let (_, semantic, _) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "ref.png"}},
                "2": {"class_type": "NeutralDynamicMediaConsumer", "inputs": {
                    "prompt": "make a video",
                    "reference_images.ref_image_0": ["1", 0]
                }},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            dynamic_image_collection_schema_value(),
        );
        let member = semantic.nodes["2"]
            .inputs
            .iter()
            .find(|input| input.name == "reference_images.ref_image_0")
            .unwrap();

        assert_eq!(member.canonical_semantic, CanonicalSemantic::ReferenceImage);
        assert!(member.explicit_reference);
    }

    #[test]
    #[allow(non_snake_case)]
    fn AUTOGROW_GENERIC_IMAGE_PARENT_REMAINS_GENERIC_IMAGE() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "input.png"}},
                "2": {"class_type": "NeutralDynamicMediaConsumer", "inputs": {
                    "prompt": "make a video",
                    "images.image_0": ["1", 0]
                }},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            dynamic_image_collection_schema_value(),
        );
        let member = semantic.nodes["2"]
            .inputs
            .iter()
            .find(|input| input.name == "images.image_0")
            .unwrap();

        assert_eq!(member.canonical_semantic, CanonicalSemantic::Image);
        assert_eq!(member.semantic_key.as_deref(), Some("image"));
        assert!(!member.explicit_reference);
        assert_eq!(
            profile.primary_capability.as_deref(),
            Some("image_to_video")
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn REFERENCE_SEMANTIC_DOES_NOT_LEAK_TO_SIBLING_DYNAMIC_GROUP() {
        let (_, semantic, _) = profile_with_schema(
            json!({
                "1": {"class_type": "ImageSource", "inputs": {"file": "ref.png"}},
                "2": {"class_type": "ImageSource", "inputs": {"file": "ordinary.png"}},
                "3": {"class_type": "NeutralDynamicMediaConsumer", "inputs": {
                    "prompt": "make a video",
                    "reference_images.ref_image_0": ["1", 0],
                    "images.image_0": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
            }),
            dynamic_image_collection_schema_value(),
        );
        let inputs = &semantic.nodes["3"].inputs;
        let reference = inputs
            .iter()
            .find(|input| input.name == "reference_images.ref_image_0")
            .unwrap();
        let generic = inputs
            .iter()
            .find(|input| input.name == "images.image_0")
            .unwrap();

        assert_eq!(
            reference.canonical_semantic,
            CanonicalSemantic::ReferenceImage
        );
        assert!(reference.explicit_reference);
        assert_eq!(generic.canonical_semantic, CanonicalSemantic::Image);
        assert!(!generic.explicit_reference);
    }

    #[test]
    #[allow(non_snake_case)]
    fn DYNAMIC_REFERENCE_SEMANTIC_DOES_NOT_LEAK_ACROSS_ROOTS() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "ImageSource", "inputs": {"file": "ref.png"}},
            "2": {"class_type": "NeutralDynamicMediaConsumer", "inputs": {
                "prompt": "make a video",
                "reference_images.ref_image_0": ["1", 0]
            }},
            "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}},
            "4": {"class_type": "TextSource", "inputs": {"text": "hello"}},
            "5": {"class_type": "VideoGenerator", "inputs": {"conditioning": ["4", 0]}},
            "6": {"class_type": "VideoOutput", "inputs": {"video": ["5", 0]}}
        }))
        .expect("workflow should parse");
        let schema = dynamic_image_collection_schema();
        let bytes = serde_json::to_vec(workflow.value()).unwrap();
        let selections = [root_selection("3", "video"), root_selection("6", "video")];
        let analysis = WorkflowAnalysisService::analyze_workflow_with_schema_and_output_roots(
            &workflow,
            &bytes,
            Some(&schema),
            &selections,
        );
        let roots = analysis.output_root_resolution.roots();
        let graph = WorkflowGraph::from_document(&workflow).unwrap();
        let root_nodes = roots
            .iter()
            .map(|root| root.node_id.clone())
            .collect::<Vec<_>>();
        let active = ActiveDependencyGraph::from_graph(&graph, &root_nodes).unwrap();
        let semantic = resolve_semantic_graph(&workflow, &schema, active);
        let closures = roots
            .iter()
            .map(|root| {
                RootDependencyClosure::from_graph(&graph, root.output_id.clone(), &root.node_id)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let profile = build_capability_profile_for_roots(
            &analysis,
            &semantic,
            &closures,
            roots,
            analysis.selected_root_id.as_deref(),
        );
        let reference_root = profile
            .roots
            .iter()
            .find(|root| root.root_node_id == "3")
            .unwrap();
        let text_root = profile
            .roots
            .iter()
            .find(|root| root.root_node_id == "6")
            .unwrap();

        assert_eq!(
            reference_root.primary_capability.as_deref(),
            Some("reference_to_video")
        );
        assert_eq!(
            text_root.primary_capability.as_deref(),
            Some("text_to_video")
        );
        assert_eq!(profile.primary_capability, None);
    }

    #[test]
    #[allow(non_snake_case)]
    fn DYNAMIC_REFERENCE_MEMBER_TYPE_MISMATCH_FAILS_CLOSED() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "VideoSource", "inputs": {"file": "ref.mp4"}},
                "2": {"class_type": "NeutralDynamicMediaConsumer", "inputs": {
                    "prompt": "make a video",
                    "reference_images.ref_image_0": ["1", 0]
                }},
                "3": {"class_type": "VideoOutput", "inputs": {"video": ["2", 0]}}
            }),
            dynamic_image_collection_schema_value(),
        );
        let member = semantic.nodes["2"]
            .inputs
            .iter()
            .find(|input| input.name == "reference_images.ref_image_0")
            .unwrap();

        assert!(matches!(
            member.type_resolution,
            SchemaTypeResolution::Conflict { .. }
        ));
        assert_eq!(member.canonical_semantic, CanonicalSemantic::Unknown);
        assert!(!member.explicit_reference);
        assert!(!profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn DYNAMIC_REFERENCE_MEMBER_ORDER_INDEPENDENT() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "ImageSource", "inputs": {"file": "a.png"}},
            "2": {"class_type": "ImageSource", "inputs": {"file": "b.png"}},
            "3": {"class_type": "NeutralDynamicMediaConsumer", "inputs": {
                "prompt": "make a video",
                "reference_images.ref_image_0": ["1", 0],
                "reference_images.ref_image_1": ["2", 0]
            }},
            "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
        }))
        .expect("dynamic reference workflow should parse");
        let schema = dynamic_image_collection_schema();
        let graph = WorkflowGraph::from_document(&workflow).expect("graph should build");
        let active = ActiveDependencyGraph::from_graph(&graph, &["4".to_owned()])
            .expect("active graph should build");
        let forward = resolve_semantic_graph(&workflow, &schema, active.clone());
        let mut reversed_active = active;
        reversed_active.active_edges.reverse();
        let reverse = resolve_semantic_graph(&workflow, &schema, reversed_active);

        assert_eq!(forward.nodes, reverse.nodes);
        assert!(forward.nodes["3"]
            .inputs
            .iter()
            .filter(|input| input.name.starts_with("reference_images.ref_image_"))
            .all(|input| input.explicit_reference));
    }

    #[test]
    #[allow(non_snake_case)]
    fn CUSTOM_TYPE_NAME_DOES_NOT_CHANGE_CAPABILITY() {
        let (_, semantic, profile) = profile_with_schema(
            json!({
                "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "ControlSource", "inputs": {}},
                "3": {"class_type": "VideoGenerator", "inputs": {
                    "prompt": ["1", 0],
                    "control": ["2", 0]
                }},
                "4": {"class_type": "VideoOutput", "inputs": {"video": ["3", 0]}}
            }),
            json!({
                "TextSource": {
                    "output": ["STRING"],
                    "input": {"required": {"text": ["STRING", {}]}}
                },
                "ControlSource": {"output": ["IMAGE_MAGIC_CONTROL"]},
                "VideoGenerator": {
                    "output": ["VIDEO"],
                    "input": {"required": {
                        "prompt": ["STRING", {}],
                        "control": ["IMAGE_MAGIC_CONTROL", {}]
                    }}
                },
                "VideoOutput": {
                    "output": ["VIDEO"],
                    "output_node": true,
                    "input": {"required": {"video": ["VIDEO", {}]}}
                }
            }),
        );
        assert!(semantic.unknown_active_dependencies.is_empty());
        assert_eq!(profile.primary_capability.as_deref(), Some("text_to_video"));
        assert!(profile.usable);
    }

    #[test]
    #[allow(non_snake_case)]
    fn OPAQUE_TYPE_ORDER_INDEPENDENT() {
        let workflow = json!({
            "1": {"class_type": "TextSource", "inputs": {"text": "hello"}},
            "2": {"class_type": "ControlA", "inputs": {}},
            "3": {"class_type": "ControlB", "inputs": {}},
            "4": {"class_type": "VideoGenerator", "inputs": {
                "prompt": ["1", 0], "control_a": ["2", 0], "control_b": ["3", 0]
            }},
            "5": {"class_type": "VideoOutput", "inputs": {"video": ["4", 0]}}
        });
        let schema_a = json!({
            "TextSource": {"output": ["STRING"], "input": {"required": {"text": ["STRING", {}]}}},
            "ControlA": {"output": ["CUSTOM_A"]},
            "ControlB": {"output": ["CUSTOM_B"]},
            "VideoGenerator": {"output": ["VIDEO"], "input": {"required": {
                "prompt": ["STRING", {}], "control_a": ["CUSTOM_A", {}], "control_b": ["CUSTOM_B", {}]
            }}},
            "VideoOutput": {"output": ["VIDEO"], "output_node": true, "input": {"required": {"video": ["VIDEO", {}]}}}
        });
        let schema_b = json!({
            "TextSource": {"output": ["STRING"], "input": {"required": {"text": ["STRING", {}]}}},
            "ControlA": {"output": ["CUSTOM_A"]},
            "ControlB": {"output": ["CUSTOM_B"]},
            "VideoGenerator": {"output": ["VIDEO"], "input": {"required": {
                "control_b": ["CUSTOM_B", {}], "control_a": ["CUSTOM_A", {}], "prompt": ["STRING", {}]
            }}},
            "VideoOutput": {"output": ["VIDEO"], "output_node": true, "input": {"required": {"video": ["VIDEO", {}]}}}
        });
        let left = profile_with_schema(workflow.clone(), schema_a);
        let right = profile_with_schema(workflow, schema_b);
        assert_eq!(left.2, right.2);
        assert_eq!(
            left.1.unknown_active_dependencies,
            right.1.unknown_active_dependencies
        );
    }
}
