//! One read-only recognition pass for imported ComfyUI API workflows.
//!
//! Recognition deliberately does not require a live ComfyUI connection.  The
//! report can therefore describe an importable workflow while its runtime
//! capability is still `NOT_CHECKED`, `OFFLINE`, or `MISSING_NODES`.

use crate::{
    application::{
        ports::WorkflowPackageFiles, workflow_graph_analysis::WorkflowGraph,
        workflow_manifest::WorkflowManifest, workflow_semantic_identity::semantic_workflow_sha256,
    },
    compiler::RecipeParser,
    domain::{OutputType, Recipe, WorkflowDocument},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowRecognitionFormat {
    Api,
    Ui,
    InvalidJson,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowIdentity {
    New,
    ExactRaw,
    ExactSemantic,
    StructuralVariant,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecognitionConfidence {
    High,
    Medium,
    Low,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecipeFreshness {
    Current,
    Outdated,
    Missing,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeCapabilityState {
    Ready,
    MissingNodes,
    Offline,
    Incompatible,
    NotChecked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionInput {
    pub semantic_key: String,
    pub field_type: String,
    pub label: String,
    pub required: bool,
    pub node_id: String,
    pub input_name: String,
    pub confidence: RecognitionConfidence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionOutput {
    pub output_id: String,
    #[serde(rename = "type")]
    pub output_type: String,
    pub node_id: String,
    pub required: bool,
    pub confidence: RecognitionConfidence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionIssue {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilitySummary {
    pub state: RuntimeCapabilityState,
    pub issues: Vec<String>,
}

impl Default for RuntimeCapabilityState {
    fn default() -> Self {
        Self::NotChecked
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionReport {
    pub format: WorkflowRecognitionFormat,
    pub recognized: bool,
    pub importable: bool,
    pub executable: bool,
    pub identity: WorkflowIdentity,
    pub raw_sha256: String,
    pub semantic_sha256: Option<String>,
    pub structural_sha256: Option<String>,
    pub existing_workflow_id: Option<String>,
    pub existing_workflow_version: Option<String>,
    pub existing_name: Option<String>,
    pub category: String,
    pub mode: String,
    pub confidence: RecognitionConfidence,
    pub inputs: Vec<WorkflowRecognitionInput>,
    pub outputs: Vec<WorkflowRecognitionOutput>,
    pub recipe_status: RecipeFreshness,
    pub runtime_capability: RuntimeCapabilityState,
    pub capability_issues: Vec<String>,
    pub issues: Vec<WorkflowRecognitionIssue>,
    pub suggested_action: String,
    pub node_count: usize,
    pub unique_class_count: usize,
}

impl WorkflowRecognitionReport {
    /// Attach a runtime check without changing importability or identity.
    pub fn with_runtime_capability(mut self, capability: RuntimeCapabilitySummary) -> Self {
        self.runtime_capability = capability.state;
        self.capability_issues = capability.issues;
        self.executable =
            self.importable && matches!(self.runtime_capability, RuntimeCapabilityState::Ready);
        self
    }
}

#[derive(Clone, Debug)]
struct ExistingCandidate {
    manifest: WorkflowManifest,
    recipe: Option<Recipe>,
    workflow: WorkflowDocument,
    raw_sha256: String,
    structural_sha256: String,
}

#[derive(Clone, Debug)]
struct GraphRecognition {
    graph: WorkflowGraph,
    inputs: Vec<WorkflowRecognitionInput>,
    outputs: Vec<WorkflowRecognitionOutput>,
    category: String,
    mode: String,
    confidence: RecognitionConfidence,
    issues: Vec<WorkflowRecognitionIssue>,
}

/// Stateless workflow recognition entry point.
pub struct WorkflowRecognitionService;

impl WorkflowRecognitionService {
    pub fn detect_format(bytes: &[u8]) -> WorkflowRecognitionFormat {
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            return WorkflowRecognitionFormat::InvalidJson;
        };
        detect_format_value(&value)
    }

    /// Recognize any selected file. Invalid, UI, and unknown JSON return a
    /// report instead of being confused with a runtime capability failure.
    pub fn recognize_bytes(
        bytes: &[u8],
        existing_packages: &[WorkflowPackageFiles],
    ) -> WorkflowRecognitionReport {
        let raw_sha256 = sha256(bytes);
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            return empty_report(
                WorkflowRecognitionFormat::InvalidJson,
                raw_sha256,
                false,
                false,
                "SELECT_OTHER_FILE",
                "INVALID_JSON",
                "无法读取这个文件，它不是有效的 JSON。",
            );
        };

        match detect_format_value(&value) {
            WorkflowRecognitionFormat::Ui => empty_report(
                WorkflowRecognitionFormat::Ui,
                raw_sha256,
                true,
                false,
                "EXPORT_API_FORMAT",
                "UI_FORMAT",
                "这是 ComfyUI 编辑器格式，请导出 API Format JSON 后重新导入。",
            ),
            WorkflowRecognitionFormat::Unknown => empty_report(
                WorkflowRecognitionFormat::Unknown,
                raw_sha256,
                false,
                false,
                "SELECT_OTHER_FILE",
                "UNKNOWN_WORKFLOW",
                "这个 JSON 不是可识别的 ComfyUI 工作流。",
            ),
            WorkflowRecognitionFormat::InvalidJson => unreachable!(),
            WorkflowRecognitionFormat::Api => {
                let Ok(workflow) = WorkflowDocument::parse(value) else {
                    return api_error_report(
                        raw_sha256,
                        "WORKFLOW_ROOT_INVALID",
                        "工作流根节点必须是 JSON object。",
                    );
                };
                Self::recognize_api_workflow(&workflow, bytes, existing_packages)
            }
        }
    }

    pub fn recognize_api_workflow(
        workflow: &WorkflowDocument,
        raw_bytes: &[u8],
        existing_packages: &[WorkflowPackageFiles],
    ) -> WorkflowRecognitionReport {
        let raw_sha256 = sha256(raw_bytes);
        let semantic_sha256 = semantic_workflow_sha256(workflow);
        let structural_sha256 = structural_workflow_sha256(workflow);
        let analysis = match recognize_graph(workflow) {
            Ok(analysis) => analysis,
            Err(message) => {
                return api_error_report_with_identity(
                    raw_sha256,
                    Some(semantic_sha256),
                    Some(structural_sha256),
                    "GRAPH_INVALID",
                    message,
                )
            }
        };

        let mut report = WorkflowRecognitionReport {
            format: WorkflowRecognitionFormat::Api,
            recognized: true,
            importable: true,
            executable: false,
            identity: WorkflowIdentity::New,
            raw_sha256,
            semantic_sha256: Some(semantic_sha256),
            structural_sha256: Some(structural_sha256),
            existing_workflow_id: None,
            existing_workflow_version: None,
            existing_name: None,
            category: analysis.category,
            mode: analysis.mode,
            confidence: analysis.confidence,
            inputs: analysis.inputs,
            outputs: analysis.outputs,
            recipe_status: RecipeFreshness::Missing,
            runtime_capability: RuntimeCapabilityState::NotChecked,
            capability_issues: Vec::new(),
            issues: analysis.issues,
            suggested_action: "ADD_TO_LIBRARY".to_owned(),
            node_count: analysis.graph.nodes.len(),
            unique_class_count: unique_class_count(workflow),
        };

        apply_existing_identity(&mut report, workflow, existing_packages);
        report.executable = false;
        report
    }
}

/// Convenience API for callers that do not need to instantiate a service.
pub fn recognize_workflow(
    bytes: &[u8],
    existing_packages: &[WorkflowPackageFiles],
) -> WorkflowRecognitionReport {
    WorkflowRecognitionService::recognize_bytes(bytes, existing_packages)
}

/// Stable V1 structural fingerprint. It preserves node IDs, classes, input
/// names, links, and output topology, while intentionally discarding literal
/// input values. It is a similarity hint only; callers must not auto-merge it.
pub fn structural_workflow_fingerprint(workflow: &WorkflowDocument) -> Value {
    let graph = WorkflowGraph::from_document(workflow).unwrap_or_default();
    let Some(root) = workflow.value().as_object() else {
        return json!({"nodes": []});
    };
    let mut nodes = Vec::new();
    for (node_id, node) in root {
        let Some(node) = node.as_object() else {
            continue;
        };
        let inputs = node
            .get("inputs")
            .and_then(Value::as_object)
            .map(|inputs| {
                inputs
                    .keys()
                    .map(|input_name| {
                        let link = graph
                            .upstream_of(node_id)
                            .iter()
                            .find(|candidate| candidate.target_input == *input_name)
                            .map(|link| json!([link.source_node_id, link.source_output_index]));
                        json!({"name": input_name, "link": link})
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let output_node = node
            .get("output_node")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        nodes.push(json!({
            "id": node_id,
            "class_type": node.get("class_type").and_then(Value::as_str).unwrap_or_default(),
            "inputs": inputs,
            "output_node": output_node,
            "terminal": graph.downstream_of(node_id).is_empty(),
        }));
    }
    nodes.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    json!({"nodes": nodes})
}

pub fn structural_workflow_sha256(workflow: &WorkflowDocument) -> String {
    let bytes = serde_json::to_vec(&structural_workflow_fingerprint(workflow))
        .expect("structural fingerprint should always serialize");
    sha256(&bytes)
}

/// Short alias used by diagnostics and tests.
pub fn structural_fingerprint(workflow: &WorkflowDocument) -> String {
    structural_workflow_sha256(workflow)
}

fn apply_existing_identity(
    report: &mut WorkflowRecognitionReport,
    workflow: &WorkflowDocument,
    packages: &[WorkflowPackageFiles],
) {
    let mut loaded = packages
        .iter()
        .filter_map(parse_existing_candidate)
        .collect::<Vec<_>>();
    if loaded.is_empty() {
        return;
    }

    let raw_matches = loaded
        .iter()
        .filter(|candidate| candidate.raw_sha256 == report.raw_sha256)
        .collect::<Vec<_>>();
    let semantic_matches = if raw_matches.is_empty() {
        loaded
            .iter()
            .filter(|candidate| {
                Some(semantic_workflow_sha256(&candidate.workflow)) == report.semantic_sha256
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let structural_matches = if raw_matches.is_empty() && semantic_matches.is_empty() {
        loaded
            .iter()
            .filter(|candidate| {
                Some(candidate.structural_sha256.clone()) == report.structural_sha256
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let (identity, matches) = if !raw_matches.is_empty() {
        (WorkflowIdentity::ExactRaw, raw_matches)
    } else if !semantic_matches.is_empty() {
        (WorkflowIdentity::ExactSemantic, semantic_matches)
    } else if !structural_matches.is_empty() {
        (WorkflowIdentity::StructuralVariant, structural_matches)
    } else {
        return;
    };

    let representative = matches
        .iter()
        .max_by(|left, right| {
            compare_version(
                &left.manifest.recipe_version,
                &right.manifest.recipe_version,
            )
        })
        .expect("identity matches are not empty");
    report.identity = identity;
    report.existing_workflow_id = Some(representative.manifest.id.clone());
    report.existing_workflow_version = Some(representative.manifest.workflow_version.clone());
    report.existing_name = Some(representative.manifest.name.clone());
    report.recipe_status = recipe_status(
        report,
        matches
            .iter()
            .filter_map(|candidate| candidate.recipe.as_ref()),
    );
    report.suggested_action = match (identity, report.recipe_status) {
        (
            WorkflowIdentity::ExactRaw | WorkflowIdentity::ExactSemantic,
            RecipeFreshness::Current,
        ) => "OPEN_EXISTING".to_owned(),
        (WorkflowIdentity::ExactRaw | WorkflowIdentity::ExactSemantic, _) => {
            "UPDATE_RECIPE".to_owned()
        }
        (WorkflowIdentity::StructuralVariant, _) => "CHOOSE_VARIANT_ACTION".to_owned(),
        (WorkflowIdentity::New, _) => "ADD_TO_LIBRARY".to_owned(),
    };
    // Structural identity is deliberately informational. It never changes
    // `importable` into a duplicate rejection and never auto-merges a graph.
    if identity == WorkflowIdentity::StructuralVariant {
        report.issues.push(WorkflowRecognitionIssue {
            code: "STRUCTURAL_VARIANT".to_owned(),
            message: "检测到一个结构相似的工作流；请选择创建新工作流或新版本。".to_owned(),
        });
    }
    let _ = workflow;
    loaded.clear();
}

fn parse_existing_candidate(files: &WorkflowPackageFiles) -> Option<ExistingCandidate> {
    let manifest = WorkflowManifest::parse(&files.manifest_yaml).ok()?;
    let workflow_value = serde_json::from_str::<Value>(&files.workflow_json).ok()?;
    if detect_format_value(&workflow_value) != WorkflowRecognitionFormat::Api {
        return None;
    }
    let workflow = WorkflowDocument::parse(workflow_value).ok()?;
    WorkflowGraph::from_document(&workflow).ok()?;
    Some(ExistingCandidate {
        raw_sha256: sha256(files.workflow_json.as_bytes()),
        structural_sha256: structural_workflow_sha256(&workflow),
        workflow,
        recipe: RecipeParser::parse(&files.recipe_yaml).ok(),
        manifest,
    })
}

fn recipe_status<'a>(
    report: &WorkflowRecognitionReport,
    recipes: impl Iterator<Item = &'a Recipe>,
) -> RecipeFreshness {
    let mut found = false;
    for recipe in recipes {
        found = true;
        if recipe_matches_report(recipe, report) {
            return RecipeFreshness::Current;
        }
    }
    if found {
        RecipeFreshness::Outdated
    } else {
        RecipeFreshness::Missing
    }
}

fn recipe_matches_report(recipe: &Recipe, report: &WorkflowRecognitionReport) -> bool {
    let inferred_keys = report
        .inputs
        .iter()
        .filter(|input| input.confidence != RecognitionConfidence::Low)
        .map(|input| input.semantic_key.as_str())
        .collect::<BTreeSet<_>>();
    if inferred_keys
        .iter()
        .any(|key| !recipe.inputs.contains_key(*key))
    {
        return false;
    }
    if report.outputs.iter().any(|output| {
        !recipe.outputs.iter().any(|candidate| {
            let output_type_matches = match candidate.output_type {
                OutputType::Image => output.output_type == "image",
                OutputType::Video => output.output_type == "video",
            };
            output_type_matches && candidate.node == output.node_id
        })
    }) {
        return false;
    }
    true
}

fn recognize_graph(workflow: &WorkflowDocument) -> Result<GraphRecognition, String> {
    let graph = WorkflowGraph::from_document(workflow).map_err(|error| error.to_string())?;
    let Some(root) = workflow.value().as_object() else {
        return Err("workflow root is not an object".to_owned());
    };
    let mut inputs = BTreeMap::<String, WorkflowRecognitionInput>::new();
    let mut media = BTreeSet::new();

    for (node_id, node) in root {
        let Some(node) = node.as_object() else {
            continue;
        };
        let class_type = node
            .get("class_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(node_inputs) = node.get("inputs").and_then(Value::as_object) else {
            return Err(format!("node {node_id} is missing object inputs"));
        };
        for (input_name, value) in node_inputs {
            let Some(candidate) = recognize_input(node_id, class_type, input_name, value) else {
                continue;
            };
            if let Some(media_kind) = media_kind(&candidate.semantic_key) {
                media.insert(media_kind);
            }
            let replace = inputs
                .get(&candidate.semantic_key)
                .is_none_or(|current| candidate.confidence > current.confidence);
            if replace {
                inputs.insert(candidate.semantic_key.clone(), candidate);
            }
        }
    }

    let mut issues = Vec::new();
    let (outputs, output_ambiguous) = recognize_outputs(workflow, &graph);
    if output_ambiguous {
        issues.push(WorkflowRecognitionIssue {
            code: "AMBIGUOUS_OUTPUT".to_owned(),
            message: "检测到多个同等优先级的输出节点，请确认输出。".to_owned(),
        });
    }
    let category = if outputs.iter().any(|output| output.output_type == "video") {
        "video"
    } else if outputs.iter().any(|output| output.output_type == "image") {
        "image"
    } else {
        "unknown"
    }
    .to_owned();
    let mode = infer_mode(&media, &category);
    let mut confidence = inputs
        .values()
        .map(|input| input.confidence)
        .chain(outputs.iter().map(|output| output.confidence))
        .min()
        .unwrap_or(RecognitionConfidence::Low);
    if output_ambiguous {
        confidence = RecognitionConfidence::Low;
    }
    Ok(GraphRecognition {
        graph,
        inputs: inputs.into_values().collect(),
        outputs,
        category,
        mode,
        confidence,
        issues,
    })
}

fn recognize_input(
    node_id: &str,
    class_type: &str,
    input_name: &str,
    value: &Value,
) -> Option<WorkflowRecognitionInput> {
    let normalized = input_name
        .to_ascii_lowercase()
        .replace(['-', ' ', '.'], "_");
    if matches!(
        normalized.as_str(),
        "filename"
            | "filename_prefix"
            | "format"
            | "codec"
            | "aspect_ratio"
            | "multiple"
            | "megapixels"
    ) || normalized.contains("model")
        || normalized.contains("vae")
        || normalized.contains("clip")
        || normalized.contains("lora")
        || normalized.contains("scheduler")
    {
        return None;
    }
    let linked = value.as_array().is_some_and(|link| {
        link.len() == 2 && link[0].as_str().is_some() && link[1].as_u64().is_some()
    });
    let (semantic_key, field_type, confidence, required, label) = if normalized == "prompt"
        || normalized.contains("prompt")
    {
        (
            "prompt",
            "textarea",
            RecognitionConfidence::High,
            true,
            "Prompt",
        )
    } else if normalized == "text"
        && (class_type.to_ascii_lowercase().contains("text")
            || class_type.to_ascii_lowercase().contains("prompt"))
    {
        (
            "prompt",
            "textarea",
            RecognitionConfidence::Medium,
            true,
            "Prompt",
        )
    } else if normalized.contains("seed") {
        ("seed", "seed", RecognitionConfidence::High, true, "Seed")
    } else if normalized == "width" || normalized.ends_with("_width") {
        (
            "width",
            "integer",
            RecognitionConfidence::High,
            true,
            "Width",
        )
    } else if normalized == "height" || normalized.ends_with("_height") {
        (
            "height",
            "integer",
            RecognitionConfidence::High,
            true,
            "Height",
        )
    } else if matches!(
        normalized.as_str(),
        "duration" | "duration_seconds" | "seconds"
    ) || normalized == "length"
    {
        (
            "duration_seconds",
            "number",
            if normalized == "length" {
                RecognitionConfidence::Medium
            } else {
                RecognitionConfidence::High
            },
            true,
            "Duration (seconds)",
        )
    } else if normalized == "steps" || normalized.ends_with("_steps") {
        (
            "steps",
            "integer",
            RecognitionConfidence::High,
            false,
            "Steps",
        )
    } else if normalized == "denoise" || normalized.contains("denoise") {
        (
            "denoise",
            "number",
            RecognitionConfidence::High,
            false,
            "Denoise",
        )
    } else if matches!(normalized.as_str(), "fps" | "frame_rate" | "framerate") {
        ("fps", "number", RecognitionConfidence::High, false, "FPS")
    } else if normalized.contains("first_frame") || normalized == "first_image" {
        (
            "first_frame",
            "image",
            RecognitionConfidence::High,
            false,
            "First frame",
        )
    } else if normalized.contains("last_frame") || normalized == "last_image" {
        (
            "last_frame",
            "image",
            RecognitionConfidence::High,
            false,
            "Last frame",
        )
    } else if normalized.contains("ref_images") || normalized.contains("reference_images") {
        (
            "reference_images",
            "images",
            RecognitionConfidence::High,
            false,
            "Reference images",
        )
    } else if normalized.contains("ref_video") || normalized.contains("reference_video") {
        (
            "reference_video",
            "video",
            RecognitionConfidence::High,
            false,
            "Reference video",
        )
    } else if normalized.contains("audio") {
        let lower_class = class_type.to_ascii_lowercase();
        if lower_class.contains("createvideo")
            || lower_class.contains("savevideo")
            || lower_class.contains("videocombine")
        {
            return None;
        }
        (
            "reference_audio",
            "audio",
            RecognitionConfidence::High,
            false,
            "Reference audio",
        )
    } else if normalized == "image" && linked && !class_type.to_ascii_lowercase().contains("load") {
        (
            "image",
            "image",
            RecognitionConfidence::Medium,
            false,
            "Image",
        )
    } else if normalized == "video" && linked && !class_type.to_ascii_lowercase().contains("load") {
        (
            "video",
            "video",
            RecognitionConfidence::Medium,
            false,
            "Video",
        )
    } else {
        return None;
    };
    Some(WorkflowRecognitionInput {
        semantic_key: semantic_key.to_owned(),
        field_type: field_type.to_owned(),
        label: label.to_owned(),
        required,
        node_id: node_id.to_owned(),
        input_name: input_name.to_owned(),
        confidence,
    })
}

fn recognize_outputs(
    workflow: &WorkflowDocument,
    graph: &WorkflowGraph,
) -> (Vec<WorkflowRecognitionOutput>, bool) {
    let Some(root) = workflow.value().as_object() else {
        return (Vec::new(), false);
    };
    let mut candidates = Vec::new();
    for (node_id, node) in root {
        let Some(node) = node.as_object() else {
            continue;
        };
        let class_type = node
            .get("class_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let lower = class_type.to_ascii_lowercase();
        if is_utility_class(&lower) {
            continue;
        }
        let explicit = node
            .get("output_node")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let video = is_video_output_class(&lower);
        let image = is_image_output_class(&lower);
        let preview = lower.contains("preview");
        if !(explicit || video || image || preview) {
            continue;
        }
        let (output_type, base_score) = if video {
            ("video", 100)
        } else if image {
            ("image", 90)
        } else {
            ("image", 30)
        };
        let score = base_score
            + if explicit { 20 } else { 0 }
            + if graph.downstream_of(node_id).is_empty() {
                10
            } else {
                0
            };
        candidates.push((score, node_id.clone(), output_type.to_owned()));
    }
    candidates.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    let Some((best_score, _, _)) = candidates.first() else {
        return (Vec::new(), false);
    };
    let ambiguous = candidates
        .iter()
        .filter(|candidate| candidate.0 == *best_score)
        .count()
        > 1;
    let outputs = candidates
        .iter()
        .filter(|candidate| candidate.0 == *best_score)
        .enumerate()
        .map(
            |(index, (_, node_id, output_type))| WorkflowRecognitionOutput {
                output_id: if index == 0 {
                    format!("generated_{output_type}")
                } else {
                    format!("generated_{output_type}_{index}")
                },
                output_type: output_type.clone(),
                node_id: node_id.clone(),
                required: true,
                confidence: if ambiguous {
                    RecognitionConfidence::Low
                } else if *best_score >= 90 {
                    RecognitionConfidence::High
                } else {
                    RecognitionConfidence::Medium
                },
            },
        )
        .collect();
    (outputs, ambiguous)
}

fn infer_mode(media: &BTreeSet<&'static str>, category: &str) -> String {
    if media.contains("reference_video") && media.contains("image") {
        "reference_video_image_video".to_owned()
    } else if media.contains("reference_image") && media.contains("reference_audio") {
        "reference_image_audio_video".to_owned()
    } else if media.contains("reference_image") {
        "reference_image_video".to_owned()
    } else if media.contains("reference_audio") {
        "reference_audio_video".to_owned()
    } else if media.contains("first_frame") && media.contains("last_frame") {
        "first_last_video".to_owned()
    } else if media.contains("first_frame")
        || media.contains("last_frame")
        || media.contains("image")
    {
        "image_to_video".to_owned()
    } else if category == "video" {
        "text_to_video".to_owned()
    } else if category == "image" {
        "text_to_image".to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn media_kind(semantic_key: &str) -> Option<&'static str> {
    match semantic_key {
        "first_frame" => Some("first_frame"),
        "last_frame" => Some("last_frame"),
        "reference_images" => Some("reference_image"),
        "reference_video" => Some("reference_video"),
        "reference_audio" => Some("reference_audio"),
        "image" => Some("image"),
        "video" => Some("video"),
        _ => None,
    }
}

fn is_utility_class(class_type: &str) -> bool {
    class_type.contains("clearcache")
        || class_type.contains("debug")
        || class_type.contains("log")
        || class_type.contains("preview") && class_type.contains("latent")
}

fn is_video_output_class(class_type: &str) -> bool {
    class_type.contains("savevideo")
        || class_type.contains("vhs_videocombine")
        || class_type.contains("videocombine")
        || class_type.contains("createvideo")
        || class_type.contains("videooutput")
}

fn is_image_output_class(class_type: &str) -> bool {
    class_type.contains("saveimage") || class_type.contains("imageoutput")
}

fn detect_format_value(value: &Value) -> WorkflowRecognitionFormat {
    let Some(object) = value.as_object() else {
        return WorkflowRecognitionFormat::Unknown;
    };
    if object.get("nodes").is_some_and(Value::is_array)
        && object.get("links").is_some_and(Value::is_array)
    {
        return WorkflowRecognitionFormat::Ui;
    }
    if !object.is_empty()
        && object.keys().all(|key| is_numeric_node_id(key))
        && object.values().all(|node| {
            node.as_object().is_some_and(|node| {
                node.get("class_type").is_some_and(Value::is_string)
                    && node.get("inputs").is_some_and(Value::is_object)
            })
        })
    {
        WorkflowRecognitionFormat::Api
    } else {
        WorkflowRecognitionFormat::Unknown
    }
}

fn empty_report(
    format: WorkflowRecognitionFormat,
    raw_sha256: String,
    recognized: bool,
    importable: bool,
    suggested_action: &str,
    issue_code: &str,
    issue_message: &str,
) -> WorkflowRecognitionReport {
    WorkflowRecognitionReport {
        format,
        recognized,
        importable,
        executable: false,
        identity: WorkflowIdentity::New,
        raw_sha256,
        semantic_sha256: None,
        structural_sha256: None,
        existing_workflow_id: None,
        existing_workflow_version: None,
        existing_name: None,
        category: "unknown".to_owned(),
        mode: "unknown".to_owned(),
        confidence: RecognitionConfidence::Low,
        inputs: Vec::new(),
        outputs: Vec::new(),
        recipe_status: RecipeFreshness::Missing,
        runtime_capability: RuntimeCapabilityState::NotChecked,
        capability_issues: Vec::new(),
        issues: vec![WorkflowRecognitionIssue {
            code: issue_code.to_owned(),
            message: issue_message.to_owned(),
        }],
        suggested_action: suggested_action.to_owned(),
        node_count: 0,
        unique_class_count: 0,
    }
}

fn api_error_report(
    raw_sha256: String,
    issue_code: &str,
    issue_message: impl Into<String>,
) -> WorkflowRecognitionReport {
    api_error_report_with_identity(raw_sha256, None, None, issue_code, issue_message)
}

fn api_error_report_with_identity(
    raw_sha256: String,
    semantic_sha256: Option<String>,
    structural_sha256: Option<String>,
    issue_code: &str,
    issue_message: impl Into<String>,
) -> WorkflowRecognitionReport {
    WorkflowRecognitionReport {
        format: WorkflowRecognitionFormat::Api,
        recognized: true,
        importable: false,
        executable: false,
        identity: WorkflowIdentity::New,
        raw_sha256,
        semantic_sha256,
        structural_sha256,
        existing_workflow_id: None,
        existing_workflow_version: None,
        existing_name: None,
        category: "unknown".to_owned(),
        mode: "unknown".to_owned(),
        confidence: RecognitionConfidence::Low,
        inputs: Vec::new(),
        outputs: Vec::new(),
        recipe_status: RecipeFreshness::Missing,
        runtime_capability: RuntimeCapabilityState::NotChecked,
        capability_issues: Vec::new(),
        issues: vec![WorkflowRecognitionIssue {
            code: issue_code.to_owned(),
            message: issue_message.into(),
        }],
        suggested_action: "REVIEW_RECOGNITION".to_owned(),
        node_count: 0,
        unique_class_count: 0,
    }
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

fn is_numeric_node_id(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn compare_version(left: &str, right: &str) -> std::cmp::Ordering {
    let parse = |value: &str| {
        value
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parse(left).cmp(&parse(right))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILTIN_WORKFLOW: &str = include_str!(
        "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/workflow_api.json"
    );
    const BUILTIN_MANIFEST: &str = include_str!(
        "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/manifest.yaml"
    );
    const BUILTIN_RECIPE: &str = include_str!(
        "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/recipe.yaml"
    );

    fn api(value: Value) -> Vec<u8> {
        serde_json::to_vec(&value).expect("fixture should serialize")
    }

    fn package() -> WorkflowPackageFiles {
        WorkflowPackageFiles {
            package_name: "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0".to_owned(),
            package_source_path: None,
            manifest_yaml: BUILTIN_MANIFEST.to_owned(),
            recipe_yaml: BUILTIN_RECIPE.to_owned(),
            workflow_json: BUILTIN_WORKFLOW.to_owned(),
        }
    }

    #[test]
    fn recognizes_api_ui_invalid_unknown_and_keeps_capability_separate() {
        let api_bytes = api(json!({
            "1": {"class_type": "SaveImage", "inputs": {"prompt": "hello"}}
        }));
        let report = recognize_workflow(&api_bytes, &[]);
        assert_eq!(report.format, WorkflowRecognitionFormat::Api);
        assert!(report.recognized);
        assert!(report.importable);
        assert!(!report.executable);
        assert_eq!(
            report.runtime_capability,
            RuntimeCapabilityState::NotChecked
        );
        assert!(!recognize_workflow(br#"{"#, &[]).importable);
        assert_eq!(
            recognize_workflow(br#"{"nodes":[],"links":[]}"#, &[]).format,
            WorkflowRecognitionFormat::Ui
        );
        assert_eq!(
            recognize_workflow(br#"{"foo":true}"#, &[]).format,
            WorkflowRecognitionFormat::Unknown
        );
    }

    #[test]
    fn offline_and_missing_nodes_do_not_change_importability() {
        let bytes = api(json!({
            "1": {"class_type": "SaveVideo", "inputs": {}}
        }));
        let report = recognize_workflow(&bytes, &[]);
        assert!(report.importable);
        assert!(
            !report
                .clone()
                .with_runtime_capability(RuntimeCapabilitySummary {
                    state: RuntimeCapabilityState::Offline,
                    issues: vec!["COMFY_OFFLINE".to_owned()],
                })
                .executable
        );
        assert!(
            report
                .with_runtime_capability(RuntimeCapabilitySummary {
                    state: RuntimeCapabilityState::MissingNodes,
                    issues: vec!["Missing CustomNode".to_owned()],
                })
                .importable
        );
    }

    #[test]
    fn recognizes_builtin_graph_inputs_and_mode_without_special_case() {
        let bytes = BUILTIN_WORKFLOW.as_bytes();
        let report = recognize_workflow(bytes, &[]);
        assert_eq!(report.category, "video");
        assert_eq!(report.mode, "text_to_video");
        for key in [
            "prompt",
            "duration_seconds",
            "width",
            "height",
            "seed",
            "steps",
            "denoise",
            "fps",
        ] {
            assert!(
                report.inputs.iter().any(|input| input.semantic_key == key),
                "missing inferred input {key}"
            );
        }
        assert!(report
            .outputs
            .iter()
            .any(|output| output.output_type == "video"));
    }

    #[test]
    fn exact_raw_then_semantic_then_structural_identity_is_deterministic() {
        let raw = api(json!({
            "1": {"class_type": "SaveVideo", "inputs": {"prompt": "A", "steps": 8}},
            "2": {"class_type": "Text Multiline", "inputs": {"text": "A"}}
        }));
        let workflow: Value = serde_json::from_slice(&raw).unwrap();
        let package = WorkflowPackageFiles {
            package_name: "fixture".to_owned(),
            package_source_path: None,
            manifest_yaml: "schema_version: 1\nid: wfl_fixture\nname: Fixture\nworkflow_version: 1.0.0\nrecipe_version: 1.0.0\ncategory: video\nmode: text_to_video\n".to_owned(),
            recipe_yaml: "schema_version: 1\nid: rcp_fixture\nname: Fixture\nworkflow:\n  file: workflow.json\ninputs: {}\nbindings: []\noutputs: []\n".to_owned(),
            workflow_json: serde_json::to_string_pretty(&workflow).unwrap(),
        };
        assert_eq!(
            recognize_workflow(&raw, &[package.clone()]).identity,
            WorkflowIdentity::ExactSemantic
        );

        let mut changed = workflow.clone();
        changed["1"]["inputs"]["prompt"] = json!("B");
        let changed_bytes = api(changed);
        assert_eq!(
            recognize_workflow(&changed_bytes, &[package.clone()]).identity,
            WorkflowIdentity::StructuralVariant
        );

        let mut new_node = serde_json::from_slice::<Value>(&raw).unwrap();
        new_node["3"] = json!({"class_type":"SaveVideo","inputs":{}});
        assert_eq!(
            recognize_workflow(&api(new_node), &[package]).identity,
            WorkflowIdentity::New
        );
    }

    #[test]
    fn builtin_old_recipe_is_reported_outdated_without_mutating_package() {
        let before = BUILTIN_RECIPE.to_owned();
        let report = recognize_workflow(BUILTIN_WORKFLOW.as_bytes(), &[package()]);
        assert_eq!(report.identity, WorkflowIdentity::ExactRaw);
        assert_eq!(report.recipe_status, RecipeFreshness::Outdated);
        assert_eq!(BUILTIN_RECIPE, before);
    }

    #[test]
    fn structural_fingerprint_ignores_literals_but_keeps_links_and_node_ids() {
        let first = WorkflowDocument::parse(json!({
            "1": {"class_type":"Sampler","inputs":{"steps":8}},
            "2": {"class_type":"SaveImage","inputs":{"images":["1",0]}}
        }))
        .unwrap();
        let second = WorkflowDocument::parse(json!({
            "1": {"class_type":"Sampler","inputs":{"steps":20}},
            "2": {"class_type":"SaveImage","inputs":{"images":["1",0]}}
        }))
        .unwrap();
        let changed_link = WorkflowDocument::parse(json!({
            "1": {"class_type":"Sampler","inputs":{"steps":20}},
            "2": {"class_type":"SaveImage","inputs":{"images":["1",1]}}
        }))
        .unwrap();
        assert_eq!(
            structural_workflow_sha256(&first),
            structural_workflow_sha256(&second)
        );
        assert_ne!(
            structural_workflow_sha256(&first),
            structural_workflow_sha256(&changed_link)
        );
    }
}
