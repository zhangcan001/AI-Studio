use crate::domain::WorkflowDocument;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Returns the API workflow with object keys sorted recursively.
///
/// Arrays retain their order and scalar values are copied unchanged. This is
/// semantic identity for the API document only; it does not attempt graph
/// isomorphism or normalize node identifiers.
pub fn canonicalize_api_workflow(workflow: &WorkflowDocument) -> Value {
    canonicalize_value(workflow.value())
}

/// Computes the SHA-256 of the compact, recursively key-sorted API workflow.
pub fn semantic_workflow_sha256(workflow: &WorkflowDocument) -> String {
    let canonical_json = serde_json::to_vec(&canonicalize_api_workflow(workflow))
        .expect("serde_json::Value should always serialize");
    format!("{:x}", Sha256::digest(canonical_json))
}

fn canonicalize_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries: Vec<_> = object.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));

            let mut canonical = Map::with_capacity(entries.len());
            for (key, value) in entries {
                canonical.insert(key.clone(), canonicalize_value(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonicalize_value).collect()),
        scalar => scalar.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_api_workflow, semantic_workflow_sha256};
    use crate::domain::WorkflowDocument;
    use serde_json::{json, Value};

    fn workflow(raw: &str) -> WorkflowDocument {
        WorkflowDocument::parse(
            serde_json::from_str::<Value>(raw).expect("test workflow should be valid JSON"),
        )
        .expect("test workflow should have an object root")
    }

    #[test]
    fn s1_sorts_nested_object_keys_and_preserves_array_order_and_values() {
        let document = workflow(
            r#"{"10":{"inputs":{"steps":8,"link":["2",0]},"class_type":"Sampler"},"2":{"inputs":{"model":"model.safetensors"},"class_type":"Model"}}"#,
        );

        assert_eq!(
            serde_json::to_string(&canonicalize_api_workflow(&document)).unwrap(),
            r#"{"10":{"class_type":"Sampler","inputs":{"link":["2",0],"steps":8}},"2":{"class_type":"Model","inputs":{"model":"model.safetensors"}}}"#
        );
    }

    #[test]
    fn s2_ignores_json_whitespace() {
        let compact = workflow(r#"{"1":{"inputs":{"text":"hello world"},"class_type":"Prompt"}}"#);
        let spaced = workflow(
            r#"
            {
              "1": {
                "inputs": { "text": "hello world" },
                "class_type": "Prompt"
              }
            }
            "#,
        );

        assert_eq!(
            semantic_workflow_sha256(&compact),
            semantic_workflow_sha256(&spaced)
        );
    }

    #[test]
    fn s3_ignores_object_key_order() {
        let first = workflow(
            r#"{"2":{"class_type":"Consumer","inputs":{"value":["1",0]}},"1":{"inputs":{"steps":8,"seed":42},"class_type":"Sampler"}}"#,
        );
        let second = workflow(
            r#"{"1":{"class_type":"Sampler","inputs":{"seed":42,"steps":8}},"2":{"inputs":{"value":["1",0]},"class_type":"Consumer"}}"#,
        );

        assert_eq!(
            semantic_workflow_sha256(&first),
            semantic_workflow_sha256(&second)
        );
    }

    #[test]
    fn s4_pretty_and_compact_json_have_the_same_identity() {
        let value = json!({
            "10": {"inputs": {"steps": 8}, "class_type": "Sampler"},
            "1": {"inputs": {"seed": 42}, "class_type": "Seed"}
        });
        let compact = workflow(&serde_json::to_string(&value).unwrap());
        let pretty = workflow(&serde_json::to_string_pretty(&value).unwrap());

        assert_eq!(
            semantic_workflow_sha256(&compact),
            semantic_workflow_sha256(&pretty)
        );
    }

    #[test]
    fn s5_steps_change_changes_identity() {
        let eight_steps = workflow(r#"{"1":{"class_type":"Sampler","inputs":{"steps":8}}}"#);
        let nine_steps = workflow(r#"{"1":{"class_type":"Sampler","inputs":{"steps":9}}}"#);

        assert_ne!(
            semantic_workflow_sha256(&eight_steps),
            semantic_workflow_sha256(&nine_steps)
        );
    }

    #[test]
    fn s6_link_and_model_changes_change_identity() {
        let original = workflow(
            r#"{"1":{"class_type":"ModelLoader","inputs":{"model":"model-a.safetensors"}},"2":{"class_type":"Consumer","inputs":{"model":["1",0]}}}"#,
        );
        let changed_link = workflow(
            r#"{"1":{"class_type":"ModelLoader","inputs":{"model":"model-a.safetensors"}},"2":{"class_type":"Consumer","inputs":{"model":["1",1]}}}"#,
        );
        let changed_model = workflow(
            r#"{"1":{"class_type":"ModelLoader","inputs":{"model":"model-b.safetensors"}},"2":{"class_type":"Consumer","inputs":{"model":["1",0]}}}"#,
        );

        assert_ne!(
            semantic_workflow_sha256(&original),
            semantic_workflow_sha256(&changed_link)
        );
        assert_ne!(
            semantic_workflow_sha256(&original),
            semantic_workflow_sha256(&changed_model)
        );
    }

    #[test]
    fn s7_node_id_class_type_and_input_changes_change_identity() {
        let original = workflow(r#"{"1":{"class_type":"Sampler","inputs":{"cfg":7.5}}}"#);
        let changed_node_id = workflow(r#"{"2":{"class_type":"Sampler","inputs":{"cfg":7.5}}}"#);
        let changed_class_type =
            workflow(r#"{"1":{"class_type":"DifferentSampler","inputs":{"cfg":7.5}}}"#);
        let changed_input = workflow(r#"{"1":{"class_type":"Sampler","inputs":{"cfg":8.0}}}"#);

        assert_ne!(
            semantic_workflow_sha256(&original),
            semantic_workflow_sha256(&changed_node_id)
        );
        assert_ne!(
            semantic_workflow_sha256(&original),
            semantic_workflow_sha256(&changed_class_type)
        );
        assert_ne!(
            semantic_workflow_sha256(&original),
            semantic_workflow_sha256(&changed_input)
        );
    }
}
