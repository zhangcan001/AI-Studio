use ai_studio_lib::application::workflow_recognition_schema::{
    RecognitionDeclaredType, RecognitionSchemaContext,
};
use ai_studio_lib::application::workflow_ui_normalizer::{
    build_source_id_mapping, normalize_ui_workflow, parse_ui_workflow, parse_ui_workflow_value,
    CompatibilityFeatureStatus, NormalizedWorkflow, WorkflowUiFeature,
};
use ai_studio_lib::application::workflow_ui_serialization::{
    canonical_schema_fingerprint, FrontendSerializationProfile, NormalizationCompatibilityContext,
    SerializerKind, SupportedNormalizationCompatibilitySet, UiInputSerializationDescriptor,
    UiNodeSerializationDescriptor, UiSerializationDescriptorSet, NORMALIZER_POLICY_VERSION,
    PROFILE_SUPPORT_MODEL, SERIALIZATION_PROFILE_VERSION,
};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const FIXTURE_NAMES: &[&str] = &["kera2_t2i", "h3_fast_t2v", "h3_reference"];

fn fixture_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/workflow_ui/phase2b")
        .join(name)
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).expect("fixture file should exist"))
        .expect("fixture JSON should parse")
}

fn descriptors_for_fixture(name: &str) -> (Value, UiSerializationDescriptorSet) {
    let root = fixture_root(name);
    let metadata = read_json(root.join("metadata.json"));
    let object_info = read_json(root.join("object_info.json"));
    let context = NormalizationCompatibilityContext {
        workflow_format_version: metadata["workflow_format_version"]
            .as_str()
            .expect("format metadata")
            .to_owned(),
        frontend_version: metadata["source_frontend_version"]
            .as_str()
            .expect("frontend metadata")
            .to_owned(),
        schema_fingerprint: metadata["schema_fingerprint"]
            .as_str()
            .expect("schema metadata")
            .to_owned(),
        serialization_profile_version: metadata["serialization_profile_version"]
            .as_str()
            .expect("profile metadata")
            .to_owned(),
        normalizer_policy_version: metadata["normalizer_policy_version"]
            .as_str()
            .expect("policy metadata")
            .to_owned(),
        source_frontend_revision: metadata["source_environment"]["source_frontend_revision"]
            .as_str()
            .map(str::to_owned),
    };
    SupportedNormalizationCompatibilitySet::exact(context.clone())
        .require(&context)
        .expect("fixture compatibility should be supported");
    assert_eq!(
        canonical_schema_fingerprint(&object_info),
        context.schema_fingerprint
    );
    let profile = FrontendSerializationProfile::from_context(&context)
        .expect("fixture should have a verified frontend profile");
    assert_eq!(profile.support_model, PROFILE_SUPPORT_MODEL);
    let schema = RecognitionSchemaContext::parse(&object_info);
    let descriptors = UiSerializationDescriptorSet::build(&schema, profile, context)
        .expect("fixture schema descriptors should build");
    (metadata, descriptors)
}

fn without_api_metadata(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut result = Map::new();
            for (key, value) in object {
                if key != "_meta" {
                    result.insert(key.clone(), without_api_metadata(value));
                }
            }
            Value::Object(result)
        }
        Value::Array(values) => Value::Array(values.iter().map(without_api_metadata).collect()),
        Value::String(value)
            if value.starts_with("Krea2-") && value[6..].chars().all(|c| c.is_ascii_digit()) =>
        {
            Value::String("Krea2-%date:yyyyMMddhhmmss%".to_owned())
        }
        other => other.clone(),
    }
}

fn normalize(name: &str) -> NormalizedWorkflow {
    let root = fixture_root(name);
    let source = fs::read(root.join("ui_workflow.json")).expect("UI fixture should exist");
    let document = parse_ui_workflow(&source).expect("UI fixture should parse");
    let (_, descriptors) = descriptors_for_fixture(name);
    normalize_ui_workflow(&document, &descriptors).expect("UI fixture should normalize")
}

fn synthetic_descriptors(
    target_type: RecognitionDeclaredType,
    include_string_target: bool,
) -> UiSerializationDescriptorSet {
    let context = NormalizationCompatibilityContext {
        workflow_format_version: "0.4".to_owned(),
        frontend_version: "1.52.7".to_owned(),
        schema_fingerprint: "synthetic-v1b".to_owned(),
        serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
        normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
        source_frontend_revision: None,
    };
    let profile = FrontendSerializationProfile::from_context(&context).unwrap();
    let input =
        |name: &str, declared_type: RecognitionDeclaredType| UiInputSerializationDescriptor {
            name: name.to_owned(),
            declared_type,
            required: true,
            hidden: false,
            serializer_kind: SerializerKind::StandardDirect,
            upload_media_kind: None,
            enum_options: Vec::new(),
            enum_values: Vec::new(),
            numeric_min: None,
            numeric_max: None,
            numeric_step: None,
            multiline: false,
            default_value: None,
            dynamic: false,
            dynamic_prefix: None,
            dynamic_names: Vec::new(),
        };
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "Source".to_owned(),
        UiNodeSerializationDescriptor {
            class_type: "Source".to_owned(),
            display_name: None,
            output_node: false,
            ordered_inputs: Vec::new(),
            output_types: vec![RecognitionDeclaredType::Integer],
            output_names: vec!["INT".to_owned()],
            output_is_list: vec![false],
            inputs: BTreeMap::new(),
            conditional_inputs: BTreeMap::new(),
        },
    );
    let mut target_inputs = BTreeMap::new();
    target_inputs.insert("value".to_owned(), input("value", target_type));
    nodes.insert(
        "Target".to_owned(),
        UiNodeSerializationDescriptor {
            class_type: "Target".to_owned(),
            display_name: None,
            output_node: false,
            ordered_inputs: vec!["value".to_owned()],
            output_types: Vec::new(),
            output_names: Vec::new(),
            output_is_list: Vec::new(),
            inputs: target_inputs,
            conditional_inputs: BTreeMap::new(),
        },
    );
    let mut image_inputs = BTreeMap::new();
    image_inputs.insert(
        "value".to_owned(),
        input("value", RecognitionDeclaredType::Image),
    );
    nodes.insert(
        "ImageTarget".to_owned(),
        UiNodeSerializationDescriptor {
            class_type: "ImageTarget".to_owned(),
            display_name: None,
            output_node: false,
            ordered_inputs: vec!["value".to_owned()],
            output_types: Vec::new(),
            output_names: Vec::new(),
            output_is_list: Vec::new(),
            inputs: image_inputs,
            conditional_inputs: BTreeMap::new(),
        },
    );
    let mut alternate_inputs = BTreeMap::new();
    alternate_inputs.insert(
        "frames_number".to_owned(),
        input("frames_number", RecognitionDeclaredType::Integer),
    );
    nodes.insert(
        "AlternateTarget".to_owned(),
        UiNodeSerializationDescriptor {
            class_type: "AlternateTarget".to_owned(),
            display_name: None,
            output_node: false,
            ordered_inputs: vec!["frames_number".to_owned()],
            output_types: Vec::new(),
            output_names: Vec::new(),
            output_is_list: Vec::new(),
            inputs: alternate_inputs,
            conditional_inputs: BTreeMap::new(),
        },
    );
    if include_string_target {
        let mut text_inputs = BTreeMap::new();
        text_inputs.insert(
            "value".to_owned(),
            input("value", RecognitionDeclaredType::String),
        );
        nodes.insert(
            "StringTarget".to_owned(),
            UiNodeSerializationDescriptor {
                class_type: "StringTarget".to_owned(),
                display_name: None,
                output_node: false,
                ordered_inputs: vec!["value".to_owned()],
                output_types: Vec::new(),
                output_names: Vec::new(),
                output_is_list: Vec::new(),
                inputs: text_inputs,
                conditional_inputs: BTreeMap::new(),
            },
        );
    }
    nodes.insert(
        "PrimitiveInt".to_owned(),
        UiNodeSerializationDescriptor {
            class_type: "PrimitiveInt".to_owned(),
            display_name: None,
            output_node: false,
            ordered_inputs: Vec::new(),
            output_types: vec![RecognitionDeclaredType::Integer],
            output_names: vec!["INT".to_owned()],
            output_is_list: vec![false],
            inputs: BTreeMap::new(),
            conditional_inputs: BTreeMap::new(),
        },
    );
    UiSerializationDescriptorSet {
        compatibility: context,
        profile,
        nodes,
    }
}

fn v1b2_descriptor(
    name: &str,
    declared_type: RecognitionDeclaredType,
    serializer_kind: SerializerKind,
    required: bool,
) -> UiInputSerializationDescriptor {
    UiInputSerializationDescriptor {
        name: name.to_owned(),
        declared_type,
        required,
        hidden: false,
        serializer_kind,
        upload_media_kind: None,
        enum_options: Vec::new(),
        enum_values: Vec::new(),
        numeric_min: None,
        numeric_max: None,
        numeric_step: None,
        multiline: false,
        default_value: None,
        dynamic: false,
        dynamic_prefix: None,
        dynamic_names: Vec::new(),
    }
}

fn v1b2_node(
    class_type: &str,
    ordered_inputs: Vec<&str>,
    inputs: BTreeMap<String, UiInputSerializationDescriptor>,
) -> UiNodeSerializationDescriptor {
    UiNodeSerializationDescriptor {
        class_type: class_type.to_owned(),
        display_name: None,
        output_node: false,
        ordered_inputs: ordered_inputs.into_iter().map(str::to_owned).collect(),
        output_types: Vec::new(),
        output_names: Vec::new(),
        output_is_list: Vec::new(),
        inputs,
        conditional_inputs: BTreeMap::new(),
    }
}

fn v1b2_descriptor_set(
    nodes: BTreeMap<String, UiNodeSerializationDescriptor>,
) -> UiSerializationDescriptorSet {
    let context = NormalizationCompatibilityContext {
        workflow_format_version: "0.4".to_owned(),
        frontend_version: "1.52.7".to_owned(),
        schema_fingerprint: "synthetic-v1b2".to_owned(),
        serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
        normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
        source_frontend_revision: None,
    };
    let profile = FrontendSerializationProfile::from_context(&context).unwrap();
    UiSerializationDescriptorSet {
        compatibility: context,
        profile,
        nodes,
    }
}

fn normalize_synthetic_with(
    value: Value,
    descriptors: &UiSerializationDescriptorSet,
) -> Result<NormalizedWorkflow, String> {
    let document = parse_ui_workflow_value(&value).map_err(|error| error.to_string())?;
    normalize_ui_workflow(&document, descriptors).map_err(|error| error.to_string())
}

fn normalize_synthetic(value: Value) -> Result<NormalizedWorkflow, String> {
    let descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    normalize_synthetic_with(value, &descriptors)
}

fn synthetic_runtime_target_node(id: i64, link_id: Option<i64>) -> Value {
    let mut input = serde_json::json!({
        "name": "value",
        "type": "INT",
        "widget": {"name": "value"}
    });
    if let Some(link_id) = link_id {
        input["link"] = Value::from(link_id);
    }
    serde_json::json!({
        "id": id,
        "type": "Target",
        "mode": 0,
        "inputs": [input],
        "outputs": [],
        "widgets_values": []
    })
}

fn synthetic_typed_target_node(
    id: i64,
    class_type: &str,
    socket_type: &str,
    link_id: i64,
) -> Value {
    serde_json::json!({
        "id": id,
        "type": class_type,
        "mode": 0,
        "inputs": [{
            "name": "value",
            "type": socket_type,
            "link": link_id
        }],
        "outputs": [],
        "widgets_values": []
    })
}

fn synthetic_widget_target_node(
    id: i64,
    class_type: &str,
    socket_name: &str,
    socket_type: &str,
    widget_name: &str,
    link_id: i64,
) -> Value {
    serde_json::json!({
        "id": id,
        "type": class_type,
        "mode": 0,
        "inputs": [{
            "name": socket_name,
            "type": socket_type,
            "widget": {"name": widget_name},
            "link": link_id
        }],
        "outputs": [],
        "widgets_values": []
    })
}

fn synthetic_header(nodes: Vec<Value>, links: Vec<Value>) -> Value {
    serde_json::json!({
        "version": 0.4,
        "extra": {"frontendVersion": "1.52.7"},
        "nodes": nodes,
        "links": links
    })
}

fn synthetic_source_node(id: i64, links: Vec<i64>) -> Value {
    serde_json::json!({
        "id": id,
        "type": "Source",
        "mode": 0,
        "inputs": [],
        "outputs": [{"name": "INT", "type": "INT", "links": links}],
        "widgets_values": []
    })
}

fn synthetic_reroute_node(
    id: i64,
    input_link: Option<i64>,
    output_links: Vec<i64>,
    output_type: &str,
) -> Value {
    let mut input = serde_json::json!({"name": "", "type": "*"});
    if let Some(input_link) = input_link {
        input["link"] = Value::from(input_link);
    }
    serde_json::json!({
        "id": id,
        "type": "Reroute",
        "mode": 0,
        "inputs": [input],
        "outputs": [{"name": "", "type": output_type, "links": output_links}],
        "widgets_values": []
    })
}

fn synthetic_primitive_node(
    id: i64,
    value: Value,
    widget_name: Option<&str>,
    output_type: &str,
    links: Vec<i64>,
) -> Value {
    let mut output = serde_json::json!({
        "name": output_type,
        "type": output_type,
        "links": links
    });
    if let Some(widget_name) = widget_name {
        output["widget"] = serde_json::json!({"name": widget_name});
    }
    serde_json::json!({
        "id": id,
        "type": "PrimitiveNode",
        "mode": 0,
        "inputs": [],
        "outputs": [output],
        "widgets_values": [value]
    })
}

#[test]
fn golden_fixture_manifest_has_three_real_pairs() {
    for name in FIXTURE_NAMES {
        let root = fixture_root(name);
        for file in [
            "ui_workflow.json",
            "api_workflow.json",
            "object_info.json",
            "metadata.json",
            "expected_normalized_api.json",
        ] {
            assert!(root.join(file).is_file(), "{name} missing {file}");
        }
        let ui = read_json(root.join("ui_workflow.json"));
        let metadata = read_json(root.join("metadata.json"));
        assert_eq!(ui["version"], 0.4);
        assert!(ui["extra"]["frontendVersion"].as_str().is_some());
        assert!(metadata["expected_output"].is_object());
        assert!(metadata["expected_core_bindings"].is_object());
        let _ = descriptors_for_fixture(name);
    }
}

#[test]
fn golden_ui_pairs_reconstruct_api_graphs() {
    for name in FIXTURE_NAMES {
        let normalized = normalize(name);
        let expected = read_json(fixture_root(name).join("expected_normalized_api.json"));
        assert_eq!(
            without_api_metadata(&normalized.api_value),
            without_api_metadata(&expected),
            "normalized graph differs for {name}"
        );
    }
}

#[test]
fn parsed_ui_source_is_not_the_normalized_api_bytes() {
    for name in FIXTURE_NAMES {
        let root = fixture_root(name);
        let source = fs::read(root.join("ui_workflow.json")).unwrap();
        let normalized = normalize(name);
        assert_ne!(source, normalized.api_bytes);
    }
}

#[test]
fn incompatible_ui_provenance_and_unknown_nodes_fail_closed() {
    let root = fixture_root("kera2_t2i");
    let (_, descriptors) = descriptors_for_fixture("kera2_t2i");
    let mut source = read_json(root.join("ui_workflow.json"));

    source["version"] = Value::from(0.5);
    let document = parse_ui_workflow_value(&source).expect("version-only mutation should parse");
    let error = normalize_ui_workflow(&document, &descriptors).expect_err("version must be gated");
    assert_eq!(error.code, "WORKFLOW_FORMAT_UNSUPPORTED");

    source["version"] = Value::from(0.4);
    source["extra"] = serde_json::json!({});
    let document = parse_ui_workflow_value(&source).expect("missing provenance should parse");
    let error = normalize_ui_workflow(&document, &descriptors)
        .expect_err("missing frontend provenance must fail closed");
    assert_eq!(error.code, "FRONTEND_VERSION_UNSUPPORTED");

    source["extra"] = serde_json::json!({"frontendVersion": "1.52.7"});
    source["nodes"][0]["type"] = Value::from("Phase2BUnknownNode");
    let document = parse_ui_workflow_value(&source).expect("unknown node mutation should parse");
    let error = normalize_ui_workflow(&document, &descriptors)
        .expect_err("unknown node classes must fail closed");
    assert_eq!(error.code, "UNKNOWN_NODE_CLASS");
}

#[test]
fn feature_detection_is_deterministic_and_separates_support_from_detection() {
    let mut source = serde_json::json!({
        "version": 0.4,
        "extra": {"frontendVersion": "1.52.7"},
        "nodes": [
            {"id": "00000000-0000-4000-8000-000000000001", "type": "Note", "mode": 0, "inputs": [], "outputs": [], "widgets_values": []},
            {"id": 42, "type": "PrimitiveNode", "mode": 0, "inputs": [], "outputs": [], "widgets_values": []},
            {"id": "43", "type": "PrimitiveFloat", "mode": 0, "inputs": [], "outputs": [], "widgets_values": []}
        ],
        "links": [[7, "uuid-b", 0, 42, 0, "*", "unexpected"]],
        "definitions": {"subgraphs": [{"id": "subgraph-1", "nodes": [], "links": []}]}
    });
    let first = parse_ui_workflow_value(&source).expect("feature fixture should parse");
    let first_features = first.features.clone();
    source["nodes"] = serde_json::json!([
        source["nodes"][2].clone(),
        source["nodes"][0].clone(),
        source["nodes"][1].clone()
    ]);
    let second = parse_ui_workflow_value(&source).expect("reordered feature fixture should parse");
    assert_eq!(first_features, second.features);
    assert!(first_features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::UuidNodeIds));
    assert!(first_features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::StringNumericNodeIds));
    assert!(first_features.observations.iter().any(|item| {
        item.feature == WorkflowUiFeature::RuntimePrimitiveNode
            && item.status == CompatibilityFeatureStatus::ConditionallySupported
    }));
    assert!(first_features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::NoteNode
            && item.status == CompatibilityFeatureStatus::Unsupported));
    assert!(first_features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::UnknownLinkEncoding));
    assert!(first_features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::DefinitionsSubgraphs));
}

#[test]
fn unsupported_ui_features_fail_before_low_level_normalization_errors() {
    let root = fixture_root("kera2_t2i");
    let (_, descriptors) = descriptors_for_fixture("kera2_t2i");
    for (feature_type, expected_feature) in [
        ("Reroute", WorkflowUiFeature::RerouteNode),
        ("PrimitiveNode", WorkflowUiFeature::PrimitiveNode),
        ("Note", WorkflowUiFeature::NoteNode),
    ] {
        let mut source = read_json(root.join("ui_workflow.json"));
        source["nodes"][0]["type"] = Value::from(feature_type);
        source["nodes"][0]["inputs"] = if feature_type == "Reroute" {
            serde_json::json!([{}])
        } else {
            serde_json::json!([])
        };
        let document = parse_ui_workflow_value(&source).expect("unsupported feature should parse");
        let error = normalize_ui_workflow(&document, &descriptors)
            .expect_err("unsupported feature must fail at the feature gate");
        assert_eq!(error.code, "UNSUPPORTED_UI_FEATURE");
        assert_eq!(error.feature, Some(expected_feature));
    }
}

#[test]
fn subgraphs_and_unknown_virtual_nodes_fail_as_feature_diagnostics() {
    let root = fixture_root("kera2_t2i");
    let (_, descriptors) = descriptors_for_fixture("kera2_t2i");

    let mut subgraph_source = read_json(root.join("ui_workflow.json"));
    subgraph_source["definitions"] = serde_json::json!({
        "subgraphs": [{"id": "subgraph-1", "nodes": [], "links": []}]
    });
    let subgraph_document = parse_ui_workflow_value(&subgraph_source).unwrap();
    assert!(subgraph_document
        .features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::DefinitionsSubgraphs));
    let subgraph_error = normalize_ui_workflow(&subgraph_document, &descriptors).unwrap_err();
    assert_eq!(subgraph_error.code, "UNSUPPORTED_UI_FEATURE");
    assert_eq!(
        subgraph_error.feature,
        Some(WorkflowUiFeature::DefinitionsSubgraphs)
    );

    let mut virtual_source = read_json(root.join("ui_workflow.json"));
    virtual_source["nodes"][0]["type"] = Value::from("UntrustedVirtualNode");
    virtual_source["nodes"][0]["properties"] = serde_json::json!({"virtual": true});
    let virtual_document = parse_ui_workflow_value(&virtual_source).unwrap();
    assert!(virtual_document
        .features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::UnknownUiNode));
    let virtual_error = normalize_ui_workflow(&virtual_document, &descriptors).unwrap_err();
    assert_eq!(virtual_error.code, "UNSUPPORTED_UI_FEATURE");
    assert_eq!(
        virtual_error.feature,
        Some(WorkflowUiFeature::UnknownUiNode)
    );

    let mut composite_source = read_json(root.join("ui_workflow.json"));
    composite_source["nodes"][0]["type"] = Value::from("00000000-0000-4000-8000-000000000001");
    let composite_document = parse_ui_workflow_value(&composite_source).unwrap();
    assert!(composite_document
        .features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::UuidCompositeNodeType));
    let composite_error = normalize_ui_workflow(&composite_document, &descriptors).unwrap_err();
    assert_eq!(composite_error.code, "UNSUPPORTED_UI_FEATURE");
    assert_eq!(
        composite_error.feature,
        Some(WorkflowUiFeature::UuidCompositeNodeType)
    );
}

#[test]
fn uuid_source_ids_map_to_frozen_safe_ids_and_links_use_the_same_map() {
    let root = fixture_root("kera2_t2i");
    let source = read_json(root.join("ui_workflow.json"));
    let (_, descriptors) = descriptors_for_fixture("kera2_t2i");
    let mut uuid_source = source.clone();
    let ids = uuid_source["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, _)| format!("00000000-0000-4000-8000-{index:012}"))
        .collect::<Vec<_>>();
    for (node, id) in uuid_source["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(&ids)
    {
        node["id"] = Value::String(id.clone());
    }
    for link in uuid_source["links"].as_array_mut().unwrap() {
        let tuple = link.as_array_mut().unwrap();
        let origin = tuple[1].as_i64().unwrap() as usize;
        let target = tuple[3].as_i64().unwrap() as usize;
        tuple[1] = Value::String(ids[origin - 1].clone());
        tuple[3] = Value::String(ids[target - 1].clone());
    }
    let document = parse_ui_workflow_value(&uuid_source).expect("UUID UI source should parse");
    let mapping = build_source_id_mapping(&document);
    assert_eq!(mapping.source_to_api.len(), ids.len());
    assert_eq!(mapping.api_to_source.len(), ids.len());
    assert!(mapping
        .source_to_api
        .values()
        .all(|id| id.chars().all(|ch| ch.is_ascii_digit() || ch == ':')));
    for (source_id, api_id) in &mapping.source_to_api {
        assert_eq!(mapping.api_to_source.get(api_id), Some(source_id));
    }
    let normalized =
        normalize_ui_workflow(&document, &descriptors).expect("UUID source should normalize");
    for link in normalized.api_value.as_object().unwrap().values() {
        let inputs = link["inputs"].as_object().unwrap();
        for value in inputs.values() {
            if let Some(connection) = value.as_array() {
                if connection.len() == 2 {
                    assert!(mapping
                        .api_to_source
                        .contains_key(connection[0].as_str().unwrap()));
                }
            }
        }
    }
    assert_eq!(normalized.api_to_source, mapping.api_to_source);
}

#[test]
fn source_id_mapping_is_order_independent_and_preserves_numeric_ids() {
    let mut value = serde_json::json!({
        "version": 0.4,
        "extra": {"frontendVersion": "1.52.7"},
        "nodes": [
            {"id": 42, "type": "A", "mode": 0, "inputs": [], "outputs": [], "widgets_values": []},
            {"id": "43", "type": "B", "mode": 0, "inputs": [], "outputs": [], "widgets_values": []},
            {"id": "uuid-source", "type": "C", "mode": 0, "inputs": [], "outputs": [], "widgets_values": []}
        ],
        "links": []
    });
    let first = parse_ui_workflow_value(&value).unwrap();
    let first_mapping = build_source_id_mapping(&first);
    value["nodes"] = serde_json::json!([
        value["nodes"][2].clone(),
        value["nodes"][0].clone(),
        value["nodes"][1].clone()
    ]);
    let second = parse_ui_workflow_value(&value).unwrap();
    assert_eq!(first_mapping, build_source_id_mapping(&second));
    assert_eq!(first_mapping.source_to_api["42"], "42");
    assert_eq!(first_mapping.source_to_api["43"], "43");
    assert_ne!(first_mapping.source_to_api["uuid-source"], "uuid-source");
}

#[test]
fn unlinked_presentation_note_is_removed_without_object_info() {
    let value = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_runtime_target_node(2, Some(1)),
            serde_json::json!({
                "id": 3,
                "type": "Note",
                "mode": 0,
                "inputs": [],
                "outputs": [],
                "widgets_values": ["presentation text"]
            }),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let normalized = normalize_synthetic(value).expect("safe Note should normalize");
    assert!(!normalized.api_value.as_object().unwrap().contains_key("3"));
    assert_eq!(
        normalized.api_value["2"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
}

#[test]
fn note_text_does_not_change_normalized_execution_graph() {
    let make = |text: &str| {
        synthetic_header(
            vec![
                synthetic_source_node(1, vec![1]),
                synthetic_runtime_target_node(2, Some(1)),
                serde_json::json!({
                    "id": 3,
                    "type": "Note",
                    "mode": 0,
                    "inputs": [],
                    "outputs": [],
                    "widgets_values": [text]
                }),
            ],
            vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
        )
    };
    let first = normalize_synthetic(make("first")).unwrap();
    let second = normalize_synthetic(make("second")).unwrap();
    assert_eq!(first.api_value, second.api_value);
}

#[test]
fn linked_note_and_note_with_runtime_output_fail_closed() {
    let linked = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            serde_json::json!({
                "id": 2,
                "type": "Note",
                "mode": 0,
                "inputs": [{"name": "value", "type": "*", "link": 1}],
                "outputs": [],
                "widgets_values": []
            }),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let linked_error = normalize_synthetic(linked).unwrap_err();
    assert!(linked_error.contains("UNSUPPORTED_UI_FEATURE"));

    let output = synthetic_header(
        vec![serde_json::json!({
            "id": 1,
            "type": "Note",
            "mode": 0,
            "inputs": [],
            "outputs": [{"name": "OUT", "type": "INT", "links": []}],
            "widgets_values": []
        })],
        vec![],
    );
    let output_error = normalize_synthetic(output).unwrap_err();
    assert!(output_error.contains("UNSUPPORTED_UI_FEATURE"));
}

#[test]
fn generic_primitive_int_and_string_bindings_materialize() {
    let int_value = synthetic_header(
        vec![
            synthetic_primitive_node(1, Value::from(489), Some("value"), "INT", vec![1]),
            synthetic_typed_target_node(2, "Target", "INT", 1),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let int_normalized = normalize_synthetic(int_value).unwrap();
    assert_eq!(
        int_normalized.api_value["2"]["inputs"]["value"],
        Value::from(489)
    );

    let string_value = synthetic_header(
        vec![
            synthetic_primitive_node(1, Value::from("hello"), Some("value"), "STRING", vec![1]),
            synthetic_typed_target_node(2, "Target", "STRING", 1),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "STRING"])],
    );
    let descriptors = synthetic_descriptors(RecognitionDeclaredType::String, true);
    let string_normalized = normalize_synthetic_with(string_value, &descriptors).unwrap();
    assert_eq!(
        string_normalized.api_value["2"]["inputs"]["value"],
        Value::from("hello")
    );
}

#[test]
fn primitive_fanout_materializes_identical_bindings() {
    let value = synthetic_header(
        vec![
            synthetic_primitive_node(1, Value::from(489), Some("value"), "INT", vec![1, 2]),
            synthetic_typed_target_node(2, "Target", "INT", 1),
            synthetic_typed_target_node(3, "Target", "INT", 2),
        ],
        vec![
            serde_json::json!([1, 1, 0, 2, 0, "INT"]),
            serde_json::json!([2, 1, 0, 3, 0, "INT"]),
        ],
    );
    let normalized = normalize_synthetic(value).unwrap();
    assert_eq!(
        normalized.api_value["2"]["inputs"]["value"],
        Value::from(489)
    );
    assert_eq!(
        normalized.api_value["3"]["inputs"]["value"],
        Value::from(489)
    );

    let cross_name_fanout = synthetic_header(
        vec![
            synthetic_primitive_node(1, Value::from(489), Some("length"), "INT", vec![1, 2]),
            synthetic_widget_target_node(2, "Target", "value", "INT", "length", 1),
            synthetic_widget_target_node(
                3,
                "AlternateTarget",
                "frames_number",
                "INT",
                "frames_number",
                2,
            ),
        ],
        vec![
            serde_json::json!([1, 1, 0, 2, 0, "INT"]),
            serde_json::json!([2, 1, 0, 3, 0, "INT"]),
        ],
    );
    let cross_name_normalized = normalize_synthetic(cross_name_fanout).unwrap();
    assert_eq!(
        cross_name_normalized.api_value["2"]["inputs"]["value"],
        Value::from(489)
    );
    assert_eq!(
        cross_name_normalized.api_value["3"]["inputs"]["frames_number"],
        Value::from(489)
    );
}

#[test]
fn primitive_target_type_conflict_and_unknown_contract_fail_closed() {
    let conflict = synthetic_header(
        vec![
            synthetic_primitive_node(1, Value::from(489), Some("value"), "INT", vec![1, 2]),
            synthetic_typed_target_node(2, "Target", "*", 1),
            synthetic_typed_target_node(3, "ImageTarget", "*", 2),
        ],
        vec![
            serde_json::json!([1, 1, 0, 2, 0, "INT"]),
            serde_json::json!([2, 1, 0, 3, 0, "INT"]),
        ],
    );
    let conflict_error = normalize_synthetic(conflict).unwrap_err();
    assert!(conflict_error.contains("PRIMITIVE_TARGET_TYPE_CONFLICT"));

    let unknown_contract = synthetic_header(
        vec![
            synthetic_primitive_node(1, Value::from(489), None, "INT", vec![1]),
            synthetic_typed_target_node(2, "Target", "INT", 1),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let unknown_error = normalize_synthetic(unknown_contract).unwrap_err();
    assert!(unknown_error.contains("UNSUPPORTED_UI_FEATURE"));
}

#[test]
fn primitive_binding_matches_direct_literal_and_is_order_independent() {
    let direct = synthetic_header(
        vec![serde_json::json!({
            "id": 2,
            "type": "Target",
            "mode": 0,
            "inputs": [{"name": "value", "type": "INT", "widget": {"name": "value"}}],
            "outputs": [],
            "widgets_values": [],
            "widgets_values_named": {"value": 489}
        })],
        vec![],
    );
    let primitive = synthetic_header(
        vec![
            synthetic_primitive_node(1, Value::from(489), Some("value"), "INT", vec![1]),
            synthetic_typed_target_node(2, "Target", "INT", 1),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let direct_normalized = normalize_synthetic(direct).unwrap();
    let primitive_normalized = normalize_synthetic(primitive.clone()).unwrap();
    assert_eq!(direct_normalized.api_value, primitive_normalized.api_value);

    let mut reordered = primitive;
    reordered["nodes"] =
        serde_json::json!([reordered["nodes"][1].clone(), reordered["nodes"][0].clone()]);
    let first = normalize_synthetic_with(
        synthetic_header(
            vec![
                synthetic_primitive_node(1, Value::from(489), Some("value"), "INT", vec![1]),
                synthetic_typed_target_node(2, "Target", "INT", 1),
            ],
            vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
        ),
        &synthetic_descriptors(RecognitionDeclaredType::Integer, true),
    )
    .unwrap();
    let second = normalize_synthetic(reordered).unwrap();
    assert_eq!(first.api_value, second.api_value);
}

#[test]
fn runtime_primitive_class_remains_a_runtime_node() {
    let value = synthetic_header(
        vec![
            serde_json::json!({
                "id": 1,
                "type": "PrimitiveInt",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "INT", "type": "INT", "links": [1]}],
                "widgets_values": [7],
                "widgets_values_named": {"value": 7}
            }),
            synthetic_typed_target_node(2, "Target", "INT", 1),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let normalized = normalize_synthetic(value).unwrap();
    assert_eq!(normalized.api_value["1"]["class_type"], "PrimitiveInt");
    assert_eq!(
        normalized.api_value["2"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
}

#[test]
fn single_reroute_chain_fanout_and_unconnected_nodes_normalize() {
    let single = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_reroute_node(3, Some(1), vec![2], "INT"),
            synthetic_typed_target_node(2, "Target", "INT", 2),
        ],
        vec![
            serde_json::json!([1, 1, 0, 3, 0, "INT"]),
            serde_json::json!([2, 3, 0, 2, 0, "INT"]),
        ],
    );
    let normalized = normalize_synthetic(single).unwrap();
    assert!(!normalized.api_value.as_object().unwrap().contains_key("3"));
    assert_eq!(
        normalized.api_value["2"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );

    let chain = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_reroute_node(3, Some(1), vec![2], "INT"),
            synthetic_reroute_node(4, Some(2), vec![3], "INT"),
            synthetic_typed_target_node(2, "Target", "INT", 3),
        ],
        vec![
            serde_json::json!([1, 1, 0, 3, 0, "INT"]),
            serde_json::json!([2, 3, 0, 4, 0, "INT"]),
            serde_json::json!([3, 4, 0, 2, 0, "INT"]),
        ],
    );
    let chain_normalized = normalize_synthetic(chain).unwrap();
    assert_eq!(
        chain_normalized.api_value["2"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );

    let fanout = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_reroute_node(4, Some(1), vec![2, 3], "INT"),
            synthetic_typed_target_node(2, "Target", "INT", 2),
            synthetic_typed_target_node(3, "Target", "INT", 3),
        ],
        vec![
            serde_json::json!([1, 1, 0, 4, 0, "INT"]),
            serde_json::json!([2, 4, 0, 2, 0, "INT"]),
            serde_json::json!([3, 4, 0, 3, 0, "INT"]),
        ],
    );
    let fanout_normalized = normalize_synthetic(fanout).unwrap();
    assert_eq!(
        fanout_normalized.api_value["2"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
    assert_eq!(
        fanout_normalized.api_value["3"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );

    let unconnected = synthetic_header(
        vec![synthetic_reroute_node(3, None, Vec::new(), "INT")],
        vec![],
    );
    let unconnected_normalized = normalize_synthetic(unconnected).unwrap();
    assert!(unconnected_normalized
        .api_value
        .as_object()
        .unwrap()
        .is_empty());
}

#[test]
fn reroute_type_fanin_cycle_and_order_fail_closed() {
    let typed = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_reroute_node(3, Some(1), vec![2], "STRING"),
            synthetic_typed_target_node(2, "Target", "STRING", 2),
        ],
        vec![
            serde_json::json!([1, 1, 0, 3, 0, "INT"]),
            serde_json::json!([2, 3, 0, 2, 0, "STRING"]),
        ],
    );
    assert!(normalize_synthetic(typed)
        .unwrap_err()
        .contains("REROUTE_TYPE_MISMATCH"));

    let fanin = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_source_node(2, vec![2]),
            synthetic_reroute_node(3, Some(1), vec![3], "INT"),
            synthetic_typed_target_node(4, "Target", "INT", 3),
        ],
        vec![
            serde_json::json!([1, 1, 0, 3, 0, "INT"]),
            serde_json::json!([2, 2, 0, 3, 0, "INT"]),
            serde_json::json!([3, 3, 0, 4, 0, "INT"]),
        ],
    );
    assert!(normalize_synthetic(fanin)
        .unwrap_err()
        .contains("REROUTE_AMBIGUOUS_FANIN"));

    let cycle = synthetic_header(
        vec![
            synthetic_reroute_node(1, Some(3), vec![1], "INT"),
            synthetic_reroute_node(2, Some(1), vec![2, 3], "INT"),
            synthetic_typed_target_node(3, "Target", "INT", 2),
        ],
        vec![
            serde_json::json!([1, 1, 0, 2, 0, "INT"]),
            serde_json::json!([2, 2, 0, 3, 0, "INT"]),
            serde_json::json!([3, 2, 0, 1, 0, "INT"]),
        ],
    );
    assert!(normalize_synthetic(cycle)
        .unwrap_err()
        .contains("REROUTE_CYCLE"));

    let direct = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_typed_target_node(2, "Target", "INT", 1),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let rerouted = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_reroute_node(3, Some(1), vec![2], "INT"),
            synthetic_typed_target_node(2, "Target", "INT", 2),
        ],
        vec![
            serde_json::json!([1, 1, 0, 3, 0, "INT"]),
            serde_json::json!([2, 3, 0, 2, 0, "INT"]),
        ],
    );
    assert_eq!(
        normalize_synthetic(direct).unwrap().api_value,
        normalize_synthetic(rerouted).unwrap().api_value
    );
}

#[test]
fn frontend_control_does_not_shift_runtime_cursor() {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "width".to_owned(),
        v1b2_descriptor(
            "width",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    inputs.insert(
        "height".to_owned(),
        v1b2_descriptor(
            "height",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "NodeCursor".to_owned(),
        v1b2_node("NodeCursor", vec!["width", "height"], inputs),
    );
    let value = synthetic_header(
        vec![serde_json::json!({
            "id": 1,
            "type": "NodeCursor",
            "mode": 0,
            "inputs": [
                {"name": "width", "type": "INT", "widget": {"name": "width"}},
                {"name": "height", "type": "INT", "widget": {"name": "height"}}
            ],
            "widgets_values": [512, 768, "fixed"],
            "widgets_values_named": {"width": 512, "height": 768}
        })],
        vec![],
    );
    let normalized = normalize_synthetic_with(value, &v1b2_descriptor_set(nodes)).unwrap();
    assert_eq!(normalized.api_value["1"]["inputs"]["width"], 512);
    assert_eq!(normalized.api_value["1"]["inputs"]["height"], 768);
}

#[test]
fn linked_input_with_stale_widget_value_does_not_shift_cursor() {
    let mut target_inputs = BTreeMap::new();
    target_inputs.insert(
        "value".to_owned(),
        v1b2_descriptor(
            "value",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    target_inputs.insert(
        "tail".to_owned(),
        v1b2_descriptor(
            "tail",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut nodes = BTreeMap::new();
    let mut source = v1b2_node("NodeSource", vec![], BTreeMap::new());
    source.output_types = vec![RecognitionDeclaredType::Integer];
    source.output_names = vec!["INT".to_owned()];
    source.output_is_list = vec![false];
    nodes.insert("NodeSource".to_owned(), source);
    nodes.insert(
        "NodeAlpha".to_owned(),
        v1b2_node("NodeAlpha", vec!["value", "tail"], target_inputs),
    );
    let value = synthetic_header(
        vec![
            serde_json::json!({
                "id": 1,
                "type": "NodeSource",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "INT", "type": "INT", "links": [1]}],
                "widgets_values": []
            }),
            serde_json::json!({
                "id": 2,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [
                    {"name": "value", "type": "INT", "widget": {"name": "value"}, "link": 1},
                    {"name": "tail", "type": "INT", "widget": {"name": "tail"}}
                ],
                "widgets_values": [999, 7],
                "widgets_values_named": {"tail": 7}
            }),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let normalized = normalize_synthetic_with(value, &v1b2_descriptor_set(nodes)).unwrap();
    assert_eq!(
        normalized.api_value["2"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
    assert_eq!(normalized.api_value["2"]["inputs"]["tail"], 7);
}

#[test]
fn converted_widget_binds_to_schema_input() {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "height".to_owned(),
        v1b2_descriptor(
            "height",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "NodeBeta".to_owned(),
        v1b2_node("NodeBeta", vec!["height"], inputs),
    );
    let value = synthetic_header(
        vec![serde_json::json!({
            "id": 1,
            "type": "NodeBeta",
            "mode": 0,
            "inputs": [{
                "name": "height_input",
                "shape": 7,
                "type": "INT",
                "widget": {"name": "height_input"}
            }],
            "widgets_values": [640],
            "widgets_values_named": {"height": 640}
        })],
        vec![],
    );
    let normalized = normalize_synthetic_with(value, &v1b2_descriptor_set(nodes)).unwrap();
    assert_eq!(normalized.api_value["1"]["inputs"]["height"], 640);
}

#[test]
fn ambiguous_converted_widget_fails_closed() {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "value".to_owned(),
        v1b2_descriptor(
            "value",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    inputs.insert(
        "other".to_owned(),
        v1b2_descriptor(
            "other",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "NodeGamma".to_owned(),
        v1b2_node("NodeGamma", vec!["value", "other"], inputs),
    );
    let value = synthetic_header(
        vec![serde_json::json!({
            "id": 1,
            "type": "NodeGamma",
            "mode": 0,
            "inputs": [{
                "name": "value_input",
                "shape": 7,
                "type": "INT",
                "widget": {"name": "other_input"}
            }],
            "widgets_values": []
        })],
        vec![],
    );
    let error = normalize_synthetic_with(value, &v1b2_descriptor_set(nodes)).unwrap_err();
    assert!(error.contains("AMBIGUOUS_CONVERTED_WIDGET"));
}

#[test]
fn dynamic_widget_names_are_schema_bounded() {
    let mut dynamic = v1b2_descriptor(
        "values",
        RecognitionDeclaredType::Integer,
        SerializerKind::Dynamic,
        true,
    );
    dynamic.dynamic = true;
    dynamic.dynamic_names = vec!["a".to_owned(), "b".to_owned()];
    let mut inputs = BTreeMap::new();
    inputs.insert("values".to_owned(), dynamic);
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "NodeDynamic".to_owned(),
        v1b2_node("NodeDynamic", vec!["values"], inputs),
    );
    let descriptors = v1b2_descriptor_set(nodes);
    assert!(descriptors.input("NodeDynamic", "values.a").is_some());
    assert!(descriptors.input("NodeDynamic", "values.unknown").is_none());
}

#[test]
fn conditional_widget_contract_uses_selected_branch() {
    let mut mode = v1b2_descriptor(
        "mode",
        RecognitionDeclaredType::Enum,
        SerializerKind::StandardCombo,
        true,
    );
    mode.enum_values = vec![
        Value::String("auto".to_owned()),
        Value::String("manual".to_owned()),
    ];
    let codec = v1b2_descriptor(
        "codec",
        RecognitionDeclaredType::String,
        SerializerKind::Conditional,
        true,
    );
    let mut conditional = BTreeMap::new();
    conditional.insert(
        "mode".to_owned(),
        BTreeMap::from([(
            "auto".to_owned(),
            BTreeMap::from([("codec".to_owned(), codec)]),
        )]),
    );
    let mut inputs = BTreeMap::new();
    inputs.insert("mode".to_owned(), mode);
    let mut node = v1b2_node("NodeConditional", vec!["mode"], inputs);
    node.conditional_inputs = conditional;
    let mut nodes = BTreeMap::new();
    nodes.insert("NodeConditional".to_owned(), node);
    let descriptors = v1b2_descriptor_set(nodes);
    let selected = BTreeMap::from([("mode".to_owned(), Value::String("auto".to_owned()))]);
    assert_eq!(
        descriptors
            .input_for_node("NodeConditional", Some(&selected), "codec")
            .unwrap()
            .name,
        "codec"
    );
    let inactive = BTreeMap::from([("mode".to_owned(), Value::String("manual".to_owned()))]);
    assert!(descriptors
        .input_for_node("NodeConditional", Some(&inactive), "codec")
        .is_none());
}

#[test]
fn unclassified_extra_widget_value_fails_closed() {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "value".to_owned(),
        v1b2_descriptor(
            "value",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "NodeExtra".to_owned(),
        v1b2_node("NodeExtra", vec!["value"], inputs),
    );
    let value = synthetic_header(
        vec![serde_json::json!({
            "id": 1,
            "type": "NodeExtra",
            "mode": 0,
            "inputs": [{"name": "value", "type": "INT", "widget": {"name": "value"}}],
            "widgets_values": [1, {"unexpected": true}]
        })],
        vec![],
    );
    let error = normalize_synthetic_with(value, &v1b2_descriptor_set(nodes)).unwrap_err();
    assert!(error.contains("WIDGET_CURSOR_MISMATCH"));
}

#[test]
fn missing_required_widget_value_fails_closed() {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "value".to_owned(),
        v1b2_descriptor(
            "value",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "NodeMissing".to_owned(),
        v1b2_node("NodeMissing", vec!["value"], inputs),
    );
    let value = synthetic_header(
        vec![serde_json::json!({
            "id": 1,
            "type": "NodeMissing",
            "mode": 0,
            "inputs": [],
            "widgets_values": []
        })],
        vec![],
    );
    let error = normalize_synthetic_with(value, &v1b2_descriptor_set(nodes)).unwrap_err();
    assert!(error.contains("WIDGET_REQUIRED_VALUE_MISSING"));
}

#[test]
fn widget_binding_order_is_independent() {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "width".to_owned(),
        v1b2_descriptor(
            "width",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    inputs.insert(
        "height".to_owned(),
        v1b2_descriptor(
            "height",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "NodeOrder".to_owned(),
        v1b2_node("NodeOrder", vec!["width", "height"], inputs),
    );
    let descriptors = v1b2_descriptor_set(nodes);
    let mut named_a = Map::new();
    named_a.insert("height".to_owned(), Value::from(768));
    named_a.insert("width".to_owned(), Value::from(512));
    let mut named_b = Map::new();
    named_b.insert("width".to_owned(), Value::from(512));
    named_b.insert("height".to_owned(), Value::from(768));
    let make = |input_names: &[&str], named: Map<String, Value>| {
        synthetic_header(
            vec![serde_json::json!({
                "id": 1,
                "type": "NodeOrder",
                "mode": 0,
                "inputs": input_names.iter().map(|name| serde_json::json!({
                    "name": name,
                    "type": "INT",
                    "widget": {"name": name}
                })).collect::<Vec<_>>(),
                "widgets_values": Value::Object(named)
            })],
            vec![],
        )
    };
    let first =
        normalize_synthetic_with(make(&["width", "height"], named_a), &descriptors).unwrap();
    let second =
        normalize_synthetic_with(make(&["height", "width"], named_b), &descriptors).unwrap();
    assert_eq!(first.api_value, second.api_value);
}

#[test]
fn schema_declared_color_string_is_direct_serializable() {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "color".to_owned(),
        v1b2_descriptor(
            "color",
            RecognitionDeclaredType::Unknown,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "NodeColor".to_owned(),
        v1b2_node("NodeColor", vec!["color"], inputs),
    );
    let value = synthetic_header(
        vec![serde_json::json!({
            "id": 1,
            "type": "NodeColor",
            "mode": 0,
            "inputs": [{"name": "color", "type": "COLOR", "widget": {"name": "color"}}],
            "widgets_values": ["#fff700"],
            "widgets_values_named": {"color": "#fff700"}
        })],
        vec![],
    );
    let normalized = normalize_synthetic_with(value, &v1b2_descriptor_set(nodes)).unwrap();
    assert_eq!(normalized.api_value["1"]["inputs"]["color"], "#fff700");
}

#[test]
fn bypass_runtime_node_collapses_to_the_compatible_source_edge() {
    let mut source = v1b2_node("NodeAlpha", Vec::new(), BTreeMap::new());
    source.output_types = vec![RecognitionDeclaredType::Integer];
    source.output_names = vec!["value".to_owned()];
    source.output_is_list = vec![false];

    let mut bypass_inputs = BTreeMap::new();
    bypass_inputs.insert(
        "value".to_owned(),
        v1b2_descriptor(
            "value",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut bypass = v1b2_node("NodeBeta", vec!["value"], bypass_inputs);
    bypass.output_types = vec![RecognitionDeclaredType::Integer];
    bypass.output_names = vec!["value".to_owned()];
    bypass.output_is_list = vec![false];

    let mut target_inputs = BTreeMap::new();
    target_inputs.insert(
        "value".to_owned(),
        v1b2_descriptor(
            "value",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let target = v1b2_node("NodeGamma", vec!["value"], target_inputs);

    let descriptors = v1b2_descriptor_set(BTreeMap::from([
        ("NodeAlpha".to_owned(), source),
        ("NodeBeta".to_owned(), bypass),
        ("NodeGamma".to_owned(), target),
    ]));
    let value = synthetic_header(
        vec![
            serde_json::json!({
                "id": 1,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "value", "type": "INT", "links": [10]}],
                "widgets_values": []
            }),
            serde_json::json!({
                "id": 2,
                "type": "NodeBeta",
                "mode": 4,
                "inputs": [{"name": "value", "type": "INT", "link": 10}],
                "outputs": [{"name": "value", "type": "INT", "links": [11]}],
                "widgets_values": []
            }),
            synthetic_typed_target_node(3, "NodeGamma", "INT", 11),
        ],
        vec![
            serde_json::json!([10, 1, 0, 2, 0, "INT"]),
            serde_json::json!([11, 2, 0, 3, 0, "INT"]),
        ],
    );
    let normalized = normalize_synthetic_with(value, &descriptors).unwrap();
    assert!(normalized.api_value.get("2").is_none());
    assert_eq!(
        normalized.api_value["3"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
}

#[test]
fn bypass_runtime_node_with_ambiguous_compatible_sources_fails_closed() {
    let mut source = v1b2_node("NodeAlpha", Vec::new(), BTreeMap::new());
    source.output_types = vec![RecognitionDeclaredType::Integer];
    source.output_names = vec!["value".to_owned()];
    source.output_is_list = vec![false];

    let mut bypass_inputs = BTreeMap::new();
    for name in ["value_a", "value_b"] {
        bypass_inputs.insert(
            name.to_owned(),
            v1b2_descriptor(
                name,
                RecognitionDeclaredType::Integer,
                SerializerKind::StandardDirect,
                true,
            ),
        );
    }
    let mut bypass = v1b2_node("NodeBeta", vec!["value_a", "value_b"], bypass_inputs);
    bypass.output_types = vec![RecognitionDeclaredType::Integer];
    bypass.output_names = vec!["value".to_owned()];
    bypass.output_is_list = vec![false];

    let mut target_inputs = BTreeMap::new();
    target_inputs.insert(
        "value".to_owned(),
        v1b2_descriptor(
            "value",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let target = v1b2_node("NodeGamma", vec!["value"], target_inputs);
    let descriptors = v1b2_descriptor_set(BTreeMap::from([
        ("NodeAlpha".to_owned(), source),
        ("NodeBeta".to_owned(), bypass),
        ("NodeGamma".to_owned(), target),
    ]));
    let value = synthetic_header(
        vec![
            serde_json::json!({
                "id": 1,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "value", "type": "INT", "links": [10]}],
                "widgets_values": []
            }),
            serde_json::json!({
                "id": 2,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "value", "type": "INT", "links": [11]}],
                "widgets_values": []
            }),
            serde_json::json!({
                "id": 3,
                "type": "NodeBeta",
                "mode": 4,
                "inputs": [
                    {"name": "value_a", "type": "INT", "link": 10},
                    {"name": "value_b", "type": "INT", "link": 11}
                ],
                "outputs": [{"name": "value", "type": "INT", "links": [12]}],
                "widgets_values": []
            }),
            synthetic_typed_target_node(4, "NodeGamma", "INT", 12),
        ],
        vec![
            serde_json::json!([10, 1, 0, 3, 0, "INT"]),
            serde_json::json!([11, 2, 0, 3, 1, "INT"]),
            serde_json::json!([12, 3, 0, 4, 0, "INT"]),
        ],
    );
    let error = normalize_synthetic_with(value, &descriptors).unwrap_err();
    assert!(error.contains("BYPASS_AMBIGUOUS_FANIN"));
}

fn synthetic_alias_producer(id: i64, identity: &str, input_link: i64, output_type: &str) -> Value {
    serde_json::json!({
        "id": id,
        "type": "NodeAlpha",
        "mode": 0,
        "inputs": [{"name": "value", "type": output_type, "link": input_link}],
        "outputs": [{"name": "value", "type": output_type, "links": []}],
        "widgets_values": [identity],
        "properties": {"previousName": identity}
    })
}

fn synthetic_alias_consumer(
    id: i64,
    identity: &str,
    output_type: &str,
    output_links: Vec<i64>,
) -> Value {
    serde_json::json!({
        "id": id,
        "type": "NodeBeta",
        "mode": 0,
        "inputs": [],
        "outputs": [{"name": "value", "type": output_type, "links": output_links}],
        "widgets_values": [identity]
    })
}

fn synthetic_alias_target(id: i64, input_type: &str, link_id: i64) -> Value {
    serde_json::json!({
        "id": id,
        "type": "Target",
        "mode": 0,
        "inputs": [{"name": "value", "type": input_type, "link": link_id}],
        "outputs": [],
        "widgets_values": []
    })
}

#[test]
fn generic_alias_producer_consumer_collapses() {
    let value = synthetic_header(
        vec![
            synthetic_source_node(1, vec![10]),
            synthetic_alias_producer(2, "shared", 10, "INT"),
            synthetic_alias_consumer(3, "shared", "INT", vec![11]),
            synthetic_alias_target(4, "INT", 11),
        ],
        vec![
            serde_json::json!([10, 1, 0, 2, 0, "INT"]),
            serde_json::json!([11, 3, 0, 4, 0, "INT"]),
        ],
    );
    let normalized = normalize_synthetic(value).expect("generic alias should normalize");
    assert!(normalized.api_value.get("2").is_none());
    assert!(normalized.api_value.get("3").is_none());
    assert_eq!(
        normalized.api_value["4"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
}

#[test]
fn generic_alias_fanout_collapses() {
    let value = synthetic_header(
        vec![
            synthetic_source_node(1, vec![10]),
            synthetic_alias_producer(2, "shared", 10, "INT"),
            synthetic_alias_consumer(3, "shared", "INT", vec![11, 12]),
            synthetic_alias_target(4, "INT", 11),
            synthetic_alias_target(5, "INT", 12),
        ],
        vec![
            serde_json::json!([10, 1, 0, 2, 0, "INT"]),
            serde_json::json!([11, 3, 0, 4, 0, "INT"]),
            serde_json::json!([12, 3, 0, 5, 0, "INT"]),
        ],
    );
    let normalized = normalize_synthetic(value).expect("generic alias fan-out should normalize");
    assert_eq!(
        normalized.api_value["4"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
    assert_eq!(
        normalized.api_value["5"]["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
}

#[test]
fn alias_consumer_without_producer_fails_closed() {
    let value = synthetic_header(
        vec![
            synthetic_alias_consumer(3, "missing", "INT", vec![11]),
            synthetic_alias_target(4, "INT", 11),
        ],
        vec![serde_json::json!([11, 3, 0, 4, 0, "INT"])],
    );
    let error = normalize_synthetic(value).unwrap_err();
    assert!(error.contains("ALIAS_CONSUMER_WITHOUT_PRODUCER"));
}

#[test]
fn ambiguous_alias_producer_fails_closed() {
    let value = synthetic_header(
        vec![
            synthetic_source_node(1, vec![10]),
            serde_json::json!({
                "id": 2,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [{"name": "value", "type": "INT", "link": 10}],
                "outputs": [{"name": "value", "type": "INT", "links": []}],
                "widgets_values": ["shared"],
                "properties": {"previousName": "shared"}
            }),
            serde_json::json!({
                "id": 3,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [{"name": "value", "type": "INT", "link": 11}],
                "outputs": [{"name": "value", "type": "INT", "links": []}],
                "widgets_values": ["shared"],
                "properties": {"previousName": "shared"}
            }),
            synthetic_alias_consumer(4, "shared", "INT", vec![12]),
            synthetic_alias_target(5, "INT", 12),
        ],
        vec![
            serde_json::json!([10, 1, 0, 2, 0, "INT"]),
            serde_json::json!([11, 1, 0, 3, 0, "INT"]),
            serde_json::json!([12, 4, 0, 5, 0, "INT"]),
        ],
    );
    let error = normalize_synthetic(value).unwrap_err();
    assert!(error.contains("AMBIGUOUS_ALIAS_PRODUCER"));
}

#[test]
fn alias_type_conflict_fails_closed() {
    let value = synthetic_header(
        vec![
            synthetic_source_node(1, vec![10]),
            synthetic_alias_producer(2, "shared", 10, "INT"),
            synthetic_alias_consumer(3, "shared", "STRING", vec![11]),
            serde_json::json!({
                "id": 4,
                "type": "StringTarget",
                "mode": 0,
                "inputs": [{"name": "value", "type": "STRING", "link": 11}],
                "outputs": [],
                "widgets_values": []
            }),
        ],
        vec![
            serde_json::json!([10, 1, 0, 2, 0, "INT"]),
            serde_json::json!([11, 3, 0, 4, 0, "STRING"]),
        ],
    );
    let error = normalize_synthetic(value).unwrap_err();
    assert!(error.contains("ALIAS_TYPE_CONFLICT"));
}

#[test]
fn alias_cycle_fails_closed() {
    let value = synthetic_header(
        vec![
            serde_json::json!({
                "id": 1,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [{"name": "value", "type": "INT", "link": 10}],
                "outputs": [{"name": "value", "type": "INT", "links": []}],
                "widgets_values": ["a"],
                "properties": {"previousName": "a"}
            }),
            serde_json::json!({
                "id": 2,
                "type": "NodeAlpha",
                "mode": 0,
                "inputs": [{"name": "value", "type": "INT", "link": 11}],
                "outputs": [{"name": "value", "type": "INT", "links": []}],
                "widgets_values": ["b"],
                "properties": {"previousName": "b"}
            }),
            serde_json::json!({
                "id": 3,
                "type": "NodeBeta",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "value", "type": "INT", "links": [10, 12]}],
                "widgets_values": ["a"]
            }),
            serde_json::json!({
                "id": 4,
                "type": "NodeBeta",
                "mode": 0,
                "inputs": [],
                "outputs": [{"name": "value", "type": "INT", "links": [11]}],
                "widgets_values": ["b"]
            }),
            synthetic_alias_target(5, "INT", 12),
        ],
        vec![
            serde_json::json!([10, 3, 0, 1, 0, "INT"]),
            serde_json::json!([11, 4, 0, 2, 0, "INT"]),
            serde_json::json!([12, 3, 0, 5, 0, "INT"]),
        ],
    );
    let error = normalize_synthetic(value).unwrap_err();
    assert!(error.contains("ALIAS_CYCLE"));
}

#[test]
fn runtime_set_like_class_is_not_treated_as_alias() {
    let mut descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "value".to_owned(),
        v1b2_descriptor(
            "value",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardDirect,
            true,
        ),
    );
    let mut runtime = v1b2_node("SetModel", vec!["value"], inputs);
    runtime.output_types = vec![RecognitionDeclaredType::Integer];
    runtime.output_names = vec!["value".to_owned()];
    runtime.output_is_list = vec![false];
    descriptors.nodes.insert("SetModel".to_owned(), runtime);

    let value = synthetic_header(
        vec![
            synthetic_source_node(1, vec![10]),
            serde_json::json!({
                "id": 2,
                "type": "SetModel",
                "mode": 0,
                "inputs": [{"name": "value", "type": "INT", "link": 10}],
                "outputs": [{"name": "value", "type": "INT", "links": [11]}],
                "widgets_values": []
            }),
            synthetic_alias_target(3, "INT", 11),
        ],
        vec![
            serde_json::json!([10, 1, 0, 2, 0, "INT"]),
            serde_json::json!([11, 2, 0, 3, 0, "INT"]),
        ],
    );
    let normalized = normalize_synthetic_with(value, &descriptors)
        .expect("schema-backed Set-like runtime node should remain runtime");
    assert_eq!(normalized.api_value["2"]["class_type"], "SetModel");
    assert_eq!(
        normalized.api_value["3"]["inputs"]["value"],
        serde_json::json!(["2", 0])
    );
}

#[test]
fn alias_class_name_does_not_create_alias_semantic() {
    let value = synthetic_header(
        vec![
            synthetic_source_node(1, vec![10]),
            serde_json::json!({
                "id": 2,
                "type": "SetLike",
                "mode": 0,
                "inputs": [{"name": "value", "type": "INT", "link": 10}],
                "outputs": [{"name": "value", "type": "INT", "links": [11]}],
                "widgets_values": []
            }),
            synthetic_alias_target(3, "INT", 11),
        ],
        vec![
            serde_json::json!([10, 1, 0, 2, 0, "INT"]),
            serde_json::json!([11, 2, 0, 3, 0, "INT"]),
        ],
    );
    let error = normalize_synthetic(value).unwrap_err();
    assert!(error.contains("UNKNOWN_NODE_CLASS"));
}
