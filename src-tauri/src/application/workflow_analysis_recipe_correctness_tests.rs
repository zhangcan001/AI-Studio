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
