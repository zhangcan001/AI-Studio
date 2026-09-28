use serde::{Deserialize, Serialize};

pub const WORKFLOW_RECOGNITION_ENGINE: &str = "WORKFLOW_RECOGNITION_V3";
pub const WORKFLOW_RECOGNITION_ENGINE_VERSION: &str = "3";

/// Persisted, value-free recognition context for one immutable workflow version.
/// Workflow inputs and full schema payloads intentionally do not belong here.
///
/// Every field defaults when absent so metadata written by older (or newer)
/// builds and partial third-party packages still deserialize. Readers treat an
/// empty `recognition_engine` as "unknown engine" instead of an integrity error.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
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
    pub input_mapping_decisions: Vec<WorkflowRecognitionInputMappingDecision>,
    pub issue_codes: Vec<String>,
    pub runtime_blockers: Vec<WorkflowRuntimeBlockerSummary>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRecognitionInputMappingDecision {
    pub semantic_key: String,
    pub item_index: Option<usize>,
    pub inferred_mapping: Option<WorkflowRecognitionMappingTarget>,
    pub final_mapping: WorkflowRecognitionMappingTarget,
    pub mapping_source: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRecognitionMappingTarget {
    pub node_id: String,
    pub input_name: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRecognitionUserOverride {
    pub status: String,
    pub workflow_type: Option<WorkflowRecognitionOverrideValue>,
    pub mode: Option<WorkflowRecognitionOverrideValue>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRecognitionOverrideValue {
    pub inferred_value: String,
    pub selected_value: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRecognitionRoot {
    pub output_id: String,
    pub output_type: String,
    pub node_id: String,
    pub label: String,
    pub evidence_tier: u8,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRecognitionEvidenceSummary {
    pub source: String,
    pub node_id: String,
    pub target: String,
    pub kind: String,
    pub weight: i32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRuntimeBlockerSummary {
    pub code: String,
    pub class_type: Option<String>,
    pub node_id: Option<String>,
    pub input_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r10_provenance_deserializes_minimal_json() {
        let provenance: WorkflowRecognitionProvenance =
            serde_json::from_str(r#"{"recognitionEngine":"WORKFLOW_RECOGNITION_V3"}"#)
                .expect("minimal provenance should deserialize");
        assert_eq!(provenance.recognition_engine, WORKFLOW_RECOGNITION_ENGINE);
        assert!(provenance.roots.is_empty());
        assert!(provenance.issue_codes.is_empty());
        assert!(provenance.user_override.is_none());
    }

    #[test]
    fn r10_provenance_tolerates_partial_nested_entries_and_unknown_fields() {
        let provenance: WorkflowRecognitionProvenance = serde_json::from_str(
            r#"{
                "roots": [{"outputId": "image"}],
                "runtimeBlockers": [{"code": "MISSING_NODE"}],
                "inputMappingDecisions": [{"semanticKey": "prompt"}],
                "futureField": {"nested": true}
            }"#,
        )
        .expect("partial nested provenance should deserialize");
        assert_eq!(provenance.roots[0].output_id, "image");
        assert_eq!(provenance.roots[0].evidence_tier, 0);
        assert_eq!(provenance.runtime_blockers[0].code, "MISSING_NODE");
        assert_eq!(provenance.input_mapping_decisions[0].semantic_key, "prompt");
        assert!(provenance.recognition_engine.is_empty());
    }
}
