use crate::domain::consistency::{BindingRole, InheritanceMode, ProfileType};
use crate::domain::ShotStage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContextSourceScope {
    Project,
    Series,
    Episode,
    Scene,
    Shot,
    Legacy,
}

impl ContextSourceScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "PROJECT",
            Self::Series => "SERIES",
            Self::Episode => "EPISODE",
            Self::Scene => "SCENE",
            Self::Shot => "SHOT",
            Self::Legacy => "LEGACY",
        }
    }

    pub const fn rank(self) -> usize {
        match self {
            Self::Project => 0,
            Self::Series => 1,
            Self::Episode => 2,
            Self::Scene => 3,
            Self::Shot => 4,
            Self::Legacy => 5,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceTrace {
    pub scope: ContextSourceScope,
    pub scope_id: String,
    pub binding_id: Option<String>,
    pub entity_id: String,
    pub inheritance_mode: InheritanceMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContextDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextDiagnostic {
    pub severity: ContextDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub scope: Option<ContextSourceScope>,
    pub entity_id: Option<String>,
}

impl ContextDiagnostic {
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ContextDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            scope: None,
            entity_id: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ContextDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            scope: None,
            entity_id: None,
        }
    }

    pub fn with_source(mut self, scope: ContextSourceScope, entity_id: impl Into<String>) -> Self {
        self.scope = Some(scope);
        self.entity_id = Some(entity_id.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PromptSegmentKind {
    GlobalStyle,
    Scene,
    Character,
    Costume,
    Props,
    ShotAction,
    Camera,
    Lighting,
    OutputSpecification,
}

impl PromptSegmentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GlobalStyle => "GLOBAL_STYLE",
            Self::Scene => "SCENE",
            Self::Character => "CHARACTER",
            Self::Costume => "COSTUME",
            Self::Props => "PROPS",
            Self::ShotAction => "SHOT_ACTION",
            Self::Camera => "CAMERA",
            Self::Lighting => "LIGHTING",
            Self::OutputSpecification => "OUTPUT_SPECIFICATION",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptContext {
    pub segments: Vec<PromptSegment>,
    pub rendered_text: String,
    pub negative_prompt: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptSegment {
    pub kind: PromptSegmentKind,
    pub text: String,
    pub source_scope: ContextSourceScope,
    pub source_entity_id: String,
    pub revision_id: Option<String>,
    pub omitted_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedCharacter {
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub reference_set_ids: Vec<String>,
    pub source: SourceTrace,
    pub revision_id: Option<String>,
    pub content_hash: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedScene {
    pub profile_id: Option<String>,
    pub reference_set_ids: Vec<String>,
    pub source: Option<SourceTrace>,
    pub revision_id: Option<String>,
    pub content_hash: Option<String>,
    pub prompt: String,
    pub lighting_prompt: Option<String>,
    pub negative_prompt: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedProp {
    pub profile_id: String,
    pub ordinal: i64,
    pub reference_set_ids: Vec<String>,
    pub source: SourceTrace,
    pub revision_id: Option<String>,
    pub content_hash: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedStyle {
    pub profile_id: Option<String>,
    pub reference_set_ids: Vec<String>,
    pub source: Option<SourceTrace>,
    pub revision_id: Option<String>,
    pub content_hash: Option<String>,
    pub prompt: String,
    pub negative_prompt: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedReferenceSet {
    pub reference_set_id: String,
    pub role: BindingRole,
    pub ordinal: i64,
    pub required: bool,
    pub source: SourceTrace,
    pub content_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedReferenceAsset {
    pub asset_id: String,
    pub sha256: String,
    pub role: BindingRole,
    pub ordinal: i64,
    pub source_reference_set_id: String,
    pub source_profile_id: Option<String>,
    pub source_scope: ContextSourceScope,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShotReferencePack {
    pub shot_id: String,
    pub characters: Vec<ResolvedCharacter>,
    pub scene: Option<ResolvedScene>,
    pub props: Vec<ResolvedProp>,
    pub style: Option<ResolvedStyle>,
    pub reference_sets: Vec<ResolvedReferenceSet>,
    pub prompt_context: PromptContext,
    pub source_trace: Vec<SourceTrace>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedStructureNode {
    pub id: String,
    pub ordinal: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedStructure {
    pub series: Option<ResolvedStructureNode>,
    pub episode: Option<ResolvedStructureNode>,
    pub scene: Option<ResolvedStructureNode>,
    pub shot: ResolvedStructureNode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedProfile {
    pub profile_id: String,
    pub profile_type: ProfileType,
    pub ordinal: i64,
    pub revision_id: Option<String>,
    pub content_hash: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub costume_variant_id: Option<String>,
    pub source: SourceTrace,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedProfiles {
    pub characters: Vec<ResolvedProfile>,
    pub scene: Option<ResolvedProfile>,
    pub props: Vec<ResolvedProfile>,
    pub style: Option<ResolvedProfile>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ResolvedWorkflowContext {
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub scalar_values: Value,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ResolvedOutputSpec {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub count: Option<i64>,
    pub duration_seconds: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LegacyContext {
    pub has_reference_pack: bool,
    pub uses_legacy_shot_references: bool,
    pub prompt: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolverIdentity {
    pub resolved_at: Option<DateTime<Utc>>,
    pub context_hash: String,
    pub reference_set_content_hashes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedShotContext {
    pub project_id: String,
    pub structure: ResolvedStructure,
    pub stage: ShotStage,
    pub reference_pack: ShotReferencePack,
    pub profiles: ResolvedProfiles,
    pub reference_assets: Vec<ResolvedReferenceAsset>,
    pub prompt_context: PromptContext,
    pub workflow: ResolvedWorkflowContext,
    pub output: ResolvedOutputSpec,
    pub legacy: LegacyContext,
    pub diagnostics: Vec<ContextDiagnostic>,
    pub partial: bool,
    pub resolver_identity: ResolverIdentity,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ContextHashInput {
    pub project_id: String,
    pub structure: ResolvedStructure,
    pub stage: String,
    pub profile_ids: Vec<String>,
    pub profile_content_hashes: Vec<String>,
    pub costume_ids: Vec<String>,
    pub reference_set_content_hashes: BTreeMap<String, String>,
    pub asset_ids: Vec<String>,
    pub asset_sha256: Vec<String>,
    pub ordered_prompt_segments: Vec<PromptSegment>,
    pub negative_prompt: String,
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub scalar_values: Value,
    pub output: ResolvedOutputSpec,
}
