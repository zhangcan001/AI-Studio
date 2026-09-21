use ai_studio_lib::application::workflow_recognition_schema::RecognitionSchemaContext;
use ai_studio_lib::application::workflow_ui_normalizer::{
    normalize_ui_workflow, parse_ui_workflow, parse_ui_workflow_value, NormalizedWorkflow,
};
use ai_studio_lib::application::workflow_ui_serialization::{
    canonical_schema_fingerprint, FrontendSerializationProfile, NormalizationCompatibilityContext,
    SupportedNormalizationCompatibilitySet, UiSerializationDescriptorSet,
};
use serde_json::{Map, Value};
use std::{
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
