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
