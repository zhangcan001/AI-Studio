//! Pure analysis for ComfyUI API workflows.
//!
//! This module deliberately stops at a report. It does not know about the
//! library, packages, registry, runtime state, project bindings, or ComfyUI.
//! Callers can use the report for import preview and let a separate command
//! decide whether anything should be persisted.

use crate::{
    application::{
        workflow_graph_analysis::{WorkflowGraph, WorkflowSource, WorkflowSourceTrace},
        workflow_recognition_schema::{
            MediaKind, RecognitionDeclaredType, RecognitionSchemaContext,
        },
        workflow_recognition_service::{
            structural_workflow_sha256, RecognitionConfidence, WorkflowIdentity,
            WorkflowRecognitionFormat,
        },
        workflow_semantic_graph::{
            canonical_semantic_hint, linked_target_semantic, semantic_field_type,
            semantic_media_family, CanonicalSemantic, SemanticInputHint,
        },
        workflow_semantic_identity::semantic_workflow_sha256,
    },
    domain::WorkflowDocument,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

// Keep all Phase 2A score weights and decision thresholds together. The
// numbers are deliberately integer and named so the same policy is used by
// input and output ranking without leaking implementation details into UI.
const SCORE_EXACT_INPUT_NAME: i32 = 50;
const SCORE_INPUT_NAME_ALIAS: i32 = 35;
const SCORE_GRAPH_DIRECT_SINK: i32 = 40;
const SCORE_GRAPH_OUTPUT_PATH: i32 = 30;
const SCORE_SCHEMA_TYPE_MATCH: i32 = 30;
const PENALTY_SCHEMA_TYPE_CONFLICT: i32 = -90;
const SCORE_MEDIA_TYPE_MATCH: i32 = 25;
const SCORE_CLASS_TYPE_HINT: i32 = 12;
const SCORE_NODE_TITLE_HINT: i32 = 10;
const SCORE_LITERAL_TYPE_MATCH: i32 = 15;
const SCORE_NUMERIC_RANGE_MATCH: i32 = 12;
const PENALTY_OFF_OUTPUT_PATH: i32 = -30;
const PENALTY_UTILITY_NODE: i32 = -45;
const HIGH_CONFIDENCE_MIN_SCORE: i32 = 45;
const MEDIUM_CONFIDENCE_MIN_SCORE: i32 = 25;
const REQUIRED_SELECTION_MARGIN: i32 = 15;
const AMBIGUITY_MARGIN: i32 = 12;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceKind {
    EXPLICIT_OUTPUT_MAPPING,
    EXACT_INPUT_NAME,
    INPUT_NAME_ALIAS,
    GRAPH_DIRECT_SINK,
    GRAPH_OUTPUT_PATH,
    SCHEMA_TYPE_MATCH,
    SCHEMA_TYPE_CONFLICT,
    SCHEMA_MEDIA_UPLOAD,
    SCHEMA_MEDIA_OUTPUT,
    MEDIA_TYPE_MATCH,
    CLASS_TYPE_HINT,
    NODE_TITLE_HINT,
    LITERAL_TYPE_MATCH,
    NUMERIC_RANGE_MATCH,
    OFF_OUTPUT_PATH,
    UTILITY_NODE,
    OUTPUT_NODE_FLAG,
    TERMINAL_OUTPUT,
    PREVIEW_OUTPUT,
    AUXILIARY_OUTPUT,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionEvidence {
    pub kind: EvidenceKind,
    pub reason: String,
    pub weight: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAnalysisInput {
    pub semantic_key: String,
    pub field_type: String,
    pub label: String,
    pub required: bool,
    pub value: Option<Value>,
    pub node_id: String,
    pub input_name: String,
    pub item_index: Option<usize>,
    pub confidence: RecognitionConfidence,
    pub source: String,
    pub score: i32,
    pub evidence: Vec<RecognitionEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutputRootResolutionReason {
    NoReliableMediaRoot,
    ConflictingCandidates,
    InvalidExplicitSelection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputRoot {
    pub output_id: String,
    #[serde(rename = "type")]
    pub output_type: String,
    pub node_id: String,
    pub label: String,
    pub score: i32,
    pub confidence: RecognitionConfidence,
    pub evidence_tier: u8,
    pub evidence: Vec<RecognitionEvidence>,
}

pub type OutputRootCandidate = OutputRoot;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutputRootResolution {
    Resolved {
        roots: Vec<OutputRoot>,
    },
    Ambiguous {
        candidates: Vec<OutputRootCandidate>,
        reason: OutputRootResolutionReason,
    },
    Unknown {
        reason: OutputRootResolutionReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputRootSelection {
    pub node_id: String,
    pub output_type: String,
}

impl OutputRootResolution {
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved { .. })
    }

    pub fn roots(&self) -> &[OutputRoot] {
        match self {
            Self::Resolved { roots } => roots,
            Self::Ambiguous { .. } | Self::Unknown { .. } => &[],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAnalysisBinding {
    pub semantic_key: String,
    pub target_node: String,
    pub target_input: String,
    pub item_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAnalysisOutput {
    pub output_id: String,
    #[serde(rename = "type")]
    pub output_type: String,
    pub node_id: String,
    pub label: String,
    pub required: bool,
    pub confidence: RecognitionConfidence,
    pub score: i32,
    pub evidence: Vec<RecognitionEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAnalysisIssue {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
    pub candidates: Vec<WorkflowAnalysisIssueCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAnalysisIssueCandidate {
    pub label: String,
    pub node_id: Option<String>,
    pub input_name: Option<String>,
    pub output_id: Option<String>,
    pub output_type: Option<String>,
    pub field_type: Option<String>,
    pub reason: String,
    pub score: i32,
    pub evidence: Vec<RecognitionEvidence>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAnalysisReport {
    pub format: WorkflowRecognitionFormat,
    pub recognized: bool,
    pub importable: bool,
    pub identity: WorkflowIdentity,
    #[serde(rename = "rawSha")]
    pub raw_sha256: String,
    #[serde(rename = "semanticSha")]
    pub semantic_sha256: String,
    #[serde(rename = "structuralSha")]
    pub structural_sha256: String,
    pub existing_workflow_id: Option<String>,
    pub existing_workflow_version_id: Option<String>,
    pub category: String,
    pub mode: String,
    pub inputs: Vec<WorkflowAnalysisInput>,
    pub bindings: Vec<WorkflowAnalysisBinding>,
    pub outputs: Vec<WorkflowAnalysisOutput>,
    pub output_root_resolution: OutputRootResolution,
    pub selected_root_id: Option<String>,
    pub confidence: RecognitionConfidence,
    pub issues: Vec<WorkflowAnalysisIssue>,
    pub suggested_actions: Vec<String>,
    pub node_count: usize,
    pub unique_class_count: usize,
}

/// Stateless, side-effect-free workflow analyzer.
pub struct WorkflowAnalysisService;

impl WorkflowAnalysisService {
    pub fn analyze_workflow(
        workflow: &WorkflowDocument,
        raw_bytes: &[u8],
    ) -> WorkflowAnalysisReport {
        analyze_workflow_with_schema(workflow, raw_bytes, None)
    }

    pub fn analyze_workflow_with_schema(
        workflow: &WorkflowDocument,
        raw_bytes: &[u8],
        schema: Option<&RecognitionSchemaContext>,
    ) -> WorkflowAnalysisReport {
        analyze_workflow_with_schema_and_output_roots(workflow, raw_bytes, schema, &[])
    }

    pub fn analyze_workflow_with_schema_and_output_roots(
        workflow: &WorkflowDocument,
        raw_bytes: &[u8],
        schema: Option<&RecognitionSchemaContext>,
        explicit_roots: &[OutputRootSelection],
    ) -> WorkflowAnalysisReport {
        analyze_workflow_with_schema_and_output_roots(workflow, raw_bytes, schema, explicit_roots)
    }

    pub fn analyze(workflow: &WorkflowDocument, raw_bytes: &[u8]) -> WorkflowAnalysisReport {
        analyze_workflow_with_schema(workflow, raw_bytes, None)
    }
}

/// Analyze an already parsed API workflow. The raw bytes are used only for
/// their exact SHA-256; no file, package, database, or runtime is consulted.
pub fn analyze_workflow(workflow: &WorkflowDocument, raw_bytes: &[u8]) -> WorkflowAnalysisReport {
    analyze_workflow_with_schema(workflow, raw_bytes, None)
}

/// Analyze an already parsed API workflow with one optional normalized schema
/// snapshot. The schema is pure input; identity hashes remain independent of
/// it so offline and online analysis preserve the same workflow identity.
pub fn analyze_workflow_with_schema(
    workflow: &WorkflowDocument,
    raw_bytes: &[u8],
    schema: Option<&RecognitionSchemaContext>,
) -> WorkflowAnalysisReport {
    analyze_workflow_with_schema_and_output_roots(workflow, raw_bytes, schema, &[])
}

/// Analyze an API workflow with an optional explicit output selection from the
/// onboarding draft. Explicit mappings are inputs to the same resolver; they
/// do not create a second root-selection policy.
pub fn analyze_workflow_with_schema_and_output_roots(
    workflow: &WorkflowDocument,
    raw_bytes: &[u8],
    schema: Option<&RecognitionSchemaContext>,
    explicit_roots: &[OutputRootSelection],
) -> WorkflowAnalysisReport {
    let raw_sha256 = sha256(raw_bytes);
    let semantic_sha256 = semantic_workflow_sha256(workflow);
    let structural_sha256 = structural_workflow_sha256(workflow);
    let node_count = workflow.value().as_object().map_or(0, |nodes| nodes.len());
    let unique_class_count = unique_class_count(workflow);

    let graph = match WorkflowGraph::from_document(workflow) {
        Ok(graph) => graph,
        Err(error) => {
            return report(
                raw_sha256,
                semantic_sha256,
                structural_sha256,
                node_count,
                unique_class_count,
                Vec::new(),
                Vec::new(),
                OutputRootResolution::Unknown {
                    reason: OutputRootResolutionReason::NoReliableMediaRoot,
                },
                None,
                "unknown".to_owned(),
                "unknown".to_owned(),
                vec![WorkflowAnalysisIssue {
                    code: "GRAPH_INVALID".to_owned(),
                    message: error.to_string(),
                    field: None,
                    candidates: Vec::new(),
                }],
            )
        }
    };

    let output_analysis = infer_outputs(workflow, &graph, schema, explicit_roots);
    let inference_scope = output_analysis
        .resolution
        .roots()
        .iter()
        .flat_map(|root| graph.upstream_closure(&root.node_id))
        .collect::<BTreeSet<_>>();

    let mut candidates = BTreeMap::<(String, Option<usize>), Vec<Candidate>>::new();
    let mut issues = output_analysis.issues;
    let selected_root_id = output_analysis.selected_root_id.clone();
    let Some(nodes) = workflow.value().as_object() else {
        return report(
            raw_sha256,
            semantic_sha256,
            structural_sha256,
            node_count,
            unique_class_count,
            Vec::new(),
            output_analysis.outputs,
            output_analysis.resolution,
            selected_root_id,
            "unknown".to_owned(),
            "unknown".to_owned(),
            issues,
        );
    };

    for node_id in &inference_scope {
        let Some(node) = nodes.get(node_id).and_then(Value::as_object) else {
            continue;
        };
        let class_type = node
            .get("class_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(inputs) = node.get("inputs").and_then(Value::as_object) else {
            continue;
        };
        for (input_name, value) in inputs {
            if is_link(value) {
                infer_linked_input(
                    workflow,
                    &graph,
                    node_id,
                    input_name,
                    schema,
                    &mut candidates,
                    &mut issues,
                );
            } else if let Some(guess) = literal_guess(class_type, input_name, value).or_else(|| {
                schema
                    .and_then(|schema| schema_literal_guess(schema, class_type, input_name, value))
            }) {
                if is_contextualized_media_literal(&graph, node_id, &guess)
                    && guess.source != "SCHEMA_MEDIA_TYPE_CONFLICT"
                {
                    continue;
                }
                append_candidate(
                    &mut candidates,
                    Candidate::literal(node_id, input_name, value, guess),
                );
            }
        }
    }

    for choices in candidates.values_mut() {
        for candidate in choices {
            enrich_candidate_with_context(
                candidate,
                workflow,
                &graph,
                schema,
                &output_analysis.roots,
            );
        }
    }

    let (inputs, _bindings, input_issues) = resolve_candidates(candidates);
    issues.extend(input_issues);
    let category = if output_analysis.resolution.is_resolved() {
        category_for_outputs(&output_analysis.outputs)
    } else {
        "unknown".to_owned()
    };
    let mode = if output_analysis.resolution.is_resolved() {
        infer_mode(&inputs, &category)
    } else {
        "unknown".to_owned()
    };
    report(
        raw_sha256,
        semantic_sha256,
        structural_sha256,
        node_count,
        unique_class_count,
        inputs,
        output_analysis.outputs,
        output_analysis.resolution,
        selected_root_id,
        category,
        mode,
        issues,
    )
}

#[derive(Clone)]
struct Candidate {
    semantic_key: String,
    field_type: String,
    required: bool,
    value: Option<Value>,
    node_id: String,
    input_name: String,
    item_index: Option<usize>,
    confidence: RecognitionConfidence,
    source: String,
    score: i32,
    evidence: Vec<RecognitionEvidence>,
    has_schema_conflict: bool,
}

impl Candidate {
    fn literal(node_id: &str, input_name: &str, value: &Value, guess: Guess) -> Self {
        Self::from_guess(
            guess,
            node_id.to_owned(),
            input_name.to_owned(),
            Some(value.clone()),
            None,
        )
    }

    fn from_guess(
        guess: Guess,
        node_id: String,
        input_name: String,
        value: Option<Value>,
        item_index: Option<usize>,
    ) -> Self {
        let evidence = evidence_for_guess_source(guess.source);
        Self {
            semantic_key: guess.semantic_key.to_owned(),
            field_type: guess.field_type.to_owned(),
            required: guess.required,
            value,
            node_id,
            input_name,
            item_index,
            confidence: guess.confidence,
            source: guess.source.to_owned(),
            score: evidence.iter().map(|evidence| evidence.weight).sum(),
            evidence,
            has_schema_conflict: false,
        }
    }

    fn linked_target(node_id: &str, input_name: &str, guess: Guess) -> Self {
        Self::from_guess(guess, node_id.to_owned(), input_name.to_owned(), None, None)
    }

    fn linked_leaf(
        node_id: &str,
        input_name: &str,
        value: &Value,
        item_index: Option<usize>,
        guess: Guess,
    ) -> Self {
        Self::from_guess(
            guess,
            node_id.to_owned(),
            input_name.to_owned(),
            Some(value.clone()),
            item_index,
        )
    }
}

fn evidence(kind: EvidenceKind, reason: impl Into<String>) -> RecognitionEvidence {
    RecognitionEvidence {
        weight: evidence_weight(kind),
        kind,
        reason: reason.into(),
    }
}

fn evidence_weight(kind: EvidenceKind) -> i32 {
    match kind {
        EvidenceKind::EXPLICIT_OUTPUT_MAPPING => 100,
        EvidenceKind::EXACT_INPUT_NAME => SCORE_EXACT_INPUT_NAME,
        EvidenceKind::INPUT_NAME_ALIAS => SCORE_INPUT_NAME_ALIAS,
        EvidenceKind::GRAPH_DIRECT_SINK => SCORE_GRAPH_DIRECT_SINK,
        EvidenceKind::GRAPH_OUTPUT_PATH => SCORE_GRAPH_OUTPUT_PATH,
        EvidenceKind::SCHEMA_TYPE_MATCH => SCORE_SCHEMA_TYPE_MATCH,
        EvidenceKind::SCHEMA_TYPE_CONFLICT => PENALTY_SCHEMA_TYPE_CONFLICT,
        EvidenceKind::SCHEMA_MEDIA_UPLOAD | EvidenceKind::SCHEMA_MEDIA_OUTPUT => {
            SCORE_SCHEMA_TYPE_MATCH
        }
        EvidenceKind::MEDIA_TYPE_MATCH => SCORE_MEDIA_TYPE_MATCH,
        EvidenceKind::CLASS_TYPE_HINT => SCORE_CLASS_TYPE_HINT,
        EvidenceKind::NODE_TITLE_HINT => SCORE_NODE_TITLE_HINT,
        EvidenceKind::LITERAL_TYPE_MATCH => SCORE_LITERAL_TYPE_MATCH,
        EvidenceKind::NUMERIC_RANGE_MATCH => SCORE_NUMERIC_RANGE_MATCH,
        EvidenceKind::OFF_OUTPUT_PATH => PENALTY_OFF_OUTPUT_PATH,
        EvidenceKind::UTILITY_NODE => PENALTY_UTILITY_NODE,
        EvidenceKind::OUTPUT_NODE_FLAG => 35,
        EvidenceKind::TERMINAL_OUTPUT => 25,
        EvidenceKind::PREVIEW_OUTPUT => -60,
        EvidenceKind::AUXILIARY_OUTPUT => -40,
    }
}

fn evidence_for_guess_source(source: &str) -> Vec<RecognitionEvidence> {
    match source {
        "INPUT_NAME_EXACT" | "INPUT_NAME_NEGATIVE_PROMPT" => {
            vec![evidence(
                EvidenceKind::EXACT_INPUT_NAME,
                "input name matches semantic field",
            )]
        }
        "INPUT_NAME_PROMPT_ALIAS"
        | "INPUT_NAME_PROMPT_HEURISTIC"
        | "INPUT_NAME_FIRST_FRAME"
        | "INPUT_NAME_LAST_FRAME" => {
            vec![evidence(
                EvidenceKind::INPUT_NAME_ALIAS,
                "input name is a supported alias",
            )]
        }
        "INPUT_NAME_SEED_AND_INTEGER_LITERAL" => vec![
            evidence(EvidenceKind::EXACT_INPUT_NAME, "seed input name matches"),
            evidence(EvidenceKind::LITERAL_TYPE_MATCH, "literal is an integer"),
        ],
        "INPUT_NAME_INTEGER_PARAMETER" | "INPUT_NAME_NUMBER_PARAMETER" => vec![
            evidence(
                EvidenceKind::INPUT_NAME_ALIAS,
                "numeric input name is a supported alias",
            ),
            evidence(EvidenceKind::LITERAL_TYPE_MATCH, "literal is numeric"),
        ],
        "INPUT_NAME_MEDIA_SEMANTICS" => vec![
            evidence(
                EvidenceKind::INPUT_NAME_ALIAS,
                "media input name is a supported alias",
            ),
            evidence(
                EvidenceKind::MEDIA_TYPE_MATCH,
                "literal is a media-like value",
            ),
        ],
        "GRAPH_LINKED_DIRECT_SINK" => vec![
            evidence(
                EvidenceKind::GRAPH_DIRECT_SINK,
                "linked input is a direct semantic sink",
            ),
            evidence(
                EvidenceKind::EXACT_INPUT_NAME,
                "target input name matches semantic field",
            ),
        ],
        "GRAPH_LINKED_MEDIA_SOURCE" => vec![
            evidence(
                EvidenceKind::GRAPH_OUTPUT_PATH,
                "media source is linked to an output path",
            ),
            evidence(
                EvidenceKind::MEDIA_TYPE_MATCH,
                "linked source belongs to the media family",
            ),
        ],
        "SCHEMA_MEDIA_UPLOAD_AND_OUTPUT" => vec![
            evidence(
                EvidenceKind::SCHEMA_MEDIA_UPLOAD,
                "schema input declares an uploaded media selector",
            ),
            evidence(
                EvidenceKind::SCHEMA_MEDIA_OUTPUT,
                "schema node output is compatible with the uploaded media",
            ),
        ],
        "GRAPH_DURATION_SOURCE" => vec![
            evidence(
                EvidenceKind::GRAPH_OUTPUT_PATH,
                "numeric leaf is on a duration path",
            ),
            evidence(EvidenceKind::LITERAL_TYPE_MATCH, "duration leaf is numeric"),
        ],
        "GRAPH_LINKED_SOURCE_LEAF" => vec![
            evidence(
                EvidenceKind::GRAPH_OUTPUT_PATH,
                "literal leaf is traced through the graph",
            ),
            evidence(
                EvidenceKind::LITERAL_TYPE_MATCH,
                "traced leaf has a compatible primitive kind",
            ),
        ],
        _ => vec![evidence(
            EvidenceKind::INPUT_NAME_ALIAS,
            "recognized by a semantic alias",
        )],
    }
}

fn evidence_reason(evidence: &[RecognitionEvidence]) -> String {
    evidence
        .iter()
        .map(|item| item.reason.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn push_evidence(candidate: &mut Candidate, item: RecognitionEvidence) {
    push_unique_evidence(&mut candidate.evidence, item);
}

fn push_unique_evidence(items: &mut Vec<RecognitionEvidence>, item: RecognitionEvidence) {
    if items
        .iter()
        .any(|existing| existing.kind == item.kind && existing.reason == item.reason)
    {
        return;
    }
    items.push(item);
}

fn schema_literal_guess(
    schema: &RecognitionSchemaContext,
    class_type: &str,
    input_name: &str,
    value: &Value,
) -> Option<Guess> {
    let schema_node = schema.node(class_type)?;
    let input = schema_node.input(input_name)?;
    let name = normalize(input_name);
    let class = class_type.to_ascii_lowercase();
    if let Some(media_kind) = input.upload_media_kind {
        if schema_node_outputs_media_kind(schema_node, media_kind)
            && literal_matches_media_kind(media_kind, value)
        {
            return Some(schema_media_guess(media_kind, input.required));
        }
    }
    if value.is_string()
        && matches!(
            input.declared_type,
            RecognitionDeclaredType::String | RecognitionDeclaredType::Enum
        )
        && is_prompt_like_name(&name, &class)
    {
        return Some(Guess {
            semantic_key: if name.contains("negative") {
                "negative_prompt"
            } else {
                "prompt"
            },
            field_type: "textarea",
            required: input.required,
            confidence: RecognitionConfidence::Medium,
            source: "SCHEMA_PROMPT_STRING",
        });
    }

    if value.is_number()
        && name.contains("image")
        && !matches!(
            input.declared_type,
            RecognitionDeclaredType::Image
                | RecognitionDeclaredType::Mask
                | RecognitionDeclaredType::Latent
        )
    {
        return Some(Guess {
            semantic_key: "reference_image",
            field_type: "image",
            required: false,
            confidence: RecognitionConfidence::Low,
            source: "SCHEMA_MEDIA_TYPE_CONFLICT",
        });
    }

    if value.is_number()
        && matches!(
            input.declared_type,
            RecognitionDeclaredType::Integer | RecognitionDeclaredType::Float
        )
    {
        if let Some(semantic_key) = schema_numeric_semantic(&name) {
            let field_type = if semantic_key == "seed" {
                "seed"
            } else if input.declared_type == RecognitionDeclaredType::Integer {
                "integer"
            } else {
                "number"
            };
            return Some(Guess {
                semantic_key,
                field_type,
                required: matches!(semantic_key, "width" | "height" | "duration_seconds"),
                confidence: RecognitionConfidence::Medium,
                source: "SCHEMA_NUMERIC_INPUT",
            });
        }
    }

    if (value.is_string() || value.is_array())
        && matches!(
            input.declared_type,
            RecognitionDeclaredType::Image
                | RecognitionDeclaredType::Mask
                | RecognitionDeclaredType::Latent
                | RecognitionDeclaredType::Video
                | RecognitionDeclaredType::Audio
        )
    {
        let semantic_key = semantic_key_for_declared_media(input.declared_type, input_name);
        let field_type = semantic_field_type(semantic_key);
        return Some(Guess {
            semantic_key,
            field_type,
            required: input.required,
            confidence: RecognitionConfidence::Medium,
            source: "SCHEMA_MEDIA_INPUT",
        });
    }

    None
}

fn schema_media_guess(media_kind: MediaKind, required: bool) -> Guess {
    let (semantic_key, field_type) = match media_kind {
        MediaKind::Image => ("reference_image", "image"),
        MediaKind::Video => ("reference_video", "video"),
        MediaKind::Audio => ("reference_audio", "audio"),
    };
    Guess {
        semantic_key,
        field_type,
        required,
        confidence: RecognitionConfidence::Medium,
        source: "SCHEMA_MEDIA_UPLOAD_AND_OUTPUT",
    }
}

fn schema_node_outputs_media_kind(
    node: &crate::application::workflow_recognition_schema::RecognitionNodeSchema,
    media_kind: MediaKind,
) -> bool {
    node.declared_output_types
        .iter()
        .any(|declared| match media_kind {
            MediaKind::Image => matches!(
                declared,
                RecognitionDeclaredType::Image
                    | RecognitionDeclaredType::Mask
                    | RecognitionDeclaredType::Latent
            ),
            MediaKind::Video => *declared == RecognitionDeclaredType::Video,
            MediaKind::Audio => *declared == RecognitionDeclaredType::Audio,
        })
}

fn literal_matches_media_kind(_media_kind: MediaKind, value: &Value) -> bool {
    value.is_string() || value.is_array() || is_link(value)
}

fn schema_numeric_semantic(name: &str) -> Option<&'static str> {
    canonical_semantic_hint(name)
        .map(|hint| hint.semantic)
        .filter(|semantic| semantic.is_numeric())
        .map(CanonicalSemantic::semantic_key)
}

fn semantic_key_for_declared_media(
    declared: RecognitionDeclaredType,
    input_name: &str,
) -> &'static str {
    let expected_family = match declared {
        RecognitionDeclaredType::Image => Some("image"),
        RecognitionDeclaredType::Video => Some("video"),
        RecognitionDeclaredType::Audio => Some("audio"),
        _ => None,
    };
    if let Some(hint) = canonical_semantic_hint(input_name) {
        if hint.semantic.media_family() == expected_family {
            return hint.semantic_key;
        }
        if hint.semantic.is_reference() {
            return match expected_family {
                Some("image") => "reference_image",
                Some("video") => "reference_video",
                Some("audio") => "reference_audio",
                _ => "image",
            };
        }
    }
    match expected_family {
        Some("video") => "video",
        Some("audio") => "audio",
        _ => "image",
    }
}

fn schema_linked_guess(
    schema: &RecognitionSchemaContext,
    class_type: &str,
    input_name: &str,
    value: &Value,
    semantic_key: &'static str,
) -> Option<Guess> {
    let schema_node = schema.node(class_type)?;
    let input = schema_node.input(input_name)?;
    let field_type = field_type_for_semantic(semantic_key);
    let declared_matches = declared_type_for_field(field_type).contains(&input.declared_type);
    let upload_matches = input.upload_media_kind.is_some_and(|media_kind| {
        media_kind_matches_field_type(media_kind, field_type)
            && schema_node_outputs_media_kind(schema_node, media_kind)
    });
    if !declared_matches && !upload_matches {
        return None;
    }
    if !literal_matches_field(field_type, value) && !upload_matches {
        return None;
    }
    Some(Guess {
        semantic_key,
        field_type,
        required: input.required,
        confidence: RecognitionConfidence::Medium,
        source: if upload_matches {
            "SCHEMA_MEDIA_UPLOAD_AND_OUTPUT"
        } else {
            "SCHEMA_GRAPH_TYPE"
        },
    })
}

fn is_prompt_like_name(name: &str, class_type: &str) -> bool {
    canonical_semantic_hint(name).is_some_and(|hint| {
        matches!(
            hint.semantic,
            CanonicalSemantic::PromptText
                | CanonicalSemantic::PositivePrompt
                | CanonicalSemantic::NegativePrompt
        )
    }) || ["prompt", "text", "caption", "conditioning"]
        .iter()
        .any(|marker| class_type.contains(marker))
}

fn declared_type_for_field(field_type: &str) -> &'static [RecognitionDeclaredType] {
    match field_type {
        "textarea" => &[
            RecognitionDeclaredType::String,
            RecognitionDeclaredType::Enum,
        ],
        "integer" | "seed" => &[RecognitionDeclaredType::Integer],
        "number" => &[
            RecognitionDeclaredType::Integer,
            RecognitionDeclaredType::Float,
        ],
        "image" | "images" => &[
            RecognitionDeclaredType::Image,
            RecognitionDeclaredType::Mask,
            RecognitionDeclaredType::Latent,
        ],
        "video" | "videos" => &[RecognitionDeclaredType::Video],
        "audio" | "audios" => &[RecognitionDeclaredType::Audio],
        _ => &[],
    }
}

fn schema_type_matches_candidate(
    class_type: &str,
    input_name: &str,
    field_type: &str,
    declared_type: RecognitionDeclaredType,
    upload_media_kind: Option<MediaKind>,
    upload_output_matches: bool,
    value: Option<&Value>,
) -> bool {
    if declared_type_for_field(field_type).contains(&declared_type) {
        return true;
    }

    if upload_output_matches
        && upload_media_kind
            .is_some_and(|media_kind| media_kind_matches_field_type(media_kind, field_type))
    {
        return true;
    }

    // ComfyUI's LoadImage filename combo is represented as an Enum in
    // object_info, while the graph value is still a media source.
    if field_type_for_media(field_type)
        && declared_type == RecognitionDeclaredType::Enum
        && class_type.eq_ignore_ascii_case("LoadImage")
        && normalize(input_name) == "image"
    {
        return true;
    }

    // ComfyUI commonly serializes lossless integer literals for FLOAT
    // sockets (for example PrimitiveFloat.value, fps, and denoise).
    declared_type == RecognitionDeclaredType::Float
        && matches!(field_type, "integer" | "seed")
        && value.is_some_and(is_integer_number)
}

fn field_type_for_media(field_type: &str) -> bool {
    matches!(
        field_type,
        "image" | "images" | "video" | "videos" | "audio" | "audios"
    )
}

fn media_kind_matches_field_type(media_kind: MediaKind, field_type: &str) -> bool {
    match media_kind {
        MediaKind::Image => matches!(field_type, "image" | "images"),
        MediaKind::Video => matches!(field_type, "video" | "videos"),
        MediaKind::Audio => matches!(field_type, "audio" | "audios"),
    }
}

fn literal_matches_field(field_type: &str, value: &Value) -> bool {
    match field_type {
        "textarea" => value.is_string(),
        "integer" | "seed" => is_integer_number(value),
        "number" => value.is_number(),
        "image" | "images" | "video" | "videos" | "audio" | "audios" => {
            value.is_string() || value.is_array() || is_link(value)
        }
        _ => false,
    }
}

fn compact_source(evidence: &[RecognitionEvidence]) -> String {
    let mut parts = BTreeSet::new();
    for item in evidence {
        match item.kind {
            EvidenceKind::EXPLICIT_OUTPUT_MAPPING => {
                parts.insert("OUTPUT");
            }
            EvidenceKind::EXACT_INPUT_NAME | EvidenceKind::INPUT_NAME_ALIAS => {
                parts.insert("NAME");
            }
            EvidenceKind::GRAPH_DIRECT_SINK | EvidenceKind::GRAPH_OUTPUT_PATH => {
                parts.insert("GRAPH");
            }
            EvidenceKind::SCHEMA_TYPE_MATCH
            | EvidenceKind::SCHEMA_TYPE_CONFLICT
            | EvidenceKind::SCHEMA_MEDIA_UPLOAD
            | EvidenceKind::SCHEMA_MEDIA_OUTPUT => {
                parts.insert("SCHEMA");
            }
            EvidenceKind::MEDIA_TYPE_MATCH => {
                parts.insert("MEDIA");
            }
            EvidenceKind::CLASS_TYPE_HINT | EvidenceKind::NODE_TITLE_HINT => {
                parts.insert("HINT");
            }
            EvidenceKind::LITERAL_TYPE_MATCH | EvidenceKind::NUMERIC_RANGE_MATCH => {
                parts.insert("LITERAL");
            }
            EvidenceKind::OFF_OUTPUT_PATH => {
                parts.insert("OFF_PATH");
            }
            EvidenceKind::UTILITY_NODE => {
                parts.insert("UTILITY");
            }
            EvidenceKind::OUTPUT_NODE_FLAG => {
                parts.insert("OUTPUT");
            }
            EvidenceKind::TERMINAL_OUTPUT => {
                parts.insert("TERMINAL");
            }
            EvidenceKind::PREVIEW_OUTPUT => {
                parts.insert("PREVIEW");
            }
            EvidenceKind::AUXILIARY_OUTPUT => {
                parts.insert("AUXILIARY");
            }
        }
    }
    if parts.is_empty() {
        "UNKNOWN".to_owned()
    } else {
        [
            "GRAPH",
            "SCHEMA",
            "NAME",
            "MEDIA",
            "LITERAL",
            "HINT",
            "OUTPUT",
            "TERMINAL",
            "PREVIEW",
            "AUXILIARY",
            "OFF_PATH",
            "UTILITY",
        ]
        .into_iter()
        .filter(|part| parts.contains(part))
        .collect::<Vec<_>>()
        .join("+")
    }
}

fn enrich_candidate_with_context(
    candidate: &mut Candidate,
    workflow: &WorkflowDocument,
    graph: &WorkflowGraph,
    schema: Option<&RecognitionSchemaContext>,
    output_roots: &[String],
) {
    if let Some(value) = candidate.value.as_ref() {
        if literal_matches_field(&candidate.field_type, value) {
            push_evidence(
                candidate,
                evidence(
                    EvidenceKind::LITERAL_TYPE_MATCH,
                    "current JSON value matches field type",
                ),
            );
        }
    }

    if graph.downstream_of(&candidate.node_id).iter().any(|link| {
        linked_target_semantic(&link.target_input)
            .is_some_and(|target| target.semantic_key == candidate.semantic_key)
    }) {
        push_evidence(
            candidate,
            evidence(
                EvidenceKind::GRAPH_DIRECT_SINK,
                "candidate feeds a direct semantic graph sink",
            ),
        );
    }

    if !output_roots.is_empty() {
        if output_roots
            .iter()
            .any(|root| graph.is_on_output_path(&candidate.node_id, root))
        {
            push_evidence(
                candidate,
                evidence(
                    EvidenceKind::GRAPH_OUTPUT_PATH,
                    "candidate is on a selected output dependency path",
                ),
            );
        } else {
            push_evidence(
                candidate,
                evidence(
                    EvidenceKind::OFF_OUTPUT_PATH,
                    "candidate is outside selected output paths",
                ),
            );
        }
    }

    let class_type = workflow.class_type(&candidate.node_id).unwrap_or_default();
    let title = workflow
        .node(&candidate.node_id)
        .and_then(|node| node.get("_meta"))
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("title"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let semantic = normalize(&candidate.semantic_key);
    if class_type.to_ascii_lowercase().contains(&semantic)
        || (candidate.semantic_key == "prompt" && class_type.to_ascii_lowercase().contains("text"))
    {
        push_evidence(
            candidate,
            evidence(
                EvidenceKind::CLASS_TYPE_HINT,
                "class type contains the semantic field hint",
            ),
        );
    }
    if title.to_ascii_lowercase().contains(&semantic)
        || (candidate.semantic_key == "prompt" && title.to_ascii_lowercase().contains("text"))
    {
        push_evidence(
            candidate,
            evidence(
                EvidenceKind::NODE_TITLE_HINT,
                "node title contains the semantic field hint",
            ),
        );
    }
    if is_utility_class(&format!("{class_type} {title}").to_ascii_lowercase()) {
        push_evidence(
            candidate,
            evidence(
                EvidenceKind::UTILITY_NODE,
                "candidate originates from a utility node",
            ),
        );
    }

    if let Some(schema) = schema {
        if let Some(schema_node) = schema.node(class_type) {
            if let Some(input_schema) = schema_node.input(&candidate.input_name) {
                if schema_type_matches_candidate(
                    class_type,
                    &candidate.input_name,
                    &candidate.field_type,
                    input_schema.declared_type,
                    input_schema.upload_media_kind,
                    input_schema.upload_media_kind.is_some_and(|media_kind| {
                        schema_node_outputs_media_kind(schema_node, media_kind)
                    }),
                    candidate.value.as_ref(),
                ) {
                    push_evidence(
                        candidate,
                        evidence(
                            EvidenceKind::SCHEMA_TYPE_MATCH,
                            "declared schema type matches field type",
                        ),
                    );
                } else if input_schema.declared_type != RecognitionDeclaredType::Unknown {
                    candidate.has_schema_conflict = true;
                    push_evidence(
                        candidate,
                        evidence(
                            EvidenceKind::SCHEMA_TYPE_CONFLICT,
                            "declared schema type conflicts with field type",
                        ),
                    );
                }
                if let Some(value) = candidate.value.as_ref().and_then(Value::as_f64) {
                    let within_min = input_schema.numeric_min.is_none_or(|min| value >= min);
                    let within_max = input_schema.numeric_max.is_none_or(|max| value <= max);
                    if within_min && within_max {
                        push_evidence(
                            candidate,
                            evidence(
                                EvidenceKind::NUMERIC_RANGE_MATCH,
                                "numeric literal is within schema range",
                            ),
                        );
                    } else {
                        candidate.has_schema_conflict = true;
                        push_evidence(
                            candidate,
                            evidence(
                                EvidenceKind::SCHEMA_TYPE_CONFLICT,
                                "numeric literal is outside schema range",
                            ),
                        );
                    }
                }
            }
        }
    }

    candidate.score = candidate.evidence.iter().map(|item| item.weight).sum();
    candidate.confidence = if candidate.has_schema_conflict {
        RecognitionConfidence::Low
    } else {
        confidence_from_score(candidate.score)
    };
    if schema.is_some() {
        candidate.source = compact_source(&candidate.evidence);
    }
}

#[derive(Clone, Copy)]
struct Guess {
    semantic_key: &'static str,
    field_type: &'static str,
    required: bool,
    confidence: RecognitionConfidence,
    source: &'static str,
}

type LinkedTargetSemantic = SemanticInputHint;

fn infer_linked_input(
    workflow: &WorkflowDocument,
    graph: &WorkflowGraph,
    target_node: &str,
    target_input: &str,
    schema: Option<&RecognitionSchemaContext>,
    candidates: &mut BTreeMap<(String, Option<usize>), Vec<Candidate>>,
    issues: &mut Vec<WorkflowAnalysisIssue>,
) {
    let Some(target_semantic) = linked_target_semantic(target_input) else {
        return;
    };
    let semantic_key = target_semantic.semantic_key;

    if matches!(semantic_key, "width" | "height") {
        append_candidate(
            candidates,
            Candidate::linked_target(
                target_node,
                target_input,
                Guess {
                    semantic_key,
                    field_type: "integer",
                    required: true,
                    confidence: RecognitionConfidence::High,
                    source: "GRAPH_LINKED_DIRECT_SINK",
                },
            ),
        );
        return;
    }

    if semantic_key == "duration_seconds" {
        if let Some(candidate) =
            infer_duration_candidate(workflow, graph, target_node, target_input)
        {
            append_candidate(candidates, candidate);
        } else if graph.incoming_source(target_node, target_input).is_some() {
            issues.push(WorkflowAnalysisIssue {
                code: "AMBIGUOUS_DURATION_SOURCE".to_owned(),
                message: "无法自动确认视频时长来源，请选择唯一的动态数值源。".to_owned(),
                field: Some(semantic_key.to_owned()),
                candidates: Vec::new(),
            });
        }
        return;
    }

    for trace in graph.trace_sources(target_node, target_input) {
        if trace_crosses_unrelated_media_input(&trace, target_node, target_input, target_semantic) {
            continue;
        }
        let leaf = trace.source;
        let class_type = workflow.class_type(&leaf.node_id).unwrap_or_default();
        let Some(guess) = literal_guess(class_type, &leaf.input, &leaf.value)
            .or_else(|| {
                schema.and_then(|schema| {
                    schema_literal_guess(schema, class_type, &leaf.input, &leaf.value)
                })
            })
            .or_else(|| {
                schema.and_then(|schema| {
                    schema_linked_guess(
                        schema,
                        class_type,
                        &leaf.input,
                        &leaf.value,
                        target_semantic.semantic_key,
                    )
                })
            })
        else {
            continue;
        };
        let Some(guess) = linked_source_guess(target_semantic, guess) else {
            continue;
        };
        // Product-level media collections may expose optional indexed slots even
        // when the upstream ComfyUI socket itself is required.
        let required = if target_semantic.item_index.is_some() {
            false
        } else if target_semantic.force_media_source {
            schema
                .and_then(|schema| {
                    workflow
                        .class_type(target_node)
                        .and_then(|class_type| schema.node(class_type))
                })
                .and_then(|node| node.input(target_input))
                .map(|input| input.required)
                .unwrap_or(guess.required)
        } else {
            guess.required
        };
        append_candidate(
            candidates,
            Candidate::linked_leaf(
                &leaf.node_id,
                &leaf.input,
                &leaf.value,
                target_semantic.item_index,
                Guess {
                    required,
                    source: match guess.source {
                        "GRAPH_LINKED_MEDIA_SOURCE" => "GRAPH_LINKED_MEDIA_SOURCE",
                        "SCHEMA_MEDIA_UPLOAD_AND_OUTPUT" => "SCHEMA_MEDIA_UPLOAD_AND_OUTPUT",
                        _ => "GRAPH_LINKED_SOURCE_LEAF",
                    },
                    ..guess
                },
            ),
        );
    }
}

fn infer_duration_candidate(
    workflow: &WorkflowDocument,
    graph: &WorkflowGraph,
    target_node: &str,
    target_input: &str,
) -> Option<Candidate> {
    let expression_node = graph.incoming_source(target_node, target_input)?;
    let leaves = graph_numeric_leaves(graph, expression_node);
    if leaves.len() != 1 {
        return None;
    }
    let expression = workflow
        .inputs(expression_node)?
        .get("expression")
        .and_then(Value::as_str)?;
    let expression_class = workflow.class_type(expression_node).unwrap_or_default();
    if !expression_proves_duration(expression)
        || !is_arithmetic_node(expression_class)
        || !arithmetic_chain_is_safe(workflow, graph, expression_node, &mut BTreeSet::new())
    {
        return None;
    }
    let leaf = leaves.into_iter().next()?;
    Some(Candidate::linked_leaf(
        &leaf.node_id,
        &leaf.input,
        &leaf.value,
        None,
        Guess {
            semantic_key: "duration_seconds",
            field_type: if is_integer_number(&leaf.value) {
                "integer"
            } else {
                "number"
            },
            required: true,
            confidence: RecognitionConfidence::High,
            source: "GRAPH_DURATION_SOURCE",
        },
    ))
}

fn linked_source_guess(
    target_semantic: LinkedTargetSemantic,
    source_guess: Guess,
) -> Option<Guess> {
    if source_guess.semantic_key == target_semantic.semantic_key {
        return Some(source_guess);
    }
    if target_semantic.force_media_source
        && media_family(target_semantic.semantic_key) == media_family(source_guess.semantic_key)
        && media_family(target_semantic.semantic_key).is_some()
    {
        return Some(Guess {
            semantic_key: target_semantic.semantic_key,
            field_type: field_type_for_semantic(target_semantic.semantic_key),
            required: true,
            confidence: RecognitionConfidence::High,
            source: if source_guess.source == "SCHEMA_MEDIA_UPLOAD_AND_OUTPUT" {
                "SCHEMA_MEDIA_UPLOAD_AND_OUTPUT"
            } else {
                "GRAPH_LINKED_MEDIA_SOURCE"
            },
        });
    }
    None
}

fn field_type_for_semantic(semantic_key: &str) -> &'static str {
    semantic_field_type(semantic_key)
}

fn media_family(semantic_key: &str) -> Option<&'static str> {
    semantic_media_family(semantic_key)
}

fn is_contextualized_media_literal(graph: &WorkflowGraph, node_id: &str, guess: &Guess) -> bool {
    media_family(guess.semantic_key).is_some()
        && graph.downstream_of(node_id).iter().any(|link| {
            linked_target_semantic(&link.target_input).is_some_and(|target| {
                media_family(target.semantic_key) == media_family(guess.semantic_key)
            })
        })
}

fn trace_crosses_unrelated_media_input(
    trace: &WorkflowSourceTrace,
    target_node: &str,
    target_input: &str,
    target_semantic: LinkedTargetSemantic,
) -> bool {
    trace.path.iter().skip(1).any(|link| {
        if link.target_node_id == target_node && link.target_input == target_input {
            return false;
        }
        linked_target_semantic(&link.target_input).is_some_and(|nested| {
            let nested_family = media_family(nested.semantic_key);
            let target_family = media_family(target_semantic.semantic_key);
            if target_semantic.item_index.is_some() {
                let same_family = nested_family
                    .zip(target_family)
                    .is_some_and(|(nested_family, target_family)| nested_family == target_family);
                !same_family
                    || nested.item_index.is_some()
                        && nested.item_index != target_semantic.item_index
            } else {
                nested_family.is_some()
                    && (nested.semantic_key != target_semantic.semantic_key
                        || nested.item_index != target_semantic.item_index)
            }
        })
    })
}

fn resolve_candidates(
    candidates: BTreeMap<(String, Option<usize>), Vec<Candidate>>,
) -> (
    Vec<WorkflowAnalysisInput>,
    Vec<WorkflowAnalysisBinding>,
    Vec<WorkflowAnalysisIssue>,
) {
    let mut inputs = Vec::new();
    let mut bindings = Vec::new();
    let mut issues = Vec::new();
    for ((semantic_key, _item_index), mut choices) in candidates {
        choices.sort_by(|left, right| {
            right.score.cmp(&left.score).then_with(|| {
                left.node_id
                    .cmp(&right.node_id)
                    .then(left.input_name.cmp(&right.input_name))
            })
        });
        let viable = choices
            .iter()
            .filter(|choice| !choice.has_schema_conflict)
            .cloned()
            .collect::<Vec<_>>();
        let selected = if viable.len() == 1 {
            Some(viable[0].clone())
        } else if viable.len() > 1
            && viable[0].confidence == RecognitionConfidence::High
            && viable[0].score - viable[1].score >= REQUIRED_SELECTION_MARGIN
        {
            Some(viable[0].clone())
        } else {
            None
        };
        let Some(selected) = selected else {
            let candidates = choices
                .iter()
                .map(|candidate| WorkflowAnalysisIssueCandidate {
                    label: format!("节点 {} · {}", candidate.node_id, candidate.input_name),
                    node_id: Some(candidate.node_id.clone()),
                    input_name: Some(candidate.input_name.clone()),
                    output_id: None,
                    output_type: None,
                    field_type: Some(candidate.field_type.clone()),
                    reason: evidence_reason(&candidate.evidence),
                    score: candidate.score,
                    evidence: candidate.evidence.clone(),
                })
                .collect();
            issues.push(WorkflowAnalysisIssue {
                code: "AMBIGUOUS_INPUT".to_owned(),
                message: format!("无法唯一判断 {semantic_key} 输入，请选择一个节点字段。"),
                field: Some(semantic_key),
                candidates,
            });
            continue;
        };
        inputs.push(WorkflowAnalysisInput {
            semantic_key: selected.semantic_key.clone(),
            field_type: selected.field_type,
            label: humanize(&selected.semantic_key),
            required: selected.required,
            value: selected.value,
            node_id: selected.node_id.clone(),
            input_name: selected.input_name.clone(),
            item_index: selected.item_index,
            confidence: selected.confidence,
            source: selected.source,
            score: selected.score,
            evidence: selected.evidence,
        });
        bindings.push(WorkflowAnalysisBinding {
            semantic_key: selected.semantic_key,
            target_node: selected.node_id,
            target_input: selected.input_name,
            item_index: selected.item_index,
        });
    }
    (inputs, bindings, issues)
}

fn append_candidate(
    candidates: &mut BTreeMap<(String, Option<usize>), Vec<Candidate>>,
    candidate: Candidate,
) {
    let choices = candidates
        .entry((candidate.semantic_key.clone(), candidate.item_index))
        .or_default();
    if let Some(existing) = choices.iter_mut().find(|existing| {
        existing.node_id == candidate.node_id
            && existing.input_name == candidate.input_name
            && existing.item_index == candidate.item_index
    }) {
        let candidate_rank = confidence_rank(candidate.confidence);
        let existing_rank = confidence_rank(existing.confidence);
        for item in candidate.evidence {
            push_unique_evidence(&mut existing.evidence, item);
        }
        existing.score = existing.evidence.iter().map(|item| item.weight).sum();
        existing.has_schema_conflict |= candidate.has_schema_conflict;
        if candidate_rank > existing_rank {
            existing.confidence = candidate.confidence;
            existing.source = candidate.source;
        }
    } else {
        choices.push(candidate);
    }
}

fn literal_guess(_class_type: &str, input_name: &str, value: &Value) -> Option<Guess> {
    let name = normalize(input_name);
    let is_text = value.is_string();
    let is_number = value.is_number();
    let is_media = is_text || value.is_array();
    if is_ignored_input(&name) {
        return None;
    }
    let hint = canonical_semantic_hint(input_name)?;
    let semantic = hint.semantic;
    if is_text
        && matches!(
            semantic,
            CanonicalSemantic::PromptText | CanonicalSemantic::PositivePrompt
        )
    {
        return Some(Guess {
            semantic_key: "prompt",
            field_type: "textarea",
            required: true,
            confidence: RecognitionConfidence::High,
            source: if name == "prompt" || name == "positive_prompt" {
                "INPUT_NAME_EXACT"
            } else {
                "INPUT_NAME_PROMPT_ALIAS"
            },
        });
    }
    if is_text && semantic == CanonicalSemantic::NegativePrompt {
        return Some(Guess {
            semantic_key: "negative_prompt",
            field_type: "textarea",
            required: false,
            confidence: RecognitionConfidence::High,
            source: "INPUT_NAME_NEGATIVE_PROMPT",
        });
    }
    if is_number && semantic == CanonicalSemantic::Seed {
        return is_integer_number(value).then_some(Guess {
            semantic_key: "seed",
            field_type: "seed",
            required: true,
            confidence: RecognitionConfidence::High,
            source: "INPUT_NAME_SEED_AND_INTEGER_LITERAL",
        });
    }
    if is_number && semantic.is_numeric() {
        if semantic == CanonicalSemantic::Seed {
            return None;
        }
        return Some(numeric_guess(
            semantic.semantic_key(),
            value,
            if matches!(
                semantic,
                CanonicalSemantic::Width
                    | CanonicalSemantic::Height
                    | CanonicalSemantic::Duration
                    | CanonicalSemantic::Frames
                    | CanonicalSemantic::Fps
                    | CanonicalSemantic::Denoise
            ) {
                RecognitionConfidence::High
            } else {
                RecognitionConfidence::Medium
            },
        ));
    }
    if is_media
        && matches!(
            semantic.media_family(),
            Some("image") | Some("video") | Some("audio")
        )
    {
        return Some(Guess {
            semantic_key: hint.semantic_key,
            field_type: semantic.field_type(),
            required: true,
            confidence: if semantic.is_reference() {
                RecognitionConfidence::Medium
            } else {
                RecognitionConfidence::Low
            },
            source: "INPUT_NAME_MEDIA_SEMANTICS",
        });
    }
    None
}

fn numeric_guess(
    semantic_key: &'static str,
    value: &Value,
    confidence: RecognitionConfidence,
) -> Guess {
    Guess {
        semantic_key,
        field_type: if is_integer_number(value) {
            "integer"
        } else {
            "number"
        },
        required: matches!(semantic_key, "width" | "height" | "duration_seconds"),
        confidence,
        source: if is_integer_number(value) {
            "INPUT_NAME_INTEGER_PARAMETER"
        } else {
            "INPUT_NAME_NUMBER_PARAMETER"
        },
    }
}

fn infer_outputs(
    workflow: &WorkflowDocument,
    graph: &WorkflowGraph,
    schema: Option<&RecognitionSchemaContext>,
    explicit_roots: &[OutputRootSelection],
) -> OutputAnalysis {
    let Some(nodes) = workflow.value().as_object() else {
        return OutputAnalysis::unknown();
    };
    let mut candidates = Vec::new();
    for (node_id, node) in nodes {
        let Some(node) = node.as_object() else {
            continue;
        };
        let class_type = node
            .get("class_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let title = node_title(node);
        let lower = format!("{class_type} {title}").to_ascii_lowercase();
        let schema_node = schema.and_then(|schema| schema.node(class_type));
        let schema_output_type = schema_node.and_then(declared_output_media_type);
        let node_output_type = media_output_type_from_node(node);
        let output_type = schema_output_type.clone().or(node_output_type);
        let explicit = node
            .get("output_node")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || schema_node.is_some_and(|node| node.output_node);
        let terminal = graph.downstream_of(node_id).is_empty();
        let input_only = is_input_only_output_class(&lower) && !explicit;
        let media_output_role = !input_only && (output_type.is_some() || explicit);
        let mut output_evidence = Vec::new();
        if schema_output_type.is_some() {
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::SCHEMA_TYPE_MATCH,
                    "declared output schema contains a media type",
                ),
            );
        }
        if explicit {
            push_unique_evidence(
                &mut output_evidence,
                evidence(EvidenceKind::OUTPUT_NODE_FLAG, "node declares output_node"),
            );
        }
        if terminal {
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::TERMINAL_OUTPUT,
                    "node has no downstream consumers",
                ),
            );
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::GRAPH_OUTPUT_PATH,
                    "node is a terminal graph output path",
                ),
            );
        }
        if media_output_role {
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::MEDIA_TYPE_MATCH,
                    "node has a recognizable media output role",
                ),
            );
        }
        if is_video_output_class(&lower)
            || is_image_output_class(&lower)
            || lower.contains("save")
            || lower.contains("output")
        {
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::CLASS_TYPE_HINT,
                    "class type identifies an output node",
                ),
            );
        }
        if ["final", "output", "publish", "deliver"]
            .iter()
            .any(|marker| title.to_ascii_lowercase().contains(marker))
        {
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::NODE_TITLE_HINT,
                    "node title identifies a final output",
                ),
            );
        }
        if lower.contains("preview") {
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::PREVIEW_OUTPUT,
                    "preview branch is auxiliary to a final output",
                ),
            );
        }
        if ["cache", "debug", "thumbnail", "aux", "temporary"]
            .iter()
            .any(|marker| lower.contains(marker))
        {
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::AUXILIARY_OUTPUT,
                    "node is an auxiliary output branch",
                ),
            );
        }
        if is_utility_class(&lower) {
            push_unique_evidence(
                &mut output_evidence,
                evidence(
                    EvidenceKind::UTILITY_NODE,
                    "utility nodes are not preferred outputs",
                ),
            );
        }
        let score = output_evidence.iter().map(|item| item.weight).sum::<i32>();
        if score <= 0 && !explicit && output_type.is_none() {
            continue;
        }
        let evidence_tier = output_root_evidence_tier(
            explicit,
            schema_output_type.is_some(),
            media_output_role,
            terminal,
            input_only,
            output_evidence
                .iter()
                .any(|evidence| evidence.kind == EvidenceKind::PREVIEW_OUTPUT),
            output_evidence
                .iter()
                .any(|evidence| evidence.kind == EvidenceKind::AUXILIARY_OUTPUT),
            output_evidence
                .iter()
                .any(|evidence| evidence.kind == EvidenceKind::UTILITY_NODE),
        );
        candidates.push(OutputCandidate {
            node_id: node_id.clone(),
            label: if title.is_empty() {
                format!("节点 {node_id}")
            } else {
                title
            },
            output_type: output_type.unwrap_or_else(|| "image".to_owned()),
            score,
            evidence: output_evidence,
            evidence_tier,
            explicit_eligible: explicit || (terminal && media_output_role && !input_only),
        });
    }
    candidates.sort_by(|left, right| {
        right
            .evidence_tier
            .cmp(&left.evidence_tier)
            .then(right.score.cmp(&left.score))
            .then(left.node_id.cmp(&right.node_id))
    });
    let resolution = resolve_output_roots(&candidates, explicit_roots);
    let selected_root_id = (explicit_roots.len() == 1)
        .then(|| explicit_roots[0].node_id.as_str())
        .and_then(|node_id| {
            resolution
                .roots()
                .iter()
                .find(|root| root.node_id == node_id)
                .map(|root| root.output_id.clone())
        });
    let outputs = match &resolution {
        OutputRootResolution::Resolved { roots } => roots
            .iter()
            .enumerate()
            .map(|(index, root)| output_from_root(root, index, false))
            .collect::<Vec<_>>(),
        OutputRootResolution::Ambiguous { candidates, .. } => candidates
            .iter()
            .enumerate()
            .map(|(index, root)| output_from_root(root, index, true))
            .collect::<Vec<_>>(),
        OutputRootResolution::Unknown { .. } => Vec::new(),
    };
    let issues = match &resolution {
        OutputRootResolution::Resolved { .. } => Vec::new(),
        OutputRootResolution::Ambiguous { .. } => vec![WorkflowAnalysisIssue {
            code: "AMBIGUOUS_OUTPUT".to_owned(),
            message: "检测到多个可能的最终输出节点，请选择要发布的输出。".to_owned(),
            field: Some("output_1".to_owned()),
            candidates: outputs.iter().map(output_issue_candidate).collect(),
        }],
        OutputRootResolution::Unknown { reason } => vec![WorkflowAnalysisIssue {
            code: "UNKNOWN_OUTPUT".to_owned(),
            message: match reason {
                OutputRootResolutionReason::InvalidExplicitSelection => {
                    "手动输出映射未指向可验证的媒体输出节点。".to_owned()
                }
                _ => "未能识别可靠的最终媒体输出节点。".to_owned(),
            },
            field: Some("output_1".to_owned()),
            candidates: Vec::new(),
        }],
    };
    OutputAnalysis {
        roots: resolution
            .roots()
            .iter()
            .map(|root| root.node_id.clone())
            .collect(),
        outputs,
        issues,
        resolution,
        selected_root_id,
    }
}

fn resolve_output_roots(
    candidates: &[OutputCandidate],
    explicit_roots: &[OutputRootSelection],
) -> OutputRootResolution {
    if !explicit_roots.is_empty() {
        let by_node = candidates
            .iter()
            .map(|candidate| (candidate.node_id.as_str(), candidate))
            .collect::<BTreeMap<_, _>>();
        let mut selections = explicit_roots.to_vec();
        selections.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        selections.dedup_by(|left, right| left.node_id == right.node_id);
        let mut selected_candidates = BTreeMap::<String, OutputCandidate>::new();
        for selection in selections {
            let Some(candidate) = by_node.get(selection.node_id.as_str()) else {
                return OutputRootResolution::Unknown {
                    reason: OutputRootResolutionReason::InvalidExplicitSelection,
                };
            };
            if !candidate.explicit_eligible || candidate.output_type != selection.output_type {
                return OutputRootResolution::Unknown {
                    reason: OutputRootResolutionReason::InvalidExplicitSelection,
                };
            }
            let mut selected = (*candidate).clone();
            push_unique_evidence(
                &mut selected.evidence,
                evidence(
                    EvidenceKind::EXPLICIT_OUTPUT_MAPPING,
                    "user-selected output mapping identifies this root",
                ),
            );
            selected.score += evidence_weight(EvidenceKind::EXPLICIT_OUTPUT_MAPPING);
            selected.evidence_tier = 4;
            selected_candidates.insert(selected.node_id.clone(), selected);
        }
        let mut root_candidates = candidates
            .iter()
            .filter(|candidate| candidate.evidence_tier >= 2)
            .cloned()
            .collect::<Vec<_>>();
        for candidate in selected_candidates.into_values() {
            if let Some(existing) = root_candidates
                .iter_mut()
                .find(|existing| existing.node_id == candidate.node_id)
            {
                *existing = candidate;
            } else {
                root_candidates.push(candidate);
            }
        }
        root_candidates.sort_by(|left, right| {
            right
                .evidence_tier
                .cmp(&left.evidence_tier)
                .then(right.score.cmp(&left.score))
                .then(left.node_id.cmp(&right.node_id))
        });
        return OutputRootResolution::Resolved {
            roots: root_candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| output_root_from_candidate(candidate, index, false))
                .collect(),
        };
    }

    let Some(max_tier) = candidates
        .iter()
        .filter(|candidate| candidate.evidence_tier >= 2)
        .map(|candidate| candidate.evidence_tier)
        .max()
    else {
        return OutputRootResolution::Unknown {
            reason: OutputRootResolutionReason::NoReliableMediaRoot,
        };
    };
    let strong = candidates
        .iter()
        .filter(|candidate| candidate.evidence_tier == max_tier)
        .collect::<Vec<_>>();
    let max_score = strong
        .iter()
        .map(|candidate| candidate.score)
        .max()
        .unwrap_or(0);
    let contenders = strong
        .into_iter()
        .filter(|candidate| max_score - candidate.score < AMBIGUITY_MARGIN)
        .cloned()
        .collect::<Vec<_>>();
    let output_types = contenders
        .iter()
        .map(|candidate| candidate.output_type.as_str())
        .collect::<BTreeSet<_>>();
    if output_types.len() > 1 {
        return OutputRootResolution::Ambiguous {
            candidates: contenders
                .iter()
                .enumerate()
                .map(|(index, candidate)| output_root_from_candidate(candidate, index, true))
                .collect(),
            reason: OutputRootResolutionReason::ConflictingCandidates,
        };
    }
    OutputRootResolution::Resolved {
        roots: contenders
            .iter()
            .enumerate()
            .map(|(index, candidate)| output_root_from_candidate(candidate, index, false))
            .collect(),
    }
}

fn output_root_evidence_tier(
    explicit: bool,
    schema_typed: bool,
    media_output_role: bool,
    terminal: bool,
    input_only: bool,
    preview: bool,
    auxiliary: bool,
    utility: bool,
) -> u8 {
    if input_only && !explicit {
        return 0;
    }
    if explicit && media_output_role {
        return 3;
    }
    if terminal && media_output_role && !(preview || auxiliary || utility) {
        return 2;
    }
    if schema_typed || preview || auxiliary || utility {
        return 1;
    }
    1
}

fn output_root_from_candidate(
    candidate: &OutputCandidate,
    index: usize,
    ambiguous: bool,
) -> OutputRoot {
    OutputRoot {
        output_id: format!("output_{}", index + 1),
        output_type: candidate.output_type.clone(),
        node_id: candidate.node_id.clone(),
        label: candidate.label.clone(),
        score: candidate.score,
        confidence: if ambiguous {
            RecognitionConfidence::Low
        } else {
            confidence_from_score(candidate.score)
        },
        evidence_tier: candidate.evidence_tier,
        evidence: candidate.evidence.clone(),
    }
}

fn output_from_root(root: &OutputRoot, index: usize, ambiguous: bool) -> WorkflowAnalysisOutput {
    WorkflowAnalysisOutput {
        output_id: format!("output_{}", index + 1),
        output_type: root.output_type.clone(),
        node_id: root.node_id.clone(),
        label: root.label.clone(),
        required: true,
        confidence: if ambiguous {
            RecognitionConfidence::Low
        } else {
            root.confidence
        },
        score: root.score,
        evidence: root.evidence.clone(),
    }
}

fn output_issue_candidate(output: &WorkflowAnalysisOutput) -> WorkflowAnalysisIssueCandidate {
    WorkflowAnalysisIssueCandidate {
        label: output.label.clone(),
        node_id: Some(output.node_id.clone()),
        input_name: None,
        output_id: Some(output.output_id.clone()),
        output_type: Some(output.output_type.clone()),
        field_type: None,
        reason: evidence_reason(&output.evidence),
        score: output.score,
        evidence: output.evidence.clone(),
    }
}

fn confidence_from_score(score: i32) -> RecognitionConfidence {
    if score >= HIGH_CONFIDENCE_MIN_SCORE {
        RecognitionConfidence::High
    } else if score >= MEDIUM_CONFIDENCE_MIN_SCORE {
        RecognitionConfidence::Medium
    } else {
        RecognitionConfidence::Low
    }
}

#[derive(Clone)]
struct OutputCandidate {
    node_id: String,
    label: String,
    output_type: String,
    score: i32,
    evidence: Vec<RecognitionEvidence>,
    evidence_tier: u8,
    explicit_eligible: bool,
}

fn declared_output_media_type(
    node: &crate::application::workflow_recognition_schema::RecognitionNodeSchema,
) -> Option<String> {
    if node
        .declared_output_types
        .iter()
        .any(|kind| *kind == RecognitionDeclaredType::Video)
    {
        Some("video".to_owned())
    } else if node.declared_output_types.iter().any(|kind| {
        matches!(
            kind,
            RecognitionDeclaredType::Image
                | RecognitionDeclaredType::Mask
                | RecognitionDeclaredType::Latent
        )
    }) {
        Some("image".to_owned())
    } else {
        None
    }
}

struct OutputAnalysis {
    roots: Vec<String>,
    outputs: Vec<WorkflowAnalysisOutput>,
    issues: Vec<WorkflowAnalysisIssue>,
    resolution: OutputRootResolution,
    selected_root_id: Option<String>,
}

impl OutputAnalysis {
    fn unknown() -> Self {
        Self::unknown_with_reason(OutputRootResolutionReason::NoReliableMediaRoot)
    }

    fn unknown_with_reason(reason: OutputRootResolutionReason) -> Self {
        Self {
            roots: Vec::new(),
            outputs: Vec::new(),
            issues: vec![WorkflowAnalysisIssue {
                code: "UNKNOWN_OUTPUT".to_owned(),
                message: "未能识别唯一的最终输出节点。".to_owned(),
                field: Some("output_1".to_owned()),
                candidates: Vec::new(),
            }],
            resolution: OutputRootResolution::Unknown { reason },
            selected_root_id: None,
        }
    }
}

fn report(
    raw_sha256: String,
    semantic_sha256: String,
    structural_sha256: String,
    node_count: usize,
    unique_class_count: usize,
    inputs: Vec<WorkflowAnalysisInput>,
    outputs: Vec<WorkflowAnalysisOutput>,
    output_root_resolution: OutputRootResolution,
    selected_root_id: Option<String>,
    category: String,
    mode: String,
    issues: Vec<WorkflowAnalysisIssue>,
) -> WorkflowAnalysisReport {
    let confidence = if issues.is_empty() {
        inputs
            .iter()
            .map(|input| confidence_rank(input.confidence))
            .chain(
                outputs
                    .iter()
                    .map(|output| confidence_rank(output.confidence)),
            )
            .min()
            .map(confidence_from_rank)
            .unwrap_or(RecognitionConfidence::Low)
    } else {
        RecognitionConfidence::Low
    };
    WorkflowAnalysisReport {
        format: WorkflowRecognitionFormat::Api,
        recognized: true,
        importable: issues.iter().all(|issue| issue.code != "GRAPH_INVALID"),
        identity: WorkflowIdentity::New,
        raw_sha256,
        semantic_sha256,
        structural_sha256,
        existing_workflow_id: None,
        existing_workflow_version_id: None,
        category,
        mode,
        bindings: inputs
            .iter()
            .map(|input| WorkflowAnalysisBinding {
                semantic_key: input.semantic_key.clone(),
                target_node: input.node_id.clone(),
                target_input: input.input_name.clone(),
                item_index: input.item_index,
            })
            .collect(),
        inputs,
        outputs,
        output_root_resolution,
        selected_root_id,
        confidence,
        suggested_actions: if issues.is_empty() {
            vec!["ADD_TO_LIBRARY".to_owned()]
        } else {
            vec!["REVIEW_RECOGNITION".to_owned()]
        },
        issues,
        node_count,
        unique_class_count,
    }
}

fn category_for_outputs(outputs: &[WorkflowAnalysisOutput]) -> String {
    if outputs.iter().any(|output| output.output_type == "video") {
        "video".to_owned()
    } else if outputs.iter().any(|output| output.output_type == "image") {
        "image".to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn infer_mode(inputs: &[WorkflowAnalysisInput], category: &str) -> String {
    let keys = inputs
        .iter()
        .map(|input| input.semantic_key.as_str())
        .collect::<BTreeSet<_>>();
    if category == "video" {
        if keys.contains("first_frame") || keys.contains("last_frame") {
            "image_to_video".to_owned()
        } else if inputs.iter().any(|input| {
            linked_target_semantic(&input.input_name).is_some_and(|hint| hint.explicit_reference)
                || matches!(
                    input.semantic_key.as_str(),
                    "reference_video" | "reference_videos" | "reference_audio" | "reference_audios"
                )
        }) {
            "reference_to_video".to_owned()
        } else if keys.contains("image") {
            "image_to_video".to_owned()
        } else if keys.contains("reference_image") {
            "image_to_video".to_owned()
        } else {
            "text_to_video".to_owned()
        }
    } else if category == "image" {
        if keys
            .iter()
            .any(|key| key.starts_with("reference_") || *key == "image")
        {
            "image_to_image".to_owned()
        } else {
            "text_to_image".to_owned()
        }
    } else {
        "unknown".to_owned()
    }
}

fn media_output_type_from_node(node: &serde_json::Map<String, Value>) -> Option<String> {
    let inputs = node.get("inputs").and_then(Value::as_object)?;
    let mut serialized_media_types = BTreeSet::new();

    for value in inputs.values() {
        if let Some(serialized) = value.as_str().map(str::trim).map(str::to_ascii_lowercase) {
            if serialized.starts_with("video/") {
                serialized_media_types.insert("video");
            } else if serialized.starts_with("image/") {
                serialized_media_types.insert("image");
            }
        }
    }

    if serialized_media_types.len() == 1 {
        return serialized_media_types.into_iter().next().map(str::to_owned);
    }

    let mut socket_media_types = BTreeSet::new();
    for (name, value) in inputs {
        let linked_socket = value
            .as_array()
            .is_some_and(|link| link.len() >= 2 && link.first().is_some_and(Value::is_string));
        if !linked_socket {
            continue;
        }
        let normalized_name = normalize(name);
        if normalized_name.contains("video") || normalized_name.contains("frames") {
            socket_media_types.insert("video");
        } else if normalized_name.contains("image")
            || normalized_name.contains("mask")
            || normalized_name.contains("latent")
        {
            socket_media_types.insert("image");
        }
    }

    (socket_media_types.len() == 1)
        .then(|| socket_media_types.into_iter().next().map(str::to_owned))
        .flatten()
}

fn node_title(node: &serde_json::Map<String, Value>) -> String {
    node.get("_meta")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("title"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn is_utility_class(text: &str) -> bool {
    text.contains("clearcache")
        || text.contains("clear cache")
        || text.contains("debug")
        || text.contains("log")
        || text.contains("utility")
}

fn is_video_output_class(text: &str) -> bool {
    ["savevideo", "createvideo", "videooutput"]
        .iter()
        .any(|marker| text.contains(marker))
}

fn is_image_output_class(text: &str) -> bool {
    text.contains("saveimage") || text.contains("imageoutput")
}

fn is_input_only_output_class(text: &str) -> bool {
    [
        "loadimage",
        "loadvideo",
        "loadaudio",
        "inputimage",
        "inputvideo",
        "inputaudio",
        "uploadimage",
        "uploadvideo",
        "uploadaudio",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn is_ignored_input(name: &str) -> bool {
    matches!(
        name,
        "filename"
            | "filename_prefix"
            | "format"
            | "codec"
            | "aspect_ratio"
            | "multiple"
            | "megapixels"
            | "pix_fmt"
            | "crf"
    ) || name.contains("model")
        || name.contains("vae")
        || name.contains("clip")
        || name.contains("lora")
        || name.contains("scheduler")
}

fn graph_numeric_leaves(graph: &WorkflowGraph, node_id: &str) -> Vec<WorkflowSource> {
    let mut unique = BTreeMap::new();
    for link in graph.upstream_of(node_id) {
        for trace in graph.trace_scalar_sources(node_id, &link.target_input) {
            unique
                .entry((trace.source.node_id.clone(), trace.source.input.clone()))
                .or_insert(trace.source);
        }
    }
    unique.into_values().collect()
}

fn arithmetic_chain_is_safe(
    workflow: &WorkflowDocument,
    graph: &WorkflowGraph,
    node_id: &str,
    visited: &mut BTreeSet<String>,
) -> bool {
    if !visited.insert(node_id.to_owned()) {
        return true;
    }
    if workflow.inputs(node_id).is_none() {
        return false;
    }
    for link in graph.upstream_of(node_id) {
        let source_node = &link.source_node_id;
        if workflow.inputs(source_node).is_none() {
            return false;
        }
        if !graph.upstream_of(source_node).is_empty()
            && (!is_arithmetic_node(workflow.class_type(source_node).unwrap_or_default())
                || !arithmetic_chain_is_safe(workflow, graph, source_node, visited))
        {
            return false;
        }
    }
    true
}

fn is_arithmetic_node(class_type: &str) -> bool {
    let lower = class_type.to_ascii_lowercase();
    ["math", "expression", "arithmetic", "convert", "conversion"]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn expression_proves_duration(expression: &str) -> bool {
    let compact = expression
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if (!compact.contains('*') && !compact.contains('×'))
        || !compact
            .chars()
            .any(|character| character.is_ascii_alphabetic())
    {
        return false;
    }
    let fps = duration_fps_literals(expression);
    fps.len() == 1 && fps[0].is_finite() && (1.0..=240.0).contains(&fps[0])
}

fn duration_fps_literals(expression: &str) -> Vec<f64> {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut literals = Vec::new();
    for (index, character) in chars.iter().enumerate() {
        if !matches!(character, '*' | '×') {
            continue;
        }
        let mut left = index;
        while left > 0 && chars[left - 1].is_whitespace() {
            left -= 1;
        }
        let mut right = index + 1;
        while right < chars.len() && chars[right].is_whitespace() {
            right += 1;
        }
        let left_number = number_ending_at(&chars, left);
        let right_number = number_starting_at(&chars, right);
        let left_identifier = left > 0 && chars[left - 1].is_ascii_alphabetic();
        let right_identifier = right < chars.len() && chars[right].is_ascii_alphabetic();
        let number = if left_number.is_some() && right_identifier {
            left_number
        } else if right_number.is_some() && left_identifier {
            right_number
        } else {
            None
        };
        if let Some(number) = number.filter(|number| !literals.contains(number)) {
            literals.push(number);
        }
    }
    literals
}

fn number_ending_at(chars: &[char], end: usize) -> Option<f64> {
    if end == 0 || (!chars[end - 1].is_ascii_digit() && chars[end - 1] != '.') {
        return None;
    }
    let mut start = end - 1;
    while start > 0 && (chars[start - 1].is_ascii_digit() || chars[start - 1] == '.') {
        start -= 1;
    }
    chars[start..end].iter().collect::<String>().parse().ok()
}

fn number_starting_at(chars: &[char], start: usize) -> Option<f64> {
    if start >= chars.len() || (!chars[start].is_ascii_digit() && chars[start] != '.') {
        return None;
    }
    let mut end = start + 1;
    while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
        end += 1;
    }
    chars[start..end].iter().collect::<String>().parse().ok()
}

fn is_link(value: &Value) -> bool {
    value.as_array().is_some_and(|link| {
        link.len() == 2 && link[0].as_str().is_some() && link[1].as_u64().is_some()
    })
}

fn normalize(value: &str) -> String {
    value.to_ascii_lowercase().replace(['-', ' ', '.'], "_")
}

fn humanize(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_class_count(workflow: &WorkflowDocument) -> usize {
    workflow
        .value()
        .as_object()
        .into_iter()
        .flat_map(|nodes| nodes.values())
        .filter_map(|node| node.get("class_type").and_then(Value::as_str))
        .collect::<BTreeSet<_>>()
        .len()
}

fn is_integer_number(value: &Value) -> bool {
    value.as_i64().is_some() || value.as_u64().is_some()
}

fn confidence_rank(confidence: RecognitionConfidence) -> u8 {
    match confidence {
        RecognitionConfidence::High => 3,
        RecognitionConfidence::Medium => 2,
        RecognitionConfidence::Low => 1,
    }
}

fn confidence_from_rank(rank: u8) -> RecognitionConfidence {
    match rank {
        3 => RecognitionConfidence::High,
        2 => RecognitionConfidence::Medium,
        _ => RecognitionConfidence::Low,
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::workflow_recognition_schema::RecognitionSchemaContext;
    use serde_json::json;

    const AITUDOU_8STEP: &str = include_str!(
        "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/workflow_api.json"
    );
    const FL2VA_FIRST_LAST: &str = include_str!(
        "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_0_0/workflow_api.json"
    );
    const REFERENCE_VIDEO: &str = include_str!(
        "../../runtime_packages/minimax_h3_reference_video_quality_2_0_0/workflow_api.json"
    );

    fn fixture_report() -> WorkflowAnalysisReport {
        let value: Value = serde_json::from_str(AITUDOU_8STEP).expect("fixture should parse");
        let workflow = WorkflowDocument::parse(value).expect("fixture should be an API workflow");
        WorkflowAnalysisService::analyze_workflow(&workflow, AITUDOU_8STEP.as_bytes())
    }

    fn phase2a_schema() -> RecognitionSchemaContext {
        RecognitionSchemaContext::parse(&json!({
            "ImageOutput": {
                "output": ["IMAGE"],
                "output_node": true,
                "input": {"required": {"image": ["IMAGE", {}]}}
            },
            "VideoOutput": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"video": ["VIDEO", {}]}}
            },
            "NodeAlpha": {
                "output": ["VIDEO"],
                "output_node": true,
                "input": {"required": {"frames": ["IMAGE", {}]}}
            },
            "PreviewImage": {
                "output": ["IMAGE"],
                "input": {"required": {"image": ["IMAGE", {}]}}
            },
            "SaveImage": {
                "output": ["IMAGE"],
                "output_node": true,
                "input": {"required": {"image": ["IMAGE", {}]}}
            },
            "CustomCacheOutput": {
                "output": ["IMAGE"],
                "input": {"required": {"image": ["IMAGE", {}]}}
            },
            "LoadImage": {
                "output": ["IMAGE"],
                "input": {"required": {"image": [["input.png"], {}]}}
            },
            "Transform": {
                "output": ["FLOAT"],
                "input": {"required": {"value": ["FLOAT", {}]}}
            },
            "PromptSource": {
                "input": {"required": {"text": ["STRING", {}]}}
            }
        }))
    }

    fn phase2a_report(
        value: Value,
        explicit_roots: &[OutputRootSelection],
    ) -> WorkflowAnalysisReport {
        let workflow = WorkflowDocument::parse(value).expect("synthetic workflow should parse");
        let schema = phase2a_schema();
        let bytes = serde_json::to_vec(workflow.value()).expect("workflow should serialize");
        WorkflowAnalysisService::analyze_workflow_with_schema_and_output_roots(
            &workflow,
            &bytes,
            Some(&schema),
            explicit_roots,
        )
    }

    #[test]
    fn no_root_returns_unknown_not_full_graph() {
        let report = phase2a_report(
            json!({
                "1": {"class_type": "PromptSource", "inputs": {"text": "hello"}},
                "2": {"class_type": "Transform", "inputs": {"value": ["1", 0]}},
                "99": {"class_type": "UnknownDisconnected", "inputs": {"prompt": "unused"}}
            }),
            &[],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Unknown { .. }
        ));
        assert!(
            report.inputs.is_empty(),
            "unknown roots must not analyze the full graph"
        );
        assert_eq!(report.category, "unknown");
        assert_eq!(report.mode, "unknown");
    }

    #[test]
    fn ambiguous_root_returns_ambiguous() {
        let report = phase2a_report(
            json!({
                "1": {"class_type": "ImageOutput", "inputs": {}},
                "2": {"class_type": "VideoOutput", "inputs": {}}
            }),
            &[],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Ambiguous { .. }
        ));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "AMBIGUOUS_OUTPUT"));
    }

    #[test]
    fn strong_root_beats_weak_preview() {
        let report = phase2a_report(
            json!({
                "1": {"class_type": "Sampler", "inputs": {}},
                "2": {"class_type": "PreviewImage", "inputs": {"image": ["1", 0]}},
                "3": {"class_type": "SaveImage", "inputs": {"image": ["1", 0]}}
            }),
            &[],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Resolved { ref roots }
                if roots.len() == 1 && roots[0].node_id == "3"
        ));
    }

    #[test]
    fn explicit_output_mapping_resolves_unknown() {
        let report = phase2a_report(
            json!({"1": {"class_type": "CustomCacheOutput", "inputs": {"image": "cached.png"}}}),
            &[OutputRootSelection {
                node_id: "1".to_owned(),
                output_type: "image".to_owned(),
            }],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Resolved { ref roots }
                if roots.len() == 1 && roots[0].node_id == "1"
        ));
        assert!(report.outputs[0]
            .evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::EXPLICIT_OUTPUT_MAPPING));
    }

    #[test]
    fn invalid_explicit_output_mapping_fails_safe() {
        let report = phase2a_report(
            json!({"1": {"class_type": "CustomCacheOutput", "inputs": {"image": "cached.png"}}}),
            &[OutputRootSelection {
                node_id: "missing".to_owned(),
                output_type: "image".to_owned(),
            }],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Unknown {
                reason: OutputRootResolutionReason::InvalidExplicitSelection
            }
        ));
        assert_eq!(report.category, "unknown");
    }

    #[test]
    fn terminal_input_node_is_not_output_root() {
        let report = phase2a_report(
            json!({"1": {"class_type": "LoadImage", "inputs": {"image": "input.png"}}}),
            &[],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Unknown { .. }
        ));
        assert!(report.outputs.is_empty());
    }

    #[test]
    fn weak_class_title_hint_alone_does_not_resolve_root() {
        let report = phase2a_report(
            json!({
                "1": {
                    "class_type": "FinalPass",
                    "_meta": {"title": "Final"},
                    "inputs": {"value": "not-a-media-output"}
                }
            }),
            &[],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Unknown { .. }
        ));
    }

    #[test]
    fn generic_typed_terminal_media_sink_is_output_root() {
        let report = phase2a_report(
            json!({
                "1": {"class_type": "Generator", "inputs": {}},
                "2": {
                    "class_type": "NodeAlpha",
                    "inputs": {"frames": ["1", 0]}
                }
            }),
            &[],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Resolved { ref roots }
                if roots.len() == 1
                    && roots[0].node_id == "2"
                    && roots[0].output_type == "video"
        ));
    }

    #[test]
    fn terminal_unknown_node_without_media_evidence_is_not_root() {
        let report = phase2a_report(
            json!({
                "1": {
                    "class_type": "OpaqueSinkA",
                    "inputs": {"value": "artifact.bin"}
                }
            }),
            &[],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Unknown { .. }
        ));
        assert!(report.outputs.is_empty());
    }

    #[test]
    fn generic_serialized_terminal_media_sink_is_output_root() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "Generator", "inputs": {}},
            "2": {
                "class_type": "GenericMediaSink",
                "inputs": {
                    "images": ["1", 0],
                    "format": "video/h264-mp4"
                }
            }
        }))
        .expect("synthetic workflow should parse");
        let bytes = serde_json::to_vec(workflow.value()).expect("workflow should serialize");
        let report = WorkflowAnalysisService::analyze_workflow(&workflow, &bytes);

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Resolved { ref roots }
                if roots.len() == 1
                    && roots[0].node_id == "2"
                    && roots[0].output_type == "video"
        ));
    }

    #[test]
    fn two_compatible_strong_roots_remain_resolved() {
        let report = phase2a_report(
            json!({
                "1": {"class_type": "ImageOutput", "inputs": {}},
                "2": {"class_type": "ImageOutput", "inputs": {}}
            }),
            &[],
        );

        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Resolved { ref roots }
                if roots.iter().map(|root| root.node_id.as_str()).collect::<Vec<_>>()
                    == vec!["1", "2"]
        ));
    }

    #[test]
    fn root_resolution_is_order_independent() {
        let first = phase2a_report(
            json!({
                "2": {"class_type": "ImageOutput", "inputs": {}},
                "1": {"class_type": "ImageOutput", "inputs": {}}
            }),
            &[],
        );
        let second = phase2a_report(
            json!({
                "1": {"class_type": "ImageOutput", "inputs": {}},
                "2": {"class_type": "ImageOutput", "inputs": {}}
            }),
            &[],
        );

        assert_eq!(first.output_root_resolution, second.output_root_resolution);
        assert_eq!(first.outputs, second.outputs);
    }

    #[test]
    fn aitudou_8step_exposes_the_shared_production_fields() {
        let report = fixture_report();

        assert_eq!(report.identity, WorkflowIdentity::New);
        assert_eq!(report.category, "video");
        assert_eq!(report.mode, "text_to_video");
        assert_eq!(report.confidence, RecognitionConfidence::High);
        assert!(
            report.issues.is_empty(),
            "unexpected issues: {:?}",
            report.issues
        );

        for (key, node, input) in [
            ("prompt", "59", "text"),
            ("width", "63", "width"),
            ("height", "63", "height"),
            ("duration_seconds", "49", "value"),
            ("seed", "2", "noise_seed"),
            ("steps", "50", "steps"),
            ("denoise", "50", "denoise"),
            ("fps", "62", "frame_rate"),
        ] {
            let analyzed = report
                .inputs
                .iter()
                .find(|candidate| candidate.semantic_key == key)
                .unwrap_or_else(|| panic!("missing input {key}"));
            assert_eq!(analyzed.node_id, node, "wrong node for {key}");
            assert_eq!(analyzed.input_name, input, "wrong input for {key}");
            assert!(report.bindings.iter().any(|binding| {
                binding.semantic_key == key
                    && binding.target_node == node
                    && binding.target_input == input
            }));
        }

        let duration = report
            .inputs
            .iter()
            .find(|candidate| candidate.semantic_key == "duration_seconds")
            .and_then(|candidate| candidate.value.as_ref())
            .and_then(Value::as_i64);
        assert_eq!(duration, Some(5));
        let video_output = report
            .outputs
            .iter()
            .find(|output| output.output_type == "video" && output.node_id == "62")
            .expect("VHS output should be recognized by generic media evidence");
        assert!(!video_output
            .evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::CLASS_TYPE_HINT));
    }

    #[test]
    fn raw_hash_is_the_only_identity_part_changed_by_bytes_formatting() {
        let value: Value = serde_json::from_str(AITUDOU_8STEP).expect("fixture should parse");
        let workflow = WorkflowDocument::parse(value.clone()).expect("fixture should parse");
        let compact = serde_json::to_vec(&value).expect("compact fixture should serialize");
        let pretty = serde_json::to_vec_pretty(&value).expect("pretty fixture should serialize");
        let first = analyze_workflow(&workflow, &compact);
        let second = analyze_workflow(&workflow, &pretty);

        assert_ne!(first.raw_sha256, second.raw_sha256);
        assert_eq!(first.semantic_sha256, second.semantic_sha256);
        assert_eq!(first.structural_sha256, second.structural_sha256);
    }

    #[test]
    fn first_last_frame_links_are_inferred_from_their_loader_nodes() {
        let value: Value = serde_json::from_str(FL2VA_FIRST_LAST).expect("fixture should parse");
        let workflow = WorkflowDocument::parse(value).expect("fixture should be an API workflow");
        let report = analyze_workflow(&workflow, FL2VA_FIRST_LAST.as_bytes());

        assert!(
            report
                .issues
                .iter()
                .all(|issue| issue.code != "AMBIGUOUS_INPUT"),
            "unexpected issues: {:?}",
            report.issues
        );
        assert_eq!(report.mode, "image_to_video");
        for (key, node) in [("first_frame", "24"), ("last_frame", "28")] {
            let input = report
                .inputs
                .iter()
                .find(|input| input.semantic_key == key)
                .unwrap_or_else(|| panic!("missing input {key}"));
            assert_eq!(input.node_id, node);
            assert_eq!(input.input_name, "image");
            assert_eq!(input.item_index, None);
            assert!(report.bindings.iter().any(|binding| {
                binding.semantic_key == key
                    && binding.target_node == node
                    && binding.target_input == "image"
                    && binding.item_index.is_none()
            }));
        }
    }

    #[test]
    fn live_schema_keeps_first_last_and_float_controls_recognized() {
        let value: Value = serde_json::from_str(FL2VA_FIRST_LAST).expect("fixture should parse");
        let workflow = WorkflowDocument::parse(value).expect("fixture should be an API workflow");
        let schema = RecognitionSchemaContext::parse(&json!({
            "LoadImage": {
                "input": {"required": {
                    "image": [["frame.png"], {}]
                }},
                "output": ["IMAGE"]
            },
            "PrimitiveFloat": {
                "input": {"required": {
                    "value": ["FLOAT", {}]
                }},
                "output": ["FLOAT"]
            },
            "MiniMaxH3ImageToVideo": {
                "input": {
                    "required": {
                        "prompt": ["STRING", {}],
                        "width": ["INT", {}],
                        "height": ["INT", {}],
                        "length": ["INT", {}]
                    },
                    "optional": {
                        "first_frame": ["IMAGE", {}],
                        "last_frame": ["IMAGE", {}]
                    }
                },
                "output": ["VIDEO"]
            },
            "BasicScheduler": {
                "input": {"required": {
                    "steps": ["INT", {}],
                    "denoise": ["FLOAT", {}]
                }},
                "output": ["SIGMAS"]
            },
            "CreateVideo": {
                "input": {"required": {
                    "images": ["IMAGE", {}],
                    "fps": ["FLOAT", {}]
                }},
                "output": ["VIDEO"]
            },
            "SaveVideo": {
                "input": {"required": {
                    "video": ["VIDEO", {}]
                }},
                "output": ["VIDEO"],
                "output_node": true
            }
        }));

        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            FL2VA_FIRST_LAST.as_bytes(),
            Some(&schema),
        );

        assert_eq!(report.mode, "image_to_video");
        assert!(report.issues.iter().all(|issue| {
            issue.code != "AMBIGUOUS_INPUT"
                || !matches!(
                    issue.field.as_deref(),
                    Some("first_frame")
                        | Some("last_frame")
                        | Some("duration_seconds")
                        | Some("fps")
                        | Some("denoise")
                )
        }));
        for key in [
            "first_frame",
            "last_frame",
            "duration_seconds",
            "fps",
            "denoise",
        ] {
            assert!(
                report.inputs.iter().any(|input| input.semantic_key == key),
                "missing schema-aware input {key}: {:?}",
                report.issues
            );
        }
        assert!(report
            .inputs
            .iter()
            .filter(|input| matches!(input.semantic_key.as_str(), "first_frame" | "last_frame"))
            .all(|input| !input.required));
        assert!(report
            .outputs
            .iter()
            .any(|output| output.output_type == "video" && output.node_id == "21"));
    }

    #[test]
    fn indexed_reference_image_links_are_inferred_as_distinct_plural_slots() {
        let audio_target = linked_target_semantic("ref_audios.ref_audio_0")
            .expect("indexed audio target should be recognized");
        assert_eq!(audio_target.semantic_key, "reference_audios");
        assert_eq!(audio_target.item_index, Some(0));

        let value: Value = serde_json::from_str(REFERENCE_VIDEO).expect("fixture should parse");
        let workflow = WorkflowDocument::parse(value).expect("fixture should be an API workflow");
        let graph = WorkflowGraph::from_document(&workflow).expect("fixture graph should build");
        let audio_guess = literal_guess("LoadAudio", "audio", &serde_json::json!("audio.wav"))
            .expect("audio literal should be recognized");
        assert!(is_contextualized_media_literal(&graph, "50", &audio_guess));
        let report = analyze_workflow(&workflow, REFERENCE_VIDEO.as_bytes());

        assert!(
            report
                .issues
                .iter()
                .all(|issue| issue.code != "AMBIGUOUS_INPUT"),
            "unexpected issues: {:?}",
            report.issues
        );
        let references = report
            .inputs
            .iter()
            .filter(|input| input.semantic_key == "reference_images")
            .collect::<Vec<_>>();
        assert_eq!(references.len(), 9);
        assert_eq!(
            references
                .iter()
                .map(|input| input.item_index)
                .collect::<Vec<_>>(),
            (0..9).map(Some).collect::<Vec<_>>()
        );
        assert!(references.iter().all(|input| input.input_name == "image"));
    }

    #[test]
    fn schema_backed_reference_video_slots_are_inferred_and_optional() {
        let value: Value = serde_json::from_str(REFERENCE_VIDEO).expect("fixture should parse");
        let workflow = WorkflowDocument::parse(value).expect("workflow should be an API workflow");
        let schema = RecognitionSchemaContext::parse(&json!({
            "LoadVideo": {
                "input": {"required": {
                    "file": ["COMBO", {"video_upload": true}]
                }},
                "output": ["VIDEO"]
            }
        }));

        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            REFERENCE_VIDEO.as_bytes(),
            Some(&schema),
        );
        let references = report
            .inputs
            .iter()
            .filter(|input| input.semantic_key == "reference_videos")
            .collect::<Vec<_>>();

        assert_eq!(references.len(), 3, "issues: {:?}", report.issues);
        assert_eq!(
            references
                .iter()
                .map(|input| (input.node_id.as_str(), input.item_index, input.required))
                .collect::<Vec<_>>(),
            vec![
                ("40", Some(0), false),
                ("42", Some(1), false),
                ("44", Some(2), false)
            ]
        );
        assert!(references.iter().all(|input| {
            input
                .evidence
                .iter()
                .any(|evidence| evidence.kind == EvidenceKind::SCHEMA_MEDIA_UPLOAD)
                && input
                    .evidence
                    .iter()
                    .any(|evidence| evidence.kind == EvidenceKind::SCHEMA_MEDIA_OUTPUT)
                && input
                    .evidence
                    .iter()
                    .any(|evidence| evidence.kind == EvidenceKind::GRAPH_OUTPUT_PATH)
                && input.confidence == RecognitionConfidence::High
        }));
    }

    #[test]
    fn static_reference_video_slots_remain_unbound_without_schema() {
        let value: Value = serde_json::from_str(REFERENCE_VIDEO).expect("fixture should parse");
        let workflow = WorkflowDocument::parse(value).expect("workflow should be an API workflow");
        let report =
            WorkflowAnalysisService::analyze_workflow(&workflow, REFERENCE_VIDEO.as_bytes());

        assert!(!report
            .inputs
            .iter()
            .any(|input| input.semantic_key == "reference_videos"));
    }

    #[test]
    fn custom_schema_video_uploader_is_recognized_without_class_allowlist() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {
                "class_type": "UnknownVideoUploaderXYZ",
                "inputs": {"file": "uploaded.mp4"}
            },
            "2": {
                "class_type": "ReferenceConsumer",
                "inputs": {"reference_video": ["1", 0]}
            },
            "3": {
                "class_type": "SaveVideo",
                "output_node": true,
                "inputs": {"video": ["2", 0]}
            }
        }))
        .expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "UnknownVideoUploaderXYZ": {
                "input": {"required": {
                    "file": ["COMBO", {"video_upload": true}]
                }},
                "output": ["VIDEO"]
            },
            "SaveVideo": {
                "input": {"required": {"video": ["VIDEO", {}]}},
                "output": ["VIDEO"],
                "output_node": true
            }
        }));

        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"custom":"video"}"#,
            Some(&schema),
        );
        let input = report
            .inputs
            .iter()
            .find(|input| input.semantic_key == "reference_video")
            .expect("schema-backed custom uploader should be recognized");
        assert_eq!(input.node_id, "1");
        assert_eq!(input.input_name, "file");
        assert_eq!(input.confidence, RecognitionConfidence::High);
        assert!(input
            .evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::SCHEMA_MEDIA_UPLOAD));
    }

    #[test]
    fn video_upload_metadata_without_video_output_is_not_a_video_binding() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {
                "class_type": "UnknownVideoUploaderXYZ",
                "inputs": {"file": "not-a-video-selector"}
            },
            "2": {
                "class_type": "ReferenceConsumer",
                "inputs": {"reference_video": ["1", 0]}
            },
            "3": {
                "class_type": "SaveVideo",
                "output_node": true,
                "inputs": {"video": ["2", 0]}
            }
        }))
        .expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "UnknownVideoUploaderXYZ": {
                "input": {"required": {
                    "file": ["COMBO", {"video_upload": true}]
                }},
                "output": ["IMAGE"]
            },
            "SaveVideo": {
                "input": {"required": {"video": ["VIDEO", {}]}},
                "output": ["VIDEO"],
                "output_node": true
            }
        }));

        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"custom":"mismatch"}"#,
            Some(&schema),
        );
        assert!(!report
            .inputs
            .iter()
            .any(|input| input.semantic_key == "reference_video"));
    }

    #[test]
    fn off_path_video_upload_does_not_beat_on_path_reference_candidate() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {
                "class_type": "UnknownVideoUploaderXYZ",
                "inputs": {"file": "on-path.mp4"}
            },
            "2": {
                "class_type": "ReferenceConsumer",
                "inputs": {"reference_video": ["1", 0]}
            },
            "3": {
                "class_type": "UnknownVideoUploaderXYZ",
                "inputs": {"file": "off-path.mp4"}
            },
            "4": {
                "class_type": "SaveVideo",
                "output_node": true,
                "inputs": {"video": ["2", 0]}
            }
        }))
        .expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "UnknownVideoUploaderXYZ": {
                "input": {"required": {
                    "file": ["COMBO", {"video_upload": true}]
                }},
                "output": ["VIDEO"]
            },
            "SaveVideo": {
                "input": {"required": {"video": ["VIDEO", {}]}},
                "output": ["VIDEO"],
                "output_node": true
            }
        }));

        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"custom":"off-path"}"#,
            Some(&schema),
        );
        let input = report
            .inputs
            .iter()
            .find(|input| input.semantic_key == "reference_video")
            .expect("one reference video should be selected");
        assert_eq!(input.node_id, "1");
        assert!(!report
            .issues
            .iter()
            .any(|issue| issue.code == "AMBIGUOUS_INPUT"));
    }

    #[test]
    fn schema_analysis_is_deterministic_and_explains_conflicting_candidates() {
        let value = json!({
            "1": {
                "class_type": "CustomPromptNode",
                "_meta": {"title": "Main prompt"},
                "inputs": {
                    "caption_text": "a prompt",
                    "image_strength": 0.75
                }
            },
            "2": {
                "class_type": "FinalOutputNode",
                "inputs": {"prompt": ["1", 0]}
            }
        });
        let workflow = WorkflowDocument::parse(value).expect("synthetic workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "CustomPromptNode": {
                "input": {"required": {
                    "caption_text": ["STRING", {}],
                    "image_strength": ["FLOAT", {"min": 0.0, "max": 1.0}]
                }}
            },
            "FinalOutputNode": {
                "input": {"required": {"prompt": ["STRING", {}]}},
                "output": ["IMAGE"],
                "output_node": true
            }
        }));

        let first = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"synthetic":true}"#,
            Some(&schema),
        );
        let second = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"synthetic":true}"#,
            Some(&schema),
        );

        assert_eq!(first, second);
        let prompt = first
            .inputs
            .iter()
            .find(|input| input.semantic_key == "prompt")
            .expect("schema and graph should identify the prompt");
        assert!(prompt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::SCHEMA_TYPE_MATCH));
        assert!(first
            .issues
            .iter()
            .flat_map(|issue| &issue.candidates)
            .any(|candidate| candidate
                .evidence
                .iter()
                .any(|evidence| evidence.kind == EvidenceKind::SCHEMA_TYPE_CONFLICT)));
        assert_ne!(first.confidence, RecognitionConfidence::High);
    }

    #[test]
    fn schema_and_graph_evidence_recognize_nonstandard_prompt_on_output_path() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {
                "class_type": "UnknownPromptNode",
                "_meta": {"title": "Prompt source"},
                "inputs": {"caption_text": "hello"}
            },
            "3": {
                "class_type": "CustomSampler",
                "inputs": {"prompt": ["1", 0]}
            },
            "4": {
                "class_type": "SaveImage",
                "output_node": true,
                "inputs": {"image": ["3", 0]}
            }
        }))
        .expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "UnknownPromptNode": {"input": {"required": {
                "caption_text": ["STRING", {}]
            }}},
            "CustomSampler": {"input": {"required": {
                "prompt": ["STRING", {}]
            }}},
            "SaveImage": {
                "input": {"required": {"image": ["IMAGE", {}]}},
                "output": ["IMAGE"],
                "output_node": true
            }
        }));

        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"prompt":"synthetic"}"#,
            Some(&schema),
        );
        let prompt = report
            .inputs
            .iter()
            .find(|input| input.semantic_key == "prompt")
            .expect("nonstandard string prompt should be recognized");
        assert_eq!(prompt.node_id, "1");
        assert_eq!(prompt.input_name, "caption_text");
        assert!(prompt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::GRAPH_OUTPUT_PATH));
        assert!(prompt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::SCHEMA_TYPE_MATCH));
        assert_eq!(report.outputs[0].node_id, "4");
    }

    #[test]
    fn misleading_image_name_with_float_schema_cannot_become_image_binding() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {
                "class_type": "CustomControl",
                "inputs": {"image_strength": 0.75}
            },
            "2": {
                "class_type": "SaveImage",
                "output_node": true,
                "inputs": {"image": ["1", 0]}
            }
        }))
        .expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "CustomControl": {"input": {"required": {
                "image_strength": ["FLOAT", {"min": 0.0, "max": 1.0}]
            }}},
            "SaveImage": {
                "input": {"required": {"image": ["IMAGE", {}]}},
                "output": ["IMAGE"],
                "output_node": true
            }
        }));
        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"image_strength":0.75}"#,
            Some(&schema),
        );

        assert!(!report
            .inputs
            .iter()
            .any(|input| input.semantic_key == "reference_image"));
        assert!(report
            .issues
            .iter()
            .flat_map(|issue| &issue.candidates)
            .any(|candidate| candidate.node_id.as_deref() == Some("1")
                && candidate.input_name.as_deref() == Some("image_strength")
                && candidate
                    .evidence
                    .iter()
                    .any(|evidence| evidence.kind == EvidenceKind::SCHEMA_TYPE_CONFLICT)));
    }

    #[test]
    fn duplicate_seed_control_off_output_path_does_not_win() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "SeedNode", "inputs": {"seed": 111}},
            "2": {"class_type": "SeedNode", "inputs": {"seed": 222}},
            "3": {"class_type": "Sampler", "inputs": {"seed": ["1", 0]}},
            "4": {
                "class_type": "SaveImage",
                "output_node": true,
                "inputs": {"image": ["3", 0]}
            }
        }))
        .expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "SeedNode": {"input": {"required": {"seed": ["INT", {}]}}},
            "Sampler": {"input": {"required": {"seed": ["INT", {}]}}},
            "SaveImage": {
                "input": {"required": {"image": ["IMAGE", {}]}},
                "output": ["IMAGE"],
                "output_node": true
            }
        }));
        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"seed":111}"#,
            Some(&schema),
        );
        let seed = report
            .inputs
            .iter()
            .find(|input| input.semantic_key == "seed")
            .expect("seed should be selected");
        assert_eq!(seed.node_id, "1");
        assert_eq!(seed.value, Some(json!(111)));
        assert!(!report
            .issues
            .iter()
            .any(|issue| issue.code == "AMBIGUOUS_INPUT"));
    }

    #[test]
    fn terminal_save_output_beats_preview_branch() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "Sampler", "inputs": {}},
            "2": {"class_type": "PreviewImage", "inputs": {"image": ["1", 0]}},
            "3": {
                "class_type": "SaveImage",
                "output_node": true,
                "inputs": {"image": ["1", 0]}
            }
        }))
        .expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "Sampler": {"output": ["IMAGE"]},
            "PreviewImage": {
                "input": {"required": {"image": ["IMAGE", {}]}},
                "output": ["IMAGE"]
            },
            "SaveImage": {
                "input": {"required": {"image": ["IMAGE", {}]}},
                "output": ["IMAGE"],
                "output_node": true
            }
        }));
        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"output":"preview-and-final"}"#,
            Some(&schema),
        );
        assert_eq!(report.outputs.len(), 1);
        assert_eq!(report.outputs[0].node_id, "3");
        assert!(report.outputs[0]
            .evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::OUTPUT_NODE_FLAG));
        assert!(
            report.outputs[0]
                .evidence
                .iter()
                .any(|evidence| evidence.kind == EvidenceKind::PREVIEW_OUTPUT)
                == false
        );
        assert!(!report
            .issues
            .iter()
            .any(|issue| issue.code == "AMBIGUOUS_OUTPUT"));
    }

    #[test]
    fn equal_real_outputs_remain_resolved_as_compatible_roots() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"class_type": "Sampler", "inputs": {}},
            "2": {
                "class_type": "SaveImage",
                "output_node": true,
                "inputs": {"image": ["1", 0]}
            },
            "3": {
                "class_type": "SaveImage",
                "output_node": true,
                "inputs": {"image": ["1", 0]}
            }
        }))
        .expect("workflow should parse");
        let schema = RecognitionSchemaContext::parse(&json!({
            "Sampler": {"output": ["IMAGE"]},
            "SaveImage": {
                "input": {"required": {"image": ["IMAGE", {}]}},
                "output": ["IMAGE"],
                "output_node": true
            }
        }));
        let report = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            br#"{"output":"double"}"#,
            Some(&schema),
        );
        assert_eq!(report.outputs.len(), 2);
        assert!(report.outputs.iter().all(|output| output
            .evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::TERMINAL_OUTPUT)));
        assert!(report
            .issues
            .iter()
            .all(|issue| issue.code != "AMBIGUOUS_OUTPUT"));
        assert!(matches!(
            report.output_root_resolution,
            OutputRootResolution::Resolved { ref roots } if roots.len() == 2
        ));
    }
}
