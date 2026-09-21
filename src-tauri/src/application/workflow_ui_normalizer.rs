use crate::application::workflow_ui_serialization::{
    NormalizationCompatibilityContext, SerializerKind, UiInputSerializationDescriptor,
    UiSerializationDescriptorSet,
};
use crate::compiler::WorkflowValidator;
use crate::domain::WorkflowDocument;
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

/// The small subset of a ComfyUI workflow document that is relevant to
/// normalization.  Layout/editor metadata is deliberately not represented.
#[derive(Clone, Debug, PartialEq)]
pub struct UiWorkflowDocument {
    pub workflow_format_version: String,
    pub frontend_version: Option<String>,
    pub nodes: Vec<UiNode>,
    pub links: Vec<UiLink>,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiOutputSocket {
    pub name: String,
    pub declared_type: Option<String>,
    pub link_ids: Vec<i64>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationError {
    pub code: &'static str,
    pub message: String,
}

impl NormalizationError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
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
    pub source_to_api: BTreeMap<String, String>,
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
        let inputs = parse_input_sockets(node.get("inputs"), &id)?;
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
        if tuple.len() < 5 || tuple.len() > 6 {
            return Err(UiParseError::new(
                "UI_LINK_TUPLE_INVALID",
                format!("link at index {index} must have five or six fields"),
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

    Ok(UiWorkflowDocument {
        workflow_format_version,
        frontend_version,
        nodes: parsed_nodes,
        links: parsed_links,
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
                .ok_or_else(|| {
                    UiParseError::new(
                        "UI_NODE_INPUT_NAME_MISSING",
                        format!("node {node_id} input {index} is missing name"),
                    )
                })?
                .to_owned();
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
    let nodes = active_node_map(document)?;
    let aliases = collect_virtual_aliases(document, &nodes)?;
    let mut links_by_id = HashMap::new();
    let mut normalized = BTreeMap::new();
    let mut seen_targets = HashSet::new();

    for link in &document.links {
        let Some(origin_raw) = nodes.get(&link.origin_node_id) else {
            continue;
        };
        let Some(target) = nodes.get(&link.target_node_id) else {
            continue;
        };
        if virtual_node_kind(target).is_some() {
            links_by_id.insert(link.id, link);
            continue;
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
        if virtual_node_kind(origin_raw) == Some(VirtualNodeKind::UiOnly) {
            links_by_id.insert(link.id, link);
            continue;
        }
        let (origin, origin_slot) = resolve_virtual_origin(
            origin_raw,
            link.origin_slot,
            &aliases,
            &nodes,
            &mut HashSet::new(),
        )?;
        if descriptors.node(&origin.class_type).is_none() {
            return Err(NormalizationError::new(
                "UNKNOWN_NODE_CLASS",
                format!(
                    "link {} resolves to class {} absent from object_info",
                    link.id, origin.class_type
                ),
            ));
        }
        if origin_slot >= origin.outputs.len() {
            return Err(NormalizationError::new(
                "LINK_ORIGIN_SLOT_INVALID",
                format!("link {} origin slot {} is absent", link.id, origin_slot),
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
        if target_socket.link_id != Some(link.id) {
            return Err(NormalizationError::new(
                "LINK_ID_MISMATCH",
                format!(
                    "link {} disagrees with node {} input {} link {:?}",
                    link.id, target.id, target_socket.name, target_socket.link_id
                ),
            ));
        }
        if !seen_targets.insert((target.id.clone(), target_socket.name.clone())) {
            return Err(NormalizationError::new(
                "DUPLICATE_TARGET_INPUT_LINK",
                format!(
                    "node {} input {} has multiple links",
                    target.id, target_socket.name
                ),
            ));
        }
        if let Some(existing) = links_by_id.insert(link.id, link) {
            return Err(NormalizationError::new(
                "DUPLICATE_LINK_ID",
                format!("link id {} is declared by more than one tuple", existing.id),
            ));
        }
        let source_type = origin
            .outputs
            .get(origin_slot)
            .and_then(|output| output.declared_type.as_deref());
        let target_type = target_socket.declared_type.as_deref();
        if !compatible_socket_types(source_type, target_type)
            || !compatible_socket_types(source_type, link.declared_type.as_deref())
            || !compatible_socket_types(target_type, link.declared_type.as_deref())
        {
            return Err(NormalizationError::new(
                "LINK_TYPE_CONTRADICTION",
                format!("link {} has contradictory socket type evidence", link.id),
            ));
        }
        if descriptors
            .input_for_node(
                &target.class_type,
                target.widgets_values_named.as_ref(),
                &target_socket.name,
            )
            .is_none()
        {
            return Err(NormalizationError::new(
                "LINK_INPUT_NOT_IN_SCHEMA",
                format!(
                    "node {} input {} is not in object_info",
                    target.id, target_socket.name
                ),
            ));
        }
        if virtual_node_kind(origin_raw).is_none() {
            if let Some(output) = origin.outputs.get(origin_slot) {
                if !output.link_ids.is_empty() && !output.link_ids.contains(&link.id) {
                    return Err(NormalizationError::new(
                        "LINK_ORIGIN_EVIDENCE_MISMATCH",
                        format!("link {} is absent from origin output evidence", link.id),
                    ));
                }
            }
        }
        normalized.insert(
            (target.id.clone(), target_socket.name.clone()),
            NormalizedLinkInput {
                origin_node_id: origin.id.clone(),
                origin_slot,
            },
        );
    }

    for node in nodes.values() {
        if descriptors.node(&node.class_type).is_none() {
            continue;
        }
        for input in &node.inputs {
            if let Some(link_id) = input.link_id {
                if !links_by_id.contains_key(&link_id) {
                    let inactive_origin = document.links.iter().any(|link| {
                        link.id == link_id && !nodes.contains_key(&link.origin_node_id)
                    });
                    if inactive_origin {
                        // ComfyUI can retain a stale socket link to a
                        // bypassed/never-executed source in the UI graph;
                        // the API export correctly omits that connection.
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
    Ok(normalized)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VirtualNodeKind {
    Alias,
    UiOnly,
}

fn virtual_node_kind(node: &UiNode) -> Option<VirtualNodeKind> {
    if matches!(node.class_type.as_str(), "SetNode" | "GetNode")
        && node
            .properties
            .get("aux_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("KJNodes"))
    {
        return Some(VirtualNodeKind::Alias);
    }
    if matches!(
        node.class_type.as_str(),
        "Label (rgthree)" | "MarkdownNote" | "Fast Groups Bypasser (rgthree)"
    ) {
        return Some(VirtualNodeKind::UiOnly);
    }
    None
}

fn virtual_variable(node: &UiNode) -> Option<String> {
    node.properties
        .get("previousName")
        .and_then(Value::as_str)
        .or_else(|| {
            node.widgets_values_named
                .as_ref()
                .and_then(|values| values.get("Constant"))
                .and_then(Value::as_str)
        })
        .or_else(|| node.widgets_values.first().and_then(Value::as_str))
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
}

fn collect_virtual_aliases<'a>(
    document: &'a UiWorkflowDocument,
    nodes: &BTreeMap<String, &'a UiNode>,
) -> Result<HashMap<String, NormalizedLinkInput>, NormalizationError> {
    let mut aliases = HashMap::new();
    for link in &document.links {
        let Some(target) = nodes.get(&link.target_node_id) else {
            continue;
        };
        if target.class_type != "SetNode"
            || virtual_node_kind(target) != Some(VirtualNodeKind::Alias)
        {
            continue;
        }
        let variable = virtual_variable(target).ok_or_else(|| {
            NormalizationError::new(
                "VIRTUAL_ALIAS_NAME_MISSING",
                format!("SetNode {} has no stable variable name", target.id),
            )
        })?;
        if aliases
            .insert(
                variable.clone(),
                NormalizedLinkInput {
                    origin_node_id: link.origin_node_id.clone(),
                    origin_slot: link.origin_slot,
                },
            )
            .is_some()
        {
            return Err(NormalizationError::new(
                "VIRTUAL_ALIAS_DUPLICATE",
                format!("virtual variable {variable} is assigned more than once"),
            ));
        }
    }
    Ok(aliases)
}

fn resolve_virtual_origin<'a>(
    origin: &'a UiNode,
    origin_slot: usize,
    aliases: &HashMap<String, NormalizedLinkInput>,
    nodes: &BTreeMap<String, &'a UiNode>,
    seen: &mut HashSet<String>,
) -> Result<(&'a UiNode, usize), NormalizationError> {
    if virtual_node_kind(origin) != Some(VirtualNodeKind::Alias) {
        return Ok((origin, origin_slot));
    }
    if !seen.insert(origin.id.clone()) {
        return Err(NormalizationError::new(
            "VIRTUAL_ALIAS_CYCLE",
            format!("virtual alias cycle reaches node {}", origin.id),
        ));
    }
    let variable = virtual_variable(origin).ok_or_else(|| {
        NormalizationError::new(
            "VIRTUAL_ALIAS_NAME_MISSING",
            format!("virtual node {} has no stable variable name", origin.id),
        )
    })?;
    let link = aliases.get(&variable).ok_or_else(|| {
        NormalizationError::new(
            "VIRTUAL_ALIAS_SOURCE_MISSING",
            format!("virtual variable {variable} has no source link"),
        )
    })?;
    let next = nodes.get(&link.origin_node_id).ok_or_else(|| {
        NormalizationError::new(
            "LINK_ORIGIN_MISSING",
            format!(
                "virtual variable {variable} points to missing node {}",
                link.origin_node_id
            ),
        )
    })?;
    resolve_virtual_origin(next, link.origin_slot, aliases, nodes, seen)
}

fn active_node_map<'a>(
    document: &'a UiWorkflowDocument,
) -> Result<BTreeMap<String, &'a UiNode>, NormalizationError> {
    let mut nodes = BTreeMap::new();
    for node in &document.nodes {
        match node.mode {
            UiNodeMode::Always => {
                nodes.insert(node.id.clone(), node);
            }
            UiNodeMode::Bypass | UiNodeMode::Never => {}
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
    let mut named_keys = HashSet::new();
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
            named_keys.insert(name.clone());
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

    for input in &node.inputs {
        if input.link_id.is_none() {
            if let Some(widget_name) = &input.widget_name {
                if descriptors
                    .input_for_node(
                        &node.class_type,
                        node.widgets_values_named.as_ref(),
                        widget_name,
                    )
                    .is_none()
                {
                    return Err(NormalizationError::new(
                        "WIDGET_INPUT_NOT_IN_SCHEMA",
                        format!(
                            "node {} widget {} is absent from object_info",
                            node.id, widget_name
                        ),
                    ));
                }
            }
        }
    }

    let positional_names = positional_widget_names(node, descriptor, descriptors, links)?;
    if !node.widgets_values.is_empty() {
        if positional_names.len() == node.widgets_values.len() {
            for (name, value) in positional_names.iter().zip(&node.widgets_values) {
                let Some(input) = descriptors.input_for_node(
                    &node.class_type,
                    node.widgets_values_named.as_ref(),
                    name,
                ) else {
                    continue;
                };
                if input.serializer_kind == SerializerKind::WorkflowOnlyControl {
                    continue;
                }
                if let Some(named_value) = node
                    .widgets_values_named
                    .as_ref()
                    .and_then(|values| values.get(name))
                {
                    if named_value != value {
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
                if !inputs.contains_key(name) {
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
        } else if node.widgets_values_named.is_none() {
            return Err(NormalizationError::new(
                "WIDGET_CURSOR_MISMATCH",
                format!(
                    "node {} has {} positional values but {} verified widget descriptors",
                    node.id,
                    node.widgets_values.len(),
                    positional_names.len()
                ),
            ));
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
    descriptor: &crate::application::workflow_ui_serialization::UiNodeSerializationDescriptor,
    descriptors: &UiSerializationDescriptorSet,
    links: &NormalizedLinkMap,
) -> Result<Vec<String>, NormalizationError> {
    let mut names = Vec::new();
    for name in &descriptor.ordered_inputs {
        let Some(input) = descriptors.input(&node.class_type, name) else {
            continue;
        };
        if input.hidden || input.serializer_kind == SerializerKind::WorkflowOnlyControl {
            continue;
        }
        let linked = links.contains_key(&(node.id.clone(), name.clone()));
        let has_widget_socket = node
            .inputs
            .iter()
            .any(|socket| socket.name == *name && socket.widget_name.is_some());
        if !linked && !has_widget_socket {
            // Optional non-widget sockets are intentionally absent from the
            // API graph when they are not connected. Required sockets are
            // reported by the completeness check below.
            continue;
        }
        if linked && !has_widget_socket && input.serializer_kind != SerializerKind::StandardSeed {
            continue;
        }
        if input.dynamic {
            // Expanded dynamic inputs must be named or linked; there is no
            // safe positional cursor for a runtime-created branch.
            continue;
        }
        if !input.serializer_kind.is_standard() {
            if linked {
                continue;
            }
            return Err(NormalizationError::new(
                "UNSUPPORTED_SERIALIZER",
                format!("node {} input {} has no verified serializer", node.id, name),
            ));
        }
        names.push(name.clone());
    }
    // An input socket with a widget name can represent a dynamic branch that
    // is not present in input_order. It is safe only when the descriptor
    // resolves it explicitly.
    for socket in &node.inputs {
        let Some(name) = &socket.widget_name else {
            continue;
        };
        if !names.contains(name) && descriptors.input(&node.class_type, name).is_some() {
            names.push(name.clone());
        }
    }
    Ok(names)
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
            return Err("declared input type is unknown".to_owned());
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
    let links = normalize_links(document, descriptors)?;
    let nodes = active_node_map(document)?;
    let mut api_root = Map::new();
    let mut source_to_api = BTreeMap::new();
    let mut output_evidence = Vec::new();

    for node in document
        .nodes
        .iter()
        .filter(|node| nodes.contains_key(&node.id))
    {
        let Some(descriptor) = descriptors.node(&node.class_type) else {
            if virtual_node_kind(node).is_some() {
                continue;
            }
            return Err(NormalizationError::new(
                "UNKNOWN_NODE_CLASS",
                format!(
                    "node {} class {} is absent from object_info",
                    node.id, node.class_type
                ),
            ));
        };
        let mut inputs = normalize_widgets(node, descriptors, &links)?;
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
                    Value::String(link.origin_node_id.clone()),
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
        api_root.insert(node.id.clone(), Value::Object(api_node));
        source_to_api.insert(node.id.clone(), node.id.clone());

        for (slot, declared_type) in descriptor.output_types.iter().enumerate() {
            output_evidence.push(NormalizedOutputEvidence {
                node_id: node.id.clone(),
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
    Ok(NormalizedWorkflow {
        workflow,
        api_value,
        api_bytes,
        links,
        output_evidence,
        source_to_api,
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
