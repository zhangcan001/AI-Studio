//! Application boundary for the pre-production Script/Draft data foundation.
//!
//! This service deliberately owns no parser, formal-production, readiness,
//! queue, or ComfyUI behavior.  It validates the source and draft contracts,
//! then delegates all persistence and revision races to the repositories.

use crate::application::ports::{
    Clock, InsertScriptDraftRevision, RepositoryError, ScriptDraftPage, ScriptDraftPageQuery,
    ScriptDraftRepository, ScriptDraftRevisionMetadata, ScriptDraftRevisionRecord,
    ScriptSourceRecord, ScriptSourceRepository,
};
use crate::domain::script_draft::{
    canonical_json, draft_checksum, sha256_hex, validate_structure, DraftId, DraftRevisionId,
    DraftRevisionKind, DraftStructureV1, ProviderMetadata, ScriptFormat, SourceId,
    DRAFT_SCHEMA_VERSION,
};
use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc};

pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
/// A serialized draft is bounded independently from the source.  This keeps
/// malformed or unexpectedly expanded payloads from becoming an accidental
/// memory/storage denial of service while leaving the documented 5000-shot
/// draft capacity available.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct ScriptSourceCreateRequest {
    pub project_id: String,
    pub format: ScriptFormat,
    pub source_text: Vec<u8>,
    pub original_filename: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptSourceView {
    pub id: SourceId,
    pub project_id: String,
    pub format: ScriptFormat,
    pub source_checksum: String,
    pub source_bytes: u64,
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    /// The filename is request metadata only.  Migration 025 intentionally
    /// stores source identity by project/checksum/format, not by filename.
    pub original_filename: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct CreateScriptDraftRequest {
    pub project_id: String,
    pub source_id: SourceId,
    pub structure: DraftStructureV1,
    pub parser_version: String,
    pub provider_metadata: Option<ProviderMetadata>,
}

#[derive(Clone, PartialEq)]
pub struct AppendScriptDraftRevisionRequest {
    pub project_id: String,
    pub draft_id: DraftId,
    pub expected_revision: u64,
    pub structure: DraftStructureV1,
    pub revision_kind: DraftRevisionKind,
    pub parser_version: String,
    pub provider_metadata: Option<ProviderMetadata>,
}

pub type CreateSourceRequest = ScriptSourceCreateRequest;
pub type CreateDraftRequest = CreateScriptDraftRequest;
pub type AppendRevisionRequest = AppendScriptDraftRevisionRequest;

#[derive(Clone, PartialEq)]
pub struct ScriptDraftRevisionView {
    pub metadata: ScriptDraftRevisionMetadata,
    pub structure: DraftStructureV1,
}

impl fmt::Debug for ScriptDraftRevisionView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScriptDraftRevisionView")
            .field("draft_id", &self.metadata.draft_id)
            .field("revision", &self.metadata.revision)
            .field("revision_id", &self.metadata.id)
            .field("counts", &self.structure.counts())
            .finish()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftSummary {
    pub episodes: usize,
    pub scenes: usize,
    pub shots: usize,
}

#[derive(Clone)]
pub struct ScriptDraftService {
    source_repository: Arc<dyn ScriptSourceRepository>,
    draft_repository: Arc<dyn ScriptDraftRepository>,
    clock: Arc<dyn Clock>,
}

impl ScriptDraftService {
    pub fn new(
        source_repository: Arc<dyn ScriptSourceRepository>,
        draft_repository: Arc<dyn ScriptDraftRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            source_repository,
            draft_repository,
            clock,
        }
    }

    /// Inserts a source or reuses the same project's source with the same
    /// raw-byte checksum and format.  Filename and line-ending spelling never
    /// participate in identity; raw bytes do, so BOM/CRLF/LF are preserved.
    pub async fn create_or_reuse_source(
        &self,
        request: ScriptSourceCreateRequest,
    ) -> Result<ScriptSourceView, AppError> {
        validate_project_id(&request.project_id)?;
        if request.source_text.len() > MAX_SOURCE_BYTES {
            return Err(invalid(
                "PAYLOAD_TOO_LARGE: source exceeds the 16 MiB limit",
            ));
        }
        std::str::from_utf8(&request.source_text)
            .map_err(|_| invalid("INVALID_SOURCE_UTF8: source must be UTF-8"))?;

        let source_id = SourceId::new();
        let checksum = sha256_hex(&request.source_text);
        let record = ScriptSourceRecord {
            id: source_id,
            project_id: request.project_id.clone(),
            format: request.format,
            source_checksum: checksum,
            source_bytes: request.source_text.len() as u64,
            source_text: request.source_text,
            original_filename: request.original_filename.clone(),
            schema_version: crate::domain::script_draft::SOURCE_SCHEMA_VERSION,
            created_at: self.clock.now(),
        };

        let stored = self
            .source_repository
            .insert_or_reuse(record)
            .await
            .map_err(map_repository_error)?;
        Ok(ScriptSourceView {
            id: stored.id,
            project_id: stored.project_id,
            format: stored.format,
            source_checksum: stored.source_checksum,
            source_bytes: stored.source_bytes,
            schema_version: stored.schema_version,
            created_at: stored.created_at,
            original_filename: stored.original_filename,
        })
    }

    /// Alias with the shorter name used by callers that do not need to expose
    /// the dedupe detail in their command/facade naming.
    pub async fn create_source(
        &self,
        request: ScriptSourceCreateRequest,
    ) -> Result<ScriptSourceView, AppError> {
        self.create_or_reuse_source(request).await
    }

    pub async fn create_draft(
        &self,
        request: CreateScriptDraftRequest,
    ) -> Result<ScriptDraftRevisionView, AppError> {
        validate_project_id(&request.project_id)?;
        require_parser_version(&request.parser_version)?;

        let source = self
            .load_source(&request.project_id, &request.source_id)
            .await?;
        let draft_id = DraftId::new();
        let revision_id = DraftRevisionId::new();
        let mut structure = request.structure;
        structure.draft_id = draft_id.clone();
        structure.source_id = request.source_id.clone();
        structure.revision_id = revision_id.clone();
        structure.status = crate::domain::script_draft::DraftStatus::Draft;
        let prepared = self.prepare_payload(&structure, &source.source_text)?;

        let metadata = self
            .draft_repository
            .insert_revision(InsertScriptDraftRevision {
                id: revision_id,
                draft_id,
                project_id: request.project_id,
                source_id: request.source_id,
                expected_revision: None,
                schema_version: structure.schema_version,
                revision_kind: DraftRevisionKind::Parsed,
                parser_version: request.parser_version,
                contract_version: structure.contract_version,
                provider_kind: provider_kind(request.provider_metadata.as_ref()),
                provider_model: provider_model(request.provider_metadata.as_ref()),
                provider_metadata: request.provider_metadata,
                payload_checksum: prepared.payload_checksum,
                summary_json: prepared.summary_json,
                payload_json: prepared.payload_json,
                created_at: self.clock.now(),
            })
            .await
            .map_err(map_repository_error)?;

        Ok(revision_view(metadata, structure))
    }

    pub async fn append_revision(
        &self,
        request: AppendScriptDraftRevisionRequest,
    ) -> Result<ScriptDraftRevisionView, AppError> {
        validate_project_id(&request.project_id)?;
        require_parser_version(&request.parser_version)?;

        let latest = self
            .draft_repository
            .get_latest(&request.project_id, &request.draft_id)
            .await
            .map_err(map_repository_error)?
            .ok_or_else(|| invalid("DRAFT_NOT_FOUND: draft does not exist"))?;
        let latest_record = self
            .draft_repository
            .get_revision(&request.project_id, &request.draft_id, latest.revision)
            .await
            .map_err(map_repository_error)?
            .ok_or_else(|| invalid("DRAFT_NOT_FOUND: latest draft revision does not exist"))?;
        let latest_structure = self.decode_and_validate_record(&latest_record).await?;
        let source = self
            .load_source(&request.project_id, &latest.source_id)
            .await?;

        let revision_id = DraftRevisionId::new();
        let mut structure = request.structure;
        structure.draft_id = request.draft_id.clone();
        structure.source_id = latest.source_id.clone();
        structure.revision_id = revision_id.clone();
        structure.status = crate::domain::script_draft::DraftStatus::Draft;
        // `revision_id` is part of the stored contract, but it is identity
        // rather than user payload.  For a semantic retry, calculate the
        // candidate checksum with the existing revision identity so the
        // repository can perform its atomic same-payload no-op.
        let same_payload = same_payload_ignoring_revision_id(&structure, &latest_structure);
        let same_retry = same_payload
            && request.revision_kind == latest.revision_kind
            && request.parser_version == latest.parser_version
            && structure.schema_version == latest.schema_version
            && structure.contract_version == latest.contract_version
            && provider_kind(request.provider_metadata.as_ref()) == latest.provider_kind
            && provider_model(request.provider_metadata.as_ref()) == latest.provider_model
            && request.provider_metadata == latest.provider_metadata;
        let prepared_structure = if same_retry {
            let mut value = structure.clone();
            value.revision_id = latest_record.id.clone();
            value
        } else {
            structure.clone()
        };
        let prepared = self.prepare_payload(&prepared_structure, &source.source_text)?;

        let metadata = self
            .draft_repository
            .insert_revision(InsertScriptDraftRevision {
                id: revision_id,
                draft_id: request.draft_id,
                project_id: request.project_id,
                source_id: latest.source_id,
                expected_revision: Some(request.expected_revision),
                schema_version: structure.schema_version,
                revision_kind: request.revision_kind,
                parser_version: request.parser_version,
                contract_version: structure.contract_version,
                provider_kind: provider_kind(request.provider_metadata.as_ref()),
                provider_model: provider_model(request.provider_metadata.as_ref()),
                provider_metadata: request.provider_metadata,
                payload_checksum: prepared.payload_checksum,
                summary_json: prepared.summary_json,
                payload_json: prepared.payload_json,
                created_at: self.clock.now(),
            })
            .await
            .map_err(map_repository_error)?;

        // The repository may return the existing row for an idempotent same-
        // payload retry.  Always use its identity in the returned structure.
        structure.revision_id = metadata.id.clone();
        Ok(revision_view(metadata, structure))
    }

    pub async fn get_revision(
        &self,
        project_id: &str,
        draft_id: &DraftId,
        revision: u64,
    ) -> Result<Option<ScriptDraftRevisionView>, AppError> {
        validate_project_id(project_id)?;
        let Some(record) = self
            .draft_repository
            .get_revision(project_id, draft_id, revision)
            .await
            .map_err(map_repository_error)?
        else {
            return Ok(None);
        };
        let structure = self.decode_and_validate_record(&record).await?;
        Ok(Some(ScriptDraftRevisionView {
            metadata: record.into(),
            structure,
        }))
    }

    pub async fn latest(
        &self,
        project_id: &str,
        draft_id: &DraftId,
    ) -> Result<Option<ScriptDraftRevisionMetadata>, AppError> {
        validate_project_id(project_id)?;
        self.draft_repository
            .get_latest(project_id, draft_id)
            .await
            .map_err(map_repository_error)
    }

    pub async fn get_latest(
        &self,
        project_id: &str,
        draft_id: &DraftId,
    ) -> Result<Option<ScriptDraftRevisionMetadata>, AppError> {
        self.latest(project_id, draft_id).await
    }

    pub async fn history(
        &self,
        project_id: &str,
        draft_id: &DraftId,
        query: ScriptDraftPageQuery,
    ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, AppError> {
        validate_project_id(project_id)?;
        self.draft_repository
            .list_history(project_id, draft_id, normalize_query(query))
            .await
            .map_err(map_repository_error)
    }

    pub async fn list_history(
        &self,
        project_id: &str,
        draft_id: &DraftId,
        query: ScriptDraftPageQuery,
    ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, AppError> {
        self.history(project_id, draft_id, query).await
    }

    pub async fn list_latest(
        &self,
        project_id: &str,
        query: ScriptDraftPageQuery,
    ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, AppError> {
        validate_project_id(project_id)?;
        self.draft_repository
            .list_latest(project_id, normalize_query(query))
            .await
            .map_err(map_repository_error)
    }

    pub async fn list_sources(
        &self,
        project_id: &str,
    ) -> Result<Vec<crate::application::ports::ScriptSourceMetadata>, AppError> {
        validate_project_id(project_id)?;
        self.source_repository
            .list_metadata(project_id)
            .await
            .map_err(map_repository_error)
    }

    async fn load_source(
        &self,
        project_id: &str,
        source_id: &SourceId,
    ) -> Result<ScriptSourceRecord, AppError> {
        let source = self
            .source_repository
            .find_by_id(project_id, source_id)
            .await
            .map_err(map_repository_error)?
            .ok_or_else(|| invalid("SOURCE_NOT_FOUND: source does not exist"))?;
        if source.project_id != project_id {
            return Err(invalid("PROJECT_MISMATCH: source is outside the project"));
        }
        if source.source_text.len() > MAX_SOURCE_BYTES {
            return Err(invalid(
                "PAYLOAD_TOO_LARGE: source exceeds the 16 MiB limit",
            ));
        }
        std::str::from_utf8(&source.source_text)
            .map_err(|_| invalid("INVALID_SOURCE_UTF8: source must be UTF-8"))?;
        if source.source_bytes != source.source_text.len() as u64
            || source.source_checksum != sha256_hex(&source.source_text)
        {
            return Err(invalid(
                "SOURCE_CHECKSUM_INVALID: source identity is inconsistent",
            ));
        }
        Ok(source)
    }

    fn prepare_payload(
        &self,
        structure: &DraftStructureV1,
        raw_source: &[u8],
    ) -> Result<PreparedPayload, AppError> {
        if structure.schema_version != DRAFT_SCHEMA_VERSION {
            return Err(invalid(
                "SCHEMA_VERSION_UNSUPPORTED: draft schema is unsupported",
            ));
        }
        validate_structure(structure, raw_source, DRAFT_SCHEMA_VERSION)
            .map_err(|error| invalid(format!("{}: draft payload is invalid", error.code())))?;
        let payload_json = canonical_json(structure)
            .map_err(|_| invalid("INVALID_PAYLOAD: draft payload cannot be canonicalized"))?;
        if payload_json.len() > MAX_PAYLOAD_BYTES {
            return Err(invalid(
                "PAYLOAD_TOO_LARGE: draft payload exceeds the size limit",
            ));
        }
        let payload_checksum = draft_checksum(structure)
            .map_err(|_| invalid("INVALID_PAYLOAD: draft checksum cannot be computed"))?;
        let counts = structure.counts();
        let summary_json = canonical_json(&DraftSummary {
            episodes: counts.episodes,
            scenes: counts.scenes,
            shots: counts.shots,
        })
        .map_err(|_| invalid("INVALID_PAYLOAD: draft summary cannot be computed"))?;
        Ok(PreparedPayload {
            payload_json,
            payload_checksum,
            summary_json,
        })
    }

    async fn decode_and_validate_record(
        &self,
        record: &ScriptDraftRevisionRecord,
    ) -> Result<DraftStructureV1, AppError> {
        if record.payload_json.len() > MAX_PAYLOAD_BYTES {
            return Err(invalid(
                "PAYLOAD_TOO_LARGE: draft payload exceeds the size limit",
            ));
        }
        let value: serde_json::Value = serde_json::from_str(&record.payload_json)
            .map_err(|_| invalid("INVALID_PAYLOAD: stored draft payload is invalid"))?;
        let structure: DraftStructureV1 =
            crate::domain::script_draft::validate_payload(&value, DRAFT_SCHEMA_VERSION).map_err(
                |error| invalid(format!("{}: stored draft payload is invalid", error.code())),
            )?;
        let source = self
            .load_source(&record.project_id, &record.source_id)
            .await?;
        if structure.draft_id != record.draft_id
            || structure.source_id != record.source_id
            || structure.revision_id != record.id
        {
            return Err(invalid(
                "PROJECT_MISMATCH: stored draft identity is inconsistent",
            ));
        }
        let prepared = self.prepare_payload(&structure, &source.source_text)?;
        if prepared.payload_checksum != record.payload_checksum
            || prepared.payload_json != record.payload_json
        {
            return Err(invalid(
                "PAYLOAD_CHECKSUM_INVALID: stored draft checksum is inconsistent",
            ));
        }
        Ok(structure)
    }
}

struct PreparedPayload {
    payload_json: String,
    payload_checksum: String,
    summary_json: String,
}

fn same_payload_ignoring_revision_id(
    candidate: &DraftStructureV1,
    latest: &DraftStructureV1,
) -> bool {
    let mut candidate = candidate.clone();
    let mut latest = latest.clone();
    candidate.revision_id = latest.revision_id.clone();
    latest.revision_id = candidate.revision_id.clone();
    candidate == latest
}

fn revision_view(
    metadata: ScriptDraftRevisionMetadata,
    mut structure: DraftStructureV1,
) -> ScriptDraftRevisionView {
    structure.draft_id = metadata.draft_id.clone();
    structure.source_id = metadata.source_id.clone();
    structure.revision_id = metadata.id.clone();
    ScriptDraftRevisionView {
        metadata,
        structure,
    }
}

fn normalize_query(mut query: ScriptDraftPageQuery) -> ScriptDraftPageQuery {
    query.limit = if query.limit == 0 {
        50
    } else {
        query.limit.min(200)
    };
    query
}

fn validate_project_id(value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(invalid("PROJECT_MISMATCH: project_id is required"));
    }
    Ok(())
}

fn require_parser_version(value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(invalid("INVALID_INPUT: parser_version is required"));
    }
    Ok(())
}

fn provider_kind(metadata: Option<&ProviderMetadata>) -> Option<String> {
    metadata
        .map(|value| value.provider_kind.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn provider_model(metadata: Option<&ProviderMetadata>) -> Option<String> {
    metadata
        .and_then(|value| value.model_label.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::invalid_input(message)
}

fn map_repository_error(error: RepositoryError) -> AppError {
    match error {
        RepositoryError::NotFound { entity, .. } => {
            let code = if entity.contains("source") {
                "SOURCE_NOT_FOUND"
            } else {
                "DRAFT_NOT_FOUND"
            };
            invalid(format!("{code}: repository record does not exist"))
        }
        RepositoryError::Integrity { message } => {
            let code = message.split(':').next().unwrap_or("REPOSITORY_INTEGRITY");
            let code = if code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                code
            } else {
                "REPOSITORY_INTEGRITY"
            };
            invalid(format!(
                "{code}: repository rejected the Script/Draft operation"
            ))
        }
        RepositoryError::Serialization { .. } => {
            AppError::internal("Script/Draft serialization failed")
        }
        RepositoryError::Database { .. } => {
            AppError::database("Script/Draft repository unavailable")
        }
        RepositoryError::WorkflowVersionConflict { .. }
        | RepositoryError::RecipeVersionConflict { .. }
        | RepositoryError::PresetNameConflict { .. } => {
            AppError::internal("unexpected repository conflict in Script/Draft operation")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::TimeZone;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct FixedClock(DateTime<Utc>);

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    #[derive(Default)]
    struct MemorySources {
        rows: Mutex<Vec<ScriptSourceRecord>>,
    }

    #[async_trait]
    impl ScriptSourceRepository for MemorySources {
        async fn insert_or_reuse(
            &self,
            source: ScriptSourceRecord,
        ) -> Result<ScriptSourceRecord, RepositoryError> {
            let mut rows = self.rows.lock().expect("source mutex");
            if let Some(existing) = rows.iter().find(|row| {
                row.project_id == source.project_id
                    && row.format == source.format
                    && row.source_checksum == source.source_checksum
            }) {
                return Ok(existing.clone());
            }
            rows.push(source.clone());
            Ok(source)
        }

        async fn find_by_id(
            &self,
            project_id: &str,
            source_id: &SourceId,
        ) -> Result<Option<ScriptSourceRecord>, RepositoryError> {
            Ok(self
                .rows
                .lock()
                .expect("source mutex")
                .iter()
                .find(|row| row.project_id == project_id && row.id == *source_id)
                .cloned())
        }

        async fn find_by_checksum(
            &self,
            project_id: &str,
            format: ScriptFormat,
            source_checksum: &str,
        ) -> Result<Option<crate::application::ports::ScriptSourceMetadata>, RepositoryError>
        {
            Ok(self
                .rows
                .lock()
                .expect("source mutex")
                .iter()
                .find(|row| {
                    row.project_id == project_id
                        && row.format == format
                        && row.source_checksum == source_checksum
                })
                .map(|row| crate::application::ports::ScriptSourceMetadata {
                    id: row.id.clone(),
                    project_id: row.project_id.clone(),
                    format: row.format,
                    source_checksum: row.source_checksum.clone(),
                    source_bytes: row.source_bytes,
                    original_filename: row.original_filename.clone(),
                    schema_version: row.schema_version,
                    created_at: row.created_at,
                }))
        }

        async fn list_metadata(
            &self,
            project_id: &str,
        ) -> Result<Vec<crate::application::ports::ScriptSourceMetadata>, RepositoryError> {
            Ok(self
                .rows
                .lock()
                .expect("source mutex")
                .iter()
                .filter(|row| row.project_id == project_id)
                .map(|row| crate::application::ports::ScriptSourceMetadata {
                    id: row.id.clone(),
                    project_id: row.project_id.clone(),
                    format: row.format,
                    source_checksum: row.source_checksum.clone(),
                    source_bytes: row.source_bytes,
                    original_filename: row.original_filename.clone(),
                    schema_version: row.schema_version,
                    created_at: row.created_at,
                })
                .collect())
        }
    }

    #[derive(Default)]
    struct MemoryDrafts {
        rows: Mutex<Vec<ScriptDraftRevisionRecord>>,
    }

    #[async_trait]
    impl ScriptDraftRepository for MemoryDrafts {
        async fn insert_revision(
            &self,
            request: InsertScriptDraftRevision,
        ) -> Result<ScriptDraftRevisionMetadata, RepositoryError> {
            let mut rows = self.rows.lock().expect("draft mutex");
            let latest = rows
                .iter()
                .filter(|row| {
                    row.project_id == request.project_id && row.draft_id == request.draft_id
                })
                .max_by_key(|row| row.revision)
                .cloned();
            if latest.as_ref().map(|row| row.revision) != request.expected_revision {
                return Err(RepositoryError::integrity("DRAFT_REVISION_CONFLICT"));
            }
            if let Some(row) = latest.as_ref() {
                if row.payload_checksum == request.payload_checksum
                    && row.revision_kind == request.revision_kind
                    && row.parser_version == request.parser_version
                    && row.provider_kind == request.provider_kind
                    && row.provider_model == request.provider_model
                    && row.provider_metadata == request.provider_metadata
                {
                    return Ok(row.clone().into());
                }
            }
            let row = ScriptDraftRevisionRecord {
                id: request.id,
                draft_id: request.draft_id,
                project_id: request.project_id,
                source_id: request.source_id,
                revision: latest.as_ref().map_or(1, |row| row.revision + 1),
                previous_revision_id: latest.map(|row| row.id),
                schema_version: request.schema_version,
                revision_kind: request.revision_kind,
                parser_version: request.parser_version,
                contract_version: request.contract_version,
                provider_kind: request.provider_kind,
                provider_model: request.provider_model,
                provider_metadata: request.provider_metadata,
                payload_checksum: request.payload_checksum,
                summary_json: request.summary_json,
                payload_json: request.payload_json,
                created_at: request.created_at,
            };
            let metadata = row.clone().into();
            rows.push(row);
            Ok(metadata)
        }

        async fn get_revision(
            &self,
            project_id: &str,
            draft_id: &DraftId,
            revision: u64,
        ) -> Result<Option<ScriptDraftRevisionRecord>, RepositoryError> {
            Ok(self
                .rows
                .lock()
                .expect("draft mutex")
                .iter()
                .find(|row| {
                    row.project_id == project_id
                        && row.draft_id == *draft_id
                        && row.revision == revision
                })
                .cloned())
        }

        async fn get_latest(
            &self,
            project_id: &str,
            draft_id: &DraftId,
        ) -> Result<Option<ScriptDraftRevisionMetadata>, RepositoryError> {
            Ok(self
                .rows
                .lock()
                .expect("draft mutex")
                .iter()
                .filter(|row| row.project_id == project_id && row.draft_id == *draft_id)
                .max_by_key(|row| row.revision)
                .cloned()
                .map(Into::into))
        }

        async fn list_latest(
            &self,
            project_id: &str,
            _query: ScriptDraftPageQuery,
        ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, RepositoryError> {
            let rows = self.rows.lock().expect("draft mutex");
            let mut items = Vec::new();
            for row in rows.iter().filter(|row| row.project_id == project_id) {
                if !items
                    .iter()
                    .any(|item: &ScriptDraftRevisionMetadata| item.draft_id == row.draft_id)
                {
                    items.push(row.clone().into());
                }
            }
            Ok(ScriptDraftPage {
                items,
                next_cursor: None,
            })
        }

        async fn list_history(
            &self,
            project_id: &str,
            draft_id: &DraftId,
            _query: ScriptDraftPageQuery,
        ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, RepositoryError> {
            Ok(ScriptDraftPage {
                items: self
                    .rows
                    .lock()
                    .expect("draft mutex")
                    .iter()
                    .filter(|row| row.project_id == project_id && row.draft_id == *draft_id)
                    .cloned()
                    .map(Into::into)
                    .collect(),
                next_cursor: None,
            })
        }
    }

    fn service() -> ScriptDraftService {
        let timestamp = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
        ScriptDraftService::new(
            Arc::new(MemorySources::default()),
            Arc::new(MemoryDrafts::default()),
            Arc::new(FixedClock(timestamp)),
        )
    }

    fn source_request(project_id: &str, filename: &str) -> ScriptSourceCreateRequest {
        ScriptSourceCreateRequest {
            project_id: project_id.to_owned(),
            format: ScriptFormat::Txt,
            source_text: b"\xef\xbb\xbfline\r\nnext".to_vec(),
            original_filename: Some(filename.to_owned()),
        }
    }

    #[test]
    fn raw_checksum_keeps_bom_and_line_endings_distinct() {
        assert_ne!(sha256_hex(b"a\r\nb"), sha256_hex(b"a\nb"));
        assert_eq!(
            sha256_hex(b"\xef\xbb\xbfa"),
            "1951c7860e968e742658b3af34e60741eb4aaf2a8d2ecc3993727016b12e81e8"
        );
    }

    #[test]
    fn canonical_hash_is_stable_and_does_not_expose_source() {
        let first = serde_json::json!({"b": 2, "a": 1});
        let second = serde_json::json!({"a": 1, "b": 2});
        assert_eq!(
            crate::domain::script_draft::canonical_sha256(&first).unwrap(),
            crate::domain::script_draft::canonical_sha256(&second).unwrap()
        );
        let error = invalid("PAYLOAD_TOO_LARGE: source exceeds limit");
        assert!(!error.message.contains("source text"));
    }

    #[test]
    fn same_payload_comparison_ignores_only_revision_identity() {
        let draft = DraftStructureV1::new(DraftId::new(), SourceId::new(), DraftRevisionId::new());
        let mut same = draft.clone();
        same.revision_id = DraftRevisionId::new();
        assert!(same_payload_ignoring_revision_id(&draft, &same));
        same.metadata.insert("edited".to_owned(), "yes".to_owned());
        assert!(!same_payload_ignoring_revision_id(&draft, &same));
    }

    #[tokio::test]
    async fn source_dedupe_is_filename_independent_but_project_isolated() {
        let service = service();
        let first = service
            .create_or_reuse_source(source_request("prj_a", "one.txt"))
            .await
            .unwrap();
        let same = service
            .create_or_reuse_source(source_request("prj_a", "renamed.md"))
            .await
            .unwrap();
        let other = service
            .create_or_reuse_source(source_request("prj_b", "one.txt"))
            .await
            .unwrap();
        assert_eq!(first.id, same.id);
        assert_eq!(first.source_checksum, same.source_checksum);
        assert_eq!(same.original_filename.as_deref(), Some("one.txt"));
        assert_ne!(first.id, other.id);
        assert_eq!(first.original_filename.as_deref(), Some("one.txt"));
    }

    #[tokio::test]
    async fn create_append_latest_history_and_stale_conflict_are_service_safe() {
        let service = service();
        let source = service
            .create_source(source_request("prj_a", "story.txt"))
            .await
            .unwrap();
        let structure =
            DraftStructureV1::new(DraftId::new(), source.id.clone(), DraftRevisionId::new());
        let created = service
            .create_draft(CreateScriptDraftRequest {
                project_id: "prj_a".to_owned(),
                source_id: source.id,
                structure: structure.clone(),
                parser_version: "test-parser".to_owned(),
                provider_metadata: None,
            })
            .await
            .unwrap();
        assert_eq!(created.metadata.revision, 1);
        assert!(created.metadata.summary_json.contains("episodes"));

        let appended = service
            .append_revision(AppendScriptDraftRevisionRequest {
                project_id: "prj_a".to_owned(),
                draft_id: created.metadata.draft_id.clone(),
                expected_revision: 1,
                structure: created.structure.clone(),
                revision_kind: DraftRevisionKind::Review,
                parser_version: "test-parser".to_owned(),
                provider_metadata: None,
            })
            .await
            .unwrap();
        assert_eq!(appended.metadata.revision, 2);
        assert_eq!(
            appended.metadata.previous_revision_id,
            Some(created.metadata.id.clone())
        );
        assert_eq!(
            service
                .latest("prj_a", &created.metadata.draft_id)
                .await
                .unwrap()
                .unwrap()
                .revision,
            2
        );
        assert_eq!(
            service
                .history(
                    "prj_a",
                    &created.metadata.draft_id,
                    ScriptDraftPageQuery::default()
                )
                .await
                .unwrap()
                .items
                .len(),
            2
        );

        let retry = service
            .append_revision(AppendScriptDraftRevisionRequest {
                project_id: "prj_a".to_owned(),
                draft_id: appended.metadata.draft_id.clone(),
                expected_revision: 2,
                structure: appended.structure.clone(),
                revision_kind: DraftRevisionKind::Review,
                parser_version: "test-parser".to_owned(),
                provider_metadata: None,
            })
            .await
            .unwrap();
        assert_eq!(retry.metadata.revision, 2);
        assert_eq!(retry.metadata.id, appended.metadata.id);

        let error = service
            .append_revision(AppendScriptDraftRevisionRequest {
                project_id: "prj_a".to_owned(),
                draft_id: created.metadata.draft_id,
                expected_revision: 1,
                structure,
                revision_kind: DraftRevisionKind::Review,
                parser_version: "test-parser".to_owned(),
                provider_metadata: None,
            })
            .await
            .unwrap_err();
        assert!(error.message.starts_with("DRAFT_REVISION_CONFLICT"));
    }

    #[tokio::test]
    async fn source_capacity_is_rejected_before_repository_write() {
        let service = service();
        let error = service
            .create_source(ScriptSourceCreateRequest {
                project_id: "prj_a".to_owned(),
                format: ScriptFormat::Txt,
                source_text: vec![b'x'; MAX_SOURCE_BYTES + 1],
                original_filename: Some("too-large.txt".to_owned()),
            })
            .await
            .unwrap_err();
        assert!(error.message.starts_with("PAYLOAD_TOO_LARGE"));
    }
}
