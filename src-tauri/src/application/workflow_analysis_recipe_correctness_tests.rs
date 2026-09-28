//! Regression tests for the PR-A recipe-correctness fixes (W-02, R-01..R-08).
use super::*;
use crate::application::workflow_recognition_schema::RecognitionSchemaContext;
use serde_json::{json, Value};
use std::sync::OnceLock;

/// The shared real ComfyUI object_info snapshot used by the V3 replay corpus.
pub(super) fn real_schema() -> &'static RecognitionSchemaContext {
    static SCHEMA: OnceLock<RecognitionSchemaContext> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("workflow_recognition_v3")
            .join("real")
            .join("object_info.json");
        let bytes = std::fs::read(path).expect("real object_info fixture should exist");
        let value: Value = serde_json::from_slice(&bytes).expect("object_info should parse");
        RecognitionSchemaContext::parse(&value)
    })
}

pub(super) fn analyze_real(workflow: Value) -> WorkflowAnalysisReport {
    let bytes = serde_json::to_vec(&workflow).unwrap();
    let document = WorkflowDocument::parse(workflow).unwrap();
    analyze_workflow_with_schema(&document, &bytes, Some(real_schema()))
}

pub(super) fn binding(report: &WorkflowAnalysisReport, key: &str) -> Option<(String, String)> {
    report
        .inputs
        .iter()
        .find(|input| input.semantic_key == key)
        .map(|input| (input.node_id.clone(), input.input_name.clone()))
}

#[test]
fn r07_evidence_kind_serializes_screaming_snake() {
    assert_eq!(
        serde_json::to_value(EvidenceKind::GRAPH_OUTPUT_PATH).unwrap(),
        json!("GRAPH_OUTPUT_PATH")
    );
    assert_eq!(
        serde_json::to_value(EvidenceKind::INPUT_NAME_ALIAS).unwrap(),
        json!("INPUT_NAME_ALIAS")
    );
    let evidence = serde_json::to_value(evidence(EvidenceKind::EXACT_INPUT_NAME, "x")).unwrap();
    assert_eq!(evidence["kind"], json!("EXACT_INPUT_NAME"));
}

/// Minimal SD1.5 text-to-image graph against the real schema.
pub(super) fn basic_t2i() -> Value {
    json!({
        "4": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": "model.safetensors"}},
        "5": {"class_type": "EmptyLatentImage", "inputs": {"width": 512, "height": 512, "batch_size": 1}},
        "6": {"class_type": "CLIPTextEncode", "inputs": {"text": "a cat", "clip": ["4", 1]}},
        "7": {"class_type": "CLIPTextEncode", "inputs": {"text": "blurry", "clip": ["4", 1]}},
        "3": {"class_type": "KSampler", "inputs": {
            "seed": 1, "steps": 20, "cfg": 7.0, "sampler_name": "euler", "scheduler": "normal",
            "denoise": 1.0, "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0],
            "latent_image": ["5", 0]}},
        "8": {"class_type": "VAEDecode", "inputs": {"samples": ["3", 0], "vae": ["4", 2]}},
        "9": {"class_type": "SaveImage", "inputs": {"filename_prefix": "out", "images": ["8", 0]}}
    })
}

fn issue_codes(report: &WorkflowAnalysisReport) -> Vec<String> {
    report
        .issues
        .iter()
        .map(|issue| issue.code.clone())
        .collect()
}

#[test]
fn r08_valid_links_produce_no_link_issues() {
    let report = analyze_real(basic_t2i());
    let codes = issue_codes(&report);
    assert!(
        !codes.iter().any(|code| code.starts_with("WORKFLOW_LINK_")),
        "unexpected link issues: {codes:?}"
    );
}

#[test]
fn r08_output_slot_out_of_range_is_reported() {
    let mut workflow = basic_t2i();
    workflow["8"]["inputs"]["vae"] = json!(["4", 5]);
    let report = analyze_real(workflow);
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == "WORKFLOW_LINK_SLOT_OUT_OF_RANGE")
        .expect("slot out of range should be reported");
    assert_eq!(issue.field.as_deref(), Some("8.vae"));
}

#[test]
fn r08_output_type_mismatch_is_reported() {
    let mut workflow = basic_t2i();
    workflow["3"]["inputs"]["model"] = json!(["4", 1]);
    let report = analyze_real(workflow);
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == "WORKFLOW_LINK_TYPE_MISMATCH")
        .expect("type mismatch should be reported");
    assert_eq!(issue.field.as_deref(), Some("3.model"));
}

fn real_sample(sample: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("workflow_recognition_v3")
        .join("real")
        .join(sample)
        .join("sample.api.json");
    serde_json::from_slice(&std::fs::read(path).expect("sample should exist")).unwrap()
}

fn all_candidate_targets(report: &WorkflowAnalysisReport, key: &str) -> Vec<(String, String)> {
    let mut targets = report
        .inputs
        .iter()
        .filter(|input| input.semantic_key == key)
        .map(|input| (input.node_id.clone(), input.input_name.clone()))
        .collect::<Vec<_>>();
    for issue in report
        .issues
        .iter()
        .filter(|issue| issue.field.as_deref() == Some(key))
    {
        for candidate in &issue.candidates {
            if let (Some(node), Some(input)) = (&candidate.node_id, &candidate.input_name) {
                targets.push((node.clone(), input.clone()));
            }
        }
    }
    targets
}

#[test]
fn w02_basic_t2i_binds_positive_and_negative_by_sampler_role() {
    let report = analyze_real(basic_t2i());
    assert_eq!(
        binding(&report, "prompt"),
        Some(("6".into(), "text".into()))
    );
    assert_eq!(
        binding(&report, "negative_prompt"),
        Some(("7".into(), "text".into()))
    );
    let negative = report
        .inputs
        .iter()
        .find(|input| input.semantic_key == "negative_prompt")
        .unwrap();
    assert!(negative
        .evidence
        .iter()
        .any(|item| item.kind == EvidenceKind::GRAPH_CONDITIONING_ROLE));
}

#[test]
fn w02_combo_input_is_never_a_prompt_candidate() {
    let report = analyze_real(basic_t2i());
    for key in ["prompt", "negative_prompt"] {
        assert!(
            !all_candidate_targets(&report, key)
                .iter()
                .any(|(_, input)| input == "ckpt_name"),
            "{key} must not consider the checkpoint combo"
        );
    }
    let text_on_combo = declared_type_for_field("textarea");
    assert!(!text_on_combo.contains(&RecognitionDeclaredType::Enum));
}

#[test]
fn w02_negative_prompt_aliases_resolve_to_negative() {
    for name in [
        "neg",
        "neg_prompt",
        "negative_text",
        "neg_text",
        "uncond",
        "unconditional",
    ] {
        assert_eq!(
            canonical_semantic_hint(name).map(|hint| hint.semantic),
            Some(CanonicalSemantic::NegativePrompt),
            "{name}"
        );
    }
    assert!(canonical_semantic_hint("system_prompt").is_none());
}

#[test]
fn r03_delimiter_is_never_a_prompt_candidate_and_user_prompt_wins() {
    let report = analyze_real(real_sample("R05_KREA2_T2I"));
    assert!(!all_candidate_targets(&report, "prompt")
        .iter()
        .any(|(_, input)| input == "delimiter"));
    assert_eq!(
        binding(&report, "prompt"),
        Some(("30:19".into(), "value".into())),
        "issues: {:?}",
        report.issues
    );
}

#[test]
fn r05_system_prompt_is_not_selected_and_negative_zero_out_has_no_text() {
    let report = analyze_real(real_sample("R05_KREA2_T2I"));
    assert_ne!(
        binding(&report, "prompt"),
        Some(("30:18".into(), "value".into()))
    );
    // KSampler.negative comes from ConditioningZeroOut(positive), so no text
    // leaf may be exposed as the negative prompt.
    assert!(binding(&report, "negative_prompt").is_none());
}

#[test]
fn r04_controlnet_router_keeps_positive_and_negative_branches_apart() {
    let mut workflow = basic_t2i();
    workflow["10"] =
        json!({"class_type": "ControlNetLoader", "inputs": {"control_net_name": "cn.safetensors"}});
    workflow["11"] = json!({"class_type": "LoadImage", "inputs": {"image": "pose.png"}});
    workflow["12"] = json!({"class_type": "ControlNetApplyAdvanced", "inputs": {
        "positive": ["6", 0], "negative": ["7", 0], "control_net": ["10", 0], "image": ["11", 0],
        "strength": 1.0, "start_percent": 0.0, "end_percent": 1.0}});
    workflow["3"]["inputs"]["positive"] = json!(["12", 0]);
    workflow["3"]["inputs"]["negative"] = json!(["12", 1]);
    let report = analyze_real(workflow.clone());
    assert_eq!(
        binding(&report, "prompt"),
        Some(("6".into(), "text".into()))
    );
    assert_eq!(
        binding(&report, "negative_prompt"),
        Some(("7".into(), "text".into()))
    );

    let document = WorkflowDocument::parse(workflow).unwrap();
    let graph = WorkflowGraph::from_document(&document).unwrap();
    let roles = sampler_conditioning_roles(&document, &graph, Some(real_schema()));
    assert_eq!(
        roles.get(&("6".to_owned(), "text".to_owned())),
        Some(&BTreeSet::from([PromptRole::Positive]))
    );
    assert_eq!(
        roles.get(&("7".to_owned(), "text".to_owned())),
        Some(&BTreeSet::from([PromptRole::Negative]))
    );
    assert!(!roles.contains_key(&("4".to_owned(), "ckpt_name".to_owned())));
}
