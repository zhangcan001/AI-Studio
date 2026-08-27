use crate::domain::script_draft::draft::{
    DraftScene, DraftShot, DraftStructureV1, DRAFT_CONTRACT_VERSION, DRAFT_SCHEMA_VERSION,
    MAX_EPISODES, MAX_SCENES, MAX_SHOTS,
};
use crate::domain::script_draft::source::{ScriptDocument, SourceSpan, SourceSpanError};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, error::Error, fmt};

pub const DRAFT_CAPACITY_EXCEEDED: &str = "DRAFT_CAPACITY_EXCEEDED";
pub const DRAFT_SOURCE_SPAN_INVALID: &str = "DRAFT_SOURCE_SPAN_INVALID";
pub const DRAFT_NODE_ID_DUPLICATE: &str = "DRAFT_NODE_ID_DUPLICATE";
pub const DRAFT_ORDINAL_INVALID: &str = "DRAFT_ORDINAL_INVALID";
pub const DRAFT_SCHEMA_VERSION_UNSUPPORTED: &str = "DRAFT_SCHEMA_VERSION_UNSUPPORTED";
pub const DRAFT_CONTRACT_VERSION_UNSUPPORTED: &str = "DRAFT_CONTRACT_VERSION_UNSUPPORTED";

/// Stable validation failures for the Script/Draft contract. Error values do
/// not carry source text, raw payloads, or offending user values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftValidationError {
    code: &'static str,
    field: &'static str,
    reason: &'static str,
}

impl DraftValidationError {
    pub const fn new(code: &'static str, field: &'static str, reason: &'static str) -> Self {
        Self {
            code,
            field,
            reason,
        }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub const fn field(&self) -> &'static str {
        self.field
    }

    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for DraftValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}: {}", self.code, self.field, self.reason)
    }
}

impl Error for DraftValidationError {}

pub fn validate_source(source: &ScriptDocument, raw: &[u8]) -> Result<(), DraftValidationError> {
    source
        .validate_raw_bytes(raw)
        .map_err(|error| DraftValidationError::new(error.code(), "source", "source is invalid"))?;

    let mut block_ids = BTreeSet::new();
    for block in &source.source_blocks {
        if block.block_id.trim().is_empty() || !block_ids.insert(&block.block_id) {
            return Err(DraftValidationError::new(
                "DUPLICATE_SOURCE_ID",
                "sourceBlocks.blockId",
                "source block identifiers must be non-empty and unique",
            ));
        }
        validate_spans(&block.span, raw, "sourceBlocks.span")?;
        for diagnostic in &block.diagnostics {
            for span in &diagnostic.source_spans {
                validate_spans(span, raw, "sourceBlocks.diagnostics.sourceSpans")?;
            }
        }
    }
    for diagnostic in &source.diagnostics {
        for span in &diagnostic.source_spans {
            validate_spans(span, raw, "diagnostics.sourceSpans")?;
        }
    }
    Ok(())
}

pub fn validate_structure(
    structure: &DraftStructureV1,
    raw_source: &[u8],
    db_schema_version: u32,
) -> Result<(), DraftValidationError> {
    validate_payload_versions(
        structure.schema_version,
        structure.contract_version,
        db_schema_version,
    )?;

    let counts = structure.counts();
    if counts.episodes > MAX_EPISODES || counts.scenes > MAX_SCENES || counts.shots > MAX_SHOTS {
        return Err(DraftValidationError::new(
            DRAFT_CAPACITY_EXCEEDED,
            "structure",
            "the draft exceeds a supported node capacity",
        ));
    }

    if structure.status != crate::domain::script_draft::DraftStatus::Draft {
        return Err(DraftValidationError::new(
            "DRAFT_STATUS_NOT_CREATABLE",
            "status",
            "this data foundation only creates DRAFT revisions",
        ));
    }

    let mut node_ids = BTreeSet::new();
    validate_ordinals(
        structure.episodes.iter().map(|episode| episode.ordinal),
        "episodes.ordinal",
    )?;

    for episode in &structure.episodes {
        validate_node_id(&episode.draft_node_id, &mut node_ids)?;
        if episode.parent_draft_node_id.is_some() {
            return Err(DraftValidationError::new(
                "INVALID_PARENT_DRAFT_NODE",
                "episodes.parentDraftNodeId",
                "an episode must be a root node",
            ));
        }
        validate_name(&episode.name)?;
        validate_node_spans(&episode.source_spans, raw_source, "episodes.sourceSpans")?;
        validate_diagnostics(&episode.diagnostics, raw_source, &node_ids)?;
        validate_ordinals(
            episode.scenes.iter().map(|scene| scene.ordinal),
            "scenes.ordinal",
        )?;

        for scene in &episode.scenes {
            validate_scene(scene, &episode.draft_node_id, &mut node_ids, raw_source)?;
        }
    }

    validate_diagnostics(&structure.diagnostics, raw_source, &node_ids)
}

pub fn validate_payload_root(
    payload: &Value,
    db_schema_version: u32,
) -> Result<(), DraftValidationError> {
    let root = payload.as_object().ok_or_else(|| {
        DraftValidationError::new("INVALID_PAYLOAD_ROOT", "payload", "root must be an object")
    })?;
    let schema_version = root
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            DraftValidationError::new(
                "DRAFT_SCHEMA_VERSION_MISSING",
                "payload.schemaVersion",
                "schema version is required",
            )
        })? as u32;
    let contract_version = root
        .get("contractVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            DraftValidationError::new(
                "DRAFT_CONTRACT_VERSION_MISSING",
                "payload.contractVersion",
                "contract version is required",
            )
        })? as u32;
    validate_payload_versions(schema_version, contract_version, db_schema_version)
}

pub fn validate_payload<T: DeserializeOwned>(
    payload: &Value,
    db_schema_version: u32,
) -> Result<T, DraftValidationError> {
    validate_payload_root(payload, db_schema_version)?;
    serde_json::from_value(payload.clone()).map_err(|_| {
        DraftValidationError::new(
            "INVALID_PAYLOAD",
            "payload",
            "payload does not satisfy the Draft contract",
        )
    })
}

pub fn validate_payload_versions(
    schema_version: u32,
    contract_version: u32,
    db_schema_version: u32,
) -> Result<(), DraftValidationError> {
    if schema_version != DRAFT_SCHEMA_VERSION {
        return Err(DraftValidationError::new(
            "DRAFT_SCHEMA_VERSION_UNSUPPORTED",
            "schemaVersion",
            "payload schema version is unsupported",
        ));
    }
    if db_schema_version != DRAFT_SCHEMA_VERSION {
        return Err(DraftValidationError::new(
            "DRAFT_DB_SCHEMA_VERSION_MISMATCH",
            "dbSchemaVersion",
            "database schema version is unsupported",
        ));
    }
    if contract_version != DRAFT_CONTRACT_VERSION {
        return Err(DraftValidationError::new(
            "DRAFT_CONTRACT_VERSION_UNSUPPORTED",
            "contractVersion",
            "payload contract version is unsupported",
        ));
    }
    Ok(())
}

pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, DraftValidationError> {
    let value = serde_json::to_value(value).map_err(|_| {
        DraftValidationError::new(
            "CANONICALIZATION_FAILED",
            "payload",
            "value cannot be serialized",
        )
    })?;
    let canonical = canonicalize_value(value);
    serde_json::to_string(&canonical).map_err(|_| {
        DraftValidationError::new(
            "CANONICALIZATION_FAILED",
            "payload",
            "value cannot be encoded",
        )
    })
}

pub fn canonical_sha256<T: Serialize>(value: &T) -> Result<String, DraftValidationError> {
    let json = canonical_json(value)?;
    let digest = Sha256::digest(json.as_bytes());
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn draft_checksum(structure: &DraftStructureV1) -> Result<String, DraftValidationError> {
    canonical_sha256(structure)
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_value).collect()),
        Value::Object(object) => {
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut canonical = Map::new();
            for (key, value) in entries {
                canonical.insert(key, canonicalize_value(value));
            }
            Value::Object(canonical)
        }
        scalar => scalar,
    }
}

fn validate_scene(
    scene: &DraftScene,
    parent_id: &crate::domain::script_draft::DraftNodeId,
    node_ids: &mut BTreeSet<String>,
    raw_source: &[u8],
) -> Result<(), DraftValidationError> {
    validate_node_id(&scene.draft_node_id, node_ids)?;
    if scene.parent_draft_node_id.as_ref() != Some(parent_id) {
        return Err(DraftValidationError::new(
            "INVALID_PARENT_DRAFT_NODE",
            "scenes.parentDraftNodeId",
            "scene parent must be its containing episode",
        ));
    }
    validate_name(&scene.name)?;
    validate_node_spans(&scene.source_spans, raw_source, "scenes.sourceSpans")?;
    validate_diagnostics(&scene.diagnostics, raw_source, node_ids)?;
    validate_ordinals(scene.shots.iter().map(|shot| shot.ordinal), "shots.ordinal")?;
    for shot in &scene.shots {
        validate_shot(shot, &scene.draft_node_id, node_ids, raw_source)?;
    }
    Ok(())
}

fn validate_shot(
    shot: &DraftShot,
    parent_id: &crate::domain::script_draft::DraftNodeId,
    node_ids: &mut BTreeSet<String>,
    raw_source: &[u8],
) -> Result<(), DraftValidationError> {
    validate_node_id(&shot.draft_node_id, node_ids)?;
    if shot.parent_draft_node_id.as_ref() != Some(parent_id)
        || &shot.parent_scene_draft_id != parent_id
    {
        return Err(DraftValidationError::new(
            "INVALID_PARENT_DRAFT_NODE",
            "shots.parentDraftNodeId",
            "shot parent must be its containing scene",
        ));
    }
    validate_name(&shot.name)?;
    if shot
        .duration_suggestion
        .is_some_and(|duration| !duration.is_finite() || duration < 0.0)
    {
        return Err(DraftValidationError::new(
            "INVALID_DURATION_SUGGESTION",
            "shots.durationSuggestion",
            "duration must be a finite non-negative number",
        ));
    }
    validate_node_spans(&shot.source_spans, raw_source, "shots.sourceSpans")?;
    validate_diagnostics(&shot.diagnostics, raw_source, node_ids)?;
    let mut mention_ids = BTreeSet::new();
    for mention in shot
        .character_mentions
        .iter()
        .chain(shot.prop_mentions.iter())
        .chain(shot.scene_mention.iter())
    {
        if mention.id.trim().is_empty()
            || !mention_ids.insert(mention.id.as_str())
            || mention.text.trim().is_empty()
            || mention.normalized_text.trim().is_empty()
        {
            return Err(DraftValidationError::new(
                "INVALID_ENTITY_MENTION",
                "shots.mentions",
                "entity mention text is required",
            ));
        }
        if mention
            .confidence
            .is_some_and(|confidence| !confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
        {
            return Err(DraftValidationError::new(
                "INVALID_ENTITY_MENTION_CONFIDENCE",
                "shots.mentions.confidence",
                "confidence must be between zero and one",
            ));
        }
        validate_node_spans(
            &mention.source_spans,
            raw_source,
            "shots.mentions.sourceSpans",
        )?;
    }
    Ok(())
}

fn validate_node_id(
    id: &crate::domain::script_draft::DraftNodeId,
    node_ids: &mut BTreeSet<String>,
) -> Result<(), DraftValidationError> {
    if !node_ids.insert(id.as_str().to_owned()) {
        return Err(DraftValidationError::new(
            "DRAFT_NODE_ID_DUPLICATE",
            "draftNodeId",
            "draft node identifiers must be unique",
        ));
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), DraftValidationError> {
    if name.trim().is_empty() {
        return Err(DraftValidationError::new(
            "DRAFT_NAME_REQUIRED",
            "name",
            "draft node name is required",
        ));
    }
    Ok(())
}

fn validate_node_spans(
    spans: &[SourceSpan],
    raw_source: &[u8],
    field: &'static str,
) -> Result<(), DraftValidationError> {
    for span in spans {
        validate_spans(span, raw_source, field)?;
    }
    Ok(())
}

fn validate_spans(
    span: &SourceSpan,
    raw_source: &[u8],
    field: &'static str,
) -> Result<(), DraftValidationError> {
    span.validate(raw_source)
        .map_err(|error| DraftValidationError::new(error.code(), field, span_reason(error)))
}

const fn span_reason(error: SourceSpanError) -> &'static str {
    match error {
        SourceSpanError::InvalidRange => "span range is invalid",
        SourceSpanError::OutOfBounds => "span exceeds source byte bounds",
        SourceSpanError::InvalidUtf8 => "source is not valid UTF-8",
        SourceSpanError::NotUtf8Boundary => "span is not on a UTF-8 boundary",
        SourceSpanError::InvalidLineRange | SourceSpanError::IncompleteLineRange => {
            "line range is invalid"
        }
        SourceSpanError::InvalidCharacterRange | SourceSpanError::IncompleteCharacterRange => {
            "character range is invalid"
        }
    }
}

fn validate_diagnostics(
    diagnostics: &[crate::domain::script_draft::Diagnostic],
    raw_source: &[u8],
    node_ids: &BTreeSet<String>,
) -> Result<(), DraftValidationError> {
    let mut diagnostic_ids = BTreeSet::new();
    for diagnostic in diagnostics {
        if !diagnostic_ids.insert(diagnostic.diagnostic_id.as_str().to_owned()) {
            return Err(DraftValidationError::new(
                "DUPLICATE_DIAGNOSTIC_ID",
                "diagnosticId",
                "diagnostic identifiers must be unique within a node",
            ));
        }
        if let Some(node_id) = &diagnostic.draft_node_id {
            if !node_ids.contains(node_id.as_str()) {
                return Err(DraftValidationError::new(
                    "UNKNOWN_DRAFT_NODE_ID",
                    "diagnostics.draftNodeId",
                    "diagnostic node reference is unknown",
                ));
            }
        }
        for span in &diagnostic.source_spans {
            validate_spans(span, raw_source, "diagnostics.sourceSpans")?;
        }
    }
    Ok(())
}

fn validate_ordinals<I>(ordinals: I, field: &'static str) -> Result<(), DraftValidationError>
where
    I: IntoIterator<Item = u32>,
{
    let ordinals: Vec<_> = ordinals.into_iter().collect();
    let mut sorted = ordinals.clone();
    sorted.sort_unstable();
    if sorted
        .iter()
        .enumerate()
        .any(|(expected, actual)| *actual != expected as u32)
    {
        return Err(DraftValidationError::new(
            "DRAFT_ORDINAL_INVALID",
            field,
            "ordinals must be unique and contiguous starting at zero",
        ));
    }
    Ok(())
}
