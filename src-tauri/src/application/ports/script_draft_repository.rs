use crate::application::ports::RepositoryError;
use crate::domain::script_draft::{
    DraftId, DraftRevisionId, DraftRevisionKind, ProviderMetadata, SourceId,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::fmt;

/// A complete immutable revision. Payload is available only through an
/// explicit revision lookup, never through list/history metadata.
#[derive(Clone, PartialEq, Eq)]
pub struct ScriptDraftRevisionRecord {
    pub id: DraftRevisionId,
    pub draft_id: DraftId,
    pub project_id: String,
    pub source_id: SourceId,
    pub revision: u64,
    pub previous_revision_id: Option<DraftRevisionId>,
    pub schema_version: u32,
    pub revision_kind: DraftRevisionKind,
    pub parser_version: String,
    pub contract_version: u32,
    pub provider_kind: Option<String>,
    pub provider_model: Option<String>,
    pub provider_metadata: Option<ProviderMetadata>,
    pub payload_checksum: String,
    pub summary_json: String,
    pub payload_json: String,
    pub created_at: DateTime<Utc>,
}

impl fmt::Debug for ScriptDraftRevisionRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScriptDraftRevisionRecord")
            .field("id", &self.id)
            .field("draft_id", &self.draft_id)
            .field("project_id", &self.project_id)
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("previous_revision_id", &self.previous_revision_id)
            .field("schema_version", &self.schema_version)
            .field("revision_kind", &self.revision_kind)
            .field("parser_version", &self.parser_version)
            .field("contract_version", &self.contract_version)
            .field("provider_kind", &self.provider_kind)
            .field("provider_model", &self.provider_model)
            .field("provider_metadata", &self.provider_metadata)
            .field("payload_checksum", &self.payload_checksum)
            .field("summary_json", &self.summary_json)
            .field("payload_json", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptDraftRevisionMetadata {
    pub id: DraftRevisionId,
    pub draft_id: DraftId,
    pub project_id: String,
    pub source_id: SourceId,
    pub revision: u64,
    pub previous_revision_id: Option<DraftRevisionId>,
    pub schema_version: u32,
    pub revision_kind: DraftRevisionKind,
    pub parser_version: String,
    pub contract_version: u32,
    pub provider_kind: Option<String>,
    pub provider_model: Option<String>,
    pub provider_metadata: Option<ProviderMetadata>,
    pub payload_checksum: String,
    pub summary_json: String,
    pub created_at: DateTime<Utc>,
}

impl From<ScriptDraftRevisionRecord> for ScriptDraftRevisionMetadata {
    fn from(record: ScriptDraftRevisionRecord) -> Self {
        Self {
            id: record.id,
            draft_id: record.draft_id,
            project_id: record.project_id,
            source_id: record.source_id,
            revision: record.revision,
            previous_revision_id: record.previous_revision_id,
            schema_version: record.schema_version,
            revision_kind: record.revision_kind,
            parser_version: record.parser_version,
            contract_version: record.contract_version,
            provider_kind: record.provider_kind,
            provider_model: record.provider_model,
            provider_metadata: record.provider_metadata,
            payload_checksum: record.payload_checksum,
            summary_json: record.summary_json,
            created_at: record.created_at,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct InsertScriptDraftRevision {
    pub id: DraftRevisionId,
    pub draft_id: DraftId,
    pub project_id: String,
    pub source_id: SourceId,
    pub expected_revision: Option<u64>,
    pub schema_version: u32,
    pub revision_kind: DraftRevisionKind,
    pub parser_version: String,
    pub contract_version: u32,
    pub provider_kind: Option<String>,
    pub provider_model: Option<String>,
    pub provider_metadata: Option<ProviderMetadata>,
    pub payload_checksum: String,
    pub summary_json: String,
    pub payload_json: String,
    pub created_at: DateTime<Utc>,
}

impl fmt::Debug for InsertScriptDraftRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InsertScriptDraftRevision")
            .field("id", &self.id)
            .field("draft_id", &self.draft_id)
            .field("project_id", &self.project_id)
            .field("source_id", &self.source_id)
            .field("expected_revision", &self.expected_revision)
            .field("schema_version", &self.schema_version)
            .field("revision_kind", &self.revision_kind)
            .field("parser_version", &self.parser_version)
            .field("contract_version", &self.contract_version)
            .field("provider_kind", &self.provider_kind)
            .field("provider_model", &self.provider_model)
            .field("provider_metadata", &self.provider_metadata)
            .field("payload_checksum", &self.payload_checksum)
            .field("summary_json", &self.summary_json)
            .field("payload_json", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScriptDraftPageQuery {
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptDraftPage<T> {
    pub items: Vec<T>,
    /// Opaque to callers. It must be passed back unchanged to continue a page.
    pub next_cursor: Option<String>,
}

#[async_trait]
pub trait ScriptDraftRepository: Send + Sync {
    /// Atomically reads the current latest revision, checks
    /// `expected_revision`, computes the next revision and previous link, and
    /// inserts an immutable row. A semantic retry with the same checksum is a
    /// no-op and returns the existing latest metadata.
    async fn insert_revision(
        &self,
        request: InsertScriptDraftRevision,
    ) -> Result<ScriptDraftRevisionMetadata, RepositoryError>;

    async fn get_revision(
        &self,
        project_id: &str,
        draft_id: &DraftId,
        revision: u64,
    ) -> Result<Option<ScriptDraftRevisionRecord>, RepositoryError>;

    async fn get_latest(
        &self,
        project_id: &str,
        draft_id: &DraftId,
    ) -> Result<Option<ScriptDraftRevisionMetadata>, RepositoryError>;

    async fn list_latest(
        &self,
        project_id: &str,
        query: ScriptDraftPageQuery,
    ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, RepositoryError>;

    async fn list_history(
        &self,
        project_id: &str,
        draft_id: &DraftId,
        query: ScriptDraftPageQuery,
    ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, RepositoryError>;
}
