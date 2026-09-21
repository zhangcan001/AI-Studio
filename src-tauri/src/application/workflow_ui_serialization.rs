use crate::application::workflow_recognition_schema::{
    MediaKind, RecognitionDeclaredType, RecognitionInputSchema, RecognitionNodeSchema,
    RecognitionSchemaContext,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

pub const SERIALIZATION_PROFILE_VERSION: &str = "comfyui-frontend-export-profile-v1";
pub const NORMALIZER_POLICY_VERSION: &str = "phase2b-normalizer-policy-v1";
pub const SUPPORTED_WORKFLOW_FORMAT_VERSION: &str = "0.4";
pub const SUPPORTED_FRONTEND_VERSION: &str = "1.52.7";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationCompatibilityContext {
    pub workflow_format_version: String,
    pub frontend_version: String,
    pub schema_fingerprint: String,
    pub serialization_profile_version: String,
    pub normalizer_policy_version: String,
    pub source_frontend_revision: Option<String>,
}

impl NormalizationCompatibilityContext {
    pub fn from_source(
        workflow_format_version: impl Into<String>,
        frontend_version: Option<&str>,
        schema_fingerprint: impl Into<String>,
    ) -> Result<Self, SerializationDescriptorError> {
        let frontend_version = frontend_version
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                SerializationDescriptorError::new(
                    "FRONTEND_VERSION_UNKNOWN",
                    "source UI workflow has no frontendVersion provenance",
                )
            })?;
        Ok(Self {
            workflow_format_version: workflow_format_version.into(),
            frontend_version: frontend_version.to_owned(),
            schema_fingerprint: schema_fingerprint.into(),
            serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
            normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
            source_frontend_revision: None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportedNormalizationCompatibilitySet {
    entries: Vec<NormalizationCompatibilityContext>,
}

impl SupportedNormalizationCompatibilitySet {
    pub fn exact(entry: NormalizationCompatibilityContext) -> Self {
        Self {
            entries: vec![entry],
        }
    }

    pub fn contains(&self, context: &NormalizationCompatibilityContext) -> bool {
        self.entries.iter().any(|entry| entry == context)
    }

    pub fn require(
        &self,
        context: &NormalizationCompatibilityContext,
    ) -> Result<(), SerializationDescriptorError> {
        if self.contains(context) {
            Ok(())
        } else {
            Err(SerializationDescriptorError::new(
                "UNSUPPORTED_NORMALIZATION_COMPATIBILITY",
                format!(
                    "workflow format {}, frontend {}, schema {}, profile {}, policy {} is not supported",
                    context.workflow_format_version,
                    context.frontend_version,
                    context.schema_fingerprint,
                    context.serialization_profile_version,
                    context.normalizer_policy_version,
                ),
            ))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontendSerializationProfile {
    pub frontend_version: String,
    pub workflow_format_version: String,
    pub serialization_profile_version: String,
    pub source_provenance: String,
    pub frontend_source_revision: Option<String>,
}

impl FrontendSerializationProfile {
    pub fn from_context(
        context: &NormalizationCompatibilityContext,
    ) -> Result<Self, SerializationDescriptorError> {
        if context.frontend_version != SUPPORTED_FRONTEND_VERSION
            || context.workflow_format_version != SUPPORTED_WORKFLOW_FORMAT_VERSION
            || context.serialization_profile_version != SERIALIZATION_PROFILE_VERSION
        {
            return Err(SerializationDescriptorError::new(
                "UNSUPPORTED_FRONTEND_SERIALIZATION_PROFILE",
                "no verified frontend serialization profile exists for this source",
            ));
        }
        Ok(Self {
            frontend_version: context.frontend_version.clone(),
            workflow_format_version: context.workflow_format_version.clone(),
            serialization_profile_version: context.serialization_profile_version.clone(),
            source_provenance: "source extra.frontendVersion".to_owned(),
            frontend_source_revision: context.source_frontend_revision.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerializerKind {
    StandardDirect,
    StandardCombo,
    StandardUpload,
    StandardSeed,
    WorkflowOnlyControl,
    UnsupportedCustom,
    Unknown,
}

impl SerializerKind {
    pub fn is_standard(self) -> bool {
        matches!(
            self,
            Self::StandardDirect | Self::StandardCombo | Self::StandardUpload | Self::StandardSeed
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiInputSerializationDescriptor {
    pub name: String,
    pub declared_type: RecognitionDeclaredType,
    pub required: bool,
    pub hidden: bool,
    pub serializer_kind: SerializerKind,
    pub upload_media_kind: Option<MediaKind>,
    pub enum_options: Vec<String>,
    pub enum_values: Vec<Value>,
    pub numeric_min: Option<f64>,
    pub numeric_max: Option<f64>,
    pub numeric_step: Option<f64>,
    pub multiline: bool,
    pub default_value: Option<Value>,
    pub dynamic: bool,
    pub dynamic_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiNodeSerializationDescriptor {
    pub class_type: String,
    pub display_name: Option<String>,
    pub output_node: bool,
    pub ordered_inputs: Vec<String>,
    pub output_types: Vec<RecognitionDeclaredType>,
    pub output_names: Vec<String>,
    pub output_is_list: Vec<bool>,
    pub inputs: BTreeMap<String, UiInputSerializationDescriptor>,
    pub conditional_inputs:
        BTreeMap<String, BTreeMap<String, BTreeMap<String, UiInputSerializationDescriptor>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiSerializationDescriptorSet {
    pub compatibility: NormalizationCompatibilityContext,
    pub profile: FrontendSerializationProfile,
    pub nodes: BTreeMap<String, UiNodeSerializationDescriptor>,
}

impl UiSerializationDescriptorSet {
    pub fn build(
        schema: &RecognitionSchemaContext,
        profile: FrontendSerializationProfile,
        compatibility: NormalizationCompatibilityContext,
    ) -> Result<Self, SerializationDescriptorError> {
        if profile.frontend_version != compatibility.frontend_version
            || profile.workflow_format_version != compatibility.workflow_format_version
            || profile.serialization_profile_version != compatibility.serialization_profile_version
        {
            return Err(SerializationDescriptorError::new(
                "SERIALIZATION_PROFILE_MISMATCH",
                "frontend profile and compatibility context do not match",
            ));
        }
        if schema.nodes.is_empty() {
            return Err(SerializationDescriptorError::new(
                "SCHEMA_EMPTY",
                "object_info did not produce any node descriptors",
            ));
        }
        let nodes = schema
            .nodes
            .iter()
            .map(|(class_type, node)| (class_type.clone(), descriptor_for_node(node)))
            .collect();
        Ok(Self {
            compatibility,
            profile,
            nodes,
        })
    }

    pub fn node(&self, class_type: &str) -> Option<&UiNodeSerializationDescriptor> {
        self.nodes.get(class_type)
    }

    pub fn input(
        &self,
        class_type: &str,
        input_name: &str,
    ) -> Option<UiInputSerializationDescriptor> {
        let node = self.nodes.get(class_type)?;
        if let Some(input) = node.inputs.get(input_name) {
            return Some(input.clone());
        }
        let base = input_name.split('.').next()?;
        let base_input = node.inputs.get(base)?;
        if base_input.dynamic || base_input.declared_type == RecognitionDeclaredType::DynamicCombo {
            if !base_input.dynamic_names.is_empty()
                && !base_input
                    .dynamic_names
                    .iter()
                    .any(|name| input_name == format!("{base}.{name}"))
                && !input_name.starts_with(&format!("{base}."))
            {
                return None;
            }
            let mut expanded = base_input.clone();
            expanded.name = input_name.to_owned();
            expanded.dynamic = true;
            if base_input.declared_type == RecognitionDeclaredType::DynamicCombo {
                expanded.serializer_kind = SerializerKind::StandardCombo;
            }
            return Some(expanded);
        }
        None
    }

    pub fn input_for_node(
        &self,
        class_type: &str,
        named_values: Option<&BTreeMap<String, Value>>,
        input_name: &str,
    ) -> Option<UiInputSerializationDescriptor> {
        if let Some(input) = self.input(class_type, input_name) {
            return Some(input);
        }
        let node = self.nodes.get(class_type)?;
        for (selector_name, by_value) in &node.conditional_inputs {
            let Some(selected_value) = named_values
                .and_then(|values| values.get(selector_name))
                .and_then(Value::as_str)
            else {
                continue;
            };
            if let Some(input) = by_value
                .get(selected_value)
                .and_then(|inputs| inputs.get(input_name))
            {
                return Some(input.clone());
            }
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerializationDescriptorError {
    pub code: &'static str,
    pub message: String,
}

impl SerializationDescriptorError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for SerializationDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SerializationDescriptorError {}

pub fn canonical_schema_fingerprint(object_info: &Value) -> String {
    let bytes = canonical_schema_bytes(object_info);
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

pub fn canonical_schema_bytes(object_info: &Value) -> Vec<u8> {
    let canonical = canonicalize(object_info);
    let mut bytes = Vec::new();
    let mut serializer = serde_json::Serializer::new(&mut bytes);
    serde::Serialize::serialize(&canonical, &mut serializer)
        .expect("canonical JSON must serialize");
    bytes
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = Map::new();
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            for (key, value) in entries {
                sorted.insert(key.clone(), canonicalize(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

fn descriptor_for_node(node: &RecognitionNodeSchema) -> UiNodeSerializationDescriptor {
    let mut inputs = BTreeMap::new();
    for name in &node.ordered_inputs {
        if let Some(schema) = node.input(name) {
            inputs.insert(name.clone(), descriptor_for_input(schema, false));
        }
    }
    for name in &node.hidden_inputs {
        if let Some(schema) = node.input(name) {
            let mut descriptor = descriptor_for_input(schema, false);
            descriptor.hidden = true;
            descriptor.serializer_kind = SerializerKind::WorkflowOnlyControl;
            inputs.insert(name.clone(), descriptor);
        }
    }
    let conditional_inputs = node
        .conditional_inputs
        .iter()
        .map(|(selector_name, by_value)| {
            (
                selector_name.clone(),
                by_value
                    .iter()
                    .map(|(selected_value, by_name)| {
                        (
                            selected_value.clone(),
                            by_name
                                .iter()
                                .map(|(name, schema)| {
                                    (name.clone(), descriptor_for_input(schema, false))
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    UiNodeSerializationDescriptor {
        class_type: node.class_type.clone(),
        display_name: node.display_name.clone(),
        output_node: node.output_node,
        ordered_inputs: node.ordered_inputs.clone(),
        output_types: node.declared_output_types.clone(),
        output_names: node.output_names.clone(),
        output_is_list: node.output_is_list.clone(),
        inputs,
        conditional_inputs,
    }
}

fn descriptor_for_input(
    schema: &RecognitionInputSchema,
    dynamic: bool,
) -> UiInputSerializationDescriptor {
    let effective_type = if schema.declared_type == RecognitionDeclaredType::Unknown {
        schema
            .dynamic_value_type
            .unwrap_or(RecognitionDeclaredType::Unknown)
    } else {
        schema.declared_type
    };
    let serializer_kind = if schema.socketless {
        SerializerKind::WorkflowOnlyControl
    } else if schema.upload_media_kind.is_some() {
        SerializerKind::StandardUpload
    } else if schema.name == "seed" || schema.name == "noise_seed" {
        SerializerKind::StandardSeed
    } else {
        match effective_type {
            RecognitionDeclaredType::Enum | RecognitionDeclaredType::DynamicCombo => {
                SerializerKind::StandardCombo
            }
            RecognitionDeclaredType::Unknown => SerializerKind::Unknown,
            _ => SerializerKind::StandardDirect,
        }
    };
    UiInputSerializationDescriptor {
        name: schema.name.clone(),
        declared_type: effective_type,
        required: schema.required,
        hidden: false,
        serializer_kind,
        upload_media_kind: schema.upload_media_kind,
        enum_options: schema.enum_options.clone(),
        enum_values: schema.enum_values.clone(),
        numeric_min: schema.numeric_min,
        numeric_max: schema.numeric_max,
        numeric_step: schema.numeric_step,
        multiline: schema.multiline,
        default_value: schema.default_value.clone(),
        dynamic: dynamic
            || schema.dynamic_prefix.is_some()
            || !schema.dynamic_names.is_empty()
            || schema.declared_type == RecognitionDeclaredType::DynamicCombo,
        dynamic_names: schema.dynamic_names.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_schema_fingerprint, FrontendSerializationProfile,
        NormalizationCompatibilityContext, SerializerKind, UiSerializationDescriptorSet,
        NORMALIZER_POLICY_VERSION, SERIALIZATION_PROFILE_VERSION, SUPPORTED_FRONTEND_VERSION,
        SUPPORTED_WORKFLOW_FORMAT_VERSION,
    };
    use crate::application::workflow_recognition_schema::RecognitionSchemaContext;
    use serde_json::json;

    fn context() -> NormalizationCompatibilityContext {
        NormalizationCompatibilityContext {
            workflow_format_version: SUPPORTED_WORKFLOW_FORMAT_VERSION.to_owned(),
            frontend_version: SUPPORTED_FRONTEND_VERSION.to_owned(),
            schema_fingerprint: "schema".to_owned(),
            serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
            normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
            source_frontend_revision: None,
        }
    }

    #[test]
    fn canonical_fingerprint_is_order_independent() {
        assert_eq!(
            canonical_schema_fingerprint(&json!({"b": 2, "a": {"d": 4, "c": 3}})),
            canonical_schema_fingerprint(&json!({"a": {"c": 3, "d": 4}, "b": 2}))
        );
    }

    #[test]
    fn profile_identity_is_exact_and_source_versioned() {
        let context = context();
        let profile = FrontendSerializationProfile::from_context(&context).unwrap();
        assert_eq!(profile.frontend_version, SUPPORTED_FRONTEND_VERSION);
        assert_eq!(
            profile.workflow_format_version,
            SUPPORTED_WORKFLOW_FORMAT_VERSION
        );
        assert_eq!(profile.source_provenance, "source extra.frontendVersion");
    }

    #[test]
    fn descriptors_classify_standard_seed_combo_and_upload_inputs() {
        let schema = RecognitionSchemaContext::parse(&json!({
            "Node": {"input": {"required": {
                "seed": ["INT", {"default": 0}],
                "choice": [["a", "b"], {}],
                "file": ["COMBO", {"image_upload": true}]
            }}, "output": ["IMAGE"], "output_name": ["IMAGE"], "output_node": true}
        }));
        let context = context();
        let set = UiSerializationDescriptorSet::build(
            &schema,
            FrontendSerializationProfile::from_context(&context).unwrap(),
            context,
        )
        .unwrap();
        assert_eq!(
            set.input("Node", "seed").unwrap().serializer_kind,
            SerializerKind::StandardSeed
        );
        assert_eq!(
            set.input("Node", "choice").unwrap().serializer_kind,
            SerializerKind::StandardCombo
        );
        assert_eq!(
            set.input("Node", "file").unwrap().serializer_kind,
            SerializerKind::StandardUpload
        );
        assert!(set.node("Node").unwrap().output_node);
    }

    #[test]
    fn dynamic_inputs_are_resolved_without_positional_guessing() {
        let schema = RecognitionSchemaContext::parse(&json!({
            "Node": {"input": {"required": {
                "values": ["COMFY_AUTOGROW_V3", {"names": ["a", "b"]}]
            }}}
        }));
        let context = context();
        let set = UiSerializationDescriptorSet::build(
            &schema,
            FrontendSerializationProfile::from_context(&context).unwrap(),
            context,
        )
        .unwrap();
        assert!(set.input("Node", "values.a").is_some());
        assert!(set.input("Node", "values.unknown").is_some());
    }
}
