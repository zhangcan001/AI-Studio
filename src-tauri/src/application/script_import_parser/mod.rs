//! Deterministic ScriptSource -> Draft parsing contracts.
//!
//! This module is deliberately a Draft-only boundary.  It does not know about
//! formal production entities, profiles, generation, queues, or ComfyUI.

pub mod json_parser;
pub mod markdown_parser;
pub mod reconcile;
pub mod source_map;
pub mod text_parser;

use crate::domain::script_draft::{
    Diagnostic, DiagnosticId, DiagnosticSeverity, DraftEpisode, DraftNodeId, DraftScene, DraftShot,
    DraftStatus, DraftStructureV1, EntityMention, ScriptFormat, SourceBlock, SourceBlockKind,
    SourceId, SourceSpan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Stable identity for every persisted PARSED/REPARSED revision produced by
/// this parser family.
pub const SCRIPT_IMPORT_PARSER_VERSION: &str = "script-import-v1";
pub const ANCHOR_MAP_METADATA_KEY: &str = "scriptImport.anchorMap.v1";
pub const MAX_UNKNOWN_JSON_METADATA_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScriptParseMode {
    #[default]
    Auto,
    Screenplay,
    Novel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptParseOptions {
    #[serde(default)]
    pub mode: ScriptParseMode,
    #[serde(default = "default_preserve_human_edits")]
    pub preserve_human_edits: bool,
}

const fn default_preserve_human_edits() -> bool {
    true
}

impl Default for ScriptParseOptions {
    fn default() -> Self {
        Self {
            mode: ScriptParseMode::Auto,
            preserve_human_edits: true,
        }
    }
}

impl From<&ScriptParseOptions> for ScriptParseOptions {
    fn from(value: &ScriptParseOptions) -> Self {
        value.clone()
    }
}

/// Cheap cooperative cancellation.  Parsers check this at finite block/node
/// boundaries; no task or background executor is created here.
#[derive(Clone, Default)]
pub struct ParseCancellationToken(Arc<AtomicBool>);

impl ParseCancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    #[allow(non_snake_case)]
    pub fn isCancelled(&self) -> bool {
        self.is_cancelled()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptImportStatistics {
    pub block_count: usize,
    pub episode_count: usize,
    pub scene_count: usize,
    pub shot_count: usize,
    pub character_mention_count: usize,
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub blocker_count: usize,
    pub parse_milliseconds: u128,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftParseDiffSummary {
    pub retained_nodes: usize,
    pub added_nodes: usize,
    pub removed_nodes: usize,
    pub changed_nodes: usize,
}

#[derive(Clone)]
pub struct ParserInput<'a> {
    pub source_id: &'a SourceId,
    pub project_id: &'a str,
    pub format: ScriptFormat,
    pub original_filename: Option<&'a str>,
    pub raw: &'a [u8],
    pub map: &'a source_map::SourceMap,
    pub options: &'a ScriptParseOptions,
    pub cancellation: Option<&'a ParseCancellationToken>,
}

pub struct ParserOutput {
    pub source_blocks: Vec<SourceBlock>,
    pub structure: Option<DraftStructureV1>,
    pub diagnostics: Vec<Diagnostic>,
    pub anchors: BTreeMap<String, DraftNodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParserError {
    InvalidUtf8,
    Cancelled,
    OutputInvalid,
}

impl ParserError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "INVALID_SOURCE_UTF8",
            Self::Cancelled => "SCRIPT_PARSE_CANCELLED",
            Self::OutputInvalid => "PARSER_OUTPUT_INVALID",
        }
    }
}

impl fmt::Display for ParserError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ParserError {}

/// Build the shared source map and dispatch to the format-specific parser.
/// Output normalization is centralized so all formats get deterministic
/// diagnostic IDs and the same private anchor-map representation.
pub fn parse_source(
    source_id: &SourceId,
    project_id: &str,
    format: ScriptFormat,
    original_filename: Option<&str>,
    raw: &[u8],
    options: &ScriptParseOptions,
    cancellation: Option<&ParseCancellationToken>,
) -> Result<ParserOutput, ParserError> {
    let map = source_map::SourceMap::new(raw).map_err(|_| ParserError::InvalidUtf8)?;
    let input = ParserInput {
        source_id,
        project_id,
        format,
        original_filename,
        raw,
        map: &map,
        options,
        cancellation,
    };
    check_cancel(&input)?;
    let output = match format {
        ScriptFormat::Txt => text_parser::parse(&input),
        ScriptFormat::Markdown => markdown_parser::parse(&input),
        ScriptFormat::Json => json_parser::parse(&input),
    }?;
    normalize_output(output)
}

pub(crate) fn check_cancel(input: &ParserInput<'_>) -> Result<(), ParserError> {
    if input
        .cancellation
        .is_some_and(ParseCancellationToken::is_cancelled)
    {
        return Err(ParserError::Cancelled);
    }
    Ok(())
}

pub(crate) fn new_structure(source_id: &SourceId, title: Option<String>) -> DraftStructureV1 {
    DraftStructureV1 {
        schema_version: crate::domain::script_draft::DRAFT_SCHEMA_VERSION,
        contract_version: crate::domain::script_draft::DRAFT_CONTRACT_VERSION,
        draft_id: crate::domain::script_draft::DraftId::new(),
        source_id: source_id.clone(),
        revision: 1,
        title,
        revision_id: crate::domain::script_draft::DraftRevisionId::new(),
        status: DraftStatus::Draft,
        episodes: Vec::new(),
        mentions: Vec::new(),
        diagnostics: Vec::new(),
        metadata: BTreeMap::new(),
    }
}

pub(crate) fn deterministic_node_id(anchor: &str) -> DraftNodeId {
    DraftNodeId::parse(format!("dnode_{}", deterministic_uuid(anchor))).expect("valid node id")
}

pub(crate) fn deterministic_diagnostic_id(anchor: &str) -> DiagnosticId {
    DiagnosticId::parse(format!("diag_{}", deterministic_uuid(anchor)))
        .expect("valid diagnostic id")
}

pub(crate) fn deterministic_block_id(
    parser_version: &str,
    format: ScriptFormat,
    start_byte: usize,
    end_byte: usize,
    kind: SourceBlockKind,
    raw: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parser_version.as_bytes());
    hasher.update([0]);
    hasher.update(format.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(start_byte.to_le_bytes());
    hasher.update(end_byte.to_le_bytes());
    hasher.update([0]);
    hasher.update(format!("{kind:?}").as_bytes());
    hasher.update([0]);
    hasher.update(raw);
    let digest = hasher.finalize();
    format!(
        "sblk_{}",
        digest
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

pub(crate) fn source_block(
    input: &ParserInput<'_>,
    start: usize,
    end: usize,
    kind: SourceBlockKind,
    parent_hint: Option<String>,
) -> SourceBlock {
    let span = input.map.span(start, end);
    SourceBlock {
        block_id: deterministic_block_id(
            SCRIPT_IMPORT_PARSER_VERSION,
            input.format,
            start,
            end,
            kind,
            &input.raw[start..end],
        ),
        span,
        preview: preview(&input.raw[start..end]),
        kind,
        parent_hint,
        diagnostics: Vec::new(),
    }
}

pub(crate) fn preview(raw: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(raw).ok()?.trim().to_owned();
    if text.is_empty() {
        return None;
    }
    Some(text.chars().take(160).collect())
}

pub(crate) fn diagnostic(
    severity: DiagnosticSeverity,
    code: &'static str,
    message: &'static str,
    span: Option<SourceSpan>,
) -> Diagnostic {
    let mut value = Diagnostic::new(severity, code, message);
    if let Some(span) = span {
        value = value.with_span(span);
    }
    value
}

pub(crate) fn normalize_output(mut output: ParserOutput) -> Result<ParserOutput, ParserError> {
    if let Some(structure) = output.structure.as_mut() {
        for diagnostic in output.diagnostics.clone() {
            if !structure
                .diagnostics
                .iter()
                .any(|existing| same_diagnostic(existing, &diagnostic))
            {
                structure.diagnostics.push(diagnostic);
            }
        }
        if !output.anchors.is_empty() {
            structure.metadata.insert(
                ANCHOR_MAP_METADATA_KEY.to_owned(),
                serde_json::to_string(&output.anchors).map_err(|_| ParserError::OutputInvalid)?,
            );
        }
        stabilize_structure_diagnostics(structure);
    }
    for (index, diagnostic) in output.diagnostics.iter_mut().enumerate() {
        diagnostic.diagnostic_id =
            deterministic_diagnostic_id(&format!("root/{index}/{}", diagnostic_key(diagnostic)));
    }
    Ok(output)
}

fn same_diagnostic(left: &Diagnostic, right: &Diagnostic) -> bool {
    left.severity == right.severity
        && left.code == right.code
        && left.field == right.field
        && left.source_spans == right.source_spans
        && left.draft_node_id == right.draft_node_id
}

fn stabilize_structure_diagnostics(structure: &mut DraftStructureV1) {
    for (index, diagnostic) in structure.diagnostics.iter_mut().enumerate() {
        diagnostic.diagnostic_id = deterministic_diagnostic_id(&format!(
            "structure/{index}/{}",
            diagnostic_key(diagnostic)
        ));
    }
    for episode in &mut structure.episodes {
        stabilize_diagnostics(&mut episode.diagnostics, episode.draft_node_id.as_str());
        for scene in &mut episode.scenes {
            stabilize_diagnostics(&mut scene.diagnostics, scene.draft_node_id.as_str());
            for shot in &mut scene.shots {
                stabilize_diagnostics(&mut shot.diagnostics, shot.draft_node_id.as_str());
            }
        }
    }
}

fn stabilize_diagnostics(diagnostics: &mut [Diagnostic], scope: &str) {
    for (index, diagnostic) in diagnostics.iter_mut().enumerate() {
        diagnostic.diagnostic_id =
            deterministic_diagnostic_id(&format!("{scope}/{index}/{}", diagnostic_key(diagnostic)));
    }
}

fn diagnostic_key(diagnostic: &Diagnostic) -> String {
    format!(
        "{}|{:?}|{}|{}",
        diagnostic.code,
        diagnostic.severity,
        diagnostic.field.as_deref().unwrap_or_default(),
        diagnostic
            .source_spans
            .first()
            .map(|span| format!("{}:{}", span.start_byte, span.end_byte))
            .unwrap_or_default()
    )
}

fn deterministic_uuid(value: &str) -> String {
    let mut digest = Sha256::digest(value.as_bytes());
    digest[6] = (digest[6] & 0x0f) | 0x40;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(digest[..16].try_into().expect("sha256 prefix"))
        .hyphenated()
        .to_string()
}

pub(crate) fn make_episode(
    anchor: &str,
    name: String,
    span: SourceSpan,
    diagnostics: Vec<Diagnostic>,
    scenes: Vec<DraftScene>,
) -> DraftEpisode {
    DraftEpisode {
        draft_node_id: deterministic_node_id(anchor),
        parent_draft_node_id: None,
        ordinal: 0,
        name,
        description: None,
        source_spans: vec![span],
        diagnostics,
        review_state: crate::domain::script_draft::DraftReviewState::Pending,
        origin: crate::domain::script_draft::DraftNodeOrigin::Imported,
        original_suggestion: None,
        current_value: None,
        scenes,
    }
}

pub(crate) fn make_scene(
    anchor: &str,
    parent_id: DraftNodeId,
    name: String,
    span: SourceSpan,
    diagnostics: Vec<Diagnostic>,
    location_suggestion: Option<String>,
    time_suggestion: Option<String>,
    shots: Vec<DraftShot>,
) -> DraftScene {
    DraftScene {
        draft_node_id: deterministic_node_id(anchor),
        parent_draft_node_id: Some(parent_id.clone()),
        ordinal: 0,
        name,
        description: None,
        source_spans: vec![span],
        diagnostics,
        review_state: crate::domain::script_draft::DraftReviewState::Pending,
        origin: crate::domain::script_draft::DraftNodeOrigin::Imported,
        original_suggestion: None,
        current_value: None,
        location_suggestion,
        time_suggestion,
        shots,
    }
}

pub(crate) fn make_shot(
    anchor: &str,
    parent_id: DraftNodeId,
    name: String,
    span: SourceSpan,
    description: Option<String>,
    action: Option<String>,
    dialogue: Vec<crate::domain::script_draft::DraftDialogue>,
    characters: Vec<EntityMention>,
    diagnostics: Vec<Diagnostic>,
) -> DraftShot {
    DraftShot {
        draft_node_id: deterministic_node_id(anchor),
        parent_draft_node_id: Some(parent_id.clone()),
        parent_scene_draft_id: parent_id,
        ordinal: 0,
        name,
        purpose: None,
        description,
        characters,
        scene_suggestion: None,
        props: Vec::new(),
        action,
        dialogue,
        camera_suggestion: None,
        lighting_suggestion: None,
        duration_suggestion: None,
        image_prompt_draft: None,
        video_prompt_draft: None,
        source_spans: vec![span],
        diagnostics,
        review_state: crate::domain::script_draft::DraftReviewState::Pending,
        origin: crate::domain::script_draft::DraftNodeOrigin::Imported,
        original_suggestion: None,
        current_value: None,
    }
}

pub(crate) fn make_mention(anchor: &str, raw_text: String, span: SourceSpan) -> EntityMention {
    EntityMention {
        mention_id: format!("mention_{}", deterministic_uuid(anchor)),
        entity_type: crate::domain::script_draft::EntityType::Character,
        raw_text: raw_text.clone(),
        normalized_text: Some(raw_text),
        draft_node_id: None,
        source_spans: vec![span],
        origin: crate::domain::script_draft::DraftNodeOrigin::Imported,
        confidence: Some(1.0),
        evidence: vec!["explicit-dialogue-speaker".to_owned()],
    }
}
