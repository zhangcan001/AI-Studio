use crate::compiler::CompileError;
use crate::domain::Recipe;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const INTERNAL_PLACEHOLDER_MARKERS: [&str; 2] =
    ["__AI_STUDIO_OPTIONAL__", "__aistudio_preflight_image__"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledMediaMapping {
    pub input_key: String,
    pub target_node: String,
    pub target_input: String,
    pub media_kind: String,
    pub reference_index: Option<usize>,
    pub asset_ids: Vec<String>,
    pub uploaded_identities: Vec<String>,
}

pub struct FinalCompiledWorkflowValidator;

impl FinalCompiledWorkflowValidator {
    pub fn validate(
        workflow: &Value,
        recipe: &Recipe,
        media_mappings: &[CompiledMediaMapping],
    ) -> Result<(), CompileError> {
        let nodes = workflow
            .as_object()
            .ok_or_else(|| CompileError::CompiledGraphInvalid {
                message: "compiled workflow root must be a JSON object".to_owned(),
            })?;

        validate_internal_placeholders(workflow)?;
        validate_node_links(nodes)?;
        validate_media_mappings(workflow, media_mappings)?;
        validate_output_nodes(nodes, recipe)?;

        Ok(())
    }

    pub fn sha256(workflow: &Value) -> Result<String, CompileError> {
        let canonical = canonicalize(workflow);
        let bytes =
            serde_json::to_vec(&canonical).map_err(|error| CompileError::CompiledGraphInvalid {
                message: format!("compiled workflow serialization failed: {error}"),
            })?;
        let digest = Sha256::digest(bytes);
        Ok(format!("{digest:x}"))
    }
}

pub fn compiled_workflow_sha256(workflow: &Value) -> Result<String, CompileError> {
    FinalCompiledWorkflowValidator::sha256(workflow)
}

fn validate_internal_placeholders(workflow: &Value) -> Result<(), CompileError> {
    let mut path = String::from("$");
    if let Some((path, marker)) = find_placeholder(workflow, &mut path) {
        return Err(CompileError::CompiledInternalPlaceholder {
            path,
            marker: marker.to_owned(),
        });
    }
    Ok(())
}

fn find_placeholder<'a>(value: &'a Value, path: &mut String) -> Option<(String, &'a str)> {
    match value {
        Value::String(text) => INTERNAL_PLACEHOLDER_MARKERS
            .iter()
            .copied()
            .find(|marker| text.contains(marker))
            .map(|marker| (path.clone(), marker)),
        Value::Array(values) => values.iter().enumerate().find_map(|(index, value)| {
            let original_len = path.len();
            path.push('[');
            path.push_str(&index.to_string());
            path.push(']');
            let found = find_placeholder(value, path);
            path.truncate(original_len);
            found
        }),
        Value::Object(values) => values.iter().find_map(|(key, value)| {
            if INTERNAL_PLACEHOLDER_MARKERS
                .iter()
                .any(|marker| key.contains(marker))
            {
                return Some((format!("{path}.{key} (key)"), key.as_str()));
            }
            let original_len = path.len();
            path.push('.');
            path.push_str(key);
            let found = find_placeholder(value, path);
            path.truncate(original_len);
            found
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

fn validate_node_links(nodes: &Map<String, Value>) -> Result<(), CompileError> {
    for (source_node, node) in nodes {
        let Some(inputs) = node.get("inputs").and_then(Value::as_object) else {
            return Err(CompileError::CompiledGraphInvalid {
                message: format!("compiled node {source_node} is missing an inputs object"),
            });
        };
        for (input_name, value) in inputs {
            validate_links_in_value(nodes, source_node, input_name, value)?;
        }
    }
    Ok(())
}

fn validate_links_in_value(
    nodes: &Map<String, Value>,
    source_node: &str,
    input_name: &str,
    value: &Value,
) -> Result<(), CompileError> {
    if let Some((referenced_node, output_index)) = possible_link(value) {
        if output_index < 0 {
            return Err(CompileError::CompiledGraphInvalid {
                message: format!(
                    "node {source_node} input {input_name} contains a negative output index"
                ),
            });
        }
        if !nodes.contains_key(referenced_node) {
            return Err(CompileError::CompiledDanglingNodeReference {
                source_node: source_node.to_owned(),
                input_name: input_name.to_owned(),
                referenced_node: referenced_node.to_owned(),
                output_index,
            });
        }
        return Ok(());
    }

    if let Some(values) = value.as_array() {
        for child in values {
            validate_links_in_value(nodes, source_node, input_name, child)?;
        }
    } else if let Some(values) = value.as_object() {
        for child in values.values() {
            validate_links_in_value(nodes, source_node, input_name, child)?;
        }
    }
    Ok(())
}

fn possible_link(value: &Value) -> Option<(&str, i64)> {
    let array = value.as_array()?;
    if array.len() != 2 {
        return None;
    }
    let node_id = array.first()?.as_str()?;
    if !node_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let output_index = array.get(1)?.as_i64()?;
    Some((node_id, output_index))
}

fn validate_media_mappings(
    workflow: &Value,
    mappings: &[CompiledMediaMapping],
) -> Result<(), CompileError> {
    for mapping in mappings {
        if mapping.asset_ids.len() != mapping.uploaded_identities.len()
            || mapping.asset_ids.is_empty()
        {
            return Err(CompileError::CompiledMediaBindingIncomplete {
                asset_id: mapping.asset_ids.first().cloned().unwrap_or_default(),
                media_kind: mapping.media_kind.clone(),
                reference_index: mapping.reference_index,
                input_key: mapping.input_key.clone(),
                expected_target: format!("{} uploaded media value(s)", mapping.asset_ids.len()),
                actual_target: format!(
                    "{} uploaded identity value(s)",
                    mapping.uploaded_identities.len()
                ),
            });
        }

        let actual = workflow
            .get(&mapping.target_node)
            .and_then(Value::as_object)
            .and_then(|node| node.get("inputs"))
            .and_then(Value::as_object)
            .and_then(|inputs| inputs.get(&mapping.target_input));
        let expected = if mapping.uploaded_identities.len() == 1 {
            Value::String(mapping.uploaded_identities[0].clone())
        } else {
            Value::Array(
                mapping
                    .uploaded_identities
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            )
        };

        if actual != Some(&expected) {
            return Err(CompileError::CompiledMediaBindingIncomplete {
                asset_id: mapping.asset_ids.first().cloned().unwrap_or_default(),
                media_kind: mapping.media_kind.clone(),
                reference_index: mapping.reference_index,
                input_key: mapping.input_key.clone(),
                expected_target: expected.to_string(),
                actual_target: actual.map_or_else(|| "<missing>".to_owned(), Value::to_string),
            });
        }
    }
    Ok(())
}

fn validate_output_nodes(nodes: &Map<String, Value>, recipe: &Recipe) -> Result<(), CompileError> {
    for output in recipe.outputs.iter().filter(|output| output.required) {
        if !nodes.contains_key(&output.node) {
            return Err(CompileError::CompiledOutputNodeMissing {
                output_id: output.id.clone(),
                node: output.node.clone(),
            });
        }
        let mut visited = BTreeSet::new();
        visit_output_graph(nodes, &output.node, &mut visited)?;
    }
    Ok(())
}

fn visit_output_graph(
    nodes: &Map<String, Value>,
    node_id: &str,
    visited: &mut BTreeSet<String>,
) -> Result<(), CompileError> {
    if !visited.insert(node_id.to_owned()) {
        return Ok(());
    }
    let node = nodes
        .get(node_id)
        .and_then(Value::as_object)
        .ok_or_else(|| CompileError::CompiledGraphInvalid {
            message: format!("output graph node {node_id} is not an object"),
        })?;
    let Some(inputs) = node.get("inputs").and_then(Value::as_object) else {
        return Err(CompileError::CompiledGraphInvalid {
            message: format!("output graph node {node_id} is missing inputs"),
        });
    };
    for value in inputs.values() {
        visit_links(nodes, value, visited)?;
    }
    Ok(())
}

fn visit_links(
    nodes: &Map<String, Value>,
    value: &Value,
    visited: &mut BTreeSet<String>,
) -> Result<(), CompileError> {
    if let Some((node_id, _)) = possible_link(value) {
        visit_output_graph(nodes, node_id, visited)?;
    } else if let Some(values) = value.as_array() {
        for child in values {
            visit_links(nodes, child, visited)?;
        }
    } else if let Some(values) = value.as_object() {
        for child in values.values() {
            visit_links(nodes, child, visited)?;
        }
    }
    Ok(())
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted: BTreeMap<_, _> = object
                .iter()
                .map(|(key, value)| (key.clone(), canonicalize(value)))
                .collect();
            let mut map = Map::new();
            for (key, value) in sorted {
                map.insert(key, value);
            }
            Value::Object(map)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{compiled_workflow_sha256, CompiledMediaMapping, FinalCompiledWorkflowValidator};
    use crate::compiler::RecipeParser;
    use serde_json::json;

    fn recipe() -> crate::domain::Recipe {
        RecipeParser::parse(
            r#"
schema_version: 1
id: final_validator
name: Final Validator
workflow:
  file: workflow.json
inputs:
  image:
    type: image
    label: Image
    required: false
bindings:
  - source: image
    target:
      node: "1"
      input: image
outputs:
  - id: output
    type: image
    node: "3"
    required: true
"#,
        )
        .expect("recipe should parse")
    }

    fn valid_workflow() -> serde_json::Value {
        json!({
            "1": {"inputs": {"image": "uploaded.png"}, "class_type": "LoadImage"},
            "2": {"inputs": {"image": ["1", 0]}, "class_type": "Process"},
            "3": {"inputs": {"images": ["2", 0]}, "class_type": "SaveImage"}
        })
    }

    #[test]
    fn valid_output_graph_passes() {
        FinalCompiledWorkflowValidator::validate(&valid_workflow(), &recipe(), &[])
            .expect("valid graph should pass");
    }

    #[test]
    fn normal_krea2_compiled_workflow_passes() {
        FinalCompiledWorkflowValidator::validate(&valid_workflow(), &recipe(), &[])
            .expect("Krea2 graph should pass");
    }

    #[test]
    fn h3_t2v_after_optional_frame_pruning_passes() {
        let recipe = RecipeParser::parse(
            r#"
schema_version: 1
id: h3_t2v
name: H3 T2V
workflow:
  file: workflow.json
inputs: {}
bindings: []
outputs:
  - id: video
    type: video
    node: "3"
    required: true
"#,
        )
        .unwrap();
        let workflow = json!({
            "3": {"inputs": {"prompt": "text"}, "class_type": "SaveVideo"}
        });
        FinalCompiledWorkflowValidator::validate(&workflow, &recipe, &[])
            .expect("pruned optional frame graph should pass");
    }

    #[test]
    fn h3_i2v_first_frame_binding_passes() {
        let mapping = CompiledMediaMapping {
            input_key: "first_frame".to_owned(),
            target_node: "1".to_owned(),
            target_input: "image".to_owned(),
            media_kind: "image".to_owned(),
            reference_index: None,
            asset_ids: vec!["ast_first".to_owned()],
            uploaded_identities: vec!["uploaded-first.png".to_owned()],
        };
        let mut workflow = valid_workflow();
        workflow["1"]["inputs"]["image"] = json!("uploaded-first.png");
        FinalCompiledWorkflowValidator::validate(&workflow, &recipe(), &[mapping])
            .expect("I2V first frame should pass");
    }

    #[test]
    fn h3_first_last_frame_bindings_pass() {
        let recipe = RecipeParser::parse(
            r#"
schema_version: 1
id: h3_first_last
name: H3 First Last
workflow:
  file: workflow.json
inputs:
  first_frame:
    type: image
    label: First
    required: false
  last_frame:
    type: image
    label: Last
    required: false
bindings:
  - source: first_frame
    target:
      node: "1"
      input: image
  - source: last_frame
    target:
      node: "2"
      input: image
outputs:
  - id: video
    type: video
    node: "3"
    required: true
"#,
        )
        .unwrap();
        let workflow = json!({
            "1": {"inputs": {"image": "first.png"}, "class_type": "LoadImage"},
            "2": {"inputs": {"image": "last.png"}, "class_type": "LoadImage"},
            "3": {"inputs": {"first": ["1", 0], "last": ["2", 0]}, "class_type": "SaveVideo"}
        });
        let mappings = vec![
            CompiledMediaMapping {
                input_key: "first_frame".to_owned(),
                target_node: "1".to_owned(),
                target_input: "image".to_owned(),
                media_kind: "image".to_owned(),
                reference_index: None,
                asset_ids: vec!["ast_first".to_owned()],
                uploaded_identities: vec!["first.png".to_owned()],
            },
            CompiledMediaMapping {
                input_key: "last_frame".to_owned(),
                target_node: "2".to_owned(),
                target_input: "image".to_owned(),
                media_kind: "image".to_owned(),
                reference_index: None,
                asset_ids: vec!["ast_last".to_owned()],
                uploaded_identities: vec!["last.png".to_owned()],
            },
        ];
        FinalCompiledWorkflowValidator::validate(&workflow, &recipe, &mappings)
            .expect("first and last bindings should pass");
    }

    #[test]
    fn ref2va_three_image_order_is_verified_at_target() {
        let recipe = RecipeParser::parse(
            r#"
schema_version: 1
id: ref2va
name: REF2VA
workflow:
  file: workflow.json
inputs:
  references:
    type: images
    label: References
    required: true
    min_items: 3
    max_items: 3
bindings:
  - source: references
    target:
      node: "1"
      input: images
outputs:
  - id: video
    type: video
    node: "3"
    required: true
"#,
        )
        .unwrap();
        let workflow = json!({
            "1": {"inputs": {"images": ["a.png", "b.png", "c.png"]}, "class_type": "ReferenceImages"},
            "3": {"inputs": {"references": ["1", 0]}, "class_type": "SaveVideo"}
        });
        let mapping = CompiledMediaMapping {
            input_key: "references".to_owned(),
            target_node: "1".to_owned(),
            target_input: "images".to_owned(),
            media_kind: "image".to_owned(),
            reference_index: None,
            asset_ids: vec!["ast_a".to_owned(), "ast_b".to_owned(), "ast_c".to_owned()],
            uploaded_identities: vec!["a.png".to_owned(), "b.png".to_owned(), "c.png".to_owned()],
        };
        FinalCompiledWorkflowValidator::validate(&workflow, &recipe, &[mapping])
            .expect("REF2VA order should pass");
    }

    #[test]
    fn dangling_reference_is_classified() {
        let mut workflow = valid_workflow();
        workflow["2"]["inputs"]["image"] = json!(["999", 0]);
        let error = FinalCompiledWorkflowValidator::validate(&workflow, &recipe(), &[])
            .expect_err("dangling reference should fail");
        assert_eq!(error.code(), "COMPILED_DANGLING_NODE_REFERENCE");
        assert!(error.to_string().contains("999"));
    }

    #[test]
    fn clear_targets_after_optional_pruning_cannot_leave_a_dangling_link() {
        let mut workflow = valid_workflow();
        workflow["3"]["inputs"]["images"] = json!(["24", 0]);
        let error = FinalCompiledWorkflowValidator::validate(&workflow, &recipe(), &[])
            .expect_err("a pruned optional node must not remain referenced");
        assert_eq!(error.code(), "COMPILED_DANGLING_NODE_REFERENCE");
    }

    #[test]
    fn internal_placeholders_are_rejected_anywhere() {
        let mut workflow = valid_workflow();
        workflow["1"]["inputs"]["image"] = json!("__AI_STUDIO_OPTIONAL__.png");
        let error = FinalCompiledWorkflowValidator::validate(&workflow, &recipe(), &[])
            .expect_err("placeholder should fail");
        assert_eq!(error.code(), "COMPILED_INTERNAL_PLACEHOLDER");

        workflow["1"]["inputs"]["image"] = json!(["__aistudio_preflight_image__", 0]);
        let error = FinalCompiledWorkflowValidator::validate(&workflow, &recipe(), &[])
            .expect_err("nested placeholder should fail");
        assert_eq!(error.code(), "COMPILED_INTERNAL_PLACEHOLDER");
    }

    #[test]
    fn preflight_placeholder_is_rejected_as_a_separate_invariant() {
        let mut workflow = valid_workflow();
        workflow["1"]["inputs"]["image"] = json!("__aistudio_preflight_image__.png");
        let error = FinalCompiledWorkflowValidator::validate(&workflow, &recipe(), &[])
            .expect_err("preflight placeholder should fail");
        assert_eq!(error.code(), "COMPILED_INTERNAL_PLACEHOLDER");
    }

    #[test]
    fn output_node_missing_is_classified() {
        let mut workflow = valid_workflow();
        workflow.as_object_mut().unwrap().remove("3");
        let error = FinalCompiledWorkflowValidator::validate(&workflow, &recipe(), &[])
            .expect_err("missing output should fail");
        assert_eq!(error.code(), "COMPILED_OUTPUT_NODE_MISSING");
    }

    #[test]
    fn missing_media_target_is_classified() {
        let mapping = CompiledMediaMapping {
            input_key: "image".to_owned(),
            target_node: "1".to_owned(),
            target_input: "missing".to_owned(),
            media_kind: "image".to_owned(),
            reference_index: None,
            asset_ids: vec!["ast_1".to_owned()],
            uploaded_identities: vec!["uploaded.png".to_owned()],
        };
        let error =
            FinalCompiledWorkflowValidator::validate(&valid_workflow(), &recipe(), &[mapping])
                .expect_err("missing target should fail");
        assert_eq!(error.code(), "COMPILED_MEDIA_BINDING_INCOMPLETE");
    }

    #[test]
    fn missing_manifest_asset_identity_is_classified() {
        let mapping = CompiledMediaMapping {
            input_key: "image".to_owned(),
            target_node: "1".to_owned(),
            target_input: "image".to_owned(),
            media_kind: "image".to_owned(),
            reference_index: Some(1),
            asset_ids: vec!["ast_1".to_owned(), "ast_2".to_owned()],
            uploaded_identities: vec!["uploaded.png".to_owned()],
        };
        let error =
            FinalCompiledWorkflowValidator::validate(&valid_workflow(), &recipe(), &[mapping])
                .expect_err("manifest and upload identity counts must match");
        assert_eq!(error.code(), "COMPILED_MEDIA_BINDING_INCOMPLETE");
    }

    #[test]
    fn negative_output_index_is_graph_invalid() {
        let mut workflow = valid_workflow();
        workflow["2"]["inputs"]["image"] = json!(["1", -1]);
        let error = FinalCompiledWorkflowValidator::validate(&workflow, &recipe(), &[])
            .expect_err("negative output index should fail");
        assert_eq!(error.code(), "COMPILED_GRAPH_INVALID");
    }

    #[test]
    fn media_mapping_requires_uploaded_identity_at_target() {
        let mapping = CompiledMediaMapping {
            input_key: "image".to_owned(),
            target_node: "1".to_owned(),
            target_input: "image".to_owned(),
            media_kind: "image".to_owned(),
            reference_index: None,
            asset_ids: vec!["ast_1".to_owned()],
            uploaded_identities: vec!["other.png".to_owned()],
        };
        let error =
            FinalCompiledWorkflowValidator::validate(&valid_workflow(), &recipe(), &[mapping])
                .expect_err("wrong uploaded identity should fail");
        assert_eq!(error.code(), "COMPILED_MEDIA_BINDING_INCOMPLETE");
    }

    #[test]
    fn compiled_sha_is_deterministic_and_changes_with_input() {
        let first = compiled_workflow_sha256(&valid_workflow()).expect("sha should compute");
        let second = compiled_workflow_sha256(&valid_workflow()).expect("sha should repeat");
        assert_eq!(first, second);
        let mut changed = valid_workflow();
        changed["2"]["inputs"]["strength"] = json!(0.5);
        assert_ne!(first, compiled_workflow_sha256(&changed).unwrap());
    }
}
