use crate::application::{
    workflow_recognition_schema::{RecognitionDeclaredType, RecognitionSchemaContext},
    workflow_ui_serialization::{FrontendSerializationContract, SUPPORTED_FRONTEND_VERSION},
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// Structural evidence extracted from a UI-workflow export.  This is
/// deliberately about the serialization contract, not the workflow's
/// provider, model, or intended use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoricalUiSerializationFingerprint {
    pub workflow_format_version: Option<String>,
    pub frontend_version: Option<String>,
    pub frontend_provenance_present: bool,
    pub node_id_representations: BTreeSet<NodeIdRepresentation>,
    pub link_tuple_shapes: BTreeSet<LinkTupleShape>,
    pub positional_widgets_present: bool,
    pub named_widgets_present: bool,
    pub explicit_widget_backed_input_slots: bool,
    pub converted_widget_evidence: bool,
    pub dynamic_input_evidence: bool,
    pub conditional_input_evidence: bool,
    pub frontend_control_serialization_evidence: bool,
    pub presentation_virtual_node_evidence: bool,
    pub subgraph_representation: SubgraphRepresentation,
    pub subgraph_boundary_representation: SubgraphBoundaryRepresentation,
    pub proxy_widgets_representation: bool,
    pub object_info_schema_fingerprint: String,
    pub schema_lineage: SchemaLineageEvidence,
    pub positional_widget_cursor_gap: bool,
    pub trailing_null_widget_evidence: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeIdRepresentation {
    Numeric,
    StringNumeric,
    Uuid,
    Opaque,
}

impl NodeIdRepresentation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Numeric => "numeric",
            Self::StringNumeric => "string_numeric",
            Self::Uuid => "uuid",
            Self::Opaque => "opaque",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkTupleShape {
    Tuple5,
    Tuple6,
    Object,
    Unknown,
}

impl LinkTupleShape {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tuple5 => "tuple_5",
            Self::Tuple6 => "tuple_6",
            Self::Object => "object",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubgraphRepresentation {
    None,
    DefinitionsSubgraphs,
}

impl Default for SubgraphRepresentation {
    fn default() -> Self {
        Self::None
    }
}

impl SubgraphRepresentation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DefinitionsSubgraphs => "definitions_subgraphs",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubgraphBoundaryRepresentation {
    None,
    ExplicitInputOutputNodes,
    IncompleteInputOutputNodes,
}

impl Default for SubgraphBoundaryRepresentation {
    fn default() -> Self {
        Self::None
    }
}

impl SubgraphBoundaryRepresentation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ExplicitInputOutputNodes => "explicit_input_output_nodes",
            Self::IncompleteInputOutputNodes => "incomplete_input_output_nodes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaLineageStatus {
    Compatible,
    Mismatch,
    Drift,
    Insufficient,
}

impl SchemaLineageStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::Mismatch => "mismatch",
            Self::Drift => "drift",
            Self::Insufficient => "insufficient",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaLineageEvidence {
    pub status: SchemaLineageStatus,
    pub missing_node_classes: Vec<String>,
    pub missing_input_slots: Vec<String>,
    pub dynamic_input_slots: Vec<String>,
    pub drifted_input_slots: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaSnapshotProvenance {
    Paired,
    LockedManifestMatch,
    RecoveryUnpaired,
    Unknown,
}

impl SchemaSnapshotProvenance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Paired => "PAIRED",
            Self::LockedManifestMatch => "LOCKED_MANIFEST_MATCH",
            Self::RecoveryUnpaired => "RECOVERY_UNPAIRED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCompatibilityProvenanceConfidence {
    VerifiedExactVersion,
    VerifiedVersionRange,
    StructurallyVerified,
    UnknownVersionStructurallyVerified,
    ProvenanceInsufficient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerializationEvidenceStatus {
    ResolvedCurrent,
    ResolvedHistorical,
    Partial,
    Unresolved,
    Conflict,
}

impl SerializationEvidenceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResolvedCurrent => "RESOLVED_CURRENT",
            Self::ResolvedHistorical => "RESOLVED_HISTORICAL",
            Self::Partial => "PARTIAL",
            Self::Unresolved => "UNRESOLVED",
            Self::Conflict => "CONFLICT",
        }
    }
}

impl UiCompatibilityProvenanceConfidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VerifiedExactVersion => "VerifiedExactVersion",
            Self::VerifiedVersionRange => "VerifiedVersionRange",
            Self::StructurallyVerified => "StructurallyVerified",
            Self::UnknownVersionStructurallyVerified => "UnknownVersionStructurallyVerified",
            Self::ProvenanceInsufficient => "ProvenanceInsufficient",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCompatibilityImplementationStatus {
    Implemented,
    DetectedButUnsupported,
    StructurallyVerified,
    EvidenceInsufficient,
    Unknown,
}

impl UiCompatibilityImplementationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Implemented => "Implemented",
            Self::DetectedButUnsupported => "DetectedButUnsupported",
            Self::StructurallyVerified => "StructurallyVerified",
            Self::EvidenceInsufficient => "EvidenceInsufficient",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedUiCompatibilityProfile {
    pub required_contracts: BTreeSet<FrontendSerializationContract>,
    pub serialization_evidence_status: SerializationEvidenceStatus,
    pub evidence: HistoricalUiSerializationFingerprint,
    pub provenance_confidence: UiCompatibilityProvenanceConfidence,
    pub schema_provenance: SchemaSnapshotProvenance,
    pub implementation_status: UiCompatibilityImplementationStatus,
    pub diagnostics: Vec<String>,
}

impl ResolvedUiCompatibilityProfile {
    pub fn is_implemented(&self) -> bool {
        if self.implementation_status != UiCompatibilityImplementationStatus::Implemented {
            return false;
        }
        match self.serialization_evidence_status {
            SerializationEvidenceStatus::ResolvedCurrent => self.required_contracts.is_empty(),
            SerializationEvidenceStatus::ResolvedHistorical => {
                !self.required_contracts.is_empty()
                    && self.required_contracts.iter().all(|contract| {
                        matches!(
                            contract,
                            FrontendSerializationContract::LegacyWidgetSlotV0
                                | FrontendSerializationContract::LegacyDynamicInputV0
                        )
                    })
            }
            SerializationEvidenceStatus::Partial
            | SerializationEvidenceStatus::Unresolved
            | SerializationEvidenceStatus::Conflict => false,
        }
    }

    pub fn execution_supported(&self) -> bool {
        self.is_implemented()
            && matches!(
                self.serialization_evidence_status,
                SerializationEvidenceStatus::ResolvedCurrent
                    | SerializationEvidenceStatus::ResolvedHistorical
            )
            && self.evidence.schema_lineage.status == SchemaLineageStatus::Compatible
            && self.provenance_confidence
                != UiCompatibilityProvenanceConfidence::ProvenanceInsufficient
            && match self.schema_provenance {
                SchemaSnapshotProvenance::Paired
                | SchemaSnapshotProvenance::LockedManifestMatch => true,
                SchemaSnapshotProvenance::Unknown => {
                    self.serialization_evidence_status
                        == SerializationEvidenceStatus::ResolvedCurrent
                        && self.required_contracts.is_empty()
                        && self.provenance_confidence
                            == UiCompatibilityProvenanceConfidence::VerifiedExactVersion
                }
                SchemaSnapshotProvenance::RecoveryUnpaired => false,
            }
    }

    pub fn primary_diagnostic(&self) -> Option<&str> {
        self.diagnostics.first().map(String::as_str)
    }

    pub fn contracts_for_normalization(&self) -> Option<BTreeSet<FrontendSerializationContract>> {
        match self.serialization_evidence_status {
            SerializationEvidenceStatus::ResolvedCurrent if self.required_contracts.is_empty() => {
                Some(BTreeSet::from([FrontendSerializationContract::Current]))
            }
            SerializationEvidenceStatus::ResolvedHistorical
                if !self.required_contracts.is_empty() =>
            {
                Some(self.required_contracts.clone())
            }
            _ => None,
        }
    }
}

pub struct UiCompatibilityResolutionInput<'a> {
    pub workflow_format_version: &'a str,
    pub frontend_version: Option<&'a str>,
    pub fingerprint: &'a HistoricalUiSerializationFingerprint,
    pub schema_provenance: SchemaSnapshotProvenance,
}

/// The only historical serialization profile selection authority.
pub struct UiCompatibilityProfileResolver;

impl UiCompatibilityProfileResolver {
    pub fn resolve(input: UiCompatibilityResolutionInput<'_>) -> ResolvedUiCompatibilityProfile {
        let fingerprint = input.fingerprint;
        let version = input
            .frontend_version
            .filter(|value| !value.trim().is_empty());
        let exact_current_version = version == Some(SUPPORTED_FRONTEND_VERSION);
        let mut required_contracts = BTreeSet::new();
        if fingerprint.has_legacy_subgraph_boundary_evidence() {
            required_contracts.insert(FrontendSerializationContract::LegacySubgraphBoundaryProxyV0);
        }
        if fingerprint.has_legacy_dynamic_input_evidence() {
            required_contracts.insert(FrontendSerializationContract::LegacyDynamicInputV0);
        }
        if fingerprint.has_legacy_widget_slot_evidence() {
            required_contracts.insert(FrontendSerializationContract::LegacyWidgetSlotV0);
        }

        let confidence = if input.workflow_format_version.trim().is_empty() {
            UiCompatibilityProvenanceConfidence::ProvenanceInsufficient
        } else {
            confidence_for(version)
        };
        let has_historical_contract = !required_contracts.is_empty();
        let serialization_evidence_status = if has_historical_contract && exact_current_version {
            SerializationEvidenceStatus::Conflict
        } else if !exact_current_version
            && fingerprint.dynamic_input_evidence
            && !fingerprint.has_legacy_dynamic_input_evidence()
        {
            SerializationEvidenceStatus::Unresolved
        } else if !exact_current_version && fingerprint.trailing_null_widget_evidence {
            SerializationEvidenceStatus::Partial
        } else if has_historical_contract {
            SerializationEvidenceStatus::ResolvedHistorical
        } else if exact_current_version && fingerprint.is_current_contract_compatible() {
            SerializationEvidenceStatus::ResolvedCurrent
        } else {
            SerializationEvidenceStatus::Unresolved
        };
        if has_historical_contract && exact_current_version {
            return resolved(
                fingerprint,
                required_contracts,
                serialization_evidence_status,
                confidence,
                input.schema_provenance,
                UiCompatibilityImplementationStatus::EvidenceInsufficient,
                "provenance_fingerprint_conflict",
            );
        }

        if !exact_current_version
            && fingerprint.dynamic_input_evidence
            && !fingerprint.has_legacy_dynamic_input_evidence()
        {
            return resolved(
                fingerprint,
                required_contracts,
                serialization_evidence_status,
                confidence,
                input.schema_provenance,
                UiCompatibilityImplementationStatus::EvidenceInsufficient,
                "dynamic_serialization_evidence_unresolved",
            );
        }

        if serialization_evidence_status == SerializationEvidenceStatus::Partial {
            return resolved(
                fingerprint,
                required_contracts,
                serialization_evidence_status,
                confidence,
                input.schema_provenance,
                UiCompatibilityImplementationStatus::EvidenceInsufficient,
                "serialization_evidence_partial",
            );
        }

        if fingerprint.schema_lineage.status != SchemaLineageStatus::Compatible {
            let diagnostic = match fingerprint.schema_lineage.status {
                SchemaLineageStatus::Drift => "object_info_schema_lineage_drift",
                SchemaLineageStatus::Mismatch => "object_info_schema_lineage_mismatch",
                SchemaLineageStatus::Insufficient => "object_info_schema_lineage_insufficient",
                SchemaLineageStatus::Compatible => unreachable!("handled above"),
            };
            return resolved(
                fingerprint,
                required_contracts,
                serialization_evidence_status,
                confidence,
                input.schema_provenance,
                UiCompatibilityImplementationStatus::EvidenceInsufficient,
                diagnostic,
            );
        }

        if !fingerprint.is_structurally_known() {
            return resolved(
                fingerprint,
                required_contracts,
                serialization_evidence_status,
                UiCompatibilityProvenanceConfidence::ProvenanceInsufficient,
                input.schema_provenance,
                UiCompatibilityImplementationStatus::Unknown,
                "historical_serialization_fingerprint_unknown",
            );
        }

        if !has_historical_contract {
            if serialization_evidence_status == SerializationEvidenceStatus::ResolvedCurrent {
                return ResolvedUiCompatibilityProfile {
                    required_contracts,
                    serialization_evidence_status,
                    evidence: fingerprint.clone(),
                    provenance_confidence: confidence,
                    schema_provenance: input.schema_provenance,
                    implementation_status: UiCompatibilityImplementationStatus::Implemented,
                    diagnostics: Vec::new(),
                };
            }
            if version.is_none() && fingerprint.is_current_contract_compatible() {
                return resolved(
                    fingerprint,
                    required_contracts,
                    SerializationEvidenceStatus::Unresolved,
                    UiCompatibilityProvenanceConfidence::UnknownVersionStructurallyVerified,
                    input.schema_provenance,
                    UiCompatibilityImplementationStatus::StructurallyVerified,
                    "frontend_provenance_insufficient",
                );
            }
            return resolved(
                fingerprint,
                required_contracts,
                serialization_evidence_status,
                confidence,
                input.schema_provenance,
                UiCompatibilityImplementationStatus::Unknown,
                "frontend_provenance_insufficient",
            );
        }

        let provenance_acceptable = match input.schema_provenance {
            SchemaSnapshotProvenance::Paired | SchemaSnapshotProvenance::LockedManifestMatch => {
                true
            }
            SchemaSnapshotProvenance::RecoveryUnpaired => false,
            SchemaSnapshotProvenance::Unknown => false,
        };
        if !provenance_acceptable {
            return resolved(
                fingerprint,
                required_contracts,
                serialization_evidence_status,
                confidence,
                input.schema_provenance,
                UiCompatibilityImplementationStatus::EvidenceInsufficient,
                "schema_snapshot_provenance_insufficient",
            );
        }

        let missing_implementation = required_contracts.iter().any(|contract| {
            !matches!(
                contract,
                FrontendSerializationContract::LegacyWidgetSlotV0
                    | FrontendSerializationContract::LegacyDynamicInputV0
            )
        });
        resolved(
            fingerprint,
            required_contracts,
            serialization_evidence_status,
            confidence,
            input.schema_provenance,
            if missing_implementation {
                UiCompatibilityImplementationStatus::DetectedButUnsupported
            } else {
                UiCompatibilityImplementationStatus::Implemented
            },
            if missing_implementation {
                "historical_profile_detected_but_not_implemented"
            } else {
                "historical_contracts_implemented"
            },
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoricalFingerprintError {
    pub code: &'static str,
    pub message: String,
}

impl HistoricalFingerprintError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for HistoricalFingerprintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for HistoricalFingerprintError {}

impl HistoricalUiSerializationFingerprint {
    pub fn from_source_value(
        source: &Value,
        schema_fingerprint: impl Into<String>,
        schema: &RecognitionSchemaContext,
    ) -> Result<Self, HistoricalFingerprintError> {
        let root = source.as_object().ok_or_else(|| {
            HistoricalFingerprintError::new("UI_ROOT_INVALID", "workflow root must be an object")
        })?;
        let mut evidence = RawSerializationEvidence::default();
        collect_graph_evidence(root, &mut evidence);
        let definitions = root
            .get("definitions")
            .and_then(Value::as_object)
            .and_then(|definitions| definitions.get("subgraphs"))
            .and_then(Value::as_array);
        if let Some(definitions) = definitions.filter(|values| !values.is_empty()) {
            evidence.subgraph_representation = SubgraphRepresentation::DefinitionsSubgraphs;
            let mut complete = true;
            for definition in definitions {
                let Some(definition) = definition.as_object() else {
                    complete = false;
                    continue;
                };
                if let Some(id) = definition.get("id").and_then(Value::as_str) {
                    evidence.subgraph_definition_ids.insert(id.to_owned());
                }
                complete &=
                    definition.get("inputNode").is_some() && definition.get("outputNode").is_some();
                collect_nested_definition_evidence(definition, &mut evidence);
            }
            evidence.subgraph_boundary_representation = if complete {
                SubgraphBoundaryRepresentation::ExplicitInputOutputNodes
            } else {
                SubgraphBoundaryRepresentation::IncompleteInputOutputNodes
            };
        }

        let current_control_evidence =
            evidence.named_widgets_present || evidence.frontend_control_serialization_evidence;
        let schema_lineage = schema_lineage(
            &evidence.nodes,
            schema,
            &evidence.subgraph_definition_ids,
            current_control_evidence,
        );
        Ok(Self {
            workflow_format_version: root
                .get("version")
                .and_then(Value::as_f64)
                .map(format_number),
            frontend_version: root
                .get("extra")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("frontendVersion"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .filter(|value| !value.trim().is_empty()),
            frontend_provenance_present: root
                .get("extra")
                .and_then(Value::as_object)
                .and_then(|extra| extra.get("frontendVersion"))
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty()),
            node_id_representations: evidence.node_id_representations,
            link_tuple_shapes: evidence.link_tuple_shapes,
            positional_widgets_present: evidence.positional_widgets_present,
            named_widgets_present: evidence.named_widgets_present,
            explicit_widget_backed_input_slots: evidence.explicit_widget_backed_input_slots,
            converted_widget_evidence: evidence.converted_widget_evidence,
            dynamic_input_evidence: evidence.dynamic_input_evidence,
            conditional_input_evidence: evidence.conditional_input_evidence,
            frontend_control_serialization_evidence: evidence
                .frontend_control_serialization_evidence,
            presentation_virtual_node_evidence: evidence.presentation_virtual_node_evidence,
            subgraph_representation: evidence.subgraph_representation,
            subgraph_boundary_representation: evidence.subgraph_boundary_representation,
            proxy_widgets_representation: evidence.proxy_widgets_representation,
            object_info_schema_fingerprint: schema_fingerprint.into(),
            schema_lineage,
            positional_widget_cursor_gap: evidence.positional_widget_cursor_gap,
            trailing_null_widget_evidence: evidence.trailing_null_widget_evidence,
        })
    }

    fn has_legacy_widget_slot_evidence(&self) -> bool {
        self.positional_widgets_present
            && !self.named_widgets_present
            && self.positional_widget_cursor_gap
    }

    fn has_legacy_dynamic_input_evidence(&self) -> bool {
        self.dynamic_input_evidence && !self.schema_lineage.dynamic_input_slots.is_empty()
    }

    fn has_legacy_subgraph_boundary_evidence(&self) -> bool {
        self.subgraph_representation == SubgraphRepresentation::DefinitionsSubgraphs
            && self.proxy_widgets_representation
    }

    fn is_current_contract_compatible(&self) -> bool {
        self.schema_lineage.status == SchemaLineageStatus::Compatible
            && !self.positional_widget_cursor_gap
            && !self.has_legacy_dynamic_input_evidence()
            && !self.has_legacy_subgraph_boundary_evidence()
            && !self.link_tuple_shapes.contains(&LinkTupleShape::Unknown)
    }

    fn is_structurally_known(&self) -> bool {
        !self.link_tuple_shapes.contains(&LinkTupleShape::Unknown)
            && self.schema_lineage.status != SchemaLineageStatus::Insufficient
    }
}

impl HistoricalUiSerializationFingerprint {
    pub fn has_historical_profile_evidence(&self) -> bool {
        self.has_legacy_subgraph_boundary_evidence()
            || self.has_legacy_dynamic_input_evidence()
            || self.has_legacy_widget_slot_evidence()
    }
}

#[derive(Default)]
struct RawSerializationEvidence {
    node_id_representations: BTreeSet<NodeIdRepresentation>,
    link_tuple_shapes: BTreeSet<LinkTupleShape>,
    positional_widgets_present: bool,
    named_widgets_present: bool,
    explicit_widget_backed_input_slots: bool,
    converted_widget_evidence: bool,
    dynamic_input_evidence: bool,
    conditional_input_evidence: bool,
    frontend_control_serialization_evidence: bool,
    presentation_virtual_node_evidence: bool,
    subgraph_representation: SubgraphRepresentation,
    subgraph_boundary_representation: SubgraphBoundaryRepresentation,
    proxy_widgets_representation: bool,
    positional_widget_cursor_gap: bool,
    trailing_null_widget_evidence: bool,
    subgraph_definition_ids: BTreeSet<String>,
    nodes: Vec<RawNodeEvidence>,
}

#[derive(Clone, Debug)]
struct RawNodeEvidence {
    class_type: String,
    source_revision: Option<String>,
    ui_only: bool,
    positional_widgets_present: bool,
    named_widgets_present: bool,
    positional_widget_values: Vec<Value>,
    inputs: Vec<RawInputEvidence>,
}

#[derive(Clone, Debug)]
struct RawInputEvidence {
    name: String,
    raw_type: Option<String>,
    linked: bool,
}

fn collect_graph_evidence(root: &Map<String, Value>, evidence: &mut RawSerializationEvidence) {
    if let Some(nodes) = root.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            let Some(node) = node.as_object() else {
                continue;
            };
            collect_node_evidence(node, evidence);
        }
    }
    if let Some(links) = root.get("links").and_then(Value::as_array) {
        for link in links {
            let shape = match link {
                Value::Array(values) => match values.len() {
                    5 => LinkTupleShape::Tuple5,
                    6 => LinkTupleShape::Tuple6,
                    _ => LinkTupleShape::Unknown,
                },
                Value::Object(_) => LinkTupleShape::Object,
                _ => LinkTupleShape::Unknown,
            };
            evidence.link_tuple_shapes.insert(shape);
        }
    }
}

fn collect_nested_definition_evidence(
    definition: &Map<String, Value>,
    evidence: &mut RawSerializationEvidence,
) {
    if let Some(id) = definition.get("id").and_then(Value::as_str) {
        evidence.subgraph_definition_ids.insert(id.to_owned());
    }
    collect_graph_evidence(definition, evidence);
    if let Some(nested) = definition
        .get("definitions")
        .and_then(Value::as_object)
        .and_then(|definitions| definitions.get("subgraphs"))
        .and_then(Value::as_array)
    {
        for nested_definition in nested {
            let Some(nested_definition) = nested_definition.as_object() else {
                continue;
            };
            collect_nested_definition_evidence(nested_definition, evidence);
        }
    }
}

fn collect_node_evidence(node: &Map<String, Value>, evidence: &mut RawSerializationEvidence) {
    if let Some(id) = node.get("id") {
        if let Some(representation) = node_id_representation(id) {
            evidence.node_id_representations.insert(representation);
        }
    }
    let class_type = node
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let ui_only = is_presentation_or_virtual_type(&class_type, node);
    evidence.presentation_virtual_node_evidence |= ui_only;
    evidence.proxy_widgets_representation |= node
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|properties| properties.contains_key("proxyWidgets"));

    let positional_widget_values = node
        .get("widgets_values")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let positional_count = positional_widget_values.len();
    evidence.trailing_null_widget_evidence |=
        positional_widget_values.last().is_some_and(Value::is_null);
    let node_named_widgets = node
        .get("widgets_values_named")
        .is_some_and(Value::is_object);
    let source_revision = node
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("ver"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    evidence.positional_widgets_present |= positional_count > 0;
    evidence.named_widgets_present |= node_named_widgets;

    let mut widget_input_count = 0;
    let mut node_has_control = false;
    let mut node_has_dynamic_input = false;
    let mut inputs = Vec::new();
    if let Some(raw_inputs) = node.get("inputs").and_then(Value::as_array) {
        for input in raw_inputs {
            let Some(input) = input.as_object() else {
                continue;
            };
            let name = input
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let raw_type = input.get("type").and_then(Value::as_str).map(str::to_owned);
            let linked = input.get("link").is_some_and(|value| !value.is_null());
            let widget_name = input
                .get("widget")
                .and_then(Value::as_object)
                .and_then(|widget| widget.get("name"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            if widget_name.is_some() {
                widget_input_count += 1;
                evidence.explicit_widget_backed_input_slots = true;
                node_has_control |= !linked;
            }
            let dynamic = name.contains('.')
                || raw_type
                    .as_deref()
                    .is_some_and(|value| value.to_ascii_uppercase().contains("DYNAMIC"));
            evidence.dynamic_input_evidence |= dynamic;
            node_has_dynamic_input |= dynamic;
            evidence.conditional_input_evidence |= input.contains_key("conditional")
                || input.contains_key("lazy")
                || input.contains_key("rawLink");
            evidence.converted_widget_evidence |=
                input.get("shape").and_then(Value::as_i64) == Some(7) || name.ends_with("_input");
            inputs.push(RawInputEvidence {
                name,
                raw_type,
                linked,
            });
        }
    }
    evidence.frontend_control_serialization_evidence |= node_has_control;
    evidence.positional_widget_cursor_gap |= positional_count > widget_input_count
        && !node_has_control
        && !node_named_widgets
        && !ui_only
        && !node_has_dynamic_input;
    evidence.nodes.push(RawNodeEvidence {
        class_type,
        source_revision,
        ui_only,
        positional_widgets_present: positional_count > 0,
        named_widgets_present: node_named_widgets,
        positional_widget_values,
        inputs,
    });
}

fn schema_lineage(
    nodes: &[RawNodeEvidence],
    schema: &RecognitionSchemaContext,
    subgraph_definition_ids: &BTreeSet<String>,
    current_control_evidence: bool,
) -> SchemaLineageEvidence {
    if schema.nodes.is_empty() {
        return SchemaLineageEvidence {
            status: SchemaLineageStatus::Insufficient,
            missing_node_classes: Vec::new(),
            missing_input_slots: Vec::new(),
            dynamic_input_slots: Vec::new(),
            drifted_input_slots: Vec::new(),
        };
    }
    let mut missing_node_classes = BTreeSet::new();
    let mut missing_input_slots = BTreeSet::new();
    let mut dynamic_input_slots = BTreeSet::new();
    let mut drifted_input_slots = BTreeSet::new();
    for node in nodes {
        if node.ui_only || subgraph_definition_ids.contains(&node.class_type) {
            continue;
        }
        let Some(node_schema) = schema.nodes.get(&node.class_type) else {
            missing_node_classes.insert(node.class_type.clone());
            continue;
        };
        if node.source_revision.as_deref().is_some_and(is_git_revision) {
            let unrepresented_inputs = node_schema
                .ordered_inputs
                .iter()
                .filter(|name| !node.inputs.iter().any(|input| input.name == **name))
                .collect::<Vec<_>>();
            if let Some((input_name, input_schema)) = node
                .positional_widget_values
                .len()
                .checked_sub(1)
                .and_then(|index| unrepresented_inputs.get(index))
                .and_then(|name| {
                    node_schema
                        .inputs
                        .get(name.as_str())
                        .map(|input| (*name, input))
                })
            {
                let trailing_value = node.positional_widget_values.last();
                let default_is_declared = input_schema
                    .default_value
                    .as_ref()
                    .is_some_and(|value| input_schema.enum_values.contains(value));
                if !input_schema.required
                    && input_schema.declared_type == RecognitionDeclaredType::Enum
                    && default_is_declared
                    && trailing_value.is_some_and(|value| !input_schema.enum_values.contains(value))
                {
                    drifted_input_slots.insert(format!("{}:{}", node.class_type, input_name));
                }
            }
        }
        for input in &node.inputs {
            let base_name = input.name.split('.').next().unwrap_or(&input.name);
            if input.name.is_empty()
                || node_schema.inputs.contains_key(&input.name)
                || node_schema.conditional_inputs.values().any(|by_value| {
                    by_value
                        .values()
                        .any(|inputs| inputs.contains_key(&input.name))
                })
            {
                continue;
            }
            let schema_declares_dynamic_base =
                node_schema.inputs.get(base_name).is_some_and(|base| {
                    base.dynamic_prefix.is_some() || !base.dynamic_names.is_empty()
                });
            let schema_declares_conditional_base =
                node_schema.conditional_inputs.contains_key(base_name)
                    && node_schema.inputs.get(base_name).is_none_or(|base| {
                        base.declared_type != RecognitionDeclaredType::DynamicCombo
                            || current_control_evidence
                    });
            let raw_dynamic_evidence = input
                .raw_type
                .as_deref()
                .is_some_and(|value| value.to_ascii_uppercase().contains("DYNAMIC"));
            let positional_combo_socket = input.name.contains('.')
                && node.positional_widgets_present
                && !node.named_widgets_present
                && node_schema.inputs.get(base_name).is_some_and(|base| {
                    base.declared_type == RecognitionDeclaredType::DynamicCombo
                })
                && node_schema
                    .conditional_inputs
                    .get(base_name)
                    .is_some_and(|options| !options.is_empty());
            let dynamic = raw_dynamic_evidence
                || (input.name.contains('.')
                    && (schema_declares_dynamic_base
                        || schema_declares_conditional_base
                        || positional_combo_socket));
            let incomplete_combo_base = node_schema.inputs.get(base_name).is_some_and(|base| {
                base.declared_type == RecognitionDeclaredType::DynamicCombo
                    && base.dynamic_prefix.is_none()
                    && base.dynamic_names.is_empty()
            }) && !node_schema
                .conditional_inputs
                .get(base_name)
                .is_some_and(|options| !options.is_empty());
            let missing_dynamic_base = !node_schema.inputs.contains_key(base_name);
            if dynamic && incomplete_combo_base {
                missing_input_slots.insert(format!("{}:{}", node.class_type, input.name));
            } else if dynamic && missing_dynamic_base {
                let evidence = format!("{}:{}", node.class_type, input.name);
                dynamic_input_slots.insert(evidence.clone());
                missing_input_slots.insert(evidence);
            } else if dynamic && (schema_declares_dynamic_base || schema_declares_conditional_base)
            {
                continue;
            } else if dynamic {
                dynamic_input_slots.insert(format!("{}:{}", node.class_type, input.name));
            } else if input.linked {
                missing_input_slots.insert(format!("{}:{}", node.class_type, input.name));
            }
        }
    }
    let status = if !drifted_input_slots.is_empty() {
        SchemaLineageStatus::Drift
    } else if missing_node_classes.is_empty() && missing_input_slots.is_empty() {
        SchemaLineageStatus::Compatible
    } else {
        SchemaLineageStatus::Mismatch
    };
    SchemaLineageEvidence {
        status,
        missing_node_classes: missing_node_classes.into_iter().collect(),
        missing_input_slots: missing_input_slots.into_iter().collect(),
        dynamic_input_slots: dynamic_input_slots.into_iter().collect(),
        drifted_input_slots: drifted_input_slots.into_iter().collect(),
    }
}

fn is_git_revision(revision: &str) -> bool {
    revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn resolved(
    fingerprint: &HistoricalUiSerializationFingerprint,
    required_contracts: BTreeSet<FrontendSerializationContract>,
    serialization_evidence_status: SerializationEvidenceStatus,
    confidence: UiCompatibilityProvenanceConfidence,
    schema_provenance: SchemaSnapshotProvenance,
    implementation_status: UiCompatibilityImplementationStatus,
    diagnostic: &'static str,
) -> ResolvedUiCompatibilityProfile {
    ResolvedUiCompatibilityProfile {
        required_contracts,
        serialization_evidence_status,
        evidence: fingerprint.clone(),
        provenance_confidence: confidence,
        schema_provenance,
        implementation_status,
        diagnostics: vec![diagnostic.to_owned()],
    }
}

fn confidence_for(version: Option<&str>) -> UiCompatibilityProvenanceConfidence {
    if version.is_some() {
        UiCompatibilityProvenanceConfidence::VerifiedExactVersion
    } else {
        UiCompatibilityProvenanceConfidence::UnknownVersionStructurallyVerified
    }
}

fn node_id_representation(value: &Value) -> Option<NodeIdRepresentation> {
    match value {
        Value::Number(_) => Some(NodeIdRepresentation::Numeric),
        Value::String(value) if is_numeric_id(value) => Some(NodeIdRepresentation::StringNumeric),
        Value::String(value) if looks_like_uuid(value) => Some(NodeIdRepresentation::Uuid),
        Value::String(value) if !value.trim().is_empty() => Some(NodeIdRepresentation::Opaque),
        _ => None,
    }
}

fn is_numeric_id(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.split(':').all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

fn looks_like_uuid(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    parts.len() == 5
        && parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && parts
            .iter()
            .all(|part| part.chars().all(|character| character.is_ascii_hexdigit()))
}

fn is_presentation_or_virtual_type(class_type: &str, node: &Map<String, Value>) -> bool {
    let lower = class_type.to_ascii_lowercase();
    lower == "note"
        || lower.contains("markdownnote")
        || lower == "label (rgthree)"
        || lower == "fast groups bypasser (rgthree)"
        || lower == "reroute"
        || lower.starts_with("primitive")
        || lower == "setnode"
        || lower == "getnode"
        || node
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|properties| properties.contains_key("proxyWidgets"))
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(value: Value) -> RecognitionSchemaContext {
        RecognitionSchemaContext::parse(&value)
    }

    fn resolve(source: Value, schema_value: Value) -> ResolvedUiCompatibilityProfile {
        resolve_with_provenance(source, schema_value, SchemaSnapshotProvenance::Paired)
    }

    fn resolve_with_provenance(
        source: Value,
        schema_value: Value,
        schema_provenance: SchemaSnapshotProvenance,
    ) -> ResolvedUiCompatibilityProfile {
        let schema = schema(schema_value);
        let fingerprint = HistoricalUiSerializationFingerprint::from_source_value(
            &source,
            "schema-fingerprint",
            &schema,
        )
        .unwrap();
        UiCompatibilityProfileResolver::resolve(UiCompatibilityResolutionInput {
            workflow_format_version: fingerprint.workflow_format_version.as_deref().unwrap_or(""),
            frontend_version: fingerprint.frontend_version.as_deref(),
            fingerprint: &fingerprint,
            schema_provenance,
        })
    }

    fn requires(
        profile: &ResolvedUiCompatibilityProfile,
        contract: FrontendSerializationContract,
    ) -> bool {
        profile.required_contracts.contains(&contract)
    }

    fn base_node(class_type: &str) -> Value {
        json!({
            "id": 1,
            "type": class_type,
            "inputs": [],
            "outputs": [],
            "widgets_values": []
        })
    }

    fn base_schema(class_type: &str) -> Value {
        json!({class_type: {"input": {"required": {}}, "output": []}})
    }

    fn composed_workflow(dynamic_first: bool) -> Value {
        let widget = json!({
            "id": 1,
            "type": "NodeAlpha",
            "inputs": [],
            "outputs": [],
            "widgets_values": [7]
        });
        let dynamic = json!({
            "id": 2,
            "type": "NodeBeta",
            "inputs": [{"name": "select.item", "type": "COMFY_DYNAMICCOMBO_V0", "link": 1}],
            "outputs": [],
            "widgets_values": ["1"]
        });
        let nodes = if dynamic_first {
            vec![dynamic, widget]
        } else {
            vec![widget, dynamic]
        };
        json!({
            "version": 0.4,
            "extra": {"frontendVersion": "1.43.1"},
            "nodes": nodes,
            "links": [[1, 3, 0, 2, 0]]
        })
    }

    fn composed_schema() -> Value {
        json!({
            "NodeAlpha": {
                "input": {"required": {"value": ["INT", {}]}},
                "output": []
            },
            "NodeBeta": {
                "input": {"required": {"select": ["COMFY_DYNAMICCOMBO_V3", {
                    "options": [{"key": "1", "inputs": {"required": {"item": ["IMAGE", {}]}}}]
                }]}},
                "output": []
            }
        })
    }

    fn composed_profile(dynamic_first: bool) -> ResolvedUiCompatibilityProfile {
        resolve(composed_workflow(dynamic_first), composed_schema())
    }

    #[test]
    fn empty_contract_set_with_resolved_current_evidence_selects_current() {
        let mut node = base_node("Target");
        node["widgets_values"] = json!([1]);
        node["widgets_values_named"] = json!({"value": 1});
        node["inputs"] = json!([{
            "name": "value",
            "type": "INT",
            "widget": {"name": "value"}
        }]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.52.7"},
                "nodes": [node],
                "links": []
            }),
            json!({"Target": {"input": {"required": {"value": ["INT", {}]}}}}),
        );
        assert!(profile.required_contracts.is_empty());
        assert_eq!(
            profile.serialization_evidence_status,
            SerializationEvidenceStatus::ResolvedCurrent
        );
        assert!(profile.execution_supported());
        assert_eq!(
            profile.contracts_for_normalization(),
            Some(BTreeSet::from([FrontendSerializationContract::Current]))
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::Implemented
        );
    }

    #[test]
    fn legacy_positional_widget_fingerprint_resolves_widget_v0() {
        let mut node = base_node("Target");
        node["widgets_values"] = json!([1]);
        let profile = resolve(
            json!({"version": 0.4, "nodes": [node], "links": []}),
            base_schema("Target"),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyWidgetSlotV0
        ));
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::Implemented
        );
    }

    #[test]
    fn unknown_version_with_legacy_widget_fingerprint_resolves_widget_v0() {
        let mut node = base_node("Target");
        node["widgets_values"] = json!([1]);
        let profile = resolve(
            json!({"version": 0.4, "nodes": [node], "links": []}),
            base_schema("Target"),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyWidgetSlotV0
        ));
        assert_eq!(
            profile.provenance_confidence,
            UiCompatibilityProvenanceConfidence::UnknownVersionStructurallyVerified
        );
    }

    #[test]
    fn locked_manifest_schema_does_not_override_proven_schema_lineage_drift() {
        // The revision represents a verified older node implementation that predates
        // `added_option`; the locked manifest independently supplies the newer schema.
        let node = json!({
            "id": 1,
            "type": "NodeAlpha",
            "properties": {"ver": "0123456789abcdef0123456789abcdef01234567"},
            "inputs": [],
            "outputs": [],
            "widgets_values": [""]
        });
        let profile = resolve_with_provenance(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.45.15"},
                "nodes": [node],
                "links": []
            }),
            json!({
                "NodeAlpha": {
                    "input": {
                        "required": {},
                        "optional": {
                            "added_option": ["COMBO", {
                                "default": "none",
                                "options": ["none", "model"]
                            }]
                        }
                    },
                    "input_order": {"required": [], "optional": ["added_option"]},
                    "output": []
                }
            }),
            SchemaSnapshotProvenance::LockedManifestMatch,
        );

        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyWidgetSlotV0
        ));
        assert_eq!(
            profile.schema_provenance,
            SchemaSnapshotProvenance::LockedManifestMatch
        );
        assert_eq!(profile.evidence.schema_lineage.status.as_str(), "drift");
        assert!(!profile.execution_supported());
    }

    #[test]
    fn empty_contract_set_with_unresolved_serialization_evidence_fails_closed() {
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.45.15"},
                "nodes": [{
                    "id": 1,
                    "type": "DynamicNode",
                    "inputs": [{"name": "select.item", "type": "IMAGE", "link": 1}],
                    "outputs": [],
                    "widgets_values": ["1"]
                }],
                "links": [[1, 2, 0, 1, 0]]
            }),
            json!({
                "DynamicNode": {
                    "input": {"required": {"select": ["COMFY_DYNAMICCOMBO_V3", {}]}},
                    "output": []
                }
            }),
        );

        assert!(profile.required_contracts.is_empty());
        assert_eq!(
            profile.serialization_evidence_status,
            SerializationEvidenceStatus::Unresolved
        );
        assert!(!profile.execution_supported());
    }

    #[test]
    fn trailing_null_widget_evidence_stays_partial_and_fails_closed() {
        let mut node = base_node("Target");
        node["widgets_values"] = json!([true, 6, "fixed", 1, null]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.42.14"},
                "nodes": [node],
                "links": []
            }),
            base_schema("Target"),
        );

        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyWidgetSlotV0
        ));
        assert_eq!(
            profile.serialization_evidence_status,
            SerializationEvidenceStatus::Partial
        );
        assert!(!profile.execution_supported());
    }

    #[test]
    fn legacy_dynamic_fingerprint_resolves_dynamic_v0() {
        let mut node = base_node("DynamicNode");
        node["inputs"] = json!([{
            "name": "values.item",
            "type": "COMFY_DYNAMICCOMBO_V0",
            "link": 1
        }]);
        let profile = resolve(
            json!({"version": 0.4, "extra": {"frontendVersion": "1.43.1"}, "nodes": [node], "links": [[1, 2, 0, 1, 0]]}),
            base_schema("DynamicNode"),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyDynamicInputV0
        ));
    }

    #[test]
    fn current_version_with_schema_declared_dynamic_socket_resolves_current_contract() {
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.52.7"},
                "nodes": [{
                    "id": 1,
                    "type": "DynamicNode",
                    "inputs": [{"name": "items", "type": "COMFY_DYNAMICCOMBO_V3", "link": 1}],
                    "outputs": [],
                    "widgets_values": []
                }],
                "links": [[1, 2, 0, 1, 0]]
            }),
            json!({
                "DynamicNode": {
                    "input": {"required": {"items": ["COMFY_DYNAMICCOMBO_V3", {}]}},
                    "output": []
                }
            }),
        );

        assert!(profile.evidence.dynamic_input_evidence);
        assert!(profile
            .evidence
            .schema_lineage
            .dynamic_input_slots
            .is_empty());
        assert_eq!(profile.required_contracts, BTreeSet::new());
        assert_eq!(
            profile.serialization_evidence_status,
            SerializationEvidenceStatus::ResolvedCurrent
        );
        assert!(profile.execution_supported());
    }

    #[test]
    fn legacy_subgraph_fingerprint_resolves_subgraph_v0() {
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.42.14"},
                "nodes": [{
                    "id": 1,
                    "type": "subgraph",
                    "inputs": [],
                    "outputs": [],
                    "properties": {"proxyWidgets": [["2", "value"]]},
                    "widgets_values": []
                }],
                "links": [{"id": 1, "origin_id": 1, "origin_slot": 0, "target_id": 1, "target_slot": 0}],
                "definitions": {"subgraphs": [{
                    "id": "subgraph",
                    "inputNode": {"id": -10},
                    "outputNode": {"id": -20},
                    "inputs": [],
                    "outputs": [],
                    "nodes": [],
                    "links": []
                }]}
            }),
            base_schema("subgraph"),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacySubgraphBoundaryProxyV0
        ));
    }

    #[test]
    fn unknown_fingerprint_fails_closed() {
        let profile = resolve(
            json!({
                "version": 0.4,
                "nodes": [base_node("Target")],
                "links": [[1, 2, 0, 1, 0, "INT", "unexpected"]]
            }),
            base_schema("Target"),
        );
        assert!(profile.required_contracts.is_empty());
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::Unknown
        );
        assert_eq!(
            profile.primary_diagnostic(),
            Some("historical_serialization_fingerprint_unknown")
        );
    }

    #[test]
    fn schema_lineage_mismatch_is_not_misclassified_as_historical_profile() {
        let mut node = base_node("Target");
        node["inputs"] = json!([{"name": "missing", "type": "INT", "link": 1}]);
        let profile = resolve(
            json!({"version": 0.4, "nodes": [node], "links": [[1, 2, 0, 1, 0]]}),
            base_schema("Target"),
        );
        assert!(profile.required_contracts.is_empty());
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::EvidenceInsufficient
        );
        assert_eq!(
            profile.primary_diagnostic(),
            Some("object_info_schema_lineage_mismatch")
        );
    }

    #[test]
    fn historical_profile_does_not_override_schema_lineage_failure() {
        let mut node = base_node("Target");
        node["inputs"] = json!([{
            "name": "required_runtime_input",
            "type": "INT",
            "link": 1
        }]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.42.14"},
                "nodes": [
                    node,
                    {
                        "id": 2,
                        "type": "subgraph",
                        "inputs": [],
                        "outputs": [],
                        "properties": {"proxyWidgets": [["2", "value"]]},
                        "widgets_values": []
                    }
                ],
                "links": [[1, 1, 0, 1, 0]],
                "definitions": {"subgraphs": [{
                    "id": "subgraph",
                    "inputNode": {"id": -10},
                    "outputNode": {"id": -20},
                    "nodes": [],
                    "links": []
                }]}
            }),
            base_schema("Target"),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacySubgraphBoundaryProxyV0
        ));
        assert_eq!(
            profile.evidence.schema_lineage.status,
            SchemaLineageStatus::Mismatch
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::EvidenceInsufficient
        );
        assert!(!profile.execution_supported());
    }

    #[test]
    fn detected_contract_does_not_override_schema_lineage_mismatch() {
        let mut node = base_node("DynamicNode");
        node["inputs"] = json!([
            {
                "name": "values.item",
                "type": "COMFY_DYNAMICCOMBO_V0",
                "link": 1
            },
            {"name": "unrelated", "type": "INT", "link": 2}
        ]);
        let profile = resolve_with_provenance(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.43.1"},
                "nodes": [node],
                "links": [[1, 1, 0, 1, 0], [2, 1, 0, 1, 1]]
            }),
            json!({
                "DynamicNode": {
                    "input": {
                        "required": {
                            "values": ["COMFY_DYNAMICCOMBO_V3", {
                                "options": [{
                                    "key": "1",
                                    "inputs": {
                                        "required": {"item": ["IMAGE", {}]}
                                    }
                                }]
                            }]
                        }
                    },
                    "output": []
                }
            }),
            SchemaSnapshotProvenance::LockedManifestMatch,
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyDynamicInputV0
        ));
        assert_eq!(
            profile.evidence.schema_lineage.status,
            SchemaLineageStatus::Mismatch
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::EvidenceInsufficient
        );
        assert!(!profile.execution_supported());
    }

    #[test]
    fn missing_dynamic_schema_contract_is_not_classified_as_legacy_dynamic() {
        let mut node = base_node("DynamicNode");
        node["widgets_values"] = json!(["1"]);
        node["inputs"] = json!([{
            "name": "values.item",
            "type": "IMAGE",
            "link": 1
        }]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.43.1"},
                "nodes": [node],
                "links": [[1, 1, 0, 1, 0]]
            }),
            json!({
                "DynamicNode": {
                    "input": {
                        "required": {
                            "values": ["COMFY_DYNAMICCOMBO_V3", {}]
                        }
                    },
                    "output": []
                }
            }),
        );
        assert!(profile.required_contracts.is_empty());
        assert_eq!(
            profile.evidence.schema_lineage.status,
            SchemaLineageStatus::Mismatch
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::EvidenceInsufficient
        );
        assert!(!profile.execution_supported());
    }

    #[test]
    fn positional_dynamic_combo_schema_and_serialized_member_select_legacy_profile() {
        let mut node = base_node("DynamicNode");
        node["widgets_values"] = json!(["1"]);
        node["inputs"] = json!([{
            "name": "values.item",
            "type": "IMAGE",
            "link": 1
        }]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.43.1"},
                "nodes": [node],
                "links": [[1, 1, 0, 1, 0]]
            }),
            json!({
                "DynamicNode": {
                    "input": {
                        "required": {
                            "values": ["COMFY_DYNAMICCOMBO_V3", {
                                "options": [{
                                    "key": "1",
                                    "inputs": {"required": {"item": ["IMAGE", {}]}}
                                }]
                            }]
                        }
                    },
                    "output": []
                }
            }),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyDynamicInputV0
        ));
        assert_eq!(
            profile.evidence.schema_lineage.status,
            SchemaLineageStatus::Compatible
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::Implemented
        );
    }

    #[test]
    fn unknown_version_positional_dynamic_combo_resolves_from_fingerprint() {
        let mut node = base_node("DynamicNode");
        node["widgets_values"] = json!(["1"]);
        node["inputs"] = json!([{
            "name": "values.item",
            "type": "IMAGE",
            "link": 1
        }]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "nodes": [node],
                "links": [[1, 1, 0, 1, 0]]
            }),
            json!({
                "DynamicNode": {
                    "input": {
                        "required": {
                            "values": ["COMFY_DYNAMICCOMBO_V3", {
                                "options": [{
                                    "key": "1",
                                    "inputs": {"required": {"item": ["IMAGE", {}]}}
                                }]
                            }]
                        }
                    },
                    "output": []
                }
            }),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyDynamicInputV0
        ));
        assert_eq!(
            profile.provenance_confidence,
            UiCompatibilityProvenanceConfidence::UnknownVersionStructurallyVerified
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::Implemented
        );
    }

    #[test]
    fn legacy_widget_profile_does_not_override_schema_lineage_failure() {
        let mut node = base_node("Target");
        node["widgets_values"] = json!([1]);
        node["inputs"] = json!([{
            "name": "required_runtime_input",
            "type": "INT",
            "link": 1
        }]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.45.15"},
                "nodes": [node],
                "links": [[1, 1, 0, 1, 0]]
            }),
            base_schema("Target"),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyWidgetSlotV0
        ));
        assert_eq!(
            profile.evidence.schema_lineage.status,
            SchemaLineageStatus::Mismatch
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::EvidenceInsufficient
        );
        assert!(!profile.execution_supported());
    }

    #[test]
    fn version_alone_does_not_override_fingerprint() {
        let mut node = base_node("Target");
        node["widgets_values"] = json!([1]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.52.7"},
                "nodes": [node],
                "links": []
            }),
            base_schema("Target"),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyWidgetSlotV0
        ));
        assert_eq!(
            profile.primary_diagnostic(),
            Some("provenance_fingerprint_conflict")
        );
    }

    #[test]
    fn missing_version_with_current_fingerprint_is_bounded_structural_support() {
        let profile = resolve(
            json!({
                "version": 0.4,
                "nodes": [base_node("Target")],
                "links": []
            }),
            base_schema("Target"),
        );
        assert!(profile.required_contracts.is_empty());
        assert_eq!(
            profile.serialization_evidence_status,
            SerializationEvidenceStatus::Unresolved
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::StructurallyVerified
        );
    }

    #[test]
    fn multiple_historical_contracts_detected_together() {
        let profile = composed_profile(false);
        assert_eq!(
            profile.required_contracts,
            BTreeSet::from([
                FrontendSerializationContract::LegacyWidgetSlotV0,
                FrontendSerializationContract::LegacyDynamicInputV0,
            ])
        );
        assert!(profile.execution_supported());
    }

    #[test]
    fn contract_set_order_independent() {
        assert_eq!(
            composed_profile(false).required_contracts,
            composed_profile(true).required_contracts
        );
    }

    #[test]
    fn contract_set_deterministic() {
        let expected = composed_profile(false).required_contracts;
        for _ in 0..4 {
            assert_eq!(composed_profile(false).required_contracts, expected);
        }
    }

    #[test]
    fn current_contract_is_never_combined_with_historical_delta() {
        let mut widget = base_node("Target");
        widget["widgets_values"] = json!([7]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.52.7"},
                "nodes": [widget],
                "links": []
            }),
            json!({"Target": {"input": {"required": {"value": ["INT", {}]}}, "output": []}}),
        );
        assert_eq!(
            profile.required_contracts,
            BTreeSet::from([FrontendSerializationContract::LegacyWidgetSlotV0])
        );
        assert!(!profile.execution_supported());
        assert_eq!(
            profile.primary_diagnostic(),
            Some("provenance_fingerprint_conflict")
        );
    }

    #[test]
    fn partially_implemented_contract_set_fails_closed() {
        let mut widget = base_node("NodeAlpha");
        widget["widgets_values"] = json!([7]);
        let profile = resolve(
            json!({
                "version": 0.4,
                "extra": {"frontendVersion": "1.43.1"},
                "nodes": [widget, {
                    "id": 2,
                    "type": "subgraph",
                    "inputs": [],
                    "outputs": [],
                    "properties": {"proxyWidgets": [["2", "value"]]},
                    "widgets_values": []
                }],
                "links": [],
                "definitions": {"subgraphs": [{
                    "id": "subgraph", "inputNode": {"id": -10}, "outputNode": {"id": -20},
                    "nodes": [], "links": []
                }]}
            }),
            json!({"NodeAlpha": {"input": {"required": {"value": ["INT", {}]}}, "output": []}}),
        );
        assert_eq!(
            profile.required_contracts,
            BTreeSet::from([
                FrontendSerializationContract::LegacyWidgetSlotV0,
                FrontendSerializationContract::LegacySubgraphBoundaryProxyV0,
            ])
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::DetectedButUnsupported
        );
        assert!(!profile.execution_supported());
    }

    #[test]
    fn recovery_schema_provenance_is_not_paired() {
        let mut widget = base_node("Target");
        widget["widgets_values"] = json!([7]);
        let profile = resolve_with_provenance(
            json!({"version": 0.4, "nodes": [widget], "links": []}),
            json!({"Target": {"input": {"required": {"value": ["INT", {}]}}, "output": []}}),
            SchemaSnapshotProvenance::RecoveryUnpaired,
        );
        assert!(!profile.execution_supported());
        assert_eq!(
            profile.schema_provenance,
            SchemaSnapshotProvenance::RecoveryUnpaired
        );
        assert_eq!(
            profile.primary_diagnostic(),
            Some("schema_snapshot_provenance_insufficient")
        );
    }

    #[test]
    fn schema_lineage_failure_overrides_implemented_contract_set() {
        let mut workflow = composed_workflow(false);
        workflow["nodes"][1]["inputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name": "unrelated", "type": "INT", "link": 9}));
        workflow["links"]
            .as_array_mut()
            .unwrap()
            .push(json!([9, 3, 0, 2, 1]));
        let profile = resolve(workflow, composed_schema());
        assert_eq!(
            profile.required_contracts,
            BTreeSet::from([
                FrontendSerializationContract::LegacyWidgetSlotV0,
                FrontendSerializationContract::LegacyDynamicInputV0,
            ])
        );
        assert_eq!(
            profile.evidence.schema_lineage.status,
            SchemaLineageStatus::Mismatch
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::EvidenceInsufficient
        );
        assert!(!profile.execution_supported());
    }

    #[test]
    fn detected_legacy_contract_plus_unrelated_schema_mismatch_fails_closed() {
        let mut widget = base_node("Target");
        widget["widgets_values"] = json!([7]);
        widget["inputs"] = json!([{"name": "missing", "type": "INT", "link": 9}]);
        let profile = resolve(
            json!({"version": 0.4, "nodes": [widget], "links": [[9, 2, 0, 1, 0]]}),
            json!({"Target": {"input": {"required": {"value": ["INT", {}]}}, "output": []}}),
        );
        assert!(requires(
            &profile,
            FrontendSerializationContract::LegacyWidgetSlotV0
        ));
        assert_eq!(
            profile.evidence.schema_lineage.status,
            SchemaLineageStatus::Mismatch
        );
        assert_eq!(
            profile.implementation_status,
            UiCompatibilityImplementationStatus::EvidenceInsufficient
        );
        assert!(!profile.execution_supported());
    }
}
