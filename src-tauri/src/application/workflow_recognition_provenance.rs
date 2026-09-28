use serde::{Deserialize, Serialize};

pub const WORKFLOW_RECOGNITION_ENGINE: &str = "WORKFLOW_RECOGNITION_V3";
pub const WORKFLOW_RECOGNITION_ENGINE_VERSION: &str = "3";

/// Persisted, value-free recognition context for one immutable workflow version.
/// Workflow inputs and full schema payloads intentionally do not belong here.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionProvenance {
    pub recognition_engine: String,
    pub recognition_engine_version: String,
    pub recognized_at: String,
    pub source_format: String,
    pub schema_source: String,
    pub schema_fingerprint: Option<String>,
    pub inferred_type: String,
    pub inferred_mode: String,
    pub final_type: String,
    pub final_mode: String,
    pub user_override: Option<WorkflowRecognitionUserOverride>,
    pub semantic_capability_status: String,
    pub runtime_import_status: String,
    pub output_root_state: String,
    pub selected_root_id: Option<String>,
    pub roots: Vec<WorkflowRecognitionRoot>,
    pub evidence_summary: Vec<WorkflowRecognitionEvidenceSummary>,
    #[serde(default)]
    pub input_mapping_decisions: Vec<WorkflowRecognitionInputMappingDecision>,
    pub issue_codes: Vec<String>,
    pub runtime_blockers: Vec<WorkflowRuntimeBlockerSummary>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionInputMappingDecision {
    pub semantic_key: String,
    pub item_index: Option<usize>,
    pub inferred_mapping: Option<WorkflowRecognitionMappingTarget>,
    pub final_mapping: WorkflowRecognitionMappingTarget,
    pub mapping_source: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionMappingTarget {
    pub node_id: String,
    pub input_name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionUserOverride {
    pub status: String,
    pub workflow_type: Option<WorkflowRecognitionOverrideValue>,
    pub mode: Option<WorkflowRecognitionOverrideValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionOverrideValue {
    pub inferred_value: String,
    pub selected_value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionRoot {
    pub output_id: String,
    pub output_type: String,
    pub node_id: String,
    pub label: String,
    pub evidence_tier: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecognitionEvidenceSummary {
    pub source: String,
    pub node_id: String,
    pub target: String,
    pub kind: String,
    pub weight: i32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRuntimeBlockerSummary {
    pub code: String,
    pub class_type: Option<String>,
    pub node_id: Option<String>,
    pub input_name: Option<String>,
}
