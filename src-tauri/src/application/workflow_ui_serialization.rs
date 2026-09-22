use crate::application::workflow_recognition_schema::{
    MediaKind, RecognitionDeclaredType, RecognitionInputSchema, RecognitionNodeSchema,
    RecognitionSchemaContext,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub const SERIALIZATION_PROFILE_VERSION: &str = "comfyui-frontend-export-profile-v1";
pub const NORMALIZER_POLICY_VERSION: &str = "phase2b-normalizer-policy-v1";
pub const SUPPORTED_WORKFLOW_FORMAT_VERSION: &str = "0.4";
pub const SUPPORTED_FRONTEND_VERSION: &str = "1.52.7";
pub const PROFILE_SUPPORT_MODEL: &str = "FEATURE_SUBSET";

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

    /// Build the provenance context selected by the historical fingerprint
    /// resolver.  The current contract intentionally continues to require an
    /// exact frontendVersion; only an already-selected historical contract may
    /// represent missing provenance as `unknown`.
    pub fn from_historical_source(
        workflow_format_version: impl Into<String>,
        frontend_version: Option<&str>,
        schema_fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            workflow_format_version: workflow_format_version.into(),
            frontend_version: frontend_version
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("unknown")
                .to_owned(),
            schema_fingerprint: schema_fingerprint.into(),
            serialization_profile_version: SERIALIZATION_PROFILE_VERSION.to_owned(),
            normalizer_policy_version: NORMALIZER_POLICY_VERSION.to_owned(),
            source_frontend_revision: None,
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendSerializationContract {
    Current,
    LegacyWidgetSlotV0,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontendSerializationProfile {
    pub frontend_version: String,
    pub workflow_format_version: String,
    pub serialization_profile_version: String,
    pub support_model: String,
    pub source_provenance: String,
    pub frontend_source_revision: Option<String>,
    pub contract: FrontendSerializationContract,
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
            support_model: PROFILE_SUPPORT_MODEL.to_owned(),
            source_provenance: "source extra.frontendVersion".to_owned(),
            frontend_source_revision: context.source_frontend_revision.clone(),
            contract: FrontendSerializationContract::Current,
        })
    }

    pub fn legacy_widget_slot_v0_from_context(
        context: &NormalizationCompatibilityContext,
    ) -> Result<Self, SerializationDescriptorError> {
        if context.workflow_format_version != SUPPORTED_WORKFLOW_FORMAT_VERSION
            || context.serialization_profile_version != SERIALIZATION_PROFILE_VERSION
        {
            return Err(SerializationDescriptorError::new(
                "UNSUPPORTED_LEGACY_WIDGET_SLOT_PROFILE",
                "no verified LegacyWidgetSlotV0 profile exists for this source",
            ));
        }
        Ok(Self {
            frontend_version: context.frontend_version.clone(),
            workflow_format_version: context.workflow_format_version.clone(),
            serialization_profile_version: context.serialization_profile_version.clone(),
            support_model: PROFILE_SUPPORT_MODEL.to_owned(),
            source_provenance: "historical fingerprint LegacyWidgetSlotV0".to_owned(),
            frontend_source_revision: context.source_frontend_revision.clone(),
            contract: FrontendSerializationContract::LegacyWidgetSlotV0,
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
    Dynamic,
    Conditional,
    Unknown,
}

impl SerializerKind {
    pub fn is_standard(self) -> bool {
        matches!(
            self,
            Self::StandardDirect
                | Self::StandardCombo
                | Self::StandardUpload
                | Self::StandardSeed
                | Self::Dynamic
                | Self::Conditional
        )
    }

    pub fn is_frontend_control(self) -> bool {
        matches!(self, Self::WorkflowOnlyControl)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetBindingKind {
    RuntimeInput,
    FrontendWidget,
    ConvertedWidget,
    ControlWidget,
    DynamicInput,
    ConditionalInput,
    UnknownWidget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiWidgetBinding {
    pub kind: WidgetBindingKind,
    pub runtime_name: Option<String>,
    pub descriptor: Option<UiInputSerializationDescriptor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiInputEvidence {
    pub name: String,
    pub widget_name: Option<String>,
    pub shape: Option<i64>,
    pub linked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyWidgetSlotKind {
    RuntimeInput,
    ResidualLinkedWidget,
    FrontendControl,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyWidgetSlot {
    pub index: usize,
    pub target: Option<String>,
    pub kind: LegacyWidgetSlotKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyWidgetSlotContract {
    pub class_type: String,
    pub slots: Vec<LegacyWidgetSlot>,
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
    pub dynamic_prefix: Option<String>,
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

    pub fn is_legacy_widget_slot(&self) -> bool {
        self.profile.contract == FrontendSerializationContract::LegacyWidgetSlotV0
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
            let suffix = input_name
                .strip_prefix(&format!("{base}."))
                .unwrap_or_default();
            if !base_input.dynamic_names.is_empty() {
                if !base_input
                    .dynamic_names
                    .iter()
                    .any(|name| name == suffix || name == input_name)
                {
                    return None;
                }
            } else if let Some(prefix) = &base_input.dynamic_prefix {
                if !suffix.starts_with(prefix) {
                    return None;
                }
            } else {
                return None;
            }
            let mut expanded = base_input.clone();
            expanded.name = input_name.to_owned();
            expanded.dynamic = true;
            expanded.serializer_kind = SerializerKind::Dynamic;
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
        self.input_for_node_with_kind(class_type, named_values, input_name)
            .map(|(descriptor, _)| descriptor)
    }

    fn input_for_node_with_kind(
        &self,
        class_type: &str,
        named_values: Option<&BTreeMap<String, Value>>,
        input_name: &str,
    ) -> Option<(UiInputSerializationDescriptor, WidgetBindingKind)> {
        if let Some(input) = self.input(class_type, input_name) {
            let kind = if input.serializer_kind == SerializerKind::Conditional {
                WidgetBindingKind::ConditionalInput
            } else if input.dynamic {
                WidgetBindingKind::DynamicInput
            } else if input.serializer_kind.is_frontend_control() {
                WidgetBindingKind::ControlWidget
            } else {
                WidgetBindingKind::RuntimeInput
            };
            return Some((input, kind));
        }
        let node = self.nodes.get(class_type)?;
        if let Some((selector_name, nested_name)) = input_name.split_once('.') {
            let Some(selected_value) = named_values
                .and_then(|values| values.get(selector_name))
                .and_then(Value::as_str)
            else {
                return None;
            };
            if let Some(input) = node
                .conditional_inputs
                .get(selector_name)
                .and_then(|by_value| by_value.get(selected_value))
                .and_then(|inputs| inputs.get(nested_name))
            {
                let mut input = input.clone();
                input.name = input_name.to_owned();
                return Some((input, WidgetBindingKind::ConditionalInput));
            }
        }
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
                return Some((input.clone(), WidgetBindingKind::ConditionalInput));
            }
        }
        None
    }

    /// Resolve one serialized UI socket to the runtime input it represents.
    /// This is the single authority for direct, converted, frontend-only, and
    /// unknown widget contracts.
    pub fn resolve_ui_input(
        &self,
        class_type: &str,
        name: &str,
        widget_name: Option<&str>,
        shape: Option<i64>,
        linked: bool,
        named_values: Option<&BTreeMap<String, Value>>,
    ) -> Result<UiWidgetBinding, SerializationDescriptorError> {
        for candidate in [widget_name, Some(name)].into_iter().flatten() {
            if let Some((descriptor, kind)) =
                self.input_for_node_with_kind(class_type, named_values, candidate)
            {
                return Ok(UiWidgetBinding {
                    kind,
                    runtime_name: Some(descriptor.name.clone()),
                    descriptor: Some(descriptor),
                });
            }
        }

        let mut converted = Vec::new();
        for candidate in [widget_name, Some(name)].into_iter().flatten() {
            let Some(base) = candidate.strip_suffix("_input") else {
                continue;
            };
            if base.is_empty() {
                continue;
            }
            if let Some((descriptor, _)) =
                self.input_for_node_with_kind(class_type, named_values, base)
            {
                converted.push((base.to_owned(), descriptor));
            }
        }
        converted.sort_by(|left, right| left.0.cmp(&right.0));
        converted.dedup_by(|left, right| left.0 == right.0);
        if converted.len() > 1 {
            return Err(SerializationDescriptorError::new(
                "AMBIGUOUS_CONVERTED_WIDGET",
                format!(
                    "UI socket {} on {} maps to multiple runtime inputs",
                    name, class_type
                ),
            ));
        }
        if shape == Some(7) {
            if let Some((runtime_name, descriptor)) = converted.pop() {
                return Ok(UiWidgetBinding {
                    kind: WidgetBindingKind::ConvertedWidget,
                    runtime_name: Some(runtime_name),
                    descriptor: Some(descriptor),
                });
            }
            if !linked
                && named_values.is_none_or(|values| {
                    !values.contains_key(name)
                        && widget_name.is_none_or(|widget| !values.contains_key(widget))
                })
            {
                return Ok(UiWidgetBinding {
                    kind: WidgetBindingKind::FrontendWidget,
                    runtime_name: None,
                    descriptor: None,
                });
            }
        }
        Err(SerializationDescriptorError::new(
            "UNKNOWN_WIDGET_CONTRACT",
            format!(
                "UI socket {} on {} has no verified runtime or frontend contract",
                name, class_type
            ),
        ))
    }

    /// Return runtime input names in the frontend cursor order.  Runtime
    /// schema order remains authoritative; UI socket evidence only proves
    /// which optional/converted inputs are present. Linked inputs without a
    /// widget do not consume a literal cursor slot.
    pub fn positional_input_names(
        &self,
        class_type: &str,
        inputs: &[UiInputEvidence],
        named_values: Option<&BTreeMap<String, Value>>,
        linked_names: &std::collections::BTreeSet<String>,
    ) -> Result<Vec<String>, SerializationDescriptorError> {
        let node = self.nodes.get(class_type).ok_or_else(|| {
            SerializationDescriptorError::new(
                "UNKNOWN_NODE_CLASS",
                format!("node class {class_type} is absent from object_info"),
            )
        })?;
        let mut names = Vec::new();

        let mut add = |name: &str, descriptor: &UiInputSerializationDescriptor| {
            if descriptor.hidden || descriptor.serializer_kind.is_frontend_control() {
                return;
            }
            if !names.iter().any(|existing| existing == name) {
                names.push(name.to_owned());
            }
        };

        for name in &node.ordered_inputs {
            let Some((descriptor, _)) =
                self.input_for_node_with_kind(class_type, named_values, name)
            else {
                continue;
            };
            if descriptor.hidden || descriptor.serializer_kind.is_frontend_control() {
                continue;
            }
            let evidence = inputs.iter().find(|input| {
                self.resolve_ui_input(
                    class_type,
                    &input.name,
                    input.widget_name.as_deref(),
                    input.shape,
                    input.linked,
                    named_values,
                )
                .ok()
                .and_then(|binding| binding.runtime_name)
                .is_some_and(|runtime_name| runtime_name == *name)
            });
            let has_widget = evidence.is_some_and(|input| input.widget_name.is_some());
            let linked = linked_names.contains(name) || evidence.is_some_and(|input| input.linked);
            let named = named_values.is_some_and(|values| values.contains_key(name));
            if linked && !has_widget && descriptor.serializer_kind != SerializerKind::StandardSeed {
                continue;
            }
            if !linked && !has_widget && !named {
                continue;
            }
            add(name, &descriptor);
        }

        for input in inputs {
            let binding = self.resolve_ui_input(
                class_type,
                &input.name,
                input.widget_name.as_deref(),
                input.shape,
                input.linked,
                named_values,
            )?;
            let Some(runtime_name) = binding.runtime_name else {
                continue;
            };
            let Some(descriptor) = binding.descriptor.as_ref() else {
                continue;
            };
            if binding.kind == WidgetBindingKind::FrontendWidget {
                continue;
            }
            if input.linked
                && input.widget_name.is_none()
                && descriptor.serializer_kind != SerializerKind::StandardSeed
            {
                continue;
            }
            if !input.linked
                && input.widget_name.is_none()
                && !named_values.is_some_and(|values| values.contains_key(&runtime_name))
            {
                continue;
            }
            add(&runtime_name, descriptor);
        }

        if let Some(named_values) = named_values {
            for name in named_values.keys() {
                if let Some((descriptor, _)) =
                    self.input_for_node_with_kind(class_type, Some(named_values), name)
                {
                    add(name, &descriptor);
                }
            }
        }

        Ok(names)
    }

    /// Consume positional values using the resolved runtime cursor. Known
    /// frontend control tokens are explicitly removed; every other extra or
    /// missing value fails closed.
    pub fn consume_positional_values(
        &self,
        class_type: &str,
        names: &[String],
        values: &[Value],
        named_values: Option<&BTreeMap<String, Value>>,
    ) -> Result<Vec<(String, Value)>, SerializationDescriptorError> {
        let mut cursor = 0usize;
        let mut result = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let descriptor = self
                .input_for_node(class_type, named_values, name)
                .ok_or_else(|| {
                    SerializationDescriptorError::new(
                        "UNKNOWN_WIDGET_CONTRACT",
                        format!("input {name} on {class_type} has no serializer descriptor"),
                    )
                })?;
            while cursor < values.len()
                && self.is_frontend_control_value(class_type, &values[cursor], named_values)
                && values.len().saturating_sub(cursor) > names.len() - index
            {
                cursor += 1;
            }
            let Some(value) = values.get(cursor) else {
                return Err(SerializationDescriptorError::new(
                    "WIDGET_CURSOR_MISMATCH",
                    format!(
                        "node class {class_type} has {} positional values but {} verified runtime inputs",
                        values.len(), names.len()
                    ),
                ));
            };
            result.push((name.clone(), value.clone()));
            cursor += 1;
            if descriptor.serializer_kind == SerializerKind::StandardSeed
                && values.get(cursor).is_some_and(|value| {
                    self.is_frontend_control_value(class_type, value, named_values)
                })
            {
                cursor += 1;
            }
        }
        while cursor < values.len() {
            if !self.is_frontend_control_value(class_type, &values[cursor], named_values) {
                return Err(SerializationDescriptorError::new(
                    "WIDGET_CURSOR_MISMATCH",
                    format!(
                        "node class {class_type} contains an unclassified extra positional widget value"
                    ),
                ));
            }
            cursor += 1;
        }
        Ok(result)
    }

    /// Reconstruct the positional widget cursor used by the historical
    /// LegacyWidgetSlotV0 contract.  This layer only decides which serialized
    /// slot corresponds to which logical input; the normalizer remains the
    /// authority for literal/type validation and runtime materialization.
    pub fn consume_legacy_positional_values(
        &self,
        class_type: &str,
        inputs: &[UiInputEvidence],
        linked_names: &std::collections::BTreeSet<String>,
        values: &[Value],
        named_values: Option<&BTreeMap<String, Value>>,
    ) -> Result<Vec<(String, Value)>, SerializationDescriptorError> {
        if !self.is_legacy_widget_slot() {
            return Err(SerializationDescriptorError::new(
                "LEGACY_WIDGET_PROFILE_REQUIRED",
                "LegacyWidgetSlotV0 cursor cannot run for the current contract",
            ));
        }
        let contract =
            self.legacy_widget_slot_contract(class_type, inputs, linked_names, named_values)?;
        let mut cursor = 0usize;
        let mut result = Vec::new();

        for slot in &contract.slots {
            match slot.kind {
                LegacyWidgetSlotKind::ResidualLinkedWidget => {
                    // A historical UI may retain the widget value even after
                    // the corresponding runtime input became linked.  It is a
                    // real cursor slot, but it must never become a literal.
                    if cursor < values.len() {
                        cursor += 1;
                    }
                }
                LegacyWidgetSlotKind::FrontendControl => {
                    if cursor < values.len() {
                        cursor += 1;
                    }
                }
                LegacyWidgetSlotKind::RuntimeInput => {
                    let name = slot.target.as_deref().expect("runtime slot has a target");
                    let descriptor = self
                        .input_for_node(class_type, named_values, name)
                        .ok_or_else(|| {
                            SerializationDescriptorError::new(
                                "legacy_widget_slot_ambiguous",
                                format!(
                                    "legacy widget slot {name} has no unique serializer target"
                                ),
                            )
                        })?;
                    while cursor < values.len()
                        && self.is_frontend_control_value(class_type, &values[cursor], named_values)
                    {
                        cursor += 1;
                    }
                    let Some(value) = values.get(cursor) else {
                        if descriptor.required {
                            return Err(SerializationDescriptorError::new(
                                "legacy_widget_slot_missing",
                                format!("legacy widget slot for input {name} is missing"),
                            ));
                        }
                        continue;
                    };
                    result.push((name.to_owned(), value.clone()));
                    cursor += 1;
                    if descriptor.serializer_kind == SerializerKind::StandardUpload
                        && values.get(cursor).is_some_and(is_upload_presentation_value)
                    {
                        // Legacy upload widgets serialized their media-mode
                        // label beside the path.  It is presentation-only,
                        // not a second runtime input.
                        cursor += 1;
                    }
                }
            }
        }

        while cursor < values.len() {
            if self.is_frontend_control_value(class_type, &values[cursor], named_values) {
                cursor += 1;
            } else {
                return Err(SerializationDescriptorError::new(
                    "legacy_widget_slot_extra",
                    "legacy widgets_values contains an unclassified extra positional slot",
                ));
            }
        }
        Ok(result)
    }

    /// Build the one historical slot correspondence model.  Schema order is
    /// the deterministic fallback only after explicit UI widget evidence and
    /// linked-input evidence have been accounted for.
    pub fn legacy_widget_slot_contract(
        &self,
        class_type: &str,
        inputs: &[UiInputEvidence],
        linked_names: &std::collections::BTreeSet<String>,
        named_values: Option<&BTreeMap<String, Value>>,
    ) -> Result<LegacyWidgetSlotContract, SerializationDescriptorError> {
        let node = self.nodes.get(class_type).ok_or_else(|| {
            SerializationDescriptorError::new(
                "UNKNOWN_NODE_CLASS",
                format!("node class {class_type} is absent from object_info"),
            )
        })?;
        let mut evidence_by_target: BTreeMap<String, (bool, bool, Option<i64>)> = BTreeMap::new();
        let mut explicit_order = Vec::new();
        let mut explicit_targets = BTreeSet::new();
        for input in inputs {
            let binding = self
                .resolve_ui_input(
                    class_type,
                    &input.name,
                    input.widget_name.as_deref(),
                    input.shape,
                    input.linked,
                    named_values,
                )
                .map_err(|error| {
                    if input.widget_name.is_some() {
                        SerializationDescriptorError::new(
                            "legacy_widget_slot_ambiguous",
                            error.message,
                        )
                    } else {
                        error
                    }
                })?;
            let Some(target) = binding.runtime_name else {
                continue;
            };
            if input.widget_name.is_some() {
                if evidence_by_target
                    .get(&target)
                    .is_some_and(|(_, explicit, _)| *explicit)
                {
                    return Err(SerializationDescriptorError::new(
                        "legacy_widget_slot_ambiguous",
                        format!("multiple historical widget slots target input {target}"),
                    ));
                }
            }
            let linked = input.linked
                || linked_names.contains(&input.name)
                || linked_names.contains(&target);
            if input.widget_name.is_some() && explicit_targets.insert(target.clone()) {
                explicit_order.push(target.clone());
            }
            evidence_by_target
                .entry(target)
                .and_modify(|entry| {
                    entry.0 |= linked;
                    entry.1 |= input.widget_name.is_some();
                    if entry.2.is_none() {
                        entry.2 = input.shape;
                    }
                })
                .or_insert((linked, input.widget_name.is_some(), input.shape));
        }

        let mut ordered_names = explicit_order;
        let mut ordered_name_set = ordered_names.iter().cloned().collect::<BTreeSet<_>>();
        for name in &node.ordered_inputs {
            if ordered_name_set.insert(name.clone()) {
                ordered_names.push(name.clone());
            }
        }

        let mut slots = Vec::new();
        for name in ordered_names {
            let Some(descriptor) = self.input_for_node(class_type, named_values, &name) else {
                continue;
            };
            let evidence = evidence_by_target.get(&name);
            let linked = linked_names.contains(&name) || evidence.is_some_and(|entry| entry.0);
            let explicit_widget = evidence.is_some_and(|entry| entry.1);
            if explicit_widget {
                let kind = if linked {
                    LegacyWidgetSlotKind::ResidualLinkedWidget
                } else if descriptor.serializer_kind.is_frontend_control() {
                    LegacyWidgetSlotKind::FrontendControl
                } else {
                    LegacyWidgetSlotKind::RuntimeInput
                };
                slots.push(LegacyWidgetSlot {
                    index: slots.len(),
                    target: (!descriptor.serializer_kind.is_frontend_control())
                        .then(|| name.clone()),
                    kind,
                });
                continue;
            }
            if descriptor.hidden {
                continue;
            }
            if linked {
                continue;
            }
            if evidence.is_some_and(|entry| entry.2 == Some(7)) {
                // Unnamed shape-7 inputs are frontend presentation/conversion
                // sockets, not proof of a serialized runtime widget.
                continue;
            }
            if evidence.is_some() {
                // An unlinked socket without widget metadata is not enough
                // historical evidence to claim a positional runtime slot.
                continue;
            }
            if descriptor.serializer_kind.is_frontend_control() {
                continue;
            }
            if legacy_widget_capable(
                descriptor.serializer_kind,
                descriptor.declared_type,
                descriptor.upload_media_kind.is_some(),
            ) {
                slots.push(LegacyWidgetSlot {
                    index: slots.len(),
                    target: Some(name.clone()),
                    kind: LegacyWidgetSlotKind::RuntimeInput,
                });
            }
        }
        Ok(LegacyWidgetSlotContract {
            class_type: class_type.to_owned(),
            slots,
        })
    }

    fn is_frontend_control_value(
        &self,
        class_type: &str,
        value: &Value,
        named_values: Option<&BTreeMap<String, Value>>,
    ) -> bool {
        if named_values.is_some_and(|values| {
            values.iter().any(|(name, candidate)| {
                candidate == value
                    && self
                        .input_for_node(class_type, Some(values), name)
                        .is_none_or(|descriptor| {
                            descriptor.hidden || descriptor.serializer_kind.is_frontend_control()
                        })
            })
        }) {
            return true;
        }
        matches!(
            value.as_str(),
            Some(
                "fixed"
                    | "randomize"
                    | "randomize_after_generate"
                    | "control_after_generate"
                    | "randomize after generate"
                    | "increment"
                    | "decrement"
            )
        )
    }
}

fn legacy_widget_capable(
    serializer_kind: SerializerKind,
    declared_type: RecognitionDeclaredType,
    upload: bool,
) -> bool {
    upload
        || matches!(
            serializer_kind,
            SerializerKind::StandardDirect
                | SerializerKind::StandardCombo
                | SerializerKind::StandardSeed
                | SerializerKind::Dynamic
                | SerializerKind::Conditional
        ) && matches!(
            declared_type,
            RecognitionDeclaredType::String
                | RecognitionDeclaredType::Integer
                | RecognitionDeclaredType::Float
                | RecognitionDeclaredType::Boolean
                | RecognitionDeclaredType::Enum
                | RecognitionDeclaredType::DynamicCombo
                | RecognitionDeclaredType::Unknown
        )
}

fn is_upload_presentation_value(value: &Value) -> bool {
    matches!(value.as_str(), Some("image" | "video" | "audio"))
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
                                    let mut descriptor = descriptor_for_input(schema, false);
                                    if !descriptor.serializer_kind.is_frontend_control() {
                                        descriptor.serializer_kind = SerializerKind::Conditional;
                                    }
                                    (name.clone(), descriptor)
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
            RecognitionDeclaredType::Unknown if schema.raw_type.eq_ignore_ascii_case("COLOR") => {
                SerializerKind::StandardDirect
            }
            RecognitionDeclaredType::Unknown => SerializerKind::Unknown,
            _ => SerializerKind::StandardDirect,
        }
    };
    let serializer_kind = if dynamic
        || schema.dynamic_prefix.is_some()
        || !schema.dynamic_names.is_empty()
        || schema.declared_type == RecognitionDeclaredType::DynamicCombo
    {
        SerializerKind::Dynamic
    } else {
        serializer_kind
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
        dynamic_prefix: schema.dynamic_prefix.clone(),
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
        assert!(set.input("Node", "values.unknown").is_none());
    }
}
