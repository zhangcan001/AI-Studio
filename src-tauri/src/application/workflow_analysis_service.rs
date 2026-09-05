//! Pure analysis for ComfyUI API workflows.
//!
//! This module deliberately stops at a report. It does not know about the
//! library, packages, registry, runtime state, project bindings, or ComfyUI.
//! Callers can use the report for import preview and let a separate command
//! decide whether anything should be persisted.

use crate::{
    application::{
        workflow_graph_analysis::{WorkflowGraph, WorkflowSource},
        workflow_recognition_service::{
            structural_workflow_sha256, RecognitionConfidence, WorkflowIdentity,
            WorkflowRecognitionFormat,
        },
        workflow_semantic_identity::semantic_workflow_sha256,
    },
    domain::WorkflowDocument,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

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
    pub confidence: RecognitionConfidence,
    pub source: String,
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
        analyze_workflow(workflow, raw_bytes)
    }

    pub fn analyze(workflow: &WorkflowDocument, raw_bytes: &[u8]) -> WorkflowAnalysisReport {
        analyze_workflow(workflow, raw_bytes)
    }
}

/// Analyze an already parsed API workflow. The raw bytes are used only for
/// their exact SHA-256; no file, package, database, or runtime is consulted.
pub fn analyze_workflow(workflow: &WorkflowDocument, raw_bytes: &[u8]) -> WorkflowAnalysisReport {
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

    let output_analysis = infer_outputs(workflow, &graph);
    let inference_scope = output_analysis
        .roots
        .iter()
        .flat_map(|node_id| graph.upstream_closure(node_id))
        .collect::<BTreeSet<_>>();
    let inference_scope = if inference_scope.is_empty() {
        graph.nodes.clone()
    } else {
        inference_scope
    };

    let mut candidates = BTreeMap::<String, Vec<Candidate>>::new();
    let mut issues = output_analysis.issues;
    let Some(nodes) = workflow.value().as_object() else {
        return report(
            raw_sha256,
            semantic_sha256,
            structural_sha256,
            node_count,
            unique_class_count,
            Vec::new(),
            output_analysis.outputs,
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
                    &mut candidates,
                    &mut issues,
                );
            } else if let Some(guess) = literal_guess(class_type, input_name, value) {
                append_candidate(
                    &mut candidates,
                    Candidate::literal(node_id, input_name, value, guess),
                );
            }
        }
    }

    let (inputs, _bindings, input_issues) = resolve_candidates(candidates);
    issues.extend(input_issues);
    let category = category_for_outputs(&output_analysis.outputs);
    let mode = infer_mode(&inputs, &category);
    report(
        raw_sha256,
        semantic_sha256,
        structural_sha256,
        node_count,
        unique_class_count,
        inputs,
        output_analysis.outputs,
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
    confidence: RecognitionConfidence,
    source: String,
}

impl Candidate {
    fn literal(node_id: &str, input_name: &str, value: &Value, guess: Guess) -> Self {
        Self {
            semantic_key: guess.semantic_key.to_owned(),
            field_type: guess.field_type.to_owned(),
            required: guess.required,
            value: Some(value.clone()),
            node_id: node_id.to_owned(),
            input_name: input_name.to_owned(),
            confidence: guess.confidence,
            source: guess.source.to_owned(),
        }
    }

    fn linked_target(node_id: &str, input_name: &str, guess: Guess) -> Self {
        Self {
            semantic_key: guess.semantic_key.to_owned(),
            field_type: guess.field_type.to_owned(),
            required: guess.required,
            value: None,
            node_id: node_id.to_owned(),
            input_name: input_name.to_owned(),
            confidence: guess.confidence,
            source: guess.source.to_owned(),
        }
    }

    fn linked_leaf(node_id: &str, input_name: &str, value: &Value, guess: Guess) -> Self {
        Self {
            semantic_key: guess.semantic_key.to_owned(),
            field_type: guess.field_type.to_owned(),
            required: guess.required,
            value: Some(value.clone()),
            node_id: node_id.to_owned(),
            input_name: input_name.to_owned(),
            confidence: guess.confidence,
            source: guess.source.to_owned(),
        }
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

fn infer_linked_input(
    workflow: &WorkflowDocument,
    graph: &WorkflowGraph,
    target_node: &str,
    target_input: &str,
    candidates: &mut BTreeMap<String, Vec<Candidate>>,
    issues: &mut Vec<WorkflowAnalysisIssue>,
) {
    let Some(semantic_key) = linked_target_semantic(target_input) else {
        return;
    };

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
        let leaf = trace.source;
        let class_type = workflow.class_type(&leaf.node_id).unwrap_or_default();
        let Some(guess) = literal_guess(class_type, &leaf.input, &leaf.value) else {
            continue;
        };
        if guess.semantic_key != semantic_key {
            continue;
        }
        append_candidate(
            candidates,
            Candidate::linked_leaf(
                &leaf.node_id,
                &leaf.input,
                &leaf.value,
                Guess {
                    source: "GRAPH_LINKED_SOURCE_LEAF",
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

fn resolve_candidates(
    candidates: BTreeMap<String, Vec<Candidate>>,
) -> (
    Vec<WorkflowAnalysisInput>,
    Vec<WorkflowAnalysisBinding>,
    Vec<WorkflowAnalysisIssue>,
) {
    let mut inputs = Vec::new();
    let mut bindings = Vec::new();
    let mut issues = Vec::new();
    for (semantic_key, mut choices) in candidates {
        choices.sort_by(|left, right| {
            left.node_id
                .cmp(&right.node_id)
                .then(left.input_name.cmp(&right.input_name))
        });
        let high = choices
            .iter()
            .filter(|choice| choice.confidence == RecognitionConfidence::High)
            .cloned()
            .collect::<Vec<_>>();
        let selected = if high.len() == 1 {
            Some(high[0].clone())
        } else if choices.len() == 1 {
            Some(choices[0].clone())
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
            confidence: selected.confidence,
            source: selected.source,
        });
        bindings.push(WorkflowAnalysisBinding {
            semantic_key: selected.semantic_key,
            target_node: selected.node_id,
            target_input: selected.input_name,
            item_index: None,
        });
    }
    (inputs, bindings, issues)
}

fn append_candidate(candidates: &mut BTreeMap<String, Vec<Candidate>>, candidate: Candidate) {
    let choices = candidates
        .entry(candidate.semantic_key.clone())
        .or_default();
    if let Some(existing) = choices.iter_mut().find(|existing| {
        existing.node_id == candidate.node_id && existing.input_name == candidate.input_name
    }) {
        if confidence_rank(candidate.confidence) > confidence_rank(existing.confidence) {
            *existing = candidate;
        }
    } else {
        choices.push(candidate);
    }
}

fn literal_guess(class_type: &str, input_name: &str, value: &Value) -> Option<Guess> {
    let name = normalize(input_name);
    let lower_class = class_type.to_ascii_lowercase();
    let is_text = value.is_string();
    let is_number = value.is_number();
    let is_media = is_text || value.is_array();
    if is_ignored_input(&name) {
        return None;
    }
    if is_text && matches!(name.as_str(), "prompt" | "positive_prompt") {
        return Some(Guess {
            semantic_key: "prompt",
            field_type: "textarea",
            required: true,
            confidence: RecognitionConfidence::High,
            source: "INPUT_NAME_EXACT",
        });
    }
    if is_text
        && (matches!(name.as_str(), "text" | "positive")
            && (lower_class.contains("text") || lower_class.contains("prompt")))
    {
        return Some(Guess {
            semantic_key: "prompt",
            field_type: "textarea",
            required: true,
            confidence: RecognitionConfidence::High,
            source: "INPUT_NAME_PROMPT_ALIAS",
        });
    }
    if is_text
        && (name == "negative" || name == "negative_prompt" || name.contains("negative_prompt"))
    {
        return Some(Guess {
            semantic_key: "negative_prompt",
            field_type: "textarea",
            required: false,
            confidence: RecognitionConfidence::High,
            source: "INPUT_NAME_NEGATIVE_PROMPT",
        });
    }
    if is_text && (name.starts_with("prompt_") || name.ends_with("_prompt")) {
        return Some(Guess {
            semantic_key: "prompt",
            field_type: "textarea",
            required: true,
            confidence: RecognitionConfidence::Medium,
            source: "INPUT_NAME_PROMPT_HEURISTIC",
        });
    }
    if is_number && matches!(name.as_str(), "seed" | "noise_seed" | "random_seed") {
        return is_integer_number(value).then_some(Guess {
            semantic_key: "seed",
            field_type: "seed",
            required: true,
            confidence: RecognitionConfidence::High,
            source: "INPUT_NAME_SEED_AND_INTEGER_LITERAL",
        });
    }
    if is_number && name == "width" {
        return Some(numeric_guess("width", value, RecognitionConfidence::High));
    }
    if is_number && name == "height" {
        return Some(numeric_guess("height", value, RecognitionConfidence::High));
    }
    if is_number && matches!(name.as_str(), "steps" | "num_steps" | "sampling_steps") {
        return Some(numeric_guess(
            "steps",
            value,
            if name == "steps" {
                RecognitionConfidence::High
            } else {
                RecognitionConfidence::Medium
            },
        ));
    }
    if is_number
        && matches!(
            name.as_str(),
            "duration" | "duration_seconds" | "seconds" | "length"
        )
    {
        return Some(numeric_guess(
            "duration_seconds",
            value,
            RecognitionConfidence::High,
        ));
    }
    if is_number {
        let semantic_key = match name.as_str() {
            "denoise" => Some("denoise"),
            "fps" | "frame_rate" | "framerate" => Some("fps"),
            "cfg" | "cfg_scale" => Some("cfg"),
            "guidance" => Some("guidance"),
            "strength" => Some("strength"),
            "shift" => Some("shift"),
            "scale" => Some("scale"),
            "weight" => Some("weight"),
            _ => None,
        };
        if let Some(semantic_key) = semantic_key {
            return Some(numeric_guess(
                semantic_key,
                value,
                if matches!(semantic_key, "denoise" | "fps") {
                    RecognitionConfidence::High
                } else {
                    RecognitionConfidence::Medium
                },
            ));
        }
    }
    if is_text && (name == "first_frame" || name == "start_frame") {
        return Some(Guess {
            semantic_key: "first_frame",
            field_type: "image",
            required: true,
            confidence: RecognitionConfidence::High,
            source: "INPUT_NAME_FIRST_FRAME",
        });
    }
    if is_text && (name == "last_frame" || name == "end_frame") {
        return Some(Guess {
            semantic_key: "last_frame",
            field_type: "image",
            required: true,
            confidence: RecognitionConfidence::High,
            source: "INPUT_NAME_LAST_FRAME",
        });
    }
    if is_media && name.contains("image") && !is_non_media_image_parameter(&name) {
        let plural = name.contains("images") || name.contains("reference_images");
        return Some(Guess {
            semantic_key: if plural {
                "reference_images"
            } else {
                "reference_image"
            },
            field_type: if plural { "images" } else { "image" },
            required: true,
            confidence: RecognitionConfidence::Medium,
            source: "INPUT_NAME_MEDIA_SEMANTICS",
        });
    }
    if is_media && name.contains("video") {
        let plural = name.contains("videos") || name.contains("reference_videos");
        return Some(Guess {
            semantic_key: if plural {
                "reference_videos"
            } else {
                "reference_video"
            },
            field_type: if plural { "videos" } else { "video" },
            required: true,
            confidence: RecognitionConfidence::Medium,
            source: "INPUT_NAME_MEDIA_SEMANTICS",
        });
    }
    if is_media && name.contains("audio") {
        let plural = name.contains("audios") || name.contains("reference_audios");
        return Some(Guess {
            semantic_key: if plural {
                "reference_audios"
            } else {
                "reference_audio"
            },
            field_type: if plural { "audios" } else { "audio" },
            required: true,
            confidence: RecognitionConfidence::Medium,
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

fn linked_target_semantic(input_name: &str) -> Option<&'static str> {
    match normalize(input_name).as_str() {
        "prompt" | "text" | "positive" | "positive_prompt" => Some("prompt"),
        "negative" | "negative_prompt" => Some("negative_prompt"),
        "width" => Some("width"),
        "height" => Some("height"),
        "seed" | "noise_seed" | "random_seed" => Some("seed"),
        "length" | "frames" | "num_frames" | "frame_count" => Some("duration_seconds"),
        "image" | "input_image" => Some("reference_image"),
        "images" | "reference_images" => Some("reference_images"),
        "video" | "input_video" => Some("reference_video"),
        "videos" | "reference_videos" => Some("reference_videos"),
        "audio" | "input_audio" => Some("reference_audio"),
        "audios" | "reference_audios" => Some("reference_audios"),
        _ => None,
    }
}

fn infer_outputs(workflow: &WorkflowDocument, graph: &WorkflowGraph) -> OutputAnalysis {
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
        if is_utility_class(&lower) {
            continue;
        }
        let explicit = node
            .get("output_node")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let output_type = output_type_for_node(node, &lower);
        let base_score = if is_video_output_class(&lower) {
            100
        } else if is_image_output_class(&lower) {
            90
        } else if lower.contains("preview") {
            30
        } else if explicit && node_has_media_input(node) {
            50
        } else {
            0
        };
        if base_score == 0 && !explicit {
            continue;
        }
        let score = base_score
            + if explicit { 20 } else { 0 }
            + if graph.downstream_of(node_id).is_empty() {
                10
            } else {
                0
            };
        candidates.push(OutputCandidate {
            node_id: node_id.clone(),
            label: if title.is_empty() {
                format!("节点 {node_id}")
            } else {
                title
            },
            output_type,
            score,
        });
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(left.node_id.cmp(&right.node_id))
    });
    let Some(best_score) = candidates.first().map(|candidate| candidate.score) else {
        return OutputAnalysis::unknown();
    };
    let best = candidates
        .iter()
        .filter(|candidate| candidate.score == best_score)
        .cloned()
        .collect::<Vec<_>>();
    let ambiguous = best.len() > 1;
    let outputs: Vec<WorkflowAnalysisOutput> = best
        .iter()
        .enumerate()
        .map(|(index, candidate)| WorkflowAnalysisOutput {
            output_id: if index == 0 {
                "output_1".to_owned()
            } else {
                format!("output_{}", index + 1)
            },
            output_type: candidate.output_type.clone(),
            node_id: candidate.node_id.clone(),
            label: candidate.label.clone(),
            required: true,
            confidence: if ambiguous {
                RecognitionConfidence::Low
            } else {
                RecognitionConfidence::High
            },
        })
        .collect();
    let issue_candidates = outputs
        .iter()
        .map(|output| WorkflowAnalysisIssueCandidate {
            label: output.label.clone(),
            node_id: Some(output.node_id.clone()),
            input_name: None,
            output_id: Some(output.output_id.clone()),
            output_type: Some(output.output_type.clone()),
            field_type: None,
        })
        .collect();
    let issues = ambiguous
        .then_some(vec![WorkflowAnalysisIssue {
            code: "AMBIGUOUS_OUTPUT".to_owned(),
            message: "检测到多个可能的最终输出节点，请选择要发布的输出。".to_owned(),
            field: Some("output_1".to_owned()),
            candidates: issue_candidates,
        }])
        .unwrap_or_default();
    OutputAnalysis {
        roots: best
            .into_iter()
            .map(|candidate| candidate.node_id)
            .collect(),
        outputs,
        issues,
    }
}

#[derive(Clone)]
struct OutputCandidate {
    node_id: String,
    label: String,
    output_type: String,
    score: i32,
}

struct OutputAnalysis {
    roots: Vec<String>,
    outputs: Vec<WorkflowAnalysisOutput>,
    issues: Vec<WorkflowAnalysisIssue>,
}

impl OutputAnalysis {
    fn unknown() -> Self {
        Self {
            roots: Vec::new(),
            outputs: Vec::new(),
            issues: vec![WorkflowAnalysisIssue {
                code: "UNKNOWN_OUTPUT".to_owned(),
                message: "未能识别唯一的最终输出节点。".to_owned(),
                field: Some("output_1".to_owned()),
                candidates: Vec::new(),
            }],
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
                item_index: None,
            })
            .collect(),
        inputs,
        outputs,
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
        } else if keys.iter().any(|key| key.starts_with("reference_")) {
            "reference_to_video".to_owned()
        } else if keys.contains("image") {
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

fn output_type_for_node(node: &serde_json::Map<String, Value>, text: &str) -> String {
    if is_video_output_class(text)
        || text.contains("video")
        || text.contains("animated")
        || text.contains("webm")
        || node_has_video_input(node)
    {
        "video".to_owned()
    } else {
        "image".to_owned()
    }
}

fn node_has_video_input(node: &serde_json::Map<String, Value>) -> bool {
    node.get("inputs")
        .and_then(Value::as_object)
        .is_some_and(|inputs| {
            inputs.keys().any(|name| {
                let name = normalize(name);
                name.contains("video") || name.contains("videos") || name.contains("frames")
            })
        })
}

fn node_has_media_input(node: &serde_json::Map<String, Value>) -> bool {
    node.get("inputs")
        .and_then(Value::as_object)
        .is_some_and(|inputs| {
            inputs.keys().any(|name| {
                let name = normalize(name);
                name.contains("image")
                    || name.contains("images")
                    || name.contains("video")
                    || name.contains("videos")
                    || name.contains("frames")
            })
        })
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
    [
        "savevideo",
        "vhs_videocombine",
        "videocombine",
        "createvideo",
        "videooutput",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn is_image_output_class(text: &str) -> bool {
    text.contains("saveimage") || text.contains("imageoutput")
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

fn is_non_media_image_parameter(name: &str) -> bool {
    ["image_size", "image_scale", "ref_image_size"]
        .iter()
        .any(|marker| name == *marker || name.ends_with(marker))
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

    const AITUDOU_8STEP: &str = include_str!(
        "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/workflow_api.json"
    );

    fn fixture_report() -> WorkflowAnalysisReport {
        let value: Value = serde_json::from_str(AITUDOU_8STEP).expect("fixture should parse");
        let workflow = WorkflowDocument::parse(value).expect("fixture should be an API workflow");
        WorkflowAnalysisService::analyze_workflow(&workflow, AITUDOU_8STEP.as_bytes())
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
        assert!(report
            .outputs
            .iter()
            .any(|output| output.output_type == "video" && output.node_id == "62"));
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
}
