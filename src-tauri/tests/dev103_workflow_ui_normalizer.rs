use ai_studio_lib::application::workflow_recognition_schema::{
    RecognitionDeclaredType, RecognitionSchemaContext,
};
use ai_studio_lib::application::workflow_ui_normalizer::{
    build_source_id_mapping, normalize_ui_workflow, parse_ui_workflow, parse_ui_workflow_value,
    parse_ui_workflow_value_with_descriptors, CompatibilityFeatureStatus, NormalizedWorkflow,
    WorkflowUiFeature,
};
use ai_studio_lib::application::workflow_ui_serialization::{
    canonical_schema_fingerprint, FrontendSerializationContract, FrontendSerializationProfile,
    NormalizationCompatibilityContext, SerializerKind, SupportedNormalizationCompatibilitySet,
    UiInputSerializationDescriptor, UiNodeSerializationDescriptor, UiSerializationDescriptorSet,
    NORMALIZER_POLICY_VERSION, PROFILE_SUPPORT_MODEL, SERIALIZATION_PROFILE_VERSION,
};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
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
            dynamic_value_type: None,
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

fn historical_subgraph_descriptors(
    contracts: BTreeSet<FrontendSerializationContract>,
) -> UiSerializationDescriptorSet {
    let current = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let context = NormalizationCompatibilityContext {
        workflow_format_version: "0.4".to_owned(),
        frontend_version: "1.42.14".to_owned(),
        schema_fingerprint: "synthetic-legacy-subgraph".to_owned(),
        serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
        normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
        source_frontend_revision: None,
    };
    let profile = FrontendSerializationProfile::from_contracts_from_context(&context, contracts)
        .expect("historical contract set should be constructible");
    UiSerializationDescriptorSet {
        compatibility: context,
        profile,
        nodes: current.nodes,
    }
}

fn historical_subgraph_only_descriptors() -> UiSerializationDescriptorSet {
    historical_subgraph_descriptors(BTreeSet::from([
        FrontendSerializationContract::LegacySubgraphBoundaryProxyV0,
    ]))
}

fn mark_historical_frontend(mut source: Value) -> Value {
    source["extra"]["frontendVersion"] = Value::from("1.42.14");
    source
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
        dynamic_value_type: None,
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

fn synthetic_subgraph_definition(
    id: &str,
    inputs: Value,
    outputs: Value,
    nodes: Vec<Value>,
    links: Vec<Value>,
) -> Value {
    serde_json::json!({
        "id": id,
        "version": 1,
        "revision": 0,
        "inputNode": {"id": -10, "bounding": [0, 0, 1, 1]},
        "outputNode": {"id": -20, "bounding": [0, 0, 1, 1]},
        "inputs": inputs,
        "outputs": outputs,
        "widgets": [],
        "nodes": nodes,
        "links": links,
        "groups": []
    })
}

fn synthetic_subgraph_workflow(
    nodes: Vec<Value>,
    links: Vec<Value>,
    definitions: Vec<Value>,
) -> Value {
    let mut value = synthetic_header(nodes, links);
    value["definitions"] = serde_json::json!({"subgraphs": definitions});
    value
}

fn synthetic_composite_node(
    id: i64,
    definition_id: &str,
    input_link: Option<i64>,
    output_links: Vec<i64>,
    output_type: &str,
) -> Value {
    let inputs = input_link
        .map(|link| {
            serde_json::json!([{
                "name": "value",
                "type": "INT",
                "widget": {"name": "value"},
                "link": link
            }])
        })
        .unwrap_or_else(|| serde_json::json!([]));
    let outputs = if output_links.is_empty() {
        serde_json::json!([])
    } else {
        serde_json::json!([{
            "name": output_type,
            "type": output_type,
            "links": output_links
        }])
    };
    serde_json::json!({
        "id": id,
        "type": definition_id,
        "mode": 0,
        "inputs": inputs,
        "outputs": outputs,
        "properties": {"proxyWidgets": []},
        "widgets_values": []
    })
}

fn synthetic_uuid_runtime_descriptor_set() -> UiSerializationDescriptorSet {
    let mut descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let uuid = "00000000-0000-4000-8000-000000000099".to_owned();
    let mut descriptor = descriptors.nodes["Source"].clone();
    descriptor.class_type = uuid.clone();
    descriptors.nodes.insert(uuid, descriptor);
    descriptors
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
        "definitions": {"subgraphs": [{
            "id": "subgraph-1",
            "inputNode": {"id": -10, "bounding": [0, 0, 1, 1]},
            "outputNode": {"id": -20, "bounding": [0, 0, 1, 1]},
            "inputs": [],
            "outputs": [],
            "nodes": [],
            "links": []
        }]}
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
    assert!(first_features.observations.iter().any(|item| {
        item.feature == WorkflowUiFeature::DefinitionsSubgraphs
            && item.status == CompatibilityFeatureStatus::Supported
    }));
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
fn subgraphs_are_flattened_and_unknown_virtual_nodes_still_fail_closed() {
    let root = fixture_root("kera2_t2i");
    let (_, descriptors) = descriptors_for_fixture("kera2_t2i");

    let mut subgraph_source = read_json(root.join("ui_workflow.json"));
    subgraph_source["definitions"] = serde_json::json!({
        "subgraphs": [{
            "id": "subgraph-1",
            "inputNode": {"id": -10, "bounding": [0, 0, 1, 1]},
            "outputNode": {"id": -20, "bounding": [0, 0, 1, 1]},
            "inputs": [],
            "outputs": [],
            "nodes": [],
            "links": []
        }]
    });
    let subgraph_document = parse_ui_workflow_value(&subgraph_source).unwrap();
    assert!(subgraph_document
        .features
        .observations
        .iter()
        .any(|item| item.feature == WorkflowUiFeature::DefinitionsSubgraphs));
    assert!(subgraph_document
        .features
        .observations
        .iter()
        .any(
            |item| item.feature == WorkflowUiFeature::DefinitionsSubgraphs
                && item.status == CompatibilityFeatureStatus::Supported
        ));
    normalize_ui_workflow(&subgraph_document, &descriptors)
        .expect("an unused valid definition must not block normalization");

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
    let composite_error = normalize_ui_workflow(&composite_document, &descriptors).unwrap_err();
    assert_eq!(composite_error.code, "UNKNOWN_NODE_CLASS");
}

#[test]
fn subgraph_definition_shape_and_external_input_boundary_flatten_provider_neutrally() {
    let definition_id = "definition-input";
    let definition = synthetic_subgraph_definition(
        definition_id,
        serde_json::json!([{
            "id": "input-slot",
            "name": "value",
            "type": "INT",
            "linkIds": [1]
        }]),
        serde_json::json!([]),
        vec![synthetic_runtime_target_node(20, Some(1))],
        vec![serde_json::json!({
            "id": 1,
            "origin_id": -10,
            "origin_slot": 0,
            "target_id": 20,
            "target_slot": 0,
            "type": "INT"
        })],
    );
    let source = synthetic_subgraph_workflow(
        vec![synthetic_source_node(1, vec![10]), {
            let mut node = synthetic_composite_node(9, definition_id, Some(10), vec![], "INT");
            node["properties"]["proxyWidgets"] = serde_json::json!([["20", "value"]]);
            node
        }],
        vec![serde_json::json!([10, 1, 0, 9, 0, "INT"])],
        vec![definition],
    );
    let descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let document = parse_ui_workflow_value(&source).expect("verified subgraph should parse");
    assert!(document
        .nodes
        .iter()
        .all(|node| node.class_type != definition_id));
    assert!(document
        .nodes
        .iter()
        .any(|node| node.id == "subgraph/instance[9]/node/20"));
    assert!(document
        .features
        .observations
        .iter()
        .any(
            |item| item.feature == WorkflowUiFeature::DefinitionsSubgraphs
                && item.status == CompatibilityFeatureStatus::Supported
        ));
    let normalized =
        normalize_ui_workflow(&document, &descriptors).expect("flattened input should normalize");
    let target = normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .expect("inner target should remain");
    let connection = target["inputs"]["value"].as_array().unwrap();
    assert_eq!(connection[0], "1");
    assert_eq!(connection[1], 0);
}

#[test]
fn legacy_subgraph_input_boundary_uses_unique_topology_and_preserves_fanout_source_slot() {
    let mut definition = synthetic_subgraph_definition(
        "legacy-input-boundary",
        serde_json::json!([{"id":"input-slot","name":"value","type":"INT"}]),
        serde_json::json!([]),
        vec![
            synthetic_runtime_target_node(20, Some(1)),
            synthetic_runtime_target_node(30, Some(2)),
        ],
        vec![
            serde_json::json!({"id":1,"origin_id":-10,"origin_slot":0,"target_id":20,"target_slot":0,"type":"INT"}),
            serde_json::json!({"id":2,"origin_id":-10,"origin_slot":0,"target_id":30,"target_slot":0,"type":"INT"}),
        ],
    );
    definition.as_object_mut().unwrap().remove("inputNode");
    let mut source_node = synthetic_source_node(1, vec![]);
    source_node["outputs"] = serde_json::json!([
        {"name":"unused","type":"INT","links":[]},
        {"name":"value","type":"INT","links":[10]}
    ]);
    let source = mark_historical_frontend(synthetic_subgraph_workflow(
        vec![
            source_node,
            synthetic_composite_node(9, "legacy-input-boundary", Some(10), vec![], "INT"),
        ],
        vec![serde_json::json!([10, 1, 1, 9, 0, "INT"])],
        vec![definition],
    ));
    let descriptors = historical_subgraph_only_descriptors();
    let document = parse_ui_workflow_value_with_descriptors(&source, &descriptors)
        .expect("unique serialized endpoint topology should canonicalize the input boundary");
    assert!(document
        .nodes
        .iter()
        .all(|node| node.class_type != "legacy-input-boundary"));
    assert_eq!(
        document.links.len(),
        2,
        "input fan-out must preserve both consumers"
    );
    assert!(document.links.iter().all(|link| link.origin_slot == 1));
    let mut scalar_sentinel = source.clone();
    scalar_sentinel["definitions"]["subgraphs"][0]["inputNode"] = Value::from(-10);
    assert!(parse_ui_workflow_value_with_descriptors(&scalar_sentinel, &descriptors).is_ok());
    let mut descriptors = descriptors;
    descriptors.nodes.get_mut("Source").unwrap().output_types = vec![
        RecognitionDeclaredType::Integer,
        RecognitionDeclaredType::Integer,
    ];
    descriptors.nodes.get_mut("Source").unwrap().output_names =
        vec!["unused".to_owned(), "value".to_owned()];
    let normalized = normalize_ui_workflow(&document, &descriptors)
        .expect("canonicalized topology should use the existing normalizer");
    let sources = normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .filter(|node| node["class_type"] == "Target")
        .map(|node| node["inputs"]["value"].clone())
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    assert!(sources.iter().all(|input| input[1] == 1));
}

#[test]
fn legacy_subgraph_output_boundary_infers_each_interface_slot_without_defaulting_to_zero() {
    let mut definition = synthetic_subgraph_definition(
        "legacy-output-boundary",
        serde_json::json!([]),
        serde_json::json!([
            {"id":"output-a","name":"a","type":"INT"},
            {"id":"output-b","name":"b","type":"INT"}
        ]),
        vec![
            synthetic_source_node(20, vec![1]),
            synthetic_source_node(21, vec![2]),
        ],
        vec![
            serde_json::json!({"id":1,"origin_id":20,"origin_slot":0,"target_id":-20,"target_slot":0,"type":"INT"}),
            serde_json::json!({"id":2,"origin_id":21,"origin_slot":0,"target_id":-20,"target_slot":1,"type":"INT"}),
        ],
    );
    definition.as_object_mut().unwrap().remove("outputNode");
    let mut composite =
        synthetic_composite_node(9, "legacy-output-boundary", None, vec![12, 13], "INT");
    composite["outputs"] = serde_json::json!([
        {"name":"a","type":"INT","links":[12]},
        {"name":"b","type":"INT","links":[13]}
    ]);
    let source = mark_historical_frontend(synthetic_subgraph_workflow(
        vec![
            composite,
            synthetic_runtime_target_node(2, Some(12)),
            synthetic_runtime_target_node(3, Some(13)),
        ],
        vec![
            serde_json::json!([12, 9, 0, 2, 0, "INT"]),
            serde_json::json!([13, 9, 1, 3, 0, "INT"]),
        ],
        vec![definition],
    ));
    let descriptors = historical_subgraph_only_descriptors();
    let document = parse_ui_workflow_value_with_descriptors(&source, &descriptors)
        .expect("unique inner producers should canonicalize both output slots");
    let normalized = normalize_ui_workflow(&document, &descriptors).unwrap();
    let targets = normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .filter(|node| node["class_type"] == "Target")
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    assert_ne!(
        targets[0]["inputs"]["value"][0],
        targets[1]["inputs"]["value"][0]
    );
    let source_ids = normalized
        .source_to_api
        .keys()
        .map(|id| id.as_str())
        .collect::<Vec<_>>();
    assert!(source_ids.iter().any(|id| id.ends_with("/node/20")));
    assert!(source_ids.iter().any(|id| id.ends_with("/node/21")));
}

#[test]
fn legacy_subgraph_boundary_ambiguity_and_type_conflict_fail_closed() {
    let mut ambiguous_definition = synthetic_subgraph_definition(
        "legacy-ambiguous-boundary",
        serde_json::json!([{"id":"input-slot","name":"value","type":"INT"}]),
        serde_json::json!([]),
        vec![
            synthetic_runtime_target_node(20, Some(1)),
            synthetic_runtime_target_node(30, Some(2)),
        ],
        vec![
            serde_json::json!({"id":1,"origin_id":-10,"origin_slot":0,"target_id":20,"target_slot":0,"type":"INT"}),
            serde_json::json!({"id":2,"origin_id":-11,"origin_slot":0,"target_id":30,"target_slot":0,"type":"INT"}),
        ],
    );
    ambiguous_definition
        .as_object_mut()
        .unwrap()
        .remove("inputNode");
    let ambiguous = synthetic_subgraph_workflow(
        vec![
            synthetic_source_node(1, vec![10]),
            synthetic_composite_node(9, "legacy-ambiguous-boundary", Some(10), vec![], "INT"),
        ],
        vec![serde_json::json!([10, 1, 0, 9, 0, "INT"])],
        vec![ambiguous_definition],
    );
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(
            &ambiguous,
            &historical_subgraph_only_descriptors()
        )
        .unwrap_err()
        .code,
        "LEGACY_SUBGRAPH_BOUNDARY_EVIDENCE_AMBIGUOUS"
    );

    let conflict = synthetic_subgraph_workflow(
        vec![
            synthetic_source_node(1, vec![10]),
            synthetic_composite_node(9, "legacy-type-conflict", Some(10), vec![], "INT"),
        ],
        vec![serde_json::json!([10, 1, 0, 9, 0, "INT"])],
        vec![synthetic_subgraph_definition(
            "legacy-type-conflict",
            serde_json::json!([{"id":"input-slot","name":"value","type":"INT","linkIds":[1]}]),
            serde_json::json!([]),
            vec![synthetic_runtime_target_node(20, Some(1))],
            vec![
                serde_json::json!({"id":1,"origin_id":-10,"origin_slot":0,"target_id":20,"target_slot":0,"type":"STRING"}),
            ],
        )],
    );
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(
            &conflict,
            &historical_subgraph_only_descriptors()
        )
        .unwrap_err()
        .code,
        "SUBGRAPH_INPUT_TYPE_CONFLICT"
    );
    let mut stale_interface = conflict;
    stale_interface["definitions"]["subgraphs"][0]["inputs"][0]["linkIds"] =
        serde_json::json!([999]);
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(
            &stale_interface,
            &historical_subgraph_only_descriptors()
        )
        .unwrap_err()
        .code,
        "LEGACY_SUBGRAPH_INTERFACE_EVIDENCE_CONFLICT"
    );
}

#[test]
fn legacy_proxy_widget_uses_schema_authority_and_rejects_unknown_or_ambiguous_targets() {
    let definition = synthetic_subgraph_definition(
        "legacy-proxy-widget",
        serde_json::json!([]),
        serde_json::json!([]),
        vec![{
            let mut target = synthetic_widget_target_node(20, "Target", "value", "INT", "value", 1);
            target["inputs"][0].as_object_mut().unwrap().remove("link");
            target["widgets_values"] = serde_json::json!([7]);
            target["widgets_values_named"] = serde_json::json!({"value":7});
            target
        }],
        vec![],
    );
    let mut instance = synthetic_composite_node(9, "legacy-proxy-widget", None, vec![], "INT");
    instance["properties"]["proxyWidgets"] = serde_json::json!([[20, "value"]]);
    let source = mark_historical_frontend(synthetic_subgraph_workflow(
        vec![instance],
        vec![],
        vec![definition.clone()],
    ));
    let descriptors = historical_subgraph_descriptors(BTreeSet::from([
        FrontendSerializationContract::LegacySubgraphBoundaryProxyV0,
        FrontendSerializationContract::LegacyWidgetSlotV0,
    ]));
    let document = parse_ui_workflow_value_with_descriptors(&source, &descriptors)
        .expect("exact node/widget/schema correspondence should canonicalize");
    assert!(document
        .nodes
        .iter()
        .all(|node| node.class_type != "legacy-proxy-widget"));
    let normalized = normalize_ui_workflow(&document, &descriptors)
        .expect("historical positional widget values should use LegacyWidgetSlotV0");
    let target = normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .unwrap();
    assert_eq!(target["inputs"]["value"], 7);

    let mut duplicate = source.clone();
    duplicate["nodes"][0]["properties"]["proxyWidgets"] =
        serde_json::json!([["20", "value"], [20, "value"]]);
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(&duplicate, &descriptors)
            .unwrap_err()
            .code,
        "AMBIGUOUS_SUBGRAPH_PROXY_WIDGET"
    );

    let mut unknown = source.clone();
    unknown["nodes"][0]["properties"]["proxyWidgets"] =
        serde_json::json!([{"node":"20","widget":"value"}]);
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(&unknown, &descriptors)
            .unwrap_err()
            .code,
        "LEGACY_SUBGRAPH_PROXY_SHAPE_UNSUPPORTED"
    );

    let mut type_conflict = source.clone();
    type_conflict["definitions"]["subgraphs"][0]["nodes"][0]["inputs"][0]["type"] =
        Value::from("IMAGE");
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(&type_conflict, &descriptors)
            .unwrap_err()
            .code,
        "LEGACY_SUBGRAPH_PROXY_TYPE_CONFLICT"
    );

    let mut stale = source;
    stale["nodes"][0]["properties"]["proxyWidgets"] = serde_json::json!([[99, "value"]]);
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(&stale, &descriptors)
            .unwrap_err()
            .code,
        "LEGACY_SUBGRAPH_PROXY_TARGET_UNRESOLVED"
    );
}

#[test]
fn legacy_proxy_widget_can_follow_exact_nested_composite_input_to_schema_authority() {
    let mut inner_target = synthetic_widget_target_node(30, "Target", "value", "INT", "value", 10);
    inner_target["widgets_values"] = serde_json::json!([7]);
    inner_target["widgets_values_named"] = serde_json::json!({"value": 7});
    let inner_definition = synthetic_subgraph_definition(
        "v1d4-inner",
        serde_json::json!([{"id":"inner-value","name":"value","type":"INT","linkIds":[10]}]),
        serde_json::json!([]),
        vec![inner_target],
        vec![
            serde_json::json!({"id":10,"origin_id":-10,"origin_slot":0,"target_id":30,"target_slot":0,"type":"INT"}),
        ],
    );

    let mut inner_instance = synthetic_composite_node(21, "v1d4-inner", Some(20), vec![], "INT");
    inner_instance["properties"]["proxyWidgets"] = serde_json::json!([[30, "value"]]);
    let outer_definition = synthetic_subgraph_definition(
        "v1d4-outer",
        serde_json::json!([{"id":"outer-value","name":"value","type":"INT","linkIds":[20]}]),
        serde_json::json!([]),
        vec![inner_instance],
        vec![
            serde_json::json!({"id":20,"origin_id":-10,"origin_slot":0,"target_id":21,"target_slot":0,"type":"INT"}),
        ],
    );

    let mut outer_instance = synthetic_composite_node(9, "v1d4-outer", None, vec![], "INT");
    outer_instance["inputs"] = serde_json::json!([{
        "name":"value","type":"INT","widget":{"name":"value"}
    }]);
    outer_instance["properties"]["proxyWidgets"] = serde_json::json!([[21, "value"]]);
    let source = mark_historical_frontend(synthetic_subgraph_workflow(
        vec![outer_instance],
        vec![],
        vec![inner_definition, outer_definition],
    ));
    let descriptors = historical_subgraph_descriptors(BTreeSet::from([
        FrontendSerializationContract::LegacySubgraphBoundaryProxyV0,
        FrontendSerializationContract::LegacyWidgetSlotV0,
    ]));

    parse_ui_workflow_value_with_descriptors(&source, &descriptors)
        .expect("exact nested composite interface and downstream schema target should resolve");
}

#[test]
fn legacy_proxy_widget_preserves_schema_backed_seed_control_for_widget_authority() {
    let mut seed_inputs = BTreeMap::new();
    seed_inputs.insert(
        "noise_seed".to_owned(),
        v1b2_descriptor(
            "noise_seed",
            RecognitionDeclaredType::Integer,
            SerializerKind::StandardSeed,
            true,
        ),
    );
    let mut descriptors = historical_subgraph_descriptors(BTreeSet::from([
        FrontendSerializationContract::LegacySubgraphBoundaryProxyV0,
        FrontendSerializationContract::LegacyWidgetSlotV0,
    ]));
    descriptors.nodes.insert(
        "SeedNode".to_owned(),
        v1b2_node("SeedNode", vec!["noise_seed"], seed_inputs),
    );

    let mut seed =
        synthetic_widget_target_node(20, "SeedNode", "noise_seed", "INT", "noise_seed", 1);
    seed["inputs"][0].as_object_mut().unwrap().remove("link");
    seed["widgets_values"] = serde_json::json!([17, "fixed"]);
    let definition = synthetic_subgraph_definition(
        "legacy-seed-control-proxy",
        serde_json::json!([]),
        serde_json::json!([]),
        vec![seed],
        vec![],
    );
    let mut instance =
        synthetic_composite_node(9, "legacy-seed-control-proxy", None, vec![], "INT");
    instance["properties"]["proxyWidgets"] = serde_json::json!([[20, "control_after_generate"]]);
    let source = mark_historical_frontend(synthetic_subgraph_workflow(
        vec![instance],
        vec![],
        vec![definition],
    ));

    parse_ui_workflow_value_with_descriptors(&source, &descriptors)
        .expect("the explicit seed-node identity and its StandardSeed schema contract identify this UI control");
}

#[test]
fn legacy_subgraph_current_contract_isolation_uuid_runtime_and_name_only_negative_controls() {
    let mut definition = synthetic_subgraph_definition(
        "legacy-no-evidence",
        serde_json::json!([{"id":"input-slot","name":"value","type":"INT"}]),
        serde_json::json!([]),
        vec![synthetic_runtime_target_node(20, None)],
        vec![],
    );
    definition.as_object_mut().unwrap().remove("inputNode");
    let source = mark_historical_frontend(synthetic_subgraph_workflow(
        vec![synthetic_composite_node(
            9,
            "legacy-no-evidence",
            None,
            vec![],
            "INT",
        )],
        vec![],
        vec![definition],
    ));
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(&source, &historical_subgraph_only_descriptors())
            .unwrap_err()
            .code,
        "LEGACY_SUBGRAPH_BOUNDARY_EVIDENCE_INSUFFICIENT"
    );
    assert_eq!(
        parse_ui_workflow_value_with_descriptors(
            &source,
            &synthetic_descriptors(RecognitionDeclaredType::Integer, true)
        )
        .unwrap_err()
        .code,
        "SUBGRAPH_INPUT_NODE_MISSING",
        "the adapter must be isolated from the frozen CurrentContract path"
    );

    let uuid = "00000000-0000-4000-8000-000000000099";
    let runtime = synthetic_header(
        vec![serde_json::json!({
            "id": 1, "type": uuid, "mode": 0, "inputs": [], "outputs": [], "widgets_values": []
        })],
        vec![],
    );
    let mut descriptors = historical_subgraph_only_descriptors();
    descriptors.nodes.insert(
        uuid.to_owned(),
        synthetic_descriptors(RecognitionDeclaredType::Integer, true).nodes["Target"].clone(),
    );
    let document = parse_ui_workflow_value_with_descriptors(&runtime, &descriptors)
        .expect("UUID-looking runtime class is not a composite without exact definition identity");
    assert_eq!(document.nodes[0].class_type, uuid);
}

#[test]
fn legacy_subgraph_multiple_instances_keep_existing_hierarchical_source_mapping() {
    let definition = synthetic_subgraph_definition(
        "legacy-reused-definition",
        serde_json::json!([]),
        serde_json::json!([{"id":"result","name":"result","type":"INT","linkIds":[1]}]),
        vec![synthetic_source_node(20, vec![1])],
        vec![
            serde_json::json!({"id":1,"origin_id":20,"origin_slot":0,"target_id":-20,"target_slot":0,"type":"INT"}),
        ],
    );
    let source = mark_historical_frontend(synthetic_subgraph_workflow(
        vec![
            synthetic_composite_node(9, "legacy-reused-definition", None, vec![12], "INT"),
            synthetic_composite_node(10, "legacy-reused-definition", None, vec![13], "INT"),
            synthetic_runtime_target_node(2, Some(12)),
            synthetic_runtime_target_node(3, Some(13)),
        ],
        vec![
            serde_json::json!([12, 9, 0, 2, 0, "INT"]),
            serde_json::json!([13, 10, 0, 3, 0, "INT"]),
        ],
        vec![definition],
    ));
    let descriptors = historical_subgraph_only_descriptors();
    let document = parse_ui_workflow_value_with_descriptors(&source, &descriptors).unwrap();
    let normalized = normalize_ui_workflow(&document, &descriptors).unwrap();
    let ids = normalized
        .source_to_api
        .keys()
        .filter(|id| id.contains("subgraph/"))
        .map(|id| id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().any(|id| id.contains("instance[9]")));
    assert!(ids.iter().any(|id| id.contains("instance[10]")));
}

#[test]
fn subgraph_instance_widget_values_override_inner_defaults_only_when_unconnected() {
    let definition = synthetic_subgraph_definition(
        "definition-instance-widget",
        serde_json::json!([{
            "id": "input-slot",
            "name": "value",
            "type": "INT",
            "linkIds": [1]
        }]),
        serde_json::json!([]),
        vec![{
            let mut target = synthetic_widget_target_node(20, "Target", "value", "INT", "value", 1);
            target["widgets_values"] = serde_json::json!([5]);
            target["widgets_values_named"] = serde_json::json!({"value": 5});
            target
        }],
        vec![serde_json::json!({
            "id": 1,
            "origin_id": -10,
            "origin_slot": 0,
            "target_id": 20,
            "target_slot": 0,
            "type": "INT"
        })],
    );
    let mut disconnected_instance =
        synthetic_composite_node(9, "definition-instance-widget", None, vec![], "INT");
    disconnected_instance["widgets_values"] = serde_json::json!([42]);
    disconnected_instance["widgets_values_named"] = serde_json::json!({"value": 42});
    let disconnected = normalize_synthetic_with(
        synthetic_subgraph_workflow(
            vec![disconnected_instance],
            vec![],
            vec![definition.clone()],
        ),
        &synthetic_descriptors(RecognitionDeclaredType::Integer, true),
    )
    .expect("disconnected promoted input should use instance value");
    let disconnected_target = disconnected
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .expect("flattened target should remain");
    assert_eq!(disconnected_target["inputs"]["value"], 42);

    let mut connected_instance =
        synthetic_composite_node(9, "definition-instance-widget", Some(10), vec![], "INT");
    connected_instance["widgets_values"] = serde_json::json!([42]);
    connected_instance["widgets_values_named"] = serde_json::json!({"value": 42});
    let connected = normalize_synthetic_with(
        synthetic_subgraph_workflow(
            vec![synthetic_source_node(1, vec![10]), connected_instance],
            vec![serde_json::json!([10, 1, 0, 9, 0, "INT"])],
            vec![definition],
        ),
        &synthetic_descriptors(RecognitionDeclaredType::Integer, true),
    )
    .expect("connected input should remain link-authoritative");
    let connected_target = connected
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .expect("flattened target should remain");
    assert_eq!(
        connected_target["inputs"]["value"],
        serde_json::json!(["1", 0])
    );
}

#[test]
fn subgraph_external_input_fanout_preserves_all_boundary_consumers() {
    let definition = synthetic_subgraph_definition(
        "definition-input-fanout",
        serde_json::json!([{
            "id": "input-slot",
            "name": "value",
            "type": "INT",
            "linkIds": [1, 2]
        }]),
        serde_json::json!([]),
        vec![
            synthetic_runtime_target_node(20, Some(1)),
            synthetic_runtime_target_node(30, Some(2)),
        ],
        vec![
            serde_json::json!({
                "id": 1, "origin_id": -10, "origin_slot": 0,
                "target_id": 20, "target_slot": 0, "type": "INT"
            }),
            serde_json::json!({
                "id": 2, "origin_id": -10, "origin_slot": 0,
                "target_id": 30, "target_slot": 0, "type": "INT"
            }),
        ],
    );
    let source = synthetic_subgraph_workflow(
        vec![
            synthetic_source_node(1, vec![10]),
            synthetic_composite_node(9, "definition-input-fanout", Some(10), vec![], "INT"),
        ],
        vec![serde_json::json!([10, 1, 0, 9, 0, "INT"])],
        vec![definition],
    );
    let normalized = normalize_synthetic_with(
        source,
        &synthetic_descriptors(RecognitionDeclaredType::Integer, true),
    )
    .expect("input fan-out should normalize");
    assert_eq!(
        normalized
            .api_value
            .as_object()
            .unwrap()
            .values()
            .filter(|node| node["class_type"] == "Target")
            .count(),
        2
    );
}

#[test]
fn ambiguous_proxy_widget_and_unknown_composite_definition_fail_closed() {
    let definition_id = "definition-proxy";
    let definition = synthetic_subgraph_definition(
        definition_id,
        serde_json::json!([{
            "id": "input-slot",
            "name": "value",
            "type": "INT",
            "linkIds": [1]
        }]),
        serde_json::json!([]),
        vec![synthetic_runtime_target_node(20, Some(1))],
        vec![serde_json::json!({
            "id": 1, "origin_id": -10, "origin_slot": 0,
            "target_id": 20, "target_slot": 0, "type": "INT"
        })],
    );
    let mut ambiguous_proxy = synthetic_composite_node(9, definition_id, Some(10), vec![], "INT");
    ambiguous_proxy["properties"]["proxyWidgets"] =
        serde_json::json!([["20", "value"], ["20", "value"]]);
    let proxy_source = synthetic_subgraph_workflow(
        vec![synthetic_source_node(1, vec![10]), ambiguous_proxy],
        vec![serde_json::json!([10, 1, 0, 9, 0, "INT"])],
        vec![definition.clone()],
    );
    assert_eq!(
        parse_ui_workflow_value(&proxy_source).unwrap_err().code,
        "AMBIGUOUS_SUBGRAPH_PROXY_WIDGET"
    );

    let mut unknown = synthetic_subgraph_workflow(
        vec![serde_json::json!({
            "id": 9,
            "type": "unregistered-composite",
            "mode": 0,
            "inputs": [],
            "outputs": [],
            "properties": {"proxyWidgets": [["20", "value"]]},
            "widgets_values": []
        })],
        vec![],
        vec![definition],
    );
    unknown["definitions"]["subgraphs"][0]["id"] = Value::from("different-definition");
    assert_eq!(
        parse_ui_workflow_value(&unknown).unwrap_err().code,
        "UNKNOWN_COMPOSITE_DEFINITION"
    );
}

#[test]
fn subgraph_output_boundary_and_multi_output_slots_preserve_execution_edges() {
    let single_output_definition = synthetic_subgraph_definition(
        "definition-output",
        serde_json::json!([]),
        serde_json::json!([{
            "id": "output-slot",
            "name": "result",
            "type": "INT",
            "linkIds": [1]
        }]),
        vec![synthetic_source_node(20, vec![1])],
        vec![serde_json::json!({
            "id": 1,
            "origin_id": 20,
            "origin_slot": 0,
            "target_id": -20,
            "target_slot": 0,
            "type": "INT"
        })],
    );
    let source = synthetic_subgraph_workflow(
        vec![
            synthetic_composite_node(9, "definition-output", None, vec![12], "INT"),
            synthetic_runtime_target_node(2, Some(12)),
        ],
        vec![serde_json::json!([12, 9, 0, 2, 0, "INT"])],
        vec![single_output_definition],
    );
    let descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let document = parse_ui_workflow_value(&source).expect("output subgraph should parse");
    let normalized =
        normalize_ui_workflow(&document, &descriptors).expect("output boundary should normalize");
    let target = normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .unwrap();
    let origin_id = target["inputs"]["value"][0].as_str().unwrap();
    assert!(origin_id.contains("0:"));

    let multi_definition = synthetic_subgraph_definition(
        "definition-multi-output",
        serde_json::json!([]),
        serde_json::json!([
            {"id": "output-a", "name": "a", "type": "INT", "linkIds": [1]},
            {"id": "output-b", "name": "b", "type": "INT", "linkIds": [2]}
        ]),
        vec![
            synthetic_source_node(20, vec![1]),
            synthetic_source_node(21, vec![2]),
        ],
        vec![
            serde_json::json!({
                "id": 1, "origin_id": 20, "origin_slot": 0,
                "target_id": -20, "target_slot": 0, "type": "INT"
            }),
            serde_json::json!({
                "id": 2, "origin_id": 21, "origin_slot": 0,
                "target_id": -20, "target_slot": 1, "type": "INT"
            }),
        ],
    );
    let mut composite =
        synthetic_composite_node(9, "definition-multi-output", None, vec![12, 13], "INT");
    composite["outputs"] = serde_json::json!([
        {"name": "a", "type": "INT", "links": [12]},
        {"name": "b", "type": "INT", "links": [13]}
    ]);
    let multi_source = synthetic_subgraph_workflow(
        vec![
            composite,
            synthetic_runtime_target_node(2, Some(12)),
            synthetic_runtime_target_node(3, Some(13)),
        ],
        vec![
            serde_json::json!([12, 9, 0, 2, 0, "INT"]),
            serde_json::json!([13, 9, 1, 3, 0, "INT"]),
        ],
        vec![multi_definition],
    );
    let multi_document = parse_ui_workflow_value(&multi_source).unwrap();
    let multi_normalized = normalize_ui_workflow(&multi_document, &descriptors).unwrap();
    let targets = multi_normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .filter(|node| node["class_type"] == "Target")
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    assert_ne!(
        targets[0]["inputs"]["value"][0],
        targets[1]["inputs"]["value"][0]
    );
}

#[test]
fn repeated_instances_get_non_colliding_hierarchical_source_ids() {
    let definition = synthetic_subgraph_definition(
        "definition-reused",
        serde_json::json!([]),
        serde_json::json!([{
            "id": "result",
            "name": "result",
            "type": "INT",
            "linkIds": [1]
        }]),
        vec![synthetic_source_node(20, vec![1])],
        vec![serde_json::json!({
            "id": 1, "origin_id": 20, "origin_slot": 0,
            "target_id": -20, "target_slot": 0, "type": "INT"
        })],
    );
    let source = synthetic_subgraph_workflow(
        vec![
            synthetic_composite_node(9, "definition-reused", None, vec![12], "INT"),
            synthetic_composite_node(10, "definition-reused", None, vec![13], "INT"),
            synthetic_runtime_target_node(2, Some(12)),
            synthetic_runtime_target_node(3, Some(13)),
        ],
        vec![
            serde_json::json!([12, 9, 0, 2, 0, "INT"]),
            serde_json::json!([13, 10, 0, 3, 0, "INT"]),
        ],
        vec![definition],
    );
    let descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let document = parse_ui_workflow_value(&source).unwrap();
    let normalized = normalize_ui_workflow(&document, &descriptors).unwrap();
    let ids = normalized
        .source_to_api
        .keys()
        .filter(|id| id.contains("subgraph/"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert!(ids.iter().any(|id| id.contains("instance[9]")));
    assert!(ids.iter().any(|id| id.contains("instance[10]")));
}

#[test]
fn repeated_subgraph_instances_isolate_alias_scopes() {
    let definition = synthetic_subgraph_definition(
        "definition-alias-scope",
        serde_json::json!([]),
        serde_json::json!([]),
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
    let source = synthetic_subgraph_workflow(
        vec![
            synthetic_composite_node(9, "definition-alias-scope", None, vec![], "INT"),
            synthetic_composite_node(10, "definition-alias-scope", None, vec![], "INT"),
        ],
        vec![],
        vec![definition],
    );
    let normalized = normalize_synthetic(source).expect(
        "identical alias names in separate subgraph instances must not create an ambiguous producer",
    );
    let target_inputs = normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .filter(|node| node["class_type"] == "Target")
        .map(|node| node["inputs"]["value"].clone())
        .collect::<Vec<_>>();
    assert_eq!(target_inputs.len(), 2);
    assert_ne!(target_inputs[0], target_inputs[1]);
}

#[test]
fn nested_subgraphs_flatten_with_deep_deterministic_ids() {
    let nested_definition = synthetic_subgraph_definition(
        "definition-nested-leaf",
        serde_json::json!([]),
        serde_json::json!([{
            "id": "result",
            "name": "result",
            "type": "INT",
            "linkIds": [2]
        }]),
        vec![synthetic_source_node(20, vec![2])],
        vec![serde_json::json!({
            "id": 2, "origin_id": 20, "origin_slot": 0,
            "target_id": -20, "target_slot": 0, "type": "INT"
        })],
    );
    let outer_definition = synthetic_subgraph_definition(
        "definition-nested-outer",
        serde_json::json!([]),
        serde_json::json!([{
            "id": "result",
            "name": "result",
            "type": "INT",
            "linkIds": [1]
        }]),
        vec![synthetic_composite_node(
            50,
            "definition-nested-leaf",
            None,
            vec![1],
            "INT",
        )],
        vec![serde_json::json!({
            "id": 1, "origin_id": 50, "origin_slot": 0,
            "target_id": -20, "target_slot": 0, "type": "INT"
        })],
    );
    let source = synthetic_subgraph_workflow(
        vec![
            synthetic_composite_node(9, "definition-nested-outer", None, vec![12], "INT"),
            synthetic_runtime_target_node(2, Some(12)),
        ],
        vec![serde_json::json!([12, 9, 0, 2, 0, "INT"])],
        vec![outer_definition, nested_definition],
    );
    let descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let first = parse_ui_workflow_value(&source).unwrap();
    let second = parse_ui_workflow_value(&source).unwrap();
    let first_normalized = normalize_ui_workflow(&first, &descriptors).unwrap();
    let second_normalized = normalize_ui_workflow(&second, &descriptors).unwrap();
    assert_eq!(first_normalized.api_value, second_normalized.api_value);
    assert!(first_normalized
        .source_to_api
        .keys()
        .any(|id| id.contains("instance[9]/instance[50]")));

    let direct = synthetic_header(
        vec![
            synthetic_source_node(20, vec![12]),
            synthetic_runtime_target_node(2, Some(12)),
        ],
        vec![serde_json::json!([12, 20, 0, 2, 0, "INT"])],
    );
    let direct_normalized = normalize_synthetic_with(direct, &descriptors).unwrap();
    let direct_classes = direct_normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .filter_map(|node| node["class_type"].as_str())
        .collect::<Vec<_>>();
    let nested_classes = first_normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .filter_map(|node| node["class_type"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        direct_classes.iter().collect::<BTreeSet<_>>(),
        nested_classes.iter().collect::<BTreeSet<_>>()
    );
    let direct_target = direct_normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .unwrap();
    let nested_target = first_normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .unwrap();
    assert_eq!(
        direct_normalized.api_value[direct_target["inputs"]["value"][0].as_str().unwrap()]
            ["class_type"],
        first_normalized.api_value[nested_target["inputs"]["value"][0].as_str().unwrap()]
            ["class_type"]
    );
}

#[test]
fn wrapped_and_direct_graphs_have_the_same_execution_shape() {
    let descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let direct = synthetic_header(
        vec![
            synthetic_source_node(1, vec![1]),
            synthetic_runtime_target_node(2, Some(1)),
        ],
        vec![serde_json::json!([1, 1, 0, 2, 0, "INT"])],
    );
    let direct_normalized = normalize_synthetic_with(direct, &descriptors).unwrap();

    let definition = synthetic_subgraph_definition(
        "definition-wrapped",
        serde_json::json!([{
            "id": "input-slot",
            "name": "value",
            "type": "INT",
            "linkIds": [2]
        }]),
        serde_json::json!([]),
        vec![synthetic_runtime_target_node(20, Some(2))],
        vec![serde_json::json!({
            "id": 2, "origin_id": -10, "origin_slot": 0,
            "target_id": 20, "target_slot": 0, "type": "INT"
        })],
    );
    let wrapped = synthetic_subgraph_workflow(
        vec![
            synthetic_source_node(1, vec![10]),
            synthetic_composite_node(9, "definition-wrapped", Some(10), vec![], "INT"),
        ],
        vec![serde_json::json!([10, 1, 0, 9, 0, "INT"])],
        vec![definition],
    );
    let wrapped_normalized = normalize_synthetic_with(wrapped, &descriptors).unwrap();

    let class_types = |normalized: &NormalizedWorkflow| {
        let mut classes = normalized
            .api_value
            .as_object()
            .unwrap()
            .values()
            .filter_map(|node| node["class_type"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        classes.sort_unstable();
        classes
    };
    assert_eq!(
        class_types(&direct_normalized),
        class_types(&wrapped_normalized)
    );
    let direct_target = direct_normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .unwrap();
    let wrapped_target = wrapped_normalized
        .api_value
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["class_type"] == "Target")
        .unwrap();
    let direct_origin = direct_target["inputs"]["value"][0].as_str().unwrap();
    let wrapped_origin = wrapped_target["inputs"]["value"][0].as_str().unwrap();
    assert_eq!(
        direct_normalized.api_value[direct_origin]["class_type"],
        wrapped_normalized.api_value[wrapped_origin]["class_type"]
    );
}

#[test]
fn recursive_and_ambiguous_subgraphs_fail_closed_with_origin_diagnostics() {
    let self_recursive = synthetic_subgraph_definition(
        "definition-self",
        serde_json::json!([]),
        serde_json::json!([]),
        vec![synthetic_composite_node(
            20,
            "definition-self",
            None,
            vec![],
            "INT",
        )],
        vec![],
    );
    let self_source = synthetic_subgraph_workflow(
        vec![synthetic_composite_node(
            9,
            "definition-self",
            None,
            vec![],
            "INT",
        )],
        vec![],
        vec![self_recursive],
    );
    let error = parse_ui_workflow_value(&self_source).unwrap_err();
    assert_eq!(error.code, "SUBGRAPH_RECURSION");
    assert!(error.message.contains("instance[9]"));

    let definition_a = synthetic_subgraph_definition(
        "definition-a",
        serde_json::json!([]),
        serde_json::json!([]),
        vec![synthetic_composite_node(
            20,
            "definition-b",
            None,
            vec![],
            "INT",
        )],
        vec![],
    );
    let definition_b = synthetic_subgraph_definition(
        "definition-b",
        serde_json::json!([]),
        serde_json::json!([]),
        vec![synthetic_composite_node(
            30,
            "definition-a",
            None,
            vec![],
            "INT",
        )],
        vec![],
    );
    let mutual_source = synthetic_subgraph_workflow(
        vec![synthetic_composite_node(
            9,
            "definition-a",
            None,
            vec![],
            "INT",
        )],
        vec![],
        vec![definition_a, definition_b],
    );
    assert_eq!(
        parse_ui_workflow_value(&mutual_source).unwrap_err().code,
        "SUBGRAPH_RECURSION"
    );

    let missing_boundary = synthetic_subgraph_definition(
        "definition-missing",
        serde_json::json!([{
            "id": "input-slot",
            "name": "value",
            "type": "INT",
            "linkIds": [1]
        }]),
        serde_json::json!([]),
        vec![synthetic_runtime_target_node(20, None)],
        vec![],
    );
    let missing_source = synthetic_subgraph_workflow(
        vec![synthetic_composite_node(
            9,
            "definition-missing",
            None,
            vec![],
            "INT",
        )],
        vec![],
        vec![missing_boundary],
    );
    assert_eq!(
        parse_ui_workflow_value(&missing_source).unwrap_err().code,
        "MISSING_SUBGRAPH_INPUT_BOUNDARY"
    );
}

#[test]
fn duplicate_definitions_and_ambiguous_output_boundaries_fail_closed() {
    let empty_definition = synthetic_subgraph_definition(
        "definition-duplicate",
        serde_json::json!([]),
        serde_json::json!([]),
        vec![],
        vec![],
    );
    let duplicate_source = synthetic_subgraph_workflow(
        vec![],
        vec![],
        vec![empty_definition.clone(), empty_definition],
    );
    assert_eq!(
        parse_ui_workflow_value(&duplicate_source).unwrap_err().code,
        "DUPLICATE_SUBGRAPH_DEFINITION"
    );

    let ambiguous_definition = synthetic_subgraph_definition(
        "definition-ambiguous-output",
        serde_json::json!([]),
        serde_json::json!([{
            "id": "result",
            "name": "result",
            "type": "INT",
            "linkIds": [1, 2]
        }]),
        vec![
            synthetic_source_node(20, vec![1]),
            synthetic_source_node(21, vec![2]),
        ],
        vec![
            serde_json::json!({
                "id": 1, "origin_id": 20, "origin_slot": 0,
                "target_id": -20, "target_slot": 0, "type": "INT"
            }),
            serde_json::json!({
                "id": 2, "origin_id": 21, "origin_slot": 0,
                "target_id": -20, "target_slot": 0, "type": "INT"
            }),
        ],
    );
    let ambiguous_source = synthetic_subgraph_workflow(
        vec![synthetic_composite_node(
            9,
            "definition-ambiguous-output",
            None,
            vec![],
            "INT",
        )],
        vec![],
        vec![ambiguous_definition],
    );
    assert_eq!(
        parse_ui_workflow_value(&ambiguous_source).unwrap_err().code,
        "AMBIGUOUS_SUBGRAPH_OUTPUT_BOUNDARY"
    );

    let missing_output_definition = synthetic_subgraph_definition(
        "definition-missing-output",
        serde_json::json!([]),
        serde_json::json!([{
            "id": "result",
            "name": "result",
            "type": "INT",
            "linkIds": []
        }]),
        vec![synthetic_source_node(20, vec![])],
        vec![],
    );
    let missing_output_source = synthetic_subgraph_workflow(
        vec![synthetic_composite_node(
            9,
            "definition-missing-output",
            None,
            vec![],
            "INT",
        )],
        vec![],
        vec![missing_output_definition],
    );
    assert_eq!(
        parse_ui_workflow_value(&missing_output_source)
            .unwrap_err()
            .code,
        "MISSING_SUBGRAPH_OUTPUT_BOUNDARY"
    );

    let ambiguous_input_definition = synthetic_subgraph_definition(
        "definition-ambiguous-input",
        serde_json::json!([
            {"id": "input-a", "name": "value", "type": "INT", "linkIds": [1]},
            {"id": "input-b", "name": "value", "type": "INT", "linkIds": [2]}
        ]),
        serde_json::json!([]),
        vec![
            synthetic_runtime_target_node(20, Some(1)),
            synthetic_runtime_target_node(30, Some(2)),
        ],
        vec![
            serde_json::json!({
                "id": 1, "origin_id": -10, "origin_slot": 0,
                "target_id": 20, "target_slot": 0, "type": "INT"
            }),
            serde_json::json!({
                "id": 2, "origin_id": -10, "origin_slot": 1,
                "target_id": 30, "target_slot": 0, "type": "INT"
            }),
        ],
    );
    let mut ambiguous_input_instance =
        synthetic_composite_node(9, "definition-ambiguous-input", Some(10), vec![], "INT");
    ambiguous_input_instance["inputs"] = serde_json::json!([{
        "name": "value",
        "type": "INT",
        "link": 10
    }]);
    let ambiguous_input_source = synthetic_subgraph_workflow(
        vec![synthetic_source_node(1, vec![10]), ambiguous_input_instance],
        vec![serde_json::json!([10, 1, 0, 9, 0, "INT"])],
        vec![ambiguous_input_definition],
    );
    assert_eq!(
        parse_ui_workflow_value(&ambiguous_input_source)
            .unwrap_err()
            .code,
        "AMBIGUOUS_SUBGRAPH_INPUT_BOUNDARY"
    );
}

#[test]
fn uuid_looking_runtime_class_is_not_composite_by_appearance() {
    let uuid = "00000000-0000-4000-8000-000000000099";
    let source = synthetic_header(
        vec![serde_json::json!({
            "id": 1,
            "type": uuid,
            "mode": 0,
            "inputs": [],
            "outputs": [{"name": "INT", "type": "INT", "links": []}],
            "widgets_values": []
        })],
        vec![],
    );
    let document = parse_ui_workflow_value(&source).unwrap();
    assert!(!document
        .features
        .observations
        .iter()
        .any(|item| { item.feature == WorkflowUiFeature::UuidCompositeNodeType }));
    normalize_ui_workflow(&document, &synthetic_uuid_runtime_descriptor_set())
        .expect("object_info-backed UUID runtime class must remain executable");
}

#[test]
fn foundation_presentation_node_inside_subgraph_is_normalized_by_existing_authority() {
    let definition = synthetic_subgraph_definition(
        "definition-foundation",
        serde_json::json!([]),
        serde_json::json!([{
            "id": "result",
            "name": "result",
            "type": "INT",
            "linkIds": [1]
        }]),
        vec![
            synthetic_source_node(20, vec![1]),
            serde_json::json!({
                "id": 30,
                "type": "Note",
                "mode": 0,
                "inputs": [],
                "outputs": [],
                "widgets_values": ["not executable"]
            }),
        ],
        vec![serde_json::json!({
            "id": 1, "origin_id": 20, "origin_slot": 0,
            "target_id": -20, "target_slot": 0, "type": "INT"
        })],
    );
    let source = synthetic_subgraph_workflow(
        vec![
            synthetic_composite_node(9, "definition-foundation", None, vec![12], "INT"),
            synthetic_runtime_target_node(2, Some(12)),
        ],
        vec![serde_json::json!([12, 9, 0, 2, 0, "INT"])],
        vec![definition],
    );
    let descriptors = synthetic_descriptors(RecognitionDeclaredType::Integer, true);
    let document = parse_ui_workflow_value(&source).unwrap();
    let normalized = normalize_ui_workflow(&document, &descriptors)
        .expect("presentation foundation node should be removed safely");
    assert_eq!(
        normalized
            .api_value
            .as_object()
            .unwrap()
            .values()
            .filter(|node| node["class_type"] == "Note")
            .count(),
        0
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
