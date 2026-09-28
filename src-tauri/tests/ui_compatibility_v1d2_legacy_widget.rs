use ai_studio_lib::application::workflow_recognition_schema::RecognitionSchemaContext;
use ai_studio_lib::application::workflow_ui_normalizer::{
    normalize_ui_workflow, parse_ui_workflow_value,
};
use ai_studio_lib::application::workflow_ui_serialization::{
    FrontendSerializationContract, FrontendSerializationProfile, NormalizationCompatibilityContext,
    UiInputEvidence, UiSerializationDescriptorSet, NORMALIZER_POLICY_VERSION,
    SERIALIZATION_PROFILE_VERSION, SUPPORTED_FRONTEND_VERSION, SUPPORTED_WORKFLOW_FORMAT_VERSION,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn context(frontend_version: &str) -> NormalizationCompatibilityContext {
    NormalizationCompatibilityContext {
        workflow_format_version: SUPPORTED_WORKFLOW_FORMAT_VERSION.to_owned(),
        frontend_version: frontend_version.to_owned(),
        schema_fingerprint: "v1d2-test-schema".to_owned(),
        serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
        normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
        source_frontend_revision: None,
    }
}

fn descriptors(
    class_type: &str,
    inputs: Value,
    frontend_version: &str,
    contract: FrontendSerializationContract,
) -> UiSerializationDescriptorSet {
    let names = inputs
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    descriptors_from_schema(
        class_type,
        json!({
            "input": {"required": inputs},
            "input_order": {"required": names},
            "output": [],
        }),
        frontend_version,
        contract,
    )
}

fn descriptors_from_schema(
    class_type: &str,
    schema_node: Value,
    frontend_version: &str,
    contract: FrontendSerializationContract,
) -> UiSerializationDescriptorSet {
    let schema = RecognitionSchemaContext::parse(&json!({class_type: schema_node}));
    let context = context(frontend_version);
    let profile = match contract {
        FrontendSerializationContract::Current => {
            assert_eq!(frontend_version, SUPPORTED_FRONTEND_VERSION);
            FrontendSerializationProfile::from_context(&context).unwrap()
        }
        FrontendSerializationContract::LegacyWidgetSlotV0 => {
            FrontendSerializationProfile::legacy_widget_slot_v0_from_context(&context).unwrap()
        }
        FrontendSerializationContract::LegacyDynamicInputV0 => {
            FrontendSerializationProfile::legacy_dynamic_input_v0_from_context(&context).unwrap()
        }
        FrontendSerializationContract::LegacySubgraphBoundaryProxyV0 => {
            panic!("LegacySubgraphBoundaryProxyV0 has no normalization profile")
        }
    };
    UiSerializationDescriptorSet::build(&schema, profile, context).unwrap()
}

fn evidence(
    name: &str,
    widget_name: Option<&str>,
    linked: bool,
    shape: Option<i64>,
) -> UiInputEvidence {
    UiInputEvidence {
        name: name.to_owned(),
        widget_name: widget_name.map(str::to_owned),
        shape,
        linked,
    }
}

fn legacy_values(
    descriptors: &UiSerializationDescriptorSet,
    class_type: &str,
    inputs: &[UiInputEvidence],
    values: &[Value],
) -> Result<Vec<(String, Value)>, String> {
    descriptors
        .consume_legacy_positional_values(class_type, inputs, &BTreeSet::new(), values, None)
        .map_err(|error| error.to_string())
}

fn legacy_document(frontend_version: Option<&str>, class_type: &str, values: Value) -> Value {
    let mut root = json!({
        "version": 0.4,
        "nodes": [{
            "id": 1,
            "type": class_type,
            "mode": 0,
            "inputs": [],
            "outputs": [],
            "widgets_values": values
        }],
        "links": []
    });
    if let Some(frontend_version) = frontend_version {
        root["extra"] = json!({"frontendVersion": frontend_version});
    }
    root
}

#[test]
fn legacy_positional_widgets_bind_in_declared_order() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"first": ["INT", {}], "second": ["STRING", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let values =
        legacy_values(&descriptors, "NodeAlpha", &[], &[json!(7), json!("second")]).unwrap();
    assert_eq!(
        values,
        vec![
            ("first".to_owned(), json!(7)),
            ("second".to_owned(), json!("second"))
        ]
    );
}

#[test]
fn explicit_legacy_widget_declaration_order_overrides_schema_order() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"first": ["INT", {}], "second": ["INT", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let inputs = vec![
        evidence("second", Some("second"), false, None),
        evidence("first", Some("first"), false, None),
    ];
    let values = legacy_values(&descriptors, "NodeAlpha", &inputs, &[json!(2), json!(1)]).unwrap();
    assert_eq!(
        values,
        vec![
            ("second".to_owned(), json!(2)),
            ("first".to_owned(), json!(1))
        ]
    );
}

#[test]
fn legacy_linked_input_residual_widget_does_not_shift_cursor() {
    let descriptors = descriptors(
        "NodeBeta",
        json!({"linked": ["INT", {}], "runtime": ["INT", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let inputs = vec![evidence("linked", Some("linked"), true, None)];
    let mut linked_names = BTreeSet::new();
    linked_names.insert("linked".to_owned());
    let values = descriptors
        .consume_legacy_positional_values(
            "NodeBeta",
            &inputs,
            &linked_names,
            &[json!(999), json!(11)],
            None,
        )
        .unwrap();
    assert_eq!(values, vec![("runtime".to_owned(), json!(11))]);
}

#[test]
fn legacy_frontend_control_does_not_bind_runtime_input() {
    let descriptors = descriptors(
        "NodeGamma",
        json!({"seed": ["INT", {}], "steps": ["INT", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let values = legacy_values(
        &descriptors,
        "NodeGamma",
        &[],
        &[json!(123), json!("fixed"), json!(20)],
    )
    .unwrap();
    assert_eq!(
        values,
        vec![
            ("seed".to_owned(), json!(123)),
            ("steps".to_owned(), json!(20))
        ]
    );
}

#[test]
fn legacy_presentation_widget_consumes_cursor_without_runtime_binding() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"file": ["STRING", {"image_upload": true}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let values = legacy_values(
        &descriptors,
        "NodeAlpha",
        &[],
        &[json!("input.png"), json!("image")],
    )
    .unwrap();
    assert_eq!(values, vec![("file".to_owned(), json!("input.png"))]);
}

#[test]
fn legacy_control_slot_consumes_cursor_without_runtime_binding() {
    let descriptors = descriptors_from_schema(
        "NodeAlpha",
        json!({
            "input": {
                "required": {"runtime": ["INT", {}]},
                "hidden": {"control": ["STRING", {}]}
            },
            "input_order": {"required": ["runtime"], "hidden": ["control"]},
            "output": []
        }),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let inputs = vec![
        evidence("control", Some("control"), false, None),
        evidence("runtime", Some("runtime"), false, None),
    ];
    let values = legacy_values(
        &descriptors,
        "NodeAlpha",
        &inputs,
        &[json!("historical-control"), json!(7)],
    )
    .unwrap();
    assert_eq!(values, vec![("runtime".to_owned(), json!(7))]);
}

#[test]
fn legacy_non_runtime_widget_before_runtime_widget_does_not_shift_binding() {
    let descriptors = descriptors_from_schema(
        "NodeBeta",
        json!({
            "input": {
                "required": {"runtime": ["INT", {}]},
                "hidden": {"control": ["STRING", {}]}
            },
            "input_order": {"required": ["runtime"], "hidden": ["control"]},
            "output": []
        }),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let inputs = vec![
        evidence("control", Some("control"), false, None),
        evidence("runtime", Some("runtime"), false, None),
    ];
    let values = legacy_values(
        &descriptors,
        "NodeBeta",
        &inputs,
        &[json!("frontend-control"), json!(11)],
    )
    .unwrap();
    assert_eq!(values, vec![("runtime".to_owned(), json!(11))]);
}

#[test]
fn unclassified_legacy_extra_widget_fails_closed() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"value": ["INT", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let error = legacy_values(&descriptors, "NodeAlpha", &[], &[json!(1), json!(2)]).unwrap_err();
    assert!(error.starts_with("legacy_widget_slot_extra:"), "{error}");
}

#[test]
fn missing_legacy_widget_value_fails_closed() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"first": ["INT", {}], "second": ["INT", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let error = legacy_values(&descriptors, "NodeAlpha", &[], &[json!(1)]).unwrap_err();
    assert!(error.starts_with("legacy_widget_slot_missing:"), "{error}");
}

#[test]
fn ambiguous_legacy_widget_binding_fails_closed() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"first": ["INT", {}], "second": ["INT", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let inputs = vec![evidence(
        "first_input",
        Some("second_input"),
        false,
        Some(7),
    )];
    let error = descriptors
        .consume_legacy_positional_values("NodeAlpha", &inputs, &BTreeSet::new(), &[json!(1)], None)
        .unwrap_err();
    assert!(error.code == "legacy_widget_slot_ambiguous", "{}", error);
}

#[test]
fn legacy_widget_type_mismatch_fails_closed() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"value": ["INT", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let document =
        parse_ui_workflow_value(&legacy_document(None, "NodeAlpha", json!(["not-int"]))).unwrap();
    let error = normalize_ui_workflow(&document, &descriptors).unwrap_err();
    assert_eq!(error.code, "legacy_widget_type_mismatch");
}

#[test]
fn unknown_version_legacy_widget_profile_normalizes() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"value": ["INT", {}]}),
        "unknown",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let document =
        parse_ui_workflow_value(&legacy_document(None, "NodeAlpha", json!([7]))).unwrap();
    let normalized = normalize_ui_workflow(&document, &descriptors).unwrap();
    assert_eq!(normalized.api_value["1"]["inputs"]["value"], json!(7));
}

#[test]
fn same_legacy_widget_profile_same_normalization() {
    let first_descriptors = descriptors(
        "NodeAlpha",
        json!({"value": ["INT", {}]}),
        "1.27.10",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let second_descriptors = descriptors(
        "NodeAlpha",
        json!({"value": ["INT", {}]}),
        "1.45.15",
        FrontendSerializationContract::LegacyWidgetSlotV0,
    );
    let first_document =
        parse_ui_workflow_value(&legacy_document(Some("1.27.10"), "NodeAlpha", json!([7])))
            .unwrap();
    let second_document =
        parse_ui_workflow_value(&legacy_document(Some("1.45.15"), "NodeAlpha", json!([7])))
            .unwrap();
    let first = normalize_ui_workflow(&first_document, &first_descriptors).unwrap();
    let second = normalize_ui_workflow(&second_document, &second_descriptors).unwrap();
    assert_eq!(first.api_value, second.api_value);
}

#[test]
fn legacy_widget_logic_not_used_for_current_profile() {
    let descriptors = descriptors(
        "NodeAlpha",
        json!({"value": ["INT", {}]}),
        SUPPORTED_FRONTEND_VERSION,
        FrontendSerializationContract::Current,
    );
    let document = parse_ui_workflow_value(&legacy_document(
        Some(SUPPORTED_FRONTEND_VERSION),
        "NodeAlpha",
        json!([7]),
    ))
    .unwrap();
    let error = normalize_ui_workflow(&document, &descriptors).unwrap_err();
    assert_ne!(error.code, "legacy_widget_slot_extra");
    assert_ne!(error.code, "legacy_widget_slot_missing");
}

#[test]
fn legacy_widget_normalization_is_class_name_independent() {
    for class_type in ["NodeAlpha", "NodeBeta", "NodeGamma"] {
        let descriptors = descriptors(
            class_type,
            json!({"value": ["INT", {}]}),
            "unknown",
            FrontendSerializationContract::LegacyWidgetSlotV0,
        );
        let values = legacy_values(&descriptors, class_type, &[], &[json!(7)]).unwrap();
        assert_eq!(values, vec![("value".to_owned(), json!(7))]);
    }
}
