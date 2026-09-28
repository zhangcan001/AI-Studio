use ai_studio_lib::application::workflow_recognition_schema::RecognitionSchemaContext;
use ai_studio_lib::application::workflow_ui_normalizer::{
    normalize_ui_workflow, parse_ui_workflow_value,
};
use ai_studio_lib::application::workflow_ui_serialization::{
    FrontendSerializationContract, FrontendSerializationProfile, NormalizationCompatibilityContext,
    UiSerializationDescriptorSet, WidgetBindingKind, NORMALIZER_POLICY_VERSION,
    SERIALIZATION_PROFILE_VERSION, SUPPORTED_WORKFLOW_FORMAT_VERSION,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn dynamic_schema() -> Value {
    json!({
        "Source": {
            "input": {"required": {}},
            "output": ["IMAGE"],
            "output_name": ["IMAGE"]
        },
        "DynamicNode": {
            "input": {
                "required": {
                    "select": ["COMFY_DYNAMICCOMBO_V3", {
                        "options": [
                            {"key": "1", "inputs": {
                                "required": {
                                    "weight_1": ["FLOAT", {"default": 1.0}]
                                },
                                "optional": {
                                    "image_1": ["IMAGE", {}]
                                }
                            }},
                            {"key": "2", "inputs": {
                                "required": {
                                    "weight_1": ["FLOAT", {"default": 1.0}],
                                    "weight_2": ["FLOAT", {"default": 1.0}]
                                },
                                "optional": {
                                    "image_1": ["IMAGE", {}],
                                    "image_2": ["IMAGE", {}]
                                }
                            }}
                        ]
                    }]
                }
            },
            "input_order": {"required": ["select"]},
            "output": ["LATENT"],
            "output_name": ["LATENT"]
        }
    })
}

fn dynamic_socket_schema(second_base: bool) -> Value {
    let mut required = serde_json::Map::new();
    for base in if second_base {
        vec!["left", "right"]
    } else {
        vec!["left"]
    } {
        required.insert(
            base.to_owned(),
            json!(["COMFY_AUTOGROW_V3", {
                "template": {
                    "prefix": "image_",
                    "input": {"image_1": ["IMAGE", {}]}
                }
            }]),
        );
    }
    json!({
        "Source": {
            "input": {"required": {}},
            "output": ["IMAGE"],
            "output_name": ["IMAGE"]
        },
        "NodeAlpha": {
            "input": {"optional": required},
            "output": []
        }
    })
}

fn dynamic_descriptors_for(
    schema_value: &Value,
    frontend_version: &str,
) -> UiSerializationDescriptorSet {
    let schema = RecognitionSchemaContext::parse(&schema_value);
    let context = NormalizationCompatibilityContext {
        workflow_format_version: SUPPORTED_WORKFLOW_FORMAT_VERSION.to_owned(),
        frontend_version: frontend_version.to_owned(),
        schema_fingerprint: "v1d3-dynamic".to_owned(),
        serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
        normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
        source_frontend_revision: None,
    };
    let profile = FrontendSerializationProfile::legacy_dynamic_input_v0_from_context(&context)
        .expect("legacy dynamic profile should be constructible");
    UiSerializationDescriptorSet::build(&schema, profile, context)
        .expect("dynamic schema descriptors should build")
}

fn dynamic_descriptors() -> UiSerializationDescriptorSet {
    dynamic_descriptors_for(&dynamic_schema(), "1.43.1")
}

fn normalize_with(
    workflow: &Value,
    schema: &Value,
    frontend_version: &str,
) -> Result<Value, String> {
    let document = parse_ui_workflow_value(workflow).map_err(|error| error.to_string())?;
    let normalized = normalize_ui_workflow(
        &document,
        &dynamic_descriptors_for(schema, frontend_version),
    )
    .map_err(|error| format!("{}: {}", error.code, error.message))?;
    Ok(normalized.api_value)
}

fn dynamic_workflow() -> Value {
    json!({
        "version": 0.4,
        "extra": {"frontendVersion": "1.43.1"},
        "nodes": [
            {
                "id": 1,
                "type": "Source",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "IMAGE", "type": "IMAGE", "links": [1, 2]}],
                "widgets_values": []
            },
            {
                "id": 2,
                "type": "DynamicNode",
                "mode": 0,
                "inputs": [
                    {"name": "select.image_1", "label": "image_1", "shape": 7, "type": "IMAGE", "link": 1},
                    {"name": "select.image_2", "label": "image_2", "shape": 7, "type": "IMAGE", "link": 2}
                ],
                "outputs": [{"name": "LATENT", "type": "LATENT", "links": []}],
                "widgets_values": ["2", 0.25, 0.75]
            }
        ],
        "links": [
            [1, 1, 0, 2, 0, "IMAGE"],
            [2, 1, 0, 2, 1, "IMAGE"]
        ]
    })
}

#[test]
fn legacy_dynamic_combo_normalizes_selector_members_and_links() {
    let document = parse_ui_workflow_value(&dynamic_workflow()).expect("UI workflow should parse");
    let normalized = normalize_ui_workflow(&document, &dynamic_descriptors())
        .expect("legacy dynamic workflow should normalize");
    let inputs = normalized.api_value["2"]["inputs"]
        .as_object()
        .expect("normalized node inputs");

    assert_eq!(inputs.get("select"), Some(&json!("2")));
    assert_eq!(inputs.get("select.weight_1"), Some(&json!(0.25)));
    assert_eq!(inputs.get("select.weight_2"), Some(&json!(0.75)));
    assert_eq!(inputs.get("select.image_1"), Some(&json!(["1", 0])));
    assert_eq!(inputs.get("select.image_2"), Some(&json!(["1", 0])));
}

#[test]
fn legacy_dynamic_multiple_instances_are_deterministic_and_keep_link_sources() {
    let workflow = dynamic_workflow();
    let schema = dynamic_schema();
    let first = normalize_with(&workflow, &schema, "1.43.1").expect("normalize");
    let second = normalize_with(&workflow, &schema, "1.43.1").expect("normalize again");
    assert_eq!(first, second);
    let inputs = first["2"]["inputs"].as_object().expect("inputs");
    assert_eq!(inputs.get("select.image_1"), Some(&json!(["1", 0])));
    assert_eq!(inputs.get("select.image_2"), Some(&json!(["1", 0])));
    assert_eq!(inputs.get("select.weight_1"), Some(&json!(0.25)));
    assert_eq!(inputs.get("select.weight_2"), Some(&json!(0.75)));
}

#[test]
fn unknown_dynamic_combo_instance_fails_closed() {
    let mut workflow = dynamic_workflow();
    workflow["nodes"][1]["inputs"]
        .as_array_mut()
        .expect("inputs")
        .push(json!({"name": "select.image_3", "type": "IMAGE"}));
    let error = normalize_with(&workflow, &dynamic_schema(), "1.43.1")
        .expect_err("unknown instance must fail closed");
    assert!(
        error.starts_with("legacy_dynamic_unknown_instance:"),
        "{error}"
    );
}

#[test]
fn named_dynamic_selector_is_validated_without_positional_widgets() {
    let mut workflow = dynamic_workflow();
    workflow["nodes"][1]["widgets_values"] = json!([]);
    workflow["nodes"][1]["widgets_values_named"] = json!({"select": "9"});

    let error = normalize_with(&workflow, &dynamic_schema(), "1.43.1")
        .expect_err("unknown named selector option must fail closed");
    assert!(
        error.starts_with("legacy_dynamic_unknown_option:"),
        "{error}"
    );
}

#[test]
fn duplicate_dynamic_identity_fails_closed() {
    let mut workflow = dynamic_workflow();
    workflow["nodes"][1]["inputs"]
        .as_array_mut()
        .expect("inputs")
        .push(json!({"name": "select.image_1", "type": "IMAGE"}));
    let error = normalize_with(&workflow, &dynamic_schema(), "1.43.1")
        .expect_err("duplicate identity must fail closed");
    assert!(
        error.starts_with("legacy_dynamic_duplicate_instance:"),
        "{error}"
    );
}

#[test]
fn dynamic_socket_type_mismatch_fails_closed() {
    let mut workflow = dynamic_workflow();
    workflow["nodes"][1]["inputs"]
        .as_array_mut()
        .expect("inputs")
        .push(json!({"name": "select.weight_1", "type": "MODEL"}));
    let error = normalize_with(&workflow, &dynamic_schema(), "1.43.1")
        .expect_err("type mismatch must fail closed");
    assert!(
        error.starts_with("legacy_dynamic_type_mismatch:"),
        "{error}"
    );
}

#[test]
fn legacy_dynamic_normalization_is_class_name_independent() {
    let mut workflow = dynamic_workflow();
    workflow["nodes"][1]["type"] = json!("NodeBeta");
    let mut schema = dynamic_schema();
    let class_schema = schema
        .as_object_mut()
        .expect("schema object")
        .remove("DynamicNode")
        .expect("neutral schema");
    schema["NodeBeta"] = class_schema;
    let normalized = normalize_with(&workflow, &schema, "1.43.1").expect("normalize renamed node");
    assert_eq!(normalized["2"]["class_type"], json!("NodeBeta"));
    assert_eq!(normalized["2"]["inputs"]["select.weight_1"], json!(0.25));
}

#[test]
fn unknown_frontend_version_can_use_fingerprint_selected_dynamic_profile() {
    let mut workflow = dynamic_workflow();
    workflow["extra"]
        .as_object_mut()
        .expect("extra")
        .remove("frontendVersion");
    let normalized = normalize_with(&workflow, &dynamic_schema(), "unknown")
        .expect("historical fingerprint selected the profile");
    assert_eq!(normalized["2"]["inputs"]["select.weight_2"], json!(0.75));
}

#[test]
fn dynamic_socket_contract_is_separate_from_dynamic_combo_options() {
    let descriptors = dynamic_descriptors_for(&dynamic_socket_schema(false), "1.43.1");
    let target = descriptors
        .legacy_dynamic_input_target("NodeAlpha", "image_1")
        .expect("formal dynamic socket resolves")
        .expect("target exists");
    assert_eq!(target.base_name, "left");
    assert_eq!(target.instance_name, "image_1");
    assert_eq!(
        target.descriptor.declared_type,
        ai_studio_lib::application::workflow_recognition_schema::RecognitionDeclaredType::Image
    );
    assert!(descriptors
        .legacy_dynamic_combo_expansion("NodeAlpha", "left", "1")
        .is_err());
}

#[test]
fn ambiguous_dynamic_socket_base_fails_closed() {
    let descriptors = dynamic_descriptors_for(&dynamic_socket_schema(true), "1.43.1");
    let error = descriptors
        .legacy_dynamic_input_target("NodeAlpha", "image_1")
        .expect_err("ambiguous formal base must fail closed");
    assert_eq!(error.code, "legacy_dynamic_ambiguous_base");
}

#[test]
fn sparse_dynamic_instance_ids_preserve_identity_and_link_sources() {
    let workflow = json!({
        "version": 0.4,
        "extra": {"frontendVersion": "1.43.1"},
        "nodes": [
            {
                "id": 1,
                "type": "Source",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "IMAGE", "type": "IMAGE", "links": [10]}],
                "widgets_values": []
            },
            {
                "id": 2,
                "type": "Source",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "IMAGE", "type": "IMAGE", "links": [30]}],
                "widgets_values": []
            },
            {
                "id": 3,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [
                    {"name": "image_1", "type": "IMAGE", "shape": 7, "link": 10},
                    {"name": "image_3", "type": "IMAGE", "shape": 7, "link": 30}
                ],
                "outputs": [],
                "widgets_values": []
            }
        ],
        "links": [
            [10, 1, 0, 3, 0, "IMAGE"],
            [30, 2, 0, 3, 1, "IMAGE"]
        ]
    });

    let normalized = normalize_with(&workflow, &dynamic_socket_schema(false), "1.43.1")
        .expect("sparse formal dynamic instances should normalize");
    let inputs = normalized["3"]["inputs"].as_object().expect("inputs");
    assert_eq!(inputs.get("image_1"), Some(&json!(["1", 0])));
    assert_eq!(inputs.get("image_3"), Some(&json!(["2", 0])));
    assert!(!inputs.contains_key("image_2"));
}

#[test]
fn converted_widget_is_not_captured_as_a_dynamic_socket() {
    let descriptors = dynamic_descriptors_for(&dynamic_socket_schema(false), "1.43.1");
    let binding = descriptors
        .resolve_ui_input("NodeAlpha", "image_1_input", None, Some(7), false, None)
        .expect("converted widget contract resolves");
    assert_eq!(binding.kind, WidgetBindingKind::ConvertedWidget);
    assert_eq!(binding.runtime_name.as_deref(), Some("image_1"));
}

#[test]
fn legacy_widget_and_dynamic_contracts_compose() {
    let mut workflow = dynamic_workflow();
    workflow["nodes"].as_array_mut().unwrap().push(json!({
        "id": 3,
        "type": "NodeAlpha",
        "mode": 0,
        "inputs": [],
        "outputs": [],
        "widgets_values": [7]
    }));
    let mut schema_value = dynamic_schema();
    schema_value["NodeAlpha"] = json!({
        "input": {"required": {"value": ["INT", {}]}},
        "output": []
    });
    let schema = RecognitionSchemaContext::parse(&schema_value);
    let context = NormalizationCompatibilityContext {
        workflow_format_version: SUPPORTED_WORKFLOW_FORMAT_VERSION.to_owned(),
        frontend_version: "1.43.1".to_owned(),
        schema_fingerprint: "paired-neutral-composition".to_owned(),
        serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
        normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
        source_frontend_revision: None,
    };
    let contracts = BTreeSet::from([
        FrontendSerializationContract::LegacyWidgetSlotV0,
        FrontendSerializationContract::LegacyDynamicInputV0,
    ]);
    let profile = FrontendSerializationProfile::from_contracts_from_context(&context, contracts)
        .expect("independent historical contracts compose");
    let descriptors = UiSerializationDescriptorSet::build(&schema, profile, context)
        .expect("combined descriptors build");
    let document = parse_ui_workflow_value(&workflow).expect("workflow parses");
    let normalized = normalize_ui_workflow(&document, &descriptors)
        .expect("each node uses its detected contract authority");
    assert_eq!(normalized.api_value["3"]["inputs"]["value"], json!(7));
    assert_eq!(
        normalized.api_value["2"]["inputs"]["select.weight_1"],
        json!(0.25)
    );
}
