use crate::application::workflow_ui_serialization::{
    NormalizationCompatibilityContext, SerializerKind, UiInputEvidence,
    UiInputSerializationDescriptor, UiSerializationDescriptorSet,
};
use crate::compiler::WorkflowValidator;
use crate::domain::WorkflowDocument;
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
};

pub type UiSourceNodeId = String;
pub type NormalizedApiNodeId = String;

/// Serialization features detected from a ComfyUI UI-workflow document.
/// Detection is intentionally independent from whether the current profile
/// can normalize the feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkflowUiFeature {
    ClassicNumericNodeIds,
    StringNumericNodeIds,
    UuidNodeIds,
    OpaqueNodeIds,
    ClassicLinks5,
    ClassicLinks6,
    UnknownLinkEncoding,
    NamedWidgets,
    PositionalWidgets,
    DynamicInputs,
    ConditionalInputs,
    AutogrowInputs,
    NoteNode,
    PrimitiveNode,
    RuntimePrimitiveNode,
    RerouteNode,
    DefinitionsSubgraphs,
    NestedSubgraphs,
    UuidCompositeNodeType,
    FrontendVirtualNode,
    UnknownUiNode,
}

impl WorkflowUiFeature {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClassicNumericNodeIds => "classic_numeric_node_ids",
            Self::StringNumericNodeIds => "string_numeric_node_ids",
            Self::UuidNodeIds => "uuid_node_ids",
            Self::OpaqueNodeIds => "opaque_node_ids",
            Self::ClassicLinks5 => "classic_links_5",
            Self::ClassicLinks6 => "classic_links_6",
            Self::UnknownLinkEncoding => "unknown_link_encoding",
            Self::NamedWidgets => "named_widgets",
            Self::PositionalWidgets => "positional_widgets",
            Self::DynamicInputs => "dynamic_inputs",
            Self::ConditionalInputs => "conditional_inputs",
            Self::AutogrowInputs => "autogrow_inputs",
            Self::NoteNode => "note_node",
            Self::PrimitiveNode => "primitive_node",
            Self::RuntimePrimitiveNode => "runtime_primitive_node",
            Self::RerouteNode => "reroute_node",
            Self::DefinitionsSubgraphs => "definitions_subgraphs",
            Self::NestedSubgraphs => "nested_subgraphs",
            Self::UuidCompositeNodeType => "uuid_composite_node_type",
            Self::FrontendVirtualNode => "frontend_virtual_node",
            Self::UnknownUiNode => "unknown_ui_node",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompatibilityFeatureStatus {
    Supported,
    Unsupported,
    ConditionallySupported,
    Unknown,
}

impl CompatibilityFeatureStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "SUPPORTED",
            Self::Unsupported => "UNSUPPORTED",
            Self::ConditionallySupported => "CONDITIONALLY_SUPPORTED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowUiFeatureObservation {
    pub feature: WorkflowUiFeature,
    pub status: CompatibilityFeatureStatus,
    pub node_ids: Vec<UiSourceNodeId>,
    pub node_types: Vec<String>,
    pub link_ids: Vec<i64>,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowUiFeatureSet {
    pub observations: Vec<WorkflowUiFeatureObservation>,
}

impl WorkflowUiFeatureSet {
    pub fn supported_count(&self) -> usize {
        self.observations
            .iter()
            .filter(|item| item.status == CompatibilityFeatureStatus::Supported)
            .count()
    }

    pub fn unsupported_count(&self) -> usize {
        self.observations
            .iter()
            .filter(|item| item.status == CompatibilityFeatureStatus::Unsupported)
            .count()
    }

    pub fn blocking_observation(&self) -> Option<&WorkflowUiFeatureObservation> {
        self.observations.iter().find(|item| {
            matches!(
                item.status,
                CompatibilityFeatureStatus::Unsupported | CompatibilityFeatureStatus::Unknown
            )
        })
    }

    pub fn with_schema(
        &self,
        document: &UiWorkflowDocument,
        descriptors: &UiSerializationDescriptorSet,
    ) -> Self {
        let mut builder = FeatureSetBuilder::from_set(self);
        let classifications = classify_ui_nodes(document, descriptors);

        // V1A reported these node families as detected-but-unsupported. V1B
        // keeps detection in the same feature authority and changes support
        // only when the node satisfies the verified frontend contract.
        for feature in [
            WorkflowUiFeature::NoteNode,
            WorkflowUiFeature::PrimitiveNode,
            WorkflowUiFeature::RerouteNode,
            WorkflowUiFeature::FrontendVirtualNode,
        ] {
            let mut matching = document.nodes.iter().filter_map(|node| {
                let result = classifications.get(&node.id)?;
                (result.feature == Some(feature)).then_some(result)
            });
            if let Some(first) = matching.next() {
                let all_supported = std::iter::once(first)
                    .chain(matching)
                    .all(|result| result.supported);
                let reason = if all_supported {
                    "verified UI compatibility contract"
                } else {
                    classifications
                        .values()
                        .find(|result| result.feature == Some(feature) && !result.supported)
                        .map(|result| result.reason.as_str())
                        .unwrap_or("node does not satisfy the verified UI compatibility contract")
                };
                builder.set_status(
                    feature,
                    if all_supported {
                        CompatibilityFeatureStatus::Supported
                    } else {
                        CompatibilityFeatureStatus::Unsupported
                    },
                    reason,
                );
            }
        }

        for node in &document.nodes {
            let Some(classification) = classifications.get(&node.id) else {
                continue;
            };
            if classification.classification == UiNodeClassification::UnknownUiNode
                && is_explicit_unknown_virtual(node)
            {
                builder.add(
                    WorkflowUiFeature::UnknownUiNode,
                    CompatibilityFeatureStatus::Unsupported,
                    classification.reason.clone(),
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            }
        }
        builder.finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiSourceIdMapping {
    pub source_to_api: BTreeMap<UiSourceNodeId, NormalizedApiNodeId>,
    pub api_to_source: BTreeMap<NormalizedApiNodeId, UiSourceNodeId>,
}

/// The small subset of a ComfyUI workflow document that is relevant to
/// normalization.  Layout/editor metadata is deliberately not represented.
#[derive(Clone, Debug, PartialEq)]
pub struct UiWorkflowDocument {
    pub workflow_format_version: String,
    pub frontend_version: Option<String>,
    pub nodes: Vec<UiNode>,
    pub links: Vec<UiLink>,
    pub features: WorkflowUiFeatureSet,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiNode {
    pub id: String,
    pub class_type: String,
    pub title: Option<String>,
    pub mode: UiNodeMode,
    pub inputs: Vec<UiInputSocket>,
    pub outputs: Vec<UiOutputSocket>,
    pub widgets_values: Vec<Value>,
    pub widgets_values_named: Option<BTreeMap<String, Value>>,
    pub properties: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiNodeMode {
    Always,
    Bypass,
    Never,
    Unknown(i64),
}

impl UiNodeMode {
    fn parse(value: i64) -> Self {
        match value {
            0 => Self::Always,
            2 => Self::Never,
            4 => Self::Bypass,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiInputSocket {
    pub name: String,
    pub declared_type: Option<String>,
    pub link_id: Option<i64>,
    pub widget_name: Option<String>,
    pub shape: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiOutputSocket {
    pub name: String,
    pub declared_type: Option<String>,
    pub link_ids: Vec<i64>,
    pub widget_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiLink {
    pub id: i64,
    pub origin_node_id: String,
    pub origin_slot: usize,
    pub target_node_id: String,
    pub target_slot: usize,
    pub declared_type: Option<String>,
}

/// The single classification result consumed by feature support resolution
/// and graph normalization.  A node's class name is only one input; the
/// result also records the verified serialization contract that justified the
/// classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiNodeClassification {
    RuntimeNode,
    PresentationOnlyNode,
    StructuralNode,
    FrontendBindingNode,
    CompositeNode,
    UnknownUiNode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiNodeContract {
    Runtime,
    Presentation,
    Reroute,
    PrimitiveBinding,
    AliasBinding,
    Composite,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiAliasRole {
    Producer,
    Consumer,
}

#[derive(Clone, Debug, PartialEq)]
struct UiAliasBinding {
    source: NormalizedLinkInput,
    value_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UiNodeClassificationResult {
    classification: UiNodeClassification,
    contract: UiNodeContract,
    feature: Option<WorkflowUiFeature>,
    supported: bool,
    reason: String,
}

struct FeatureEvidence {
    status: CompatibilityFeatureStatus,
    node_ids: BTreeSet<String>,
    node_types: BTreeSet<String>,
    link_ids: BTreeSet<i64>,
    reason: String,
}

#[derive(Default)]
struct FeatureSetBuilder {
    entries: BTreeMap<WorkflowUiFeature, FeatureEvidence>,
}

impl FeatureSetBuilder {
    fn from_set(set: &WorkflowUiFeatureSet) -> Self {
        let mut builder = Self::default();
        for observation in &set.observations {
            let entry = builder
                .entries
                .entry(observation.feature)
                .or_insert_with(|| FeatureEvidence {
                    status: observation.status,
                    node_ids: BTreeSet::new(),
                    node_types: BTreeSet::new(),
                    link_ids: BTreeSet::new(),
                    reason: observation.reason.clone(),
                });
            entry.status = merge_feature_status(entry.status, observation.status);
            entry.node_ids.extend(observation.node_ids.iter().cloned());
            entry
                .node_types
                .extend(observation.node_types.iter().cloned());
            entry.link_ids.extend(observation.link_ids.iter().copied());
        }
        builder
    }

    fn add(
        &mut self,
        feature: WorkflowUiFeature,
        status: CompatibilityFeatureStatus,
        reason: impl Into<String>,
        node_id: Option<&str>,
        node_type: Option<&str>,
        link_id: Option<i64>,
    ) {
        let reason = reason.into();
        let entry = self
            .entries
            .entry(feature)
            .or_insert_with(|| FeatureEvidence {
                status,
                node_ids: BTreeSet::new(),
                node_types: BTreeSet::new(),
                link_ids: BTreeSet::new(),
                reason: reason.clone(),
            });
        entry.status = merge_feature_status(entry.status, status);
        if entry.reason.is_empty() {
            entry.reason = reason;
        }
        if let Some(node_id) = node_id {
            entry.node_ids.insert(node_id.to_owned());
        }
        if let Some(node_type) = node_type {
            entry.node_types.insert(node_type.to_owned());
        }
        if let Some(link_id) = link_id {
            entry.link_ids.insert(link_id);
        }
    }

    fn set_status(
        &mut self,
        feature: WorkflowUiFeature,
        status: CompatibilityFeatureStatus,
        reason: impl Into<String>,
    ) {
        let reason = reason.into();
        let entry = self
            .entries
            .entry(feature)
            .or_insert_with(|| FeatureEvidence {
                status,
                node_ids: BTreeSet::new(),
                node_types: BTreeSet::new(),
                link_ids: BTreeSet::new(),
                reason: reason.clone(),
            });
        entry.status = status;
        entry.reason = reason;
    }

    fn finish(self) -> WorkflowUiFeatureSet {
        WorkflowUiFeatureSet {
            observations: self
                .entries
                .into_iter()
                .map(|(feature, evidence)| WorkflowUiFeatureObservation {
                    feature,
                    status: evidence.status,
                    node_ids: evidence.node_ids.into_iter().collect(),
                    node_types: evidence.node_types.into_iter().collect(),
                    link_ids: evidence.link_ids.into_iter().collect(),
                    reason: evidence.reason,
                })
                .collect(),
        }
    }
}

fn merge_feature_status(
    left: CompatibilityFeatureStatus,
    right: CompatibilityFeatureStatus,
) -> CompatibilityFeatureStatus {
    use CompatibilityFeatureStatus::*;
    match (left, right) {
        (Unsupported, _) | (_, Unsupported) => Unsupported,
        (Unknown, _) | (_, Unknown) => Unknown,
        (ConditionallySupported, _) | (_, ConditionallySupported) => ConditionallySupported,
        _ => Supported,
    }
}

fn supported_status(feature: WorkflowUiFeature) -> CompatibilityFeatureStatus {
    use CompatibilityFeatureStatus::*;
    match feature {
        WorkflowUiFeature::UnknownLinkEncoding
        | WorkflowUiFeature::NoteNode
        | WorkflowUiFeature::PrimitiveNode
        | WorkflowUiFeature::RerouteNode
        | WorkflowUiFeature::DefinitionsSubgraphs
        | WorkflowUiFeature::NestedSubgraphs
        | WorkflowUiFeature::UuidCompositeNodeType
        | WorkflowUiFeature::UnknownUiNode => Unsupported,
        WorkflowUiFeature::DynamicInputs
        | WorkflowUiFeature::ConditionalInputs
        | WorkflowUiFeature::AutogrowInputs
        | WorkflowUiFeature::RuntimePrimitiveNode => ConditionallySupported,
        _ => Supported,
    }
}

fn is_frozen_api_node_id(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.split(':').all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

fn looks_like_uuid(value: &str) -> bool {
    let value = value.trim();
    let parts = value.split('-').collect::<Vec<_>>();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .into_iter()
            .zip(parts)
            .all(|(length, part)| {
                part.len() == length && part.chars().all(|character| character.is_ascii_hexdigit())
            })
}

fn is_reroute_type(class_type: &str) -> bool {
    class_type.trim().eq_ignore_ascii_case("reroute")
}

fn is_note_type(class_type: &str) -> bool {
    class_type.trim().eq_ignore_ascii_case("note")
}

fn is_generic_primitive_type(class_type: &str) -> bool {
    class_type.trim().eq_ignore_ascii_case("primitivenode")
}

fn is_runtime_primitive_type(class_type: &str) -> bool {
    matches!(
        class_type,
        "PrimitiveFloat" | "PrimitiveInt" | "PrimitiveBoolean" | "PrimitiveStringMultiline"
    )
}

fn is_explicit_unknown_virtual(node: &UiNode) -> bool {
    node.properties
        .get("virtual")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || node
            .properties
            .get("is_virtual")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || node.class_type.to_ascii_lowercase().contains("virtual")
}

fn is_known_frontend_virtual_class(class_type: &str) -> bool {
    matches!(
        class_type,
        "Label (rgthree)" | "MarkdownNote" | "Fast Groups Bypasser (rgthree)"
    )
}

fn is_known_presentation_class(class_type: &str) -> bool {
    matches!(
        class_type,
        "Label (rgthree)" | "MarkdownNote" | "Fast Groups Bypasser (rgthree)"
    )
}

fn alias_role(node: &UiNode) -> Option<UiAliasRole> {
    if node.mode != UiNodeMode::Always || node.outputs.len() != 1 {
        return None;
    }
    let output = node.outputs.first()?;
    let output_type = output.declared_type.as_deref()?.trim();
    if output_type.is_empty() {
        return None;
    }
    let output_has_links = !output.link_ids.is_empty();
    let linked_inputs = node
        .inputs
        .iter()
        .filter(|input| input.link_id.is_some())
        .count();
    if node.inputs.len() == 1 && linked_inputs == 1 && !output_has_links {
        let input_type = node.inputs.first()?.declared_type.as_deref()?.trim();
        if !input_type.is_empty() && compatible_socket_types(Some(input_type), Some(output_type)) {
            return Some(UiAliasRole::Producer);
        }
    }
    if node.inputs.is_empty() && output_has_links {
        return Some(UiAliasRole::Consumer);
    }
    None
}

fn serialized_alias_identity(node: &UiNode, role: UiAliasRole) -> Option<String> {
    let property_identity = ["alias_id", "aliasId", "alias", "key", "previousName"]
        .into_iter()
        .find_map(|name| {
            node.properties
                .get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        });
    let widget_identity = node
        .widgets_values_named
        .as_ref()
        .and_then(|values| values.get("Constant"))
        .and_then(Value::as_str)
        .or_else(|| node.widgets_values.first().and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    match (property_identity, widget_identity, role) {
        (Some(property), Some(widget), _) if property != widget => None,
        (Some(property), _, _) => Some(property),
        (None, Some(widget), UiAliasRole::Consumer) => Some(widget),
        (None, Some(_), UiAliasRole::Producer) => None,
        (None, None, _) => None,
    }
}

fn is_serialized_alias_candidate(node: &UiNode) -> bool {
    alias_role(node).is_some_and(|role| serialized_alias_identity(node, role).is_some())
}

fn has_execution_link(document: &UiWorkflowDocument, node_id: &str) -> bool {
    document
        .links
        .iter()
        .any(|link| link.origin_node_id == node_id || link.target_node_id == node_id)
}

fn primitive_control_value(value: &Value) -> bool {
    matches!(
        value.as_str(),
        Some(
            "fixed"
                | "randomize"
                | "randomize_after_generate"
                | "control_after_generate"
                | "randomize after generate"
        )
    )
}

fn primitive_binding_value(node: &UiNode) -> Result<Value, String> {
    let named_value = node
        .widgets_values_named
        .as_ref()
        .and_then(|values| values.get("value"));
    if let Some(named) = &node.widgets_values_named {
        for name in named.keys() {
            if name != "value"
                && name != "fixed"
                && name != "control_after_generate"
                && name != "randomize"
            {
                return Err(format!("unknown PrimitiveNode widget {name}"));
            }
        }
        if named_value.is_none() {
            return Err("PrimitiveNode widget contract has no value field".to_owned());
        }
    }

    if node.widgets_values.len() > 2 {
        return Err("PrimitiveNode has more than one value and one control field".to_owned());
    }
    if node.widgets_values.len() == 2 && !primitive_control_value(&node.widgets_values[1]) {
        return Err("PrimitiveNode control field is not a verified frontend value".to_owned());
    }
    if let (Some(named), Some(positional)) = (named_value, node.widgets_values.first()) {
        if named != positional {
            return Err("PrimitiveNode named and positional values disagree".to_owned());
        }
    }
    named_value
        .cloned()
        .or_else(|| node.widgets_values.first().cloned())
        .ok_or_else(|| "PrimitiveNode has no serialized value".to_owned())
}

/// The only UI-node classification authority.  All downstream stages consume
/// these results instead of maintaining independent class-name allowlists.
fn classify_ui_nodes(
    document: &UiWorkflowDocument,
    descriptors: &UiSerializationDescriptorSet,
) -> BTreeMap<String, UiNodeClassificationResult> {
    document
        .nodes
        .iter()
        .map(|node| {
            let result = if looks_like_uuid(&node.class_type) {
                UiNodeClassificationResult {
                    classification: UiNodeClassification::CompositeNode,
                    contract: UiNodeContract::Composite,
                    feature: Some(WorkflowUiFeature::UuidCompositeNodeType),
                    supported: false,
                    reason: "UUID node types remain reserved for unsupported composite serialization"
                        .to_owned(),
                }
            } else if is_reroute_type(&node.class_type) {
                let incoming_links = document
                    .links
                    .iter()
                    .filter(|link| link.target_node_id == node.id)
                    .collect::<Vec<_>>();
                let outgoing_links = document
                    .links
                    .iter()
                    .filter(|link| link.origin_node_id == node.id)
                    .collect::<Vec<_>>();
                let input_evidence = node.inputs.first().is_some_and(|input| {
                    incoming_links.is_empty()
                        || (input.link_id.is_some()
                            && incoming_links.iter().any(|link| {
                                input.link_id == Some(link.id)
                            }))
                });
                let output_evidence = node.outputs.first().is_some_and(|output| {
                    outgoing_links.is_empty()
                        || outgoing_links
                            .iter()
                            .all(|link| output.link_ids.contains(&link.id))
                });
                let safe = node.inputs.len() == 1
                    && node.outputs.len() == 1
                    && node
                        .outputs
                        .first()
                        .and_then(|output| output.declared_type.as_deref())
                        .is_some_and(|value| !value.trim().is_empty())
                    && input_evidence
                    && output_evidence
                    && node.widgets_values.is_empty()
                    && node.widgets_values_named.is_none();
                UiNodeClassificationResult {
                    classification: if safe {
                        UiNodeClassification::StructuralNode
                    } else {
                        UiNodeClassification::UnknownUiNode
                    },
                    contract: if safe {
                        UiNodeContract::Reroute
                    } else {
                        UiNodeContract::Unknown
                    },
                    feature: Some(WorkflowUiFeature::RerouteNode),
                    supported: safe,
                    reason: if safe {
                        "Reroute has the verified one-input/one-output transparent structural contract"
                            .to_owned()
                    } else {
                        "Reroute does not match the verified one-input/one-output structural contract"
                            .to_owned()
                    },
                }
            } else if is_generic_primitive_type(&node.class_type) {
                let outgoing_links = document
                    .links
                    .iter()
                    .filter(|link| link.origin_node_id == node.id)
                    .collect::<Vec<_>>();
                let output_evidence = node.outputs.first().is_some_and(|output| {
                    !output.link_ids.is_empty()
                        && !outgoing_links.is_empty()
                        && outgoing_links
                            .iter()
                            .all(|link| output.link_ids.contains(&link.id))
                });
                let safe = node.inputs.is_empty()
                    && node.outputs.len() == 1
                    && node
                        .outputs
                        .first()
                        .and_then(|output| output.declared_type.as_deref())
                        .is_some_and(|value| !value.trim().is_empty())
                    && node
                        .outputs
                        .first()
                        .and_then(|output| output.widget_name.as_deref())
                        .is_some_and(|value| !value.trim().is_empty())
                    && output_evidence
                    && primitive_binding_value(node).is_ok();
                UiNodeClassificationResult {
                    classification: if safe {
                        UiNodeClassification::FrontendBindingNode
                    } else {
                        UiNodeClassification::UnknownUiNode
                    },
                    contract: if safe {
                        UiNodeContract::PrimitiveBinding
                    } else {
                        UiNodeContract::Unknown
                    },
                    feature: Some(WorkflowUiFeature::PrimitiveNode),
                    supported: safe,
                    reason: if safe {
                        "PrimitiveNode has a verified output widget/value binding contract".to_owned()
                    } else {
                        "PrimitiveNode is missing a verified output widget/value binding contract"
                            .to_owned()
                    },
                }
            } else if is_note_type(&node.class_type) {
                let safe = node.inputs.is_empty()
                    && node.outputs.is_empty()
                    && !has_execution_link(document, &node.id);
                UiNodeClassificationResult {
                    classification: if safe {
                        UiNodeClassification::PresentationOnlyNode
                    } else {
                        UiNodeClassification::UnknownUiNode
                    },
                    contract: if safe {
                        UiNodeContract::Presentation
                    } else {
                        UiNodeContract::Unknown
                    },
                    feature: Some(WorkflowUiFeature::NoteNode),
                    supported: safe,
                    reason: if safe {
                        "Note has no runtime sockets or execution links".to_owned()
                    } else if !node.outputs.is_empty() {
                        "Note has runtime outputs and cannot be dropped safely".to_owned()
                    } else {
                        "Note has runtime sockets or execution links and cannot be dropped safely"
                            .to_owned()
                    },
                }
            } else if is_known_presentation_class(&node.class_type) {
                let safe = node.inputs.is_empty()
                    && node.outputs.iter().all(|output| output.link_ids.is_empty())
                    && !has_execution_link(document, &node.id);
                UiNodeClassificationResult {
                    classification: if safe {
                        UiNodeClassification::PresentationOnlyNode
                    } else {
                        UiNodeClassification::UnknownUiNode
                    },
                    contract: if safe {
                        UiNodeContract::Presentation
                    } else {
                        UiNodeContract::Unknown
                    },
                    feature: Some(WorkflowUiFeature::FrontendVirtualNode),
                    supported: safe,
                    reason: if safe {
                        "known frontend presentation node has no execution links".to_owned()
                    } else {
                        "known frontend presentation node has runtime sockets or execution links"
                            .to_owned()
                    },
                }
            } else if descriptors.node(&node.class_type).is_some()
                || is_runtime_primitive_type(&node.class_type)
            {
                UiNodeClassificationResult {
                    classification: UiNodeClassification::RuntimeNode,
                    contract: UiNodeContract::Runtime,
                    feature: is_runtime_primitive_type(&node.class_type)
                        .then_some(WorkflowUiFeature::RuntimePrimitiveNode),
                    supported: true,
                    reason: "object_info schema-backed runtime node".to_owned(),
                }
            } else if is_serialized_alias_candidate(node) {
                UiNodeClassificationResult {
                    classification: UiNodeClassification::FrontendBindingNode,
                    contract: UiNodeContract::AliasBinding,
                    feature: Some(WorkflowUiFeature::FrontendVirtualNode),
                    supported: true,
                    reason: "serialized typed alias producer/consumer contract is verified"
                        .to_owned(),
                }
            } else if is_explicit_unknown_virtual(node) {
                UiNodeClassificationResult {
                    classification: UiNodeClassification::UnknownUiNode,
                    contract: UiNodeContract::Unknown,
                    feature: Some(WorkflowUiFeature::UnknownUiNode),
                    supported: false,
                    reason: "node is not present in object_info and has no verified UI compatibility contract"
                        .to_owned(),
                }
            } else {
                UiNodeClassificationResult {
                    classification: UiNodeClassification::UnknownUiNode,
                    contract: UiNodeContract::Unknown,
                    feature: None,
                    supported: false,
                    reason: "node is not present in object_info and has no verified UI compatibility contract"
                        .to_owned(),
                }
            };
            (node.id.clone(), result)
        })
        .collect()
}

fn add_source_id_feature(
    builder: &mut FeatureSetBuilder,
    value: &Value,
    canonical_id: &str,
    node_type: Option<&str>,
) {
    let feature = match value {
        Value::Number(_) => WorkflowUiFeature::ClassicNumericNodeIds,
        Value::String(value) if is_frozen_api_node_id(value) => {
            WorkflowUiFeature::StringNumericNodeIds
        }
        Value::String(value) if looks_like_uuid(value) => WorkflowUiFeature::UuidNodeIds,
        Value::String(_) => WorkflowUiFeature::OpaqueNodeIds,
        _ => return,
    };
    builder.add(
        feature,
        supported_status(feature),
        "source node identifiers are represented as canonical strings",
        Some(canonical_id),
        node_type,
        None,
    );
}

fn detect_ui_features(root: &Map<String, Value>, nodes: &[UiNode]) -> WorkflowUiFeatureSet {
    let mut builder = FeatureSetBuilder::default();
    let raw_nodes = root.get("nodes").and_then(Value::as_array);
    if let Some(raw_nodes) = raw_nodes {
        for (index, node) in nodes.iter().enumerate() {
            if let Some(raw_node) = raw_nodes.get(index).and_then(Value::as_object) {
                if let Some(raw_id) = raw_node.get("id") {
                    add_source_id_feature(
                        &mut builder,
                        raw_id,
                        &node.id,
                        Some(node.class_type.as_str()),
                    );
                }
                if let Some(inputs) = raw_node.get("inputs").and_then(Value::as_array) {
                    if inputs.iter().any(|input| {
                        input.as_object().is_some_and(|input| {
                            input.contains_key("conditional")
                                || input.contains_key("lazy")
                                || input.contains_key("rawLink")
                        })
                    }) {
                        builder.add(
                            WorkflowUiFeature::ConditionalInputs,
                            supported_status(WorkflowUiFeature::ConditionalInputs),
                            "conditional or lazy input metadata requires schema evidence",
                            Some(&node.id),
                            Some(&node.class_type),
                            None,
                        );
                    }
                }
            }
            if node.widgets_values_named.is_some() {
                builder.add(
                    WorkflowUiFeature::NamedWidgets,
                    supported_status(WorkflowUiFeature::NamedWidgets),
                    "node exports named widget values",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            }
            if !node.widgets_values.is_empty() {
                builder.add(
                    WorkflowUiFeature::PositionalWidgets,
                    supported_status(WorkflowUiFeature::PositionalWidgets),
                    "node exports positional widget values",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            }
            if is_reroute_type(&node.class_type) {
                builder.add(
                    WorkflowUiFeature::RerouteNode,
                    supported_status(WorkflowUiFeature::RerouteNode),
                    "Reroute normalization is not implemented in UI compatibility V1A",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            } else if is_generic_primitive_type(&node.class_type) {
                builder.add(
                    WorkflowUiFeature::PrimitiveNode,
                    supported_status(WorkflowUiFeature::PrimitiveNode),
                    "generic PrimitiveNode materialization is not implemented in UI compatibility V1A",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            } else if is_note_type(&node.class_type) {
                builder.add(
                    WorkflowUiFeature::NoteNode,
                    supported_status(WorkflowUiFeature::NoteNode),
                    "generic Note serialization is not implemented in UI compatibility V1A",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            } else if is_runtime_primitive_type(&node.class_type) {
                builder.add(
                    WorkflowUiFeature::RuntimePrimitiveNode,
                    supported_status(WorkflowUiFeature::RuntimePrimitiveNode),
                    "runtime primitive support depends on object_info schema evidence",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            }
            if looks_like_uuid(&node.class_type) {
                builder.add(
                    WorkflowUiFeature::UuidCompositeNodeType,
                    supported_status(WorkflowUiFeature::UuidCompositeNodeType),
                    "UUID node types are reserved for subgraph/composite serialization",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            }
            if is_known_frontend_virtual_class(&node.class_type)
                || is_serialized_alias_candidate(node)
            {
                builder.add(
                    WorkflowUiFeature::FrontendVirtualNode,
                    supported_status(WorkflowUiFeature::FrontendVirtualNode),
                    "verified frontend virtual or serialized alias contract",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            } else if is_explicit_unknown_virtual(node) {
                builder.add(
                    WorkflowUiFeature::UnknownUiNode,
                    supported_status(WorkflowUiFeature::UnknownUiNode),
                    "virtual node is not covered by a verified frontend contract",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            }
            if node.inputs.iter().any(|input| {
                input.name.contains('.')
                    || input
                        .declared_type
                        .as_deref()
                        .is_some_and(|kind| kind.to_ascii_uppercase().contains("DYNAMIC"))
            }) {
                builder.add(
                    WorkflowUiFeature::DynamicInputs,
                    supported_status(WorkflowUiFeature::DynamicInputs),
                    "expanded or dynamic input names require schema evidence",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            }
            if node.inputs.iter().any(|input| {
                input
                    .declared_type
                    .as_deref()
                    .is_some_and(|kind| kind.to_ascii_uppercase().contains("AUTOGROW"))
            }) {
                builder.add(
                    WorkflowUiFeature::AutogrowInputs,
                    supported_status(WorkflowUiFeature::AutogrowInputs),
                    "autogrow input serialization requires schema evidence",
                    Some(&node.id),
                    Some(&node.class_type),
                    None,
                );
            }
        }
    }

    if let Some(raw_links) = root.get("links").and_then(Value::as_array) {
        for raw_link in raw_links {
            let Some(tuple) = raw_link.as_array() else {
                continue;
            };
            let feature = match tuple.len() {
                5 => WorkflowUiFeature::ClassicLinks5,
                6 => WorkflowUiFeature::ClassicLinks6,
                _ => WorkflowUiFeature::UnknownLinkEncoding,
            };
            builder.add(
                feature,
                supported_status(feature),
                if feature == WorkflowUiFeature::UnknownLinkEncoding {
                    "link tuple is not a verified five- or six-field serialization"
                } else {
                    "verified ComfyUI link tuple shape"
                },
                None,
                None,
                tuple.first().and_then(Value::as_i64),
            );
        }
    }

    let subgraphs = root
        .get("definitions")
        .and_then(Value::as_object)
        .and_then(|definitions| definitions.get("subgraphs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .filter(|subgraph| {
            subgraph
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.trim().is_empty())
        })
        .collect::<Vec<_>>();
    if !subgraphs.is_empty() {
        builder.add(
            WorkflowUiFeature::DefinitionsSubgraphs,
            supported_status(WorkflowUiFeature::DefinitionsSubgraphs),
            "definitions.subgraphs normalization is not implemented in UI compatibility V1A",
            None,
            None,
            None,
        );
        if subgraphs.iter().any(|subgraph| {
            subgraph
                .get("definitions")
                .and_then(Value::as_object)
                .and_then(|definitions| definitions.get("subgraphs"))
                .and_then(Value::as_array)
                .is_some_and(|nested| !nested.is_empty())
        }) {
            builder.add(
                WorkflowUiFeature::NestedSubgraphs,
                supported_status(WorkflowUiFeature::NestedSubgraphs),
                "nested definitions.subgraphs normalization is not implemented in UI compatibility V1A",
                None,
                None,
                None,
            );
        }
    }
    builder.finish()
}

pub fn build_source_id_mapping(document: &UiWorkflowDocument) -> UiSourceIdMapping {
    let mut source_ids = BTreeSet::new();
    source_ids.extend(document.nodes.iter().map(|node| node.id.clone()));
    source_ids.extend(
        document
            .links
            .iter()
            .flat_map(|link| [&link.origin_node_id, &link.target_node_id])
            .cloned(),
    );

    let mut used_api_ids = source_ids
        .iter()
        .filter(|source_id| is_frozen_api_node_id(source_id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut source_to_api = BTreeMap::new();
    for source_id in source_ids {
        let api_id = if is_frozen_api_node_id(&source_id) {
            source_id.clone()
        } else {
            let mut ordinal = 1_u64;
            loop {
                let candidate = format!("0:{ordinal}");
                if used_api_ids.insert(candidate.clone()) {
                    break candidate;
                }
                ordinal += 1;
            }
        };
        source_to_api.insert(source_id, api_id);
    }
    let api_to_source = source_to_api
        .iter()
        .map(|(source_id, api_id)| (api_id.clone(), source_id.clone()))
        .collect();
    UiSourceIdMapping {
        source_to_api,
        api_to_source,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationError {
    pub code: &'static str,
    pub message: String,
    pub feature: Option<WorkflowUiFeature>,
    pub node_id: Option<String>,
    pub node_type: Option<String>,
    pub link_id: Option<i64>,
}

impl NormalizationError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            feature: None,
            node_id: None,
            node_type: None,
            link_id: None,
        }
    }

    fn unsupported_feature(observation: &WorkflowUiFeatureObservation) -> Self {
        let location = observation
            .node_ids
            .first()
            .map(|node_id| format!(" node {node_id}"))
            .or_else(|| {
                observation
                    .link_ids
                    .first()
                    .map(|link_id| format!(" link {link_id}"))
            })
            .unwrap_or_default();
        Self {
            code: "UNSUPPORTED_UI_FEATURE",
            message: format!(
                "UI feature {} is not supported{}: {}",
                observation.feature.as_str(),
                location,
                observation.reason
            ),
            feature: Some(observation.feature),
            node_id: observation.node_ids.first().cloned(),
            node_type: observation.node_types.first().cloned(),
            link_id: observation.link_ids.first().copied(),
        }
    }
}

impl fmt::Display for NormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for NormalizationError {}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedLinkInput {
    pub origin_node_id: String,
    pub origin_slot: usize,
}

pub type NormalizedLinkMap = BTreeMap<(String, String), NormalizedLinkInput>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedOutputEvidence {
    pub node_id: String,
    pub slot: usize,
    pub name: Option<String>,
    pub declared_type: Option<String>,
    pub output_node: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedWorkflow {
    pub workflow: WorkflowDocument,
    pub api_value: Value,
    pub api_bytes: Vec<u8>,
    pub links: NormalizedLinkMap,
    pub output_evidence: Vec<NormalizedOutputEvidence>,
    pub source_to_api: BTreeMap<UiSourceNodeId, NormalizedApiNodeId>,
    pub api_to_source: BTreeMap<NormalizedApiNodeId, UiSourceNodeId>,
    pub compatibility: NormalizationCompatibilityContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiParseError {
    pub code: &'static str,
    pub message: String,
}

impl UiParseError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for UiParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for UiParseError {}

/// Parse a source ComfyUI workflow without performing conversion or I/O.
pub fn parse_ui_workflow(bytes: &[u8]) -> Result<UiWorkflowDocument, UiParseError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| UiParseError::new("UI_JSON_INVALID", error.to_string()))?;
    parse_ui_workflow_value(&value)
}

pub fn parse_ui_workflow_value(value: &Value) -> Result<UiWorkflowDocument, UiParseError> {
    let root = value
        .as_object()
        .ok_or_else(|| UiParseError::new("UI_ROOT_INVALID", "workflow root must be an object"))?;
    let version = root.get("version").and_then(Value::as_f64).ok_or_else(|| {
        UiParseError::new(
            "UI_WORKFLOW_FORMAT_VERSION_MISSING",
            "workflow root is missing numeric version",
        )
    })?;
    let workflow_format_version = format_number(version);
    let frontend_version = root
        .get("extra")
        .and_then(Value::as_object)
        .and_then(|extra| extra.get("frontendVersion"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty());

    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| UiParseError::new("UI_NODES_MISSING", "workflow nodes must be an array"))?;
    let links = root
        .get("links")
        .and_then(Value::as_array)
        .ok_or_else(|| UiParseError::new("UI_LINKS_MISSING", "workflow links must be an array"))?;

    let mut parsed_nodes = Vec::with_capacity(nodes.len());
    let mut ids = std::collections::HashSet::new();
    for (index, node_value) in nodes.iter().enumerate() {
        let node = node_value.as_object().ok_or_else(|| {
            UiParseError::new(
                "UI_NODE_INVALID",
                format!("node at index {index} must be an object"),
            )
        })?;
        let id = parse_node_id(node.get("id"), index)?;
        if !ids.insert(id.clone()) {
            return Err(UiParseError::new(
                "UI_DUPLICATE_NODE_ID",
                format!("node id {id} appears more than once"),
            ));
        }
        let class_type = node
            .get("type")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                UiParseError::new("UI_NODE_TYPE_MISSING", format!("node {id} is missing type"))
            })?
            .to_owned();
        let title = node
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|value| !value.trim().is_empty());
        let mode = UiNodeMode::parse(node.get("mode").and_then(Value::as_i64).unwrap_or(0));
        let inputs = parse_input_sockets(node.get("inputs"), &id, &class_type)?;
        let outputs = parse_output_sockets(node.get("outputs"), &id)?;
        let mut widgets_values_named = node
            .get("widgets_values_named")
            .map(|value| {
                let object = value.as_object().ok_or_else(|| {
                    UiParseError::new(
                        "UI_NAMED_WIDGETS_INVALID",
                        format!("node {id} widgets_values_named must be an object"),
                    )
                })?;
                Ok(object
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect())
            })
            .transpose()?;
        let widgets_values = match node.get("widgets_values") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(values)) => values.clone(),
            Some(Value::Object(values)) => {
                if widgets_values_named.is_none() {
                    widgets_values_named = Some(
                        values
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect(),
                    );
                }
                Vec::new()
            }
            Some(_) => {
                return Err(UiParseError::new(
                    "UI_WIDGETS_VALUES_INVALID",
                    format!("node {id} widgets_values must be an array or object"),
                ));
            }
        };
        let properties = node
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        parsed_nodes.push(UiNode {
            id,
            class_type,
            title,
            mode,
            inputs,
            outputs,
            widgets_values,
            widgets_values_named,
            properties,
        });
    }

    let mut parsed_links = Vec::with_capacity(links.len());
    for (index, link_value) in links.iter().enumerate() {
        let tuple = link_value.as_array().ok_or_else(|| {
            UiParseError::new(
                "UI_LINK_TUPLE_INVALID",
                format!("link at index {index} must be an array"),
            )
        })?;
        if tuple.len() < 5 {
            return Err(UiParseError::new(
                "UI_LINK_TUPLE_INVALID",
                format!("link at index {index} must have at least five fields"),
            ));
        }
        let id = tuple[0].as_i64().ok_or_else(|| {
            UiParseError::new(
                "UI_LINK_TUPLE_INVALID",
                format!("link {index} id is invalid"),
            )
        })?;
        let origin_node_id = parse_node_id(Some(&tuple[1]), index)?;
        let origin_slot = parse_slot(&tuple[2], "origin", id)?;
        let target_node_id = parse_node_id(Some(&tuple[3]), index)?;
        let target_slot = parse_slot(&tuple[4], "target", id)?;
        let declared_type = tuple.get(5).and_then(Value::as_str).map(str::to_owned);
        parsed_links.push(UiLink {
            id,
            origin_node_id,
            origin_slot,
            target_node_id,
            target_slot,
            declared_type,
        });
    }

    let features = detect_ui_features(root, &parsed_nodes);
    Ok(UiWorkflowDocument {
        workflow_format_version,
        frontend_version,
        nodes: parsed_nodes,
        links: parsed_links,
        features,
    })
}

fn parse_node_id(value: Option<&Value>, index: usize) -> Result<String, UiParseError> {
    let value = value.ok_or_else(|| {
        UiParseError::new(
            "UI_NODE_ID_MISSING",
            format!("node/link field {index} is missing"),
        )
    })?;
    match value {
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) if !value.trim().is_empty() => Ok(value.clone()),
        _ => Err(UiParseError::new(
            "UI_NODE_ID_INVALID",
            format!("node/link field {index} has an invalid id"),
        )),
    }
}

fn parse_slot(value: &Value, side: &str, link_id: i64) -> Result<usize, UiParseError> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            UiParseError::new(
                "UI_LINK_SLOT_INVALID",
                format!("{side} slot for link {link_id} is invalid"),
            )
        })
}

fn parse_input_sockets(
    value: Option<&Value>,
    node_id: &str,
    class_type: &str,
) -> Result<Vec<UiInputSocket>, UiParseError> {
    let values = value.and_then(Value::as_array).ok_or_else(|| {
        UiParseError::new(
            "UI_NODE_INPUTS_INVALID",
            format!("node {node_id} inputs must be an array"),
        )
    })?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let object = value.as_object().ok_or_else(|| {
                UiParseError::new(
                    "UI_NODE_INPUT_INVALID",
                    format!("node {node_id} input {index} must be an object"),
                )
            })?;
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .or_else(|| is_reroute_type(class_type).then(|| format!("__ui_socket_{index}")))
                .ok_or_else(|| {
                    UiParseError::new(
                        "UI_NODE_INPUT_NAME_MISSING",
                        format!("node {node_id} input {index} is missing name"),
                    )
                })?;
            let link_id = match object.get("link") {
                None | Some(Value::Null) => None,
                Some(value) => Some(value.as_i64().ok_or_else(|| {
                    UiParseError::new(
                        "UI_NODE_INPUT_LINK_INVALID",
                        format!("node {node_id} input {name} has an invalid link"),
                    )
                })?),
            };
            let widget_name = object
                .get("widget")
                .and_then(Value::as_object)
                .and_then(|widget| widget.get("name"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            Ok(UiInputSocket {
                name,
                declared_type: object
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                link_id,
                widget_name,
                shape: object.get("shape").and_then(Value::as_i64),
            })
        })
        .collect()
}

fn parse_output_sockets(
    value: Option<&Value>,
    node_id: &str,
) -> Result<Vec<UiOutputSocket>, UiParseError> {
    let Some(values) = value else {
        return Ok(Vec::new());
    };
    let values = values.as_array().ok_or_else(|| {
        UiParseError::new(
            "UI_NODE_OUTPUTS_INVALID",
            format!("node {node_id} outputs must be an array"),
        )
    })?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let object = value.as_object().ok_or_else(|| {
                UiParseError::new(
                    "UI_NODE_OUTPUT_INVALID",
                    format!("node {node_id} output {index} must be an object"),
                )
            })?;
            Ok(UiOutputSocket {
                name: object
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                declared_type: object
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                link_ids: object
                    .get("links")
                    .and_then(Value::as_array)
                    .map(|links| links.iter().filter_map(Value::as_i64).collect())
                    .unwrap_or_default(),
                widget_name: object
                    .get("widget")
                    .and_then(Value::as_object)
                    .and_then(|widget| widget.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}

/// Reconstruct only API-style links.  Widget values are intentionally not
/// inspected here so link corruption cannot be hidden by a later cursor.
pub fn normalize_links(
    document: &UiWorkflowDocument,
    descriptors: &UiSerializationDescriptorSet,
) -> Result<NormalizedLinkMap, NormalizationError> {
    let classifications = classify_ui_nodes(document, descriptors);
    Ok(normalize_graph(document, descriptors, &classifications)?.links)
}

fn active_node_map<'a>(
    document: &'a UiWorkflowDocument,
) -> Result<BTreeMap<String, &'a UiNode>, NormalizationError> {
    let mut nodes = BTreeMap::new();
    for node in &document.nodes {
        match node.mode {
            UiNodeMode::Always | UiNodeMode::Bypass => {
                nodes.insert(node.id.clone(), node);
            }
            UiNodeMode::Never => {}
            UiNodeMode::Unknown(mode) => {
                return Err(NormalizationError::new(
                    "UNSUPPORTED_NODE_MODE",
                    format!("node {} has unsupported mode {mode}", node.id),
                ));
            }
        }
    }
    Ok(nodes)
}

#[derive(Clone, Debug, PartialEq)]
struct NormalizedUiGraph {
    links: NormalizedLinkMap,
    literal_inputs: BTreeMap<(String, String), Value>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ResolvedOrigin<'a> {
    Runtime { node: &'a UiNode, slot: usize },
    Primitive { node: &'a UiNode, slot: usize },
}

fn classification_for<'a>(
    classifications: &'a BTreeMap<String, UiNodeClassificationResult>,
    node: &UiNode,
) -> &'a UiNodeClassificationResult {
    classifications
        .get(&node.id)
        .expect("every parsed node must have a classification")
}

fn validate_raw_link(
    link: &UiLink,
    origin: &UiNode,
    target: &UiNode,
) -> Result<(), NormalizationError> {
    let output = origin.outputs.get(link.origin_slot).ok_or_else(|| {
        NormalizationError::new(
            "LINK_ORIGIN_SLOT_INVALID",
            format!(
                "link {} origin slot {} is absent from node {}",
                link.id, link.origin_slot, origin.id
            ),
        )
    })?;
    let target_socket = target.inputs.get(link.target_slot).ok_or_else(|| {
        NormalizationError::new(
            "LINK_TARGET_SLOT_INVALID",
            format!(
                "link {} target slot {} is absent from node {}",
                link.id, link.target_slot, target.id
            ),
        )
    })?;
    if target_socket.link_id != Some(link.id) {
        return Err(NormalizationError::new(
            "LINK_ID_MISMATCH",
            format!(
                "link {} disagrees with node {} input {} link {:?}",
                link.id, target.id, target_socket.name, target_socket.link_id
            ),
        ));
    }
    if !compatible_socket_types(
        output.declared_type.as_deref(),
        target_socket.declared_type.as_deref(),
    ) || !compatible_socket_types(
        output.declared_type.as_deref(),
        link.declared_type.as_deref(),
    ) || !compatible_socket_types(
        target_socket.declared_type.as_deref(),
        link.declared_type.as_deref(),
    ) {
        return Err(NormalizationError::new(
            "LINK_TYPE_CONTRADICTION",
            format!("link {} has contradictory socket type evidence", link.id),
        ));
    }
    if !output.link_ids.is_empty() && !output.link_ids.contains(&link.id) {
        return Err(NormalizationError::new(
            "LINK_ORIGIN_EVIDENCE_MISMATCH",
            format!("link {} is absent from origin output evidence", link.id),
        ));
    }
    Ok(())
}

fn resolved_output_type<'a>(origin: &ResolvedOrigin<'a>) -> Option<&'a str> {
    match origin {
        ResolvedOrigin::Runtime { node, slot } | ResolvedOrigin::Primitive { node, slot } => node
            .outputs
            .get(*slot)
            .and_then(|output| output.declared_type.as_deref()),
    }
}

fn resolve_bypass_origin<'a>(
    origin: &'a UiNode,
    origin_slot: usize,
    aliases: &HashMap<String, UiAliasBinding>,
    nodes: &BTreeMap<String, &'a UiNode>,
    classifications: &BTreeMap<String, UiNodeClassificationResult>,
    incoming: &HashMap<String, Vec<UiLink>>,
    seen: &mut HashSet<String>,
) -> Result<ResolvedOrigin<'a>, NormalizationError> {
    if classification_for(classifications, origin).contract != UiNodeContract::Runtime {
        return Err(NormalizationError::new(
            "BYPASS_CONTRACT_UNSUPPORTED",
            format!(
                "bypass node {} has no schema-backed runtime pass-through contract",
                origin.id
            ),
        ));
    }
    if !seen.insert(origin.id.clone()) {
        return Err(NormalizationError::new(
            "BYPASS_CYCLE",
            format!("bypass cycle reaches node {}", origin.id),
        ));
    }
    let result = (|| {
        let output = origin.outputs.get(origin_slot).ok_or_else(|| {
            NormalizationError::new(
                "BYPASS_OUTPUT_SLOT_INVALID",
                format!(
                    "bypass node {} output slot {} is absent",
                    origin.id, origin_slot
                ),
            )
        })?;
        let incoming_links = incoming.get(&origin.id).cloned().unwrap_or_default();
        let mut candidates = Vec::new();
        for bridge in &incoming_links {
            let bridge_target_socket = origin.inputs.get(bridge.target_slot).ok_or_else(|| {
                NormalizationError::new(
                    "LINK_TARGET_SLOT_INVALID",
                    format!(
                        "bypass node {} target slot {} is absent",
                        origin.id, bridge.target_slot
                    ),
                )
            })?;
            let bridge_origin = nodes.get(&bridge.origin_node_id).ok_or_else(|| {
                NormalizationError::new(
                    "LINK_ORIGIN_MISSING",
                    format!(
                        "bypass node {} points to missing node {}",
                        origin.id, bridge.origin_node_id
                    ),
                )
            })?;
            let resolved = resolve_origin(
                bridge_origin,
                bridge.origin_slot,
                aliases,
                nodes,
                classifications,
                incoming,
                seen,
            )?;
            let source_type = resolved_output_type(&resolved);
            if compatible_socket_types(source_type, bridge_target_socket.declared_type.as_deref())
                && compatible_socket_types(source_type, bridge.declared_type.as_deref())
                && compatible_socket_types(source_type, output.declared_type.as_deref())
            {
                candidates.push(resolved);
            }
        }
        match candidates.as_slice() {
            [] => Err(NormalizationError::new(
                "BYPASS_SOURCE_UNAVAILABLE",
                format!(
                    "bypass node {} has no incoming source compatible with output slot {}",
                    origin.id, origin_slot
                ),
            )),
            [resolved] => Ok(*resolved),
            _ => Err(NormalizationError::new(
                "BYPASS_AMBIGUOUS_FANIN",
                format!(
                    "bypass node {} has multiple compatible incoming sources",
                    origin.id
                ),
            )),
        }
    })();
    seen.remove(&origin.id);
    result
}

fn resolve_origin<'a>(
    origin: &'a UiNode,
    origin_slot: usize,
    aliases: &HashMap<String, UiAliasBinding>,
    nodes: &BTreeMap<String, &'a UiNode>,
    classifications: &BTreeMap<String, UiNodeClassificationResult>,
    incoming: &HashMap<String, Vec<UiLink>>,
    seen: &mut HashSet<String>,
) -> Result<ResolvedOrigin<'a>, NormalizationError> {
    if origin.mode == UiNodeMode::Bypass {
        return resolve_bypass_origin(
            origin,
            origin_slot,
            aliases,
            nodes,
            classifications,
            incoming,
            seen,
        );
    }
    let classification = classification_for(classifications, origin);
    match classification.contract {
        UiNodeContract::Runtime => {
            if origin_slot >= origin.outputs.len() {
                return Err(NormalizationError::new(
                    "LINK_ORIGIN_SLOT_INVALID",
                    format!("node {} origin slot {} is absent", origin.id, origin_slot),
                ));
            }
            Ok(ResolvedOrigin::Runtime {
                node: origin,
                slot: origin_slot,
            })
        }
        UiNodeContract::PrimitiveBinding => {
            if origin_slot >= origin.outputs.len() {
                return Err(NormalizationError::new(
                    "LINK_ORIGIN_SLOT_INVALID",
                    format!("node {} origin slot {} is absent", origin.id, origin_slot),
                ));
            }
            Ok(ResolvedOrigin::Primitive {
                node: origin,
                slot: origin_slot,
            })
        }
        UiNodeContract::Reroute => {
            if !seen.insert(origin.id.clone()) {
                return Err(NormalizationError::new(
                    "REROUTE_CYCLE",
                    format!("Reroute cycle reaches node {}", origin.id),
                ));
            }
            let incoming_links = incoming.get(&origin.id).cloned().unwrap_or_default();
            let result = if incoming_links.is_empty() {
                Err(NormalizationError::new(
                    "REROUTE_SOURCE_MISSING",
                    format!("Reroute {} has no incoming source", origin.id),
                ))
            } else if incoming_links.len() != 1 {
                Err(NormalizationError::new(
                    "REROUTE_AMBIGUOUS_FANIN",
                    format!(
                        "Reroute {} has {} incoming sources",
                        origin.id,
                        incoming_links.len()
                    ),
                ))
            } else {
                let bridge = &incoming_links[0];
                let bridge_target_socket =
                    origin.inputs.get(bridge.target_slot).ok_or_else(|| {
                        NormalizationError::new(
                            "LINK_TARGET_SLOT_INVALID",
                            format!(
                                "Reroute {} target slot {} is absent",
                                origin.id, bridge.target_slot
                            ),
                        )
                    })?;
                let bridge_origin = nodes.get(&bridge.origin_node_id).ok_or_else(|| {
                    NormalizationError::new(
                        "LINK_ORIGIN_MISSING",
                        format!(
                            "Reroute {} points to missing node {}",
                            origin.id, bridge.origin_node_id
                        ),
                    )
                })?;
                let resolved = resolve_origin(
                    bridge_origin,
                    bridge.origin_slot,
                    aliases,
                    nodes,
                    classifications,
                    incoming,
                    seen,
                )?;
                let source_type = resolved_output_type(&resolved);
                let reroute_type = origin
                    .outputs
                    .get(origin_slot)
                    .and_then(|output| output.declared_type.as_deref());
                if !compatible_socket_types(
                    source_type,
                    bridge_target_socket.declared_type.as_deref(),
                ) || !compatible_socket_types(source_type, bridge.declared_type.as_deref())
                    || !compatible_socket_types(source_type, reroute_type)
                {
                    Err(NormalizationError::new(
                        "REROUTE_TYPE_MISMATCH",
                        format!("Reroute {} changes or contradicts runtime type", origin.id),
                    ))
                } else {
                    Ok(resolved)
                }
            };
            seen.remove(&origin.id);
            result
        }
        UiNodeContract::AliasBinding => {
            if !seen.insert(origin.id.clone()) {
                return Err(NormalizationError::new(
                    "ALIAS_CYCLE",
                    format!("alias cycle reaches node {}", origin.id),
                ));
            }
            let result = (|| {
                let role = alias_role(origin).ok_or_else(|| {
                    NormalizationError::new(
                        "ALIAS_CONTRACT_INVALID",
                        format!("alias node {} has no typed role", origin.id),
                    )
                })?;
                let variable = serialized_alias_identity(origin, role).ok_or_else(|| {
                    NormalizationError::new(
                        "ALIAS_IDENTITY_MISSING",
                        format!("alias node {} has no stable identity", origin.id),
                    )
                })?;
                let binding = aliases.get(&variable).ok_or_else(|| {
                    NormalizationError::new(
                        "ALIAS_CONSUMER_WITHOUT_PRODUCER",
                        format!("alias {variable} has no producer"),
                    )
                })?;
                let consumer_type = origin
                    .outputs
                    .get(origin_slot)
                    .and_then(|output| output.declared_type.as_deref());
                if !compatible_socket_types(consumer_type, Some(&binding.value_type)) {
                    return Err(NormalizationError::new(
                        "ALIAS_TYPE_CONFLICT",
                        format!("alias {variable} consumer type conflicts with producer type"),
                    ));
                }
                let next = nodes.get(&binding.source.origin_node_id).ok_or_else(|| {
                    NormalizationError::new(
                        "LINK_ORIGIN_MISSING",
                        format!(
                            "alias {variable} points to missing node {}",
                            binding.source.origin_node_id
                        ),
                    )
                })?;
                let resolved = resolve_origin(
                    next,
                    binding.source.origin_slot,
                    aliases,
                    nodes,
                    classifications,
                    incoming,
                    seen,
                )?;
                if !compatible_socket_types(
                    resolved_output_type(&resolved),
                    Some(&binding.value_type),
                ) {
                    return Err(NormalizationError::new(
                        "ALIAS_TYPE_CONFLICT",
                        format!("alias {variable} source type conflicts with producer type"),
                    ));
                }
                Ok(resolved)
            })();
            seen.remove(&origin.id);
            result
        }
        UiNodeContract::Presentation => Err(NormalizationError::new(
            "UI_PRESENTATION_NODE_LINKED",
            format!(
                "presentation node {} cannot be used as a runtime source",
                origin.id
            ),
        )),
        UiNodeContract::Composite => Err(NormalizationError::new(
            "UNSUPPORTED_COMPOSITE_NODE",
            format!("composite node {} is not normalized in V1B", origin.id),
        )),
        UiNodeContract::Unknown => Err(NormalizationError::new(
            "UNKNOWN_NODE_CLASS",
            format!(
                "node {} class {} has no verified UI compatibility contract",
                origin.id, origin.class_type
            ),
        )),
    }
}

fn materialize_primitive_binding(
    origin: &UiNode,
    origin_slot: usize,
    target: &UiNode,
    target_socket: &UiInputSocket,
    link: &UiLink,
    descriptors: &UiSerializationDescriptorSet,
) -> Result<Value, NormalizationError> {
    let output = origin.outputs.get(origin_slot).ok_or_else(|| {
        NormalizationError::new(
            "LINK_ORIGIN_SLOT_INVALID",
            format!(
                "PrimitiveNode {} output slot {} is absent",
                origin.id, origin_slot
            ),
        )
    })?;
    let widget_name = output
        .widget_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            NormalizationError::new(
                "PRIMITIVE_UNKNOWN_WIDGET_CONTRACT",
                format!("PrimitiveNode {} output has no widget name", origin.id),
            )
        })?;
    // A ComfyUI PrimitiveNode's widget name identifies the shared frontend
    // control, while fan-out targets may expose different runtime input names
    // (for example length, frames_number, and max_frames). A target socket
    // therefore proves compatibility either by the exact name or by carrying
    // its own serialized widget descriptor.
    if target_socket.name != widget_name && target_socket.widget_name.is_none() {
        return Err(NormalizationError::new(
            "PRIMITIVE_WIDGET_TARGET_MISMATCH",
            format!(
                "PrimitiveNode {} widget {} does not target node {} input {}",
                origin.id, widget_name, target.id, target_socket.name
            ),
        ));
    }
    let target_binding = descriptors
        .resolve_ui_input(
            &target.class_type,
            &target_socket.name,
            target_socket.widget_name.as_deref(),
            target_socket.shape,
            target_socket.link_id.is_some(),
            target.widgets_values_named.as_ref(),
        )
        .map_err(|error| {
            NormalizationError::new(
                error.code,
                format!(
                    "PrimitiveNode {} target {} input {}: {}",
                    origin.id, target.id, target_socket.name, error.message
                ),
            )
        })?;
    let target_name = target_binding.runtime_name.ok_or_else(|| {
        NormalizationError::new(
            "PRIMITIVE_TARGET_SCHEMA_MISSING",
            format!(
                "PrimitiveNode {} target {} input {} has no runtime schema binding",
                origin.id, target.id, target_socket.name
            ),
        )
    })?;
    let descriptor = target_binding.descriptor.ok_or_else(|| {
        NormalizationError::new(
            "PRIMITIVE_TARGET_SCHEMA_MISSING",
            format!(
                "PrimitiveNode {} target {} input {} is absent from object_info",
                origin.id, target.id, target_name
            ),
        )
    })?;
    let expected_type = declared_type_name(descriptor.declared_type);
    if !compatible_socket_types(output.declared_type.as_deref(), Some(expected_type))
        || !compatible_socket_types(
            output.declared_type.as_deref(),
            target_socket.declared_type.as_deref(),
        )
        || !compatible_socket_types(
            output.declared_type.as_deref(),
            link.declared_type.as_deref(),
        )
    {
        return Err(NormalizationError::new(
            "PRIMITIVE_TARGET_TYPE_CONFLICT",
            format!(
                "PrimitiveNode {} cannot bind {} to node {} input {}",
                origin.id, expected_type, target.id, target_name
            ),
        ));
    }
    let value = primitive_binding_value(origin).map_err(|reason| {
        NormalizationError::new(
            "PRIMITIVE_UNKNOWN_WIDGET_CONTRACT",
            format!("PrimitiveNode {}: {reason}", origin.id),
        )
    })?;
    validate_literal(&descriptor, &value).map_err(|reason| {
        NormalizationError::new(
            "PRIMITIVE_VALUE_INVALID",
            format!(
                "PrimitiveNode {} target {}: {reason}",
                origin.id, target_name
            ),
        )
    })?;
    Ok(value)
}

fn collect_virtual_aliases<'a>(
    document: &'a UiWorkflowDocument,
    nodes: &BTreeMap<String, &'a UiNode>,
    classifications: &BTreeMap<String, UiNodeClassificationResult>,
) -> Result<HashMap<String, UiAliasBinding>, NormalizationError> {
    let mut aliases = HashMap::new();
    for producer in nodes.values() {
        if classification_for(classifications, producer).contract != UiNodeContract::AliasBinding
            || alias_role(producer) != Some(UiAliasRole::Producer)
        {
            continue;
        }
        let variable =
            serialized_alias_identity(producer, UiAliasRole::Producer).ok_or_else(|| {
                NormalizationError::new(
                    "ALIAS_IDENTITY_MISSING",
                    format!("alias producer {} has no stable identity", producer.id),
                )
            })?;
        let incoming = document
            .links
            .iter()
            .filter(|link| link.target_node_id == producer.id)
            .collect::<Vec<_>>();
        if incoming.len() != 1 {
            return Err(NormalizationError::new(
                "AMBIGUOUS_ALIAS_PRODUCER",
                format!(
                    "alias producer {} has {} incoming links",
                    producer.id,
                    incoming.len()
                ),
            ));
        }
        let link = incoming[0];
        let input = producer.inputs.first().ok_or_else(|| {
            NormalizationError::new(
                "ALIAS_PRODUCER_SOURCE_MISSING",
                format!("alias producer {} has no input socket", producer.id),
            )
        })?;
        if input.link_id != Some(link.id) {
            return Err(NormalizationError::new(
                "ALIAS_PRODUCER_SOURCE_MISSING",
                format!(
                    "alias producer {} input evidence is inconsistent",
                    producer.id
                ),
            ));
        }
        let input_type = input.declared_type.as_deref();
        let output_type = producer
            .outputs
            .first()
            .and_then(|output| output.declared_type.as_deref());
        if !compatible_socket_types(input_type, output_type)
            || !compatible_socket_types(input_type, link.declared_type.as_deref())
        {
            return Err(NormalizationError::new(
                "ALIAS_TYPE_CONFLICT",
                format!("alias producer {variable} has incompatible typed evidence"),
            ));
        }
        let value_type = output_type.ok_or_else(|| {
            NormalizationError::new(
                "ALIAS_TYPE_CONFLICT",
                format!("alias producer {variable} has no output type"),
            )
        })?;
        if aliases
            .insert(
                variable.clone(),
                UiAliasBinding {
                    source: NormalizedLinkInput {
                        origin_node_id: link.origin_node_id.clone(),
                        origin_slot: link.origin_slot,
                    },
                    value_type: value_type.to_owned(),
                },
            )
            .is_some()
        {
            return Err(NormalizationError::new(
                "AMBIGUOUS_ALIAS_PRODUCER",
                format!("alias {variable} has more than one producer"),
            ));
        }
    }
    Ok(aliases)
}

fn normalize_graph(
    document: &UiWorkflowDocument,
    descriptors: &UiSerializationDescriptorSet,
    classifications: &BTreeMap<String, UiNodeClassificationResult>,
) -> Result<NormalizedUiGraph, NormalizationError> {
    if let Some(observation) = document
        .features
        .with_schema(document, descriptors)
        .blocking_observation()
    {
        return Err(NormalizationError::unsupported_feature(observation));
    }
    let nodes = active_node_map(document)?;
    let aliases = collect_virtual_aliases(document, &nodes, classifications)?;
    let mut links_by_id = HashMap::new();
    let mut incoming = HashMap::<String, Vec<UiLink>>::new();

    for link in &document.links {
        if links_by_id.insert(link.id, link.clone()).is_some() {
            return Err(NormalizationError::new(
                "DUPLICATE_LINK_ID",
                format!("link id {} is declared by more than one tuple", link.id),
            ));
        }
        let (Some(_origin), Some(target)) = (
            nodes.get(&link.origin_node_id),
            nodes.get(&link.target_node_id),
        ) else {
            continue;
        };
        incoming
            .entry(target.id.clone())
            .or_default()
            .push(link.clone());
    }
    for link in &document.links {
        let (Some(origin), Some(target)) = (
            nodes.get(&link.origin_node_id),
            nodes.get(&link.target_node_id),
        ) else {
            continue;
        };
        if classification_for(classifications, target).contract == UiNodeContract::Reroute
            && incoming
                .get(&target.id)
                .is_some_and(|links| links.len() > 1)
        {
            return Err(NormalizationError::new(
                "REROUTE_AMBIGUOUS_FANIN",
                format!("Reroute {} has multiple incoming sources", target.id),
            ));
        }
        validate_raw_link(link, origin, target)?;
    }

    // Preflight every connected structural node. This catches cycles and
    // ambiguous fan-in even when a malformed Reroute has no runtime sink.
    for node in nodes.values() {
        if classification_for(classifications, node).contract != UiNodeContract::Reroute {
            continue;
        }
        if document
            .links
            .iter()
            .any(|link| link.origin_node_id == node.id && nodes.contains_key(&link.target_node_id))
        {
            let _ = resolve_origin(
                node,
                0,
                &aliases,
                &nodes,
                classifications,
                &incoming,
                &mut HashSet::new(),
            )?;
        }
    }

    let mut normalized = BTreeMap::new();
    let mut literal_inputs = BTreeMap::new();
    let mut seen_targets = HashSet::new();
    for link in &document.links {
        let (Some(origin_raw), Some(target)) = (
            nodes.get(&link.origin_node_id),
            nodes.get(&link.target_node_id),
        ) else {
            continue;
        };
        if target.mode == UiNodeMode::Bypass {
            continue;
        }
        let target_classification = classification_for(classifications, target);
        match target_classification.contract {
            UiNodeContract::Presentation => continue,
            UiNodeContract::Reroute => continue,
            UiNodeContract::AliasBinding => continue,
            UiNodeContract::PrimitiveBinding => {
                return Err(NormalizationError::new(
                    "PRIMITIVE_TARGET_INVALID",
                    format!(
                        "PrimitiveNode {} cannot be a runtime link target",
                        target.id
                    ),
                ));
            }
            UiNodeContract::Composite | UiNodeContract::Unknown => {
                return Err(NormalizationError::new(
                    "UNKNOWN_NODE_CLASS",
                    format!(
                        "node {} class {} has no verified UI compatibility contract",
                        target.id, target.class_type
                    ),
                ));
            }
            UiNodeContract::Runtime => {}
        }
        if descriptors.node(&target.class_type).is_none() {
            return Err(NormalizationError::new(
                "UNKNOWN_NODE_CLASS",
                format!(
                    "node {} class {} is absent from object_info",
                    target.id, target.class_type
                ),
            ));
        }
        let target_socket = target.inputs.get(link.target_slot).ok_or_else(|| {
            NormalizationError::new(
                "LINK_TARGET_SLOT_INVALID",
                format!(
                    "link {} target slot {} is absent",
                    link.id, link.target_slot
                ),
            )
        })?;
        let target_binding = descriptors
            .resolve_ui_input(
                &target.class_type,
                &target_socket.name,
                target_socket.widget_name.as_deref(),
                target_socket.shape,
                target_socket.link_id.is_some(),
                target.widgets_values_named.as_ref(),
            )
            .map_err(|error| {
                NormalizationError::new(
                    error.code,
                    format!(
                        "node {} input {}: {}",
                        target.id, target_socket.name, error.message
                    ),
                )
            })?;
        let target_name = target_binding.runtime_name.ok_or_else(|| {
            NormalizationError::new(
                "LINK_INPUT_NOT_IN_SCHEMA",
                format!(
                    "node {} input {} has no runtime schema binding",
                    target.id, target_socket.name
                ),
            )
        })?;
        if !seen_targets.insert((target.id.clone(), target_name.clone())) {
            return Err(NormalizationError::new(
                "DUPLICATE_TARGET_INPUT_LINK",
                format!(
                    "node {} input {} has multiple links",
                    target.id, target_name
                ),
            ));
        }
        let resolved = match resolve_origin(
            origin_raw,
            link.origin_slot,
            &aliases,
            &nodes,
            classifications,
            &incoming,
            &mut HashSet::new(),
        ) {
            Err(error) if error.code == "BYPASS_SOURCE_UNAVAILABLE" => continue,
            result => result?,
        };
        match resolved {
            ResolvedOrigin::Primitive { node, slot } => {
                let value = materialize_primitive_binding(
                    node,
                    slot,
                    target,
                    target_socket,
                    link,
                    descriptors,
                )?;
                literal_inputs.insert((target.id.clone(), target_name.clone()), value);
            }
            ResolvedOrigin::Runtime { node, slot } => {
                descriptors
                    .input_for_node(
                        &target.class_type,
                        target.widgets_values_named.as_ref(),
                        &target_name,
                    )
                    .ok_or_else(|| {
                        NormalizationError::new(
                            "LINK_INPUT_NOT_IN_SCHEMA",
                            format!(
                                "node {} input {} is not in object_info",
                                target.id, target_name
                            ),
                        )
                    })?;
                let _ = node.outputs.get(slot);
                normalized.insert(
                    (target.id.clone(), target_name),
                    NormalizedLinkInput {
                        origin_node_id: node.id.clone(),
                        origin_slot: slot,
                    },
                );
            }
        }
    }

    for node in nodes.values() {
        if classification_for(classifications, node).contract != UiNodeContract::Runtime {
            continue;
        }
        for input in &node.inputs {
            if let Some(link_id) = input.link_id {
                if !links_by_id.contains_key(&link_id) {
                    let inactive_origin = document.links.iter().any(|link| {
                        link.id == link_id && !nodes.contains_key(&link.origin_node_id)
                    });
                    if inactive_origin {
                        continue;
                    }
                    return Err(NormalizationError::new(
                        "LINK_NODE_EVIDENCE_MISSING",
                        format!(
                            "node {} input {} references missing link {}",
                            node.id, input.name, link_id
                        ),
                    ));
                }
            }
        }
    }
    Ok(NormalizedUiGraph {
        links: normalized,
        literal_inputs,
    })
}

fn compatible_socket_types(left: Option<&str>, right: Option<&str>) -> bool {
    let normalize = |value: &str| value.trim().to_ascii_uppercase();
    let (Some(left), Some(right)) = (left, right) else {
        return true;
    };
    let left = normalize(left);
    let right = normalize(right);
    let left_types = left.split(',').map(str::trim);
    let right_types: Vec<_> = right.split(',').map(str::trim).collect();
    left_types.into_iter().any(|left_type| {
        right_types.iter().any(|right_type| {
            left_type == *right_type
                || left_type == "*"
                || *right_type == "*"
                || left_type == "ANY"
                || *right_type == "ANY"
                || left_type == "COMFY_AUTOGROW_V3"
                || *right_type == "COMFY_AUTOGROW_V3"
        })
    })
}

/// Normalize literal widget values using the schema-derived descriptor.  A
/// named value is preferred; positional values are accepted only when the
/// descriptor can account for the complete cursor.
pub fn normalize_widgets(
    node: &UiNode,
    descriptors: &UiSerializationDescriptorSet,
    links: &NormalizedLinkMap,
) -> Result<Map<String, Value>, NormalizationError> {
    let descriptor = descriptors.node(&node.class_type).ok_or_else(|| {
        NormalizationError::new(
            "UNKNOWN_NODE_CLASS",
            format!(
                "node {} class {} is absent from object_info",
                node.id, node.class_type
            ),
        )
    })?;
    let mut inputs = Map::new();
    if let Some(named) = &node.widgets_values_named {
        for (name, value) in named {
            let Some(input) = descriptors.input_for_node(
                &node.class_type,
                node.widgets_values_named.as_ref(),
                name,
            ) else {
                // Frontend-only controls such as upload labels and
                // randomization controls are not proof of API inclusion.
                continue;
            };
            if input.hidden || input.serializer_kind == SerializerKind::WorkflowOnlyControl {
                continue;
            }
            if links.contains_key(&(node.id.clone(), name.clone())) {
                continue;
            }
            if !input.serializer_kind.is_standard() {
                return Err(NormalizationError::new(
                    "UNSUPPORTED_SERIALIZER",
                    format!("node {} input {} has no verified serializer", node.id, name),
                ));
            }
            validate_literal(&input, value).map_err(|message| {
                NormalizationError::new(
                    "WIDGET_VALUE_INVALID",
                    format!("node {} input {}: {message}", node.id, name),
                )
            })?;
            inputs.insert(name.clone(), value.clone());
        }
    }

    let positional_names = positional_widget_names(node, descriptors, links)?;
    if !node.widgets_values.is_empty() {
        let positional_values =
            if positional_names.is_empty() && node.widgets_values_named.is_some() {
                Vec::new()
            } else {
                descriptors
                    .consume_positional_values(
                        &node.class_type,
                        &positional_names,
                        &node.widgets_values,
                        node.widgets_values_named.as_ref(),
                    )
                    .map_err(|error| {
                        NormalizationError::new(
                            error.code,
                            format!("node {}: {}", node.id, error.message),
                        )
                    })?
            };
        for (name, value) in positional_values {
            let Some(input) = descriptors.input_for_node(
                &node.class_type,
                node.widgets_values_named.as_ref(),
                &name,
            ) else {
                continue;
            };
            if input.serializer_kind == SerializerKind::WorkflowOnlyControl {
                continue;
            }
            if let Some(named_value) = node
                .widgets_values_named
                .as_ref()
                .and_then(|values| values.get(&name))
            {
                if *named_value != value {
                    return Err(NormalizationError::new(
                        "WIDGET_NAMED_POSITIONAL_CONFLICT",
                        format!(
                            "node {} input {} has conflicting widget values",
                            node.id, name
                        ),
                    ));
                }
            }
            if links.contains_key(&(node.id.clone(), name.clone())) {
                continue;
            }
            if !inputs.contains_key(&name) {
                if !input.serializer_kind.is_standard() {
                    return Err(NormalizationError::new(
                        "UNSUPPORTED_SERIALIZER",
                        format!("node {} input {} has no verified serializer", node.id, name),
                    ));
                }
                validate_literal(&input, &value).map_err(|message| {
                    NormalizationError::new(
                        "WIDGET_VALUE_INVALID",
                        format!("node {} input {}: {message}", node.id, name),
                    )
                })?;
                inputs.insert(name.clone(), value.clone());
            }
        }
    }

    for name in &descriptor.ordered_inputs {
        let Some(input) = descriptors.input(&node.class_type, name) else {
            continue;
        };
        if input.hidden || input.serializer_kind == SerializerKind::WorkflowOnlyControl {
            continue;
        }
        let has_dynamic_value = inputs
            .keys()
            .any(|key| key == name || key.starts_with(&format!("{name}.")));
        let has_dynamic_link = links.keys().any(|(node_id, key)| {
            node_id == &node.id && (key == name || key.starts_with(&format!("{name}.")))
        });
        let has_inactive_source_link = node.inputs.iter().any(|socket| {
            socket.name == *name
                && socket.link_id.is_some()
                && !links.contains_key(&(node.id.clone(), name.clone()))
        });
        if input.required
            && !has_dynamic_value
            && !has_dynamic_link
            && !has_inactive_source_link
            && input.default_value.is_none()
        {
            return Err(NormalizationError::new(
                "WIDGET_REQUIRED_VALUE_MISSING",
                format!(
                    "node {} required input {} has no value or link",
                    node.id, name
                ),
            ));
        }
    }
    Ok(inputs)
}

fn positional_widget_names(
    node: &UiNode,
    descriptors: &UiSerializationDescriptorSet,
    links: &NormalizedLinkMap,
) -> Result<Vec<String>, NormalizationError> {
    let inputs = node
        .inputs
        .iter()
        .map(|socket| UiInputEvidence {
            name: socket.name.clone(),
            widget_name: socket.widget_name.clone(),
            shape: socket.shape,
            linked: socket.link_id.is_some(),
        })
        .collect::<Vec<_>>();
    let linked_names = links
        .keys()
        .filter(|(node_id, _)| node_id == &node.id)
        .map(|(_, input_name)| input_name.clone())
        .collect::<BTreeSet<_>>();
    descriptors
        .positional_input_names(
            &node.class_type,
            &inputs,
            node.widgets_values_named.as_ref(),
            &linked_names,
        )
        .map_err(|error| {
            NormalizationError::new(error.code, format!("node {}: {}", node.id, error.message))
        })
}

fn validate_literal(
    descriptor: &UiInputSerializationDescriptor,
    value: &Value,
) -> Result<(), String> {
    match descriptor.declared_type {
        crate::application::workflow_recognition_schema::RecognitionDeclaredType::String
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::Standard
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::Conditioning
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::Image
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::Video
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::Audio
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::Mask
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::Latent
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::Model
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::VideoModel
        | crate::application::workflow_recognition_schema::RecognitionDeclaredType::DynamicCombo => {
            if !value.is_string() {
                return Err("expected a string literal".to_owned());
            }
        }
        crate::application::workflow_recognition_schema::RecognitionDeclaredType::Enum => {
            if descriptor.enum_values.is_empty() {
                if !value.is_string() {
                    return Err("expected a string literal".to_owned());
                }
            } else if !descriptor.enum_values.iter().any(|option| option == value)
                && !is_environmental_combo_value(value)
            {
                return Err("value is not in the declared options".to_owned());
            }
        }
        crate::application::workflow_recognition_schema::RecognitionDeclaredType::Integer => {
            if !value.as_i64().is_some() && !value.as_u64().is_some() {
                return Err("expected an integer literal".to_owned());
            }
        }
        crate::application::workflow_recognition_schema::RecognitionDeclaredType::Float => {
            if !value.is_number() {
                return Err("expected a numeric literal".to_owned());
            }
        }
        crate::application::workflow_recognition_schema::RecognitionDeclaredType::Boolean => {
            if !value.is_boolean() {
                return Err("expected a boolean literal".to_owned());
            }
        }
        crate::application::workflow_recognition_schema::RecognitionDeclaredType::Unknown => {
            if descriptor.serializer_kind != SerializerKind::StandardDirect || !value.is_string() {
                return Err("declared input type is unknown".to_owned());
            }
        }
    }
    if !descriptor.enum_options.is_empty() && descriptor.enum_values.is_empty() {
        let value = value.as_str().unwrap_or_default();
        if !descriptor.enum_options.iter().any(|option| option == value) {
            return Err(format!("value {value:?} is not in the declared options"));
        }
    }
    if let Some(value) = value.as_f64() {
        if descriptor.numeric_min.is_some_and(|min| value < min)
            || descriptor.numeric_max.is_some_and(|max| value > max)
        {
            return Err("numeric value is outside the declared range".to_owned());
        }
    }
    Ok(())
}

fn is_environmental_combo_value(value: &Value) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    let lower = value.to_ascii_lowercase();
    value.contains('\\')
        || value.contains('/')
        || [
            ".safetensors",
            ".ckpt",
            ".pt",
            ".pth",
            ".png",
            ".jpg",
            ".jpeg",
            ".webp",
            ".mp4",
            ".webm",
            ".mov",
        ]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

/// Convert a verified UI document into a complete API-shaped graph.  This
/// function does not select semantic outputs; it only preserves schema output
/// evidence for the existing WorkflowAnalysisService.
pub fn normalize_ui_workflow(
    document: &UiWorkflowDocument,
    descriptors: &UiSerializationDescriptorSet,
) -> Result<NormalizedWorkflow, NormalizationError> {
    if document.workflow_format_version != descriptors.compatibility.workflow_format_version {
        return Err(NormalizationError::new(
            "WORKFLOW_FORMAT_UNSUPPORTED",
            format!(
                "source workflow format {} does not match supported {}",
                document.workflow_format_version, descriptors.compatibility.workflow_format_version
            ),
        ));
    }
    if document.frontend_version.as_deref()
        != Some(descriptors.compatibility.frontend_version.as_str())
    {
        return Err(NormalizationError::new(
            "FRONTEND_VERSION_UNSUPPORTED",
            "source frontendVersion is missing or not in the verified compatibility set",
        ));
    }
    let feature_set = document.features.with_schema(document, descriptors);
    if let Some(observation) = feature_set.blocking_observation() {
        return Err(NormalizationError::unsupported_feature(observation));
    }
    let id_mapping = build_source_id_mapping(document);
    let classifications = classify_ui_nodes(document, descriptors);
    let graph = normalize_graph(document, descriptors, &classifications)?;
    let links = graph.links;
    let nodes = active_node_map(document)?;
    let mut api_root = Map::new();
    let mut output_evidence = Vec::new();

    for node in document
        .nodes
        .iter()
        .filter(|node| nodes.contains_key(&node.id) && node.mode == UiNodeMode::Always)
    {
        let api_node_id = id_mapping
            .source_to_api
            .get(&node.id)
            .expect("every parsed node must have a source ID mapping");
        let classification = classification_for(&classifications, node);
        if classification.classification != UiNodeClassification::RuntimeNode {
            continue;
        }
        let Some(descriptor) = descriptors.node(&node.class_type) else {
            return Err(NormalizationError::new(
                "UNKNOWN_NODE_CLASS",
                format!(
                    "node {} class {} is absent from object_info",
                    node.id, node.class_type
                ),
            ));
        };
        let mut inputs = normalize_widgets(node, descriptors, &links)?;
        for ((target_node_id, input_name), value) in &graph.literal_inputs {
            if target_node_id != &node.id {
                continue;
            }
            if inputs.contains_key(input_name)
                || links.contains_key(&(node.id.clone(), input_name.clone()))
            {
                return Err(NormalizationError::new(
                    "LINK_LITERAL_COLLISION",
                    format!(
                        "node {} input {} has both link and literal values",
                        node.id, input_name
                    ),
                ));
            }
            inputs.insert(input_name.clone(), value.clone());
        }
        for ((target_node_id, input_name), link) in &links {
            if target_node_id != &node.id {
                continue;
            }
            if inputs.contains_key(input_name) {
                return Err(NormalizationError::new(
                    "LINK_LITERAL_COLLISION",
                    format!(
                        "node {} input {} has both link and literal values",
                        node.id, input_name
                    ),
                ));
            }
            inputs.insert(
                input_name.clone(),
                Value::Array(vec![
                    Value::String(
                        id_mapping
                            .source_to_api
                            .get(&link.origin_node_id)
                            .cloned()
                            .expect("every link origin must have a source ID mapping"),
                    ),
                    Value::from(link.origin_slot as u64),
                ]),
            );
        }
        let mut api_node = Map::new();
        api_node.insert("inputs".to_owned(), Value::Object(inputs));
        api_node.insert(
            "class_type".to_owned(),
            Value::String(node.class_type.clone()),
        );
        if let Some(title) = node.title.as_ref() {
            api_node.insert(
                "_meta".to_owned(),
                Value::Object(Map::from_iter([(
                    "title".to_owned(),
                    Value::String(title.clone()),
                )])),
            );
        }
        api_root.insert(api_node_id.clone(), Value::Object(api_node));

        for (slot, declared_type) in descriptor.output_types.iter().enumerate() {
            output_evidence.push(NormalizedOutputEvidence {
                node_id: api_node_id.clone(),
                slot,
                name: descriptor.output_names.get(slot).cloned(),
                declared_type: Some(declared_type_name(*declared_type).to_owned()),
                output_node: descriptor.output_node,
            });
        }
    }

    let api_value = Value::Object(api_root);
    let workflow = WorkflowDocument::parse(api_value.clone())
        .map_err(|error| NormalizationError::new("NORMALIZED_API_INVALID", error.to_string()))?;
    WorkflowValidator::validate(&workflow)
        .map_err(|error| NormalizationError::new("NORMALIZED_API_INVALID", error.to_string()))?;
    let api_bytes = serde_json::to_vec(&api_value).map_err(|error| {
        NormalizationError::new("NORMALIZED_API_SERIALIZE_FAILED", error.to_string())
    })?;
    let mapped_links = links
        .iter()
        .map(|((target_node_id, input_name), link)| {
            let mapped_target = id_mapping
                .source_to_api
                .get(target_node_id)
                .cloned()
                .expect("every link target must have a source ID mapping");
            let mapped_origin = id_mapping
                .source_to_api
                .get(&link.origin_node_id)
                .cloned()
                .expect("every link origin must have a source ID mapping");
            (
                (mapped_target, input_name.clone()),
                NormalizedLinkInput {
                    origin_node_id: mapped_origin,
                    origin_slot: link.origin_slot,
                },
            )
        })
        .collect();
    Ok(NormalizedWorkflow {
        workflow,
        api_value,
        api_bytes,
        links: mapped_links,
        output_evidence,
        source_to_api: id_mapping.source_to_api,
        api_to_source: id_mapping.api_to_source,
        compatibility: descriptors.compatibility.clone(),
    })
}

fn declared_type_name(
    value: crate::application::workflow_recognition_schema::RecognitionDeclaredType,
) -> &'static str {
    use crate::application::workflow_recognition_schema::RecognitionDeclaredType::*;
    match value {
        String => "STRING",
        Integer => "INT",
        Float => "FLOAT",
        Boolean => "BOOLEAN",
        Standard => "STANDARD",
        Image => "IMAGE",
        Mask => "MASK",
        Latent => "LATENT",
        Video => "VIDEO",
        Audio => "AUDIO",
        Conditioning => "CONDITIONING",
        Model => "MODEL",
        VideoModel => "VIDEO_MODEL",
        Enum => "COMBO",
        DynamicCombo => "COMFY_DYNAMICCOMBO_V3",
        Unknown => "UNKNOWN",
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod parser_tests {
    use super::{parse_ui_workflow_value, UiNodeMode};
    use serde_json::json;

    fn minimal_workflow() -> serde_json::Value {
        json!({
            "version": 0.4,
            "extra": {"frontendVersion": "1.52.7"},
            "nodes": [{
                "id": 1,
                "type": "Example",
                "mode": 0,
                "inputs": [{"name": "value", "type": "INT", "widget": {"name": "value"}}],
                "outputs": [{"name": "INT", "type": "INT", "links": [1]}],
                "widgets_values": [3],
                "widgets_values_named": {"value": 3},
                "properties": {"cnr_id": "comfy-core"}
            }, {
                "id": 2,
                "type": "Sink",
                "mode": 0,
                "inputs": [{"name": "value", "type": "INT", "link": 1}],
                "outputs": []
            }],
            "links": [[1, 1, 0, 2, 0, "INT"]]
        })
    }

    #[test]
    fn parses_workflow_format_version() {
        let document = parse_ui_workflow_value(&minimal_workflow()).unwrap();
        assert_eq!(document.workflow_format_version, "0.4");
    }

    #[test]
    fn parses_extra_frontend_version() {
        let document = parse_ui_workflow_value(&minimal_workflow()).unwrap();
        assert_eq!(document.frontend_version.as_deref(), Some("1.52.7"));
    }

    #[test]
    fn missing_frontend_version_is_preserved_as_unknown() {
        let mut value = minimal_workflow();
        value.as_object_mut().unwrap().remove("extra");
        let document = parse_ui_workflow_value(&value).unwrap();
        assert_eq!(document.frontend_version, None);
    }

    #[test]
    fn parses_nodes_links_widgets_and_mode() {
        let document = parse_ui_workflow_value(&minimal_workflow()).unwrap();
        assert_eq!(document.nodes.len(), 2);
        assert_eq!(document.links[0].target_slot, 0);
        assert_eq!(
            document.nodes[0].widgets_values_named.as_ref().unwrap()["value"],
            json!(3)
        );
        assert_eq!(document.nodes[0].mode, UiNodeMode::Always);
    }

    #[test]
    fn unknown_node_mode_is_not_silently_treated_as_active() {
        let mut value = minimal_workflow();
        value["nodes"][0]["mode"] = json!(99);
        let document = parse_ui_workflow_value(&value).unwrap();
        assert_eq!(document.nodes[0].mode, UiNodeMode::Unknown(99));
    }

    #[test]
    fn malformed_widget_values_fail() {
        let mut value = minimal_workflow();
        value["nodes"][0]["widgets_values"] = json!("not-an-array");
        let error = parse_ui_workflow_value(&value).unwrap_err();
        assert_eq!(error.code, "UI_WIDGETS_VALUES_INVALID");
    }

    #[test]
    fn duplicate_node_ids_fail() {
        let mut value = minimal_workflow();
        value["nodes"][1]["id"] = json!(1);
        let error = parse_ui_workflow_value(&value).unwrap_err();
        assert_eq!(error.code, "UI_DUPLICATE_NODE_ID");
    }

    #[test]
    fn malformed_link_tuple_fails() {
        let mut value = minimal_workflow();
        value["links"][0] = json!([1, 1, 0]);
        let error = parse_ui_workflow_value(&value).unwrap_err();
        assert_eq!(error.code, "UI_LINK_TUPLE_INVALID");
    }
}
