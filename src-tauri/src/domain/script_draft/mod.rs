//! Script import and storyboard draft domain contracts.
//!
//! This module intentionally owns no persistence, parser, provider, formal
//! production structure, queue, or ComfyUI behavior. It is the versioned,
//! serializable boundary consumed by later application services.

pub mod diagnostic;
pub mod draft;
pub mod ids;
pub mod source;
pub mod validation;

#[cfg(test)]
mod tests;

pub use diagnostic::{
    Diagnostic, DiagnosticSeverity, CODE_DRAFT_CAPACITY_EXCEEDED,
    CODE_DRAFT_CAPACITY_EXCEEDED as DRAFT_CAPACITY_DIAGNOSTIC, CODE_DUPLICATE_SOURCE_ID,
    CODE_EMPTY_DRAFT_NODE, CODE_ENCODING_OR_BOM, CODE_INVALID_PARENT, CODE_MISSING_NAME,
    CODE_PROVIDER_INVALID_JSON, CODE_SOURCE_SPAN_OUT_OF_BOUNDS, CODE_UNCERTAIN_SCENE_BOUNDARY,
    CODE_UNKNOWN_JSON_SCHEMA, CODE_UNRESOLVED_SPEAKER,
};
pub use draft::{
    has_blocking_diagnostics, has_unresolved_nodes, DraftCounts, DraftEpisode, DraftEpisodeV1,
    DraftNodeOrigin, DraftReviewState, DraftRevision, DraftRevisionKind, DraftRevisionMetadata,
    DraftScene, DraftSceneV1, DraftShot, DraftShotV1, DraftStatus, DraftStructureV1, EntityMention,
    EntityType, Episode, Scene, Shot, DRAFT_CONTRACT_VERSION, DRAFT_SCHEMA_VERSION, MAX_EPISODES,
    MAX_SCENES, MAX_SHOTS,
};
pub use ids::{
    validate_diagnostic_id, validate_draft_id, validate_draft_node_id, validate_draft_revision_id,
    validate_source_id, DiagnosticId, DraftId, DraftNodeId, DraftRevisionId, ScriptDraftIdError,
    ScriptDraftIdKind, SourceId,
};
pub use source::{
    sha256_hex, source_checksum, ProviderMetadata, ScriptDocument, ScriptFormat, ScriptSource,
    SourceBlock, SourceBlockKind, SourceDomainError, SourceSpan, SourceSpanError, SourceSpanV1,
    SOURCE_SCHEMA_VERSION,
};
pub use validation::{
    canonical_json, canonical_sha256, draft_checksum, validate_payload, validate_payload_root,
    validate_payload_versions, validate_source, validate_structure, DraftValidationError,
    DRAFT_CAPACITY_EXCEEDED, DRAFT_CONTRACT_VERSION_UNSUPPORTED, DRAFT_NODE_ID_DUPLICATE,
    DRAFT_ORDINAL_INVALID, DRAFT_SCHEMA_VERSION_UNSUPPORTED, DRAFT_SOURCE_SPAN_INVALID,
};
