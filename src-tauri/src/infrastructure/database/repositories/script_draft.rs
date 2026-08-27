use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    InsertScriptDraftRevision, RepositoryError, ScriptDraftPage, ScriptDraftPageQuery,
    ScriptDraftRepository, ScriptDraftRevisionMetadata, ScriptDraftRevisionRecord,
};
use crate::domain::script_draft::{
    canonical_json as domain_canonical_json, canonical_sha256, validate_payload,
    validate_structure, DraftId, DraftRevisionId, DraftRevisionKind, DraftStructureV1,
    ProviderMetadata, SourceId,
};
use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 200;
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct SqliteScriptDraftRepository {
    pool: SqlitePool,
}

impl SqliteScriptDraftRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ScriptDraftRepository for SqliteScriptDraftRepository {
    async fn insert_revision(
        &self,
        request: InsertScriptDraftRevision,
    ) -> Result<ScriptDraftRevisionMetadata, RepositoryError> {
        validate_request(&request)?;

        let provider_metadata_json = request
            .provider_metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| RepositoryError::serialization("provider metadata", "invalid value"))?;

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;

        let source = sqlx::query_as::<_, SourceForValidation>(
            "SELECT source_checksum, source_bytes, source_text
             FROM script_sources WHERE id = ? AND project_id = ? LIMIT 1",
        )
        .bind(request.source_id.as_str())
        .bind(&request.project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        let source = match source {
            Some(source) => source,
            None => {
                return Err(RepositoryError::integrity(
                    "PROJECT_MISMATCH_OR_SOURCE_NOT_FOUND",
                ))
            }
        };
        if source.source_bytes != source.source_text.len() as i64
            || sha256_hex(source.source_text.as_bytes()) != source.source_checksum
        {
            return Err(RepositoryError::integrity("SOURCE_CHECKSUM_MISMATCH"));
        }
        validate_structure_payload(&request, source.source_text.as_bytes())?;

        let latest = sqlx::query_as::<_, DraftMetadataRow>(
            "SELECT id, draft_id, project_id, source_id, revision, previous_revision_id,
                    schema_version, revision_kind, parser_version, contract_version,
                    provider_kind, provider_model, provider_metadata_json,
                    payload_checksum, summary_json, created_at
             FROM script_import_drafts
             WHERE project_id = ? AND draft_id = ?
             ORDER BY revision DESC
             LIMIT 1",
        )
        .bind(&request.project_id)
        .bind(request.draft_id.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let latest_revision = latest.as_ref().map(|row| row.revision);
        let latest_id = latest.as_ref().map(|row| row.id.clone());
        let expected_revision = request
            .expected_revision
            .map(|value| {
                i64::try_from(value)
                    .map_err(|_| RepositoryError::integrity("REVISION_OUT_OF_RANGE"))
            })
            .transpose()?;
        if latest_revision != expected_revision {
            return Err(revision_conflict(
                request.expected_revision,
                latest_revision,
            ));
        }

        if let Some(latest) = latest {
            let latest_metadata = latest.try_into_metadata()?;
            if same_semantic_revision(
                &latest_metadata,
                &request,
                provider_metadata_json.as_deref(),
            ) {
                transaction.commit().await.map_err(map_sqlx_error)?;
                return Ok(latest_metadata);
            }
        }

        let payload_revision_id = payload_revision_id(&request.payload_json)?;
        if payload_revision_id.as_deref() != Some(request.id.as_str()) {
            return Err(RepositoryError::integrity("DRAFT_REVISION_ID_MISMATCH"));
        }

        let revision = latest_revision.map(|value| value + 1).unwrap_or(1);
        let previous_revision_id: Option<String> = if revision == 1 {
            None
        } else {
            let adjacent_id: Option<String> = sqlx::query_scalar(
                "SELECT id FROM script_import_drafts
                 WHERE project_id = ? AND draft_id = ? AND revision = ?",
            )
            .bind(&request.project_id)
            .bind(request.draft_id.as_str())
            .bind(revision - 1)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            if adjacent_id.as_deref() != latest_id.as_deref() {
                return Err(RepositoryError::integrity("DRAFT_REVISION_CHAIN_BROKEN"));
            }
            adjacent_id
        };

        sqlx::query(
            "INSERT INTO script_import_drafts
             (id, draft_id, project_id, source_id, revision, previous_revision_id,
              schema_version, revision_kind, parser_version, contract_version,
              provider_kind, provider_model, provider_metadata_json,
              payload_checksum, summary_json, payload_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(request.id.as_str())
        .bind(request.draft_id.as_str())
        .bind(&request.project_id)
        .bind(request.source_id.as_str())
        .bind(revision)
        .bind(previous_revision_id)
        .bind(i64::from(request.schema_version))
        .bind(revision_kind_to_storage(request.revision_kind))
        .bind(&request.parser_version)
        .bind(i64::from(request.contract_version))
        .bind(&request.provider_kind)
        .bind(&request.provider_model)
        .bind(provider_metadata_json)
        .bind(&request.payload_checksum)
        .bind(&request.summary_json)
        .bind(&request.payload_json)
        .bind(format_datetime(request.created_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let inserted = sqlx::query_as::<_, DraftMetadataRow>(
            "SELECT id, draft_id, project_id, source_id, revision, previous_revision_id,
                    schema_version, revision_kind, parser_version, contract_version,
                    provider_kind, provider_model, provider_metadata_json,
                    payload_checksum, summary_json, created_at
             FROM script_import_drafts
             WHERE id = ?",
        )
        .bind(request.id.as_str())
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        inserted.try_into_metadata()
    }

    async fn get_revision(
        &self,
        project_id: &str,
        draft_id: &DraftId,
        revision: u64,
    ) -> Result<Option<ScriptDraftRevisionRecord>, RepositoryError> {
        let revision = i64::try_from(revision)
            .map_err(|_| RepositoryError::integrity("REVISION_OUT_OF_RANGE"))?;
        sqlx::query_as::<_, DraftRecordRow>(
            "SELECT id, draft_id, project_id, source_id, revision, previous_revision_id,
                    schema_version, revision_kind, parser_version, contract_version,
                    provider_kind, provider_model, provider_metadata_json,
                    payload_checksum, summary_json, payload_json, created_at
             FROM script_import_drafts
             WHERE project_id = ? AND draft_id = ? AND revision = ?",
        )
        .bind(project_id)
        .bind(draft_id.as_str())
        .bind(revision)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(DraftRecordRow::try_into_record)
        .transpose()
    }

    async fn get_latest(
        &self,
        project_id: &str,
        draft_id: &DraftId,
    ) -> Result<Option<ScriptDraftRevisionMetadata>, RepositoryError> {
        sqlx::query_as::<_, DraftMetadataRow>(
            "SELECT id, draft_id, project_id, source_id, revision, previous_revision_id,
                    schema_version, revision_kind, parser_version, contract_version,
                    provider_kind, provider_model, provider_metadata_json,
                    payload_checksum, summary_json, created_at
             FROM script_import_drafts
             WHERE project_id = ? AND draft_id = ?
             ORDER BY revision DESC
             LIMIT 1",
        )
        .bind(project_id)
        .bind(draft_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(DraftMetadataRow::try_into_metadata)
        .transpose()
    }

    async fn list_latest(
        &self,
        project_id: &str,
        query: ScriptDraftPageQuery,
    ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, RepositoryError> {
        let limit = page_size(query.limit);
        let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

        let mut sql = String::from(
            "SELECT r.id, r.draft_id, r.project_id, r.source_id, r.revision,
                    r.previous_revision_id, r.schema_version, r.revision_kind,
                    r.parser_version, r.contract_version, r.provider_kind, r.provider_model,
                    r.provider_metadata_json,
                    r.payload_checksum, r.summary_json, r.created_at
             FROM script_import_drafts r
             WHERE r.project_id = ?
               AND r.revision = (SELECT MAX(r2.revision) FROM script_import_drafts r2
                                 WHERE r2.project_id = r.project_id AND r2.draft_id = r.draft_id)",
        );
        if cursor.is_some() {
            sql.push_str(
                " AND (r.created_at < ? OR (r.created_at = ? AND r.draft_id > ?)
                       OR (r.created_at = ? AND r.draft_id = ? AND r.revision < ?)
                       OR (r.created_at = ? AND r.draft_id = ? AND r.revision = ? AND r.id > ?))",
            );
        }
        sql.push_str(
            " ORDER BY r.created_at DESC, r.draft_id ASC, r.revision DESC, r.id ASC LIMIT ?",
        );

        let mut query_builder = sqlx::query_as::<_, DraftMetadataRow>(&sql).bind(project_id);
        if let Some(cursor) = cursor {
            query_builder = query_builder
                .bind(cursor.created_at.clone())
                .bind(cursor.created_at.clone())
                .bind(cursor.draft_id.clone())
                .bind(cursor.created_at.clone())
                .bind(cursor.draft_id.clone())
                .bind(cursor.revision)
                .bind(cursor.created_at.clone())
                .bind(cursor.draft_id.clone())
                .bind(cursor.revision)
                .bind(cursor.id.clone());
        }
        let mut rows = query_builder
            .bind(i64::from(limit) + 1)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        let has_more = rows.len() > limit as usize;
        if has_more {
            rows.truncate(limit as usize);
        }
        let next_cursor = rows
            .last()
            .map(|row| encode_cursor(&row.created_at, &row.draft_id, row.revision, &row.id))
            .filter(|_| has_more);
        let items = rows
            .into_iter()
            .map(DraftMetadataRow::try_into_metadata)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ScriptDraftPage { items, next_cursor })
    }

    async fn list_history(
        &self,
        project_id: &str,
        draft_id: &DraftId,
        query: ScriptDraftPageQuery,
    ) -> Result<ScriptDraftPage<ScriptDraftRevisionMetadata>, RepositoryError> {
        let limit = page_size(query.limit);
        let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

        let mut sql = String::from(
            "SELECT id, draft_id, project_id, source_id, revision, previous_revision_id,
                    schema_version, revision_kind, parser_version, contract_version,
                    provider_kind, provider_model, provider_metadata_json,
                    payload_checksum, summary_json, created_at
             FROM script_import_drafts
             WHERE project_id = ? AND draft_id = ?",
        );
        if cursor.is_some() {
            sql.push_str(
                " AND (created_at < ? OR (created_at = ? AND draft_id > ?)
                       OR (created_at = ? AND draft_id = ? AND revision < ?)
                       OR (created_at = ? AND draft_id = ? AND revision = ? AND id > ?))",
            );
        }
        sql.push_str(" ORDER BY created_at DESC, draft_id ASC, revision DESC, id ASC LIMIT ?");
        let mut query_builder = sqlx::query_as::<_, DraftMetadataRow>(&sql)
            .bind(project_id)
            .bind(draft_id.as_str());
        if let Some(cursor) = cursor {
            query_builder = query_builder
                .bind(cursor.created_at.clone())
                .bind(cursor.created_at.clone())
                .bind(cursor.draft_id.clone())
                .bind(cursor.created_at.clone())
                .bind(cursor.draft_id.clone())
                .bind(cursor.revision)
                .bind(cursor.created_at.clone())
                .bind(cursor.draft_id.clone())
                .bind(cursor.revision)
                .bind(cursor.id.clone());
        }
        let mut rows = query_builder
            .bind(i64::from(limit) + 1)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        let has_more = rows.len() > limit as usize;
        if has_more {
            rows.truncate(limit as usize);
        }
        let next_cursor = rows
            .last()
            .map(|row| encode_cursor(&row.created_at, &row.draft_id, row.revision, &row.id))
            .filter(|_| has_more);
        let items = rows
            .into_iter()
            .map(DraftMetadataRow::try_into_metadata)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ScriptDraftPage { items, next_cursor })
    }
}

fn validate_request(request: &InsertScriptDraftRevision) -> Result<(), RepositoryError> {
    if request.project_id.trim().is_empty() || request.parser_version.trim().is_empty() {
        return Err(RepositoryError::integrity("INVALID_DRAFT_METADATA"));
    }
    if request.schema_version == 0 || request.contract_version == 0 {
        return Err(RepositoryError::integrity("SCHEMA_VERSION_UNSUPPORTED"));
    }
    if request.payload_json.len() > MAX_PAYLOAD_BYTES {
        return Err(RepositoryError::integrity("PAYLOAD_TOO_LARGE"));
    }
    let payload: Value = serde_json::from_str(&request.payload_json)
        .map_err(|_| RepositoryError::serialization("draft payload", "invalid JSON"))?;
    let object = payload
        .as_object()
        .ok_or_else(|| RepositoryError::integrity("INVALID_DRAFT_PAYLOAD_ROOT"))?;
    let schema_version = object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| RepositoryError::integrity("SCHEMA_VERSION_UNSUPPORTED"))?;
    if schema_version != u64::from(request.schema_version) {
        return Err(RepositoryError::integrity("SCHEMA_VERSION_UNSUPPORTED"));
    }
    if let Some(value) = object.get("draftId").and_then(Value::as_str) {
        if value != request.draft_id.as_str() {
            return Err(RepositoryError::integrity("DRAFT_ID_MISMATCH"));
        }
    }
    if let Some(value) = object.get("sourceId").and_then(Value::as_str) {
        if value != request.source_id.as_str() {
            return Err(RepositoryError::integrity("SOURCE_ID_MISMATCH"));
        }
    }
    let canonical = domain_canonical_json(&payload)
        .map_err(|_| RepositoryError::serialization("draft payload", "canonicalization failed"))?;
    let checksum = sha256_hex(canonical.as_bytes());
    if checksum != request.payload_checksum {
        return Err(RepositoryError::integrity("PAYLOAD_CHECKSUM_MISMATCH"));
    }
    let _: Value = serde_json::from_str(&request.summary_json)
        .map_err(|_| RepositoryError::serialization("draft summary", "invalid JSON"))?;
    validate_checksum(&request.payload_checksum)
}

fn validate_checksum(value: &str) -> Result<(), RepositoryError> {
    if value.len() != 64
        || value
            .as_bytes()
            .iter()
            .any(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(RepositoryError::integrity("INVALID_PAYLOAD_CHECKSUM"));
    }
    Ok(())
}

fn same_semantic_revision(
    latest: &ScriptDraftRevisionMetadata,
    request: &InsertScriptDraftRevision,
    provider_metadata_json: Option<&str>,
) -> bool {
    latest.draft_id == request.draft_id
        && latest.source_id == request.source_id
        && latest.schema_version == request.schema_version
        && latest.revision_kind == request.revision_kind
        && latest.parser_version == request.parser_version
        && latest.contract_version == request.contract_version
        && latest.provider_kind == request.provider_kind
        && latest.provider_model == request.provider_model
        && latest.payload_checksum == request.payload_checksum
        && latest.summary_json == request.summary_json
        && provider_json(latest.provider_metadata.as_ref()).as_deref() == provider_metadata_json
}

fn provider_json(value: Option<&ProviderMetadata>) -> Option<String> {
    value.and_then(|metadata| serde_json::to_string(metadata).ok())
}

fn revision_conflict(expected: Option<u64>, actual: Option<i64>) -> RepositoryError {
    RepositoryError::integrity(format!(
        "DRAFT_REVISION_CONFLICT: expected={expected:?} actual={actual:?}"
    ))
}

fn revision_kind_to_storage(kind: DraftRevisionKind) -> &'static str {
    kind.as_str()
}

fn revision_kind_from_storage(value: &str) -> Result<DraftRevisionKind, RepositoryError> {
    DraftRevisionKind::try_from_storage(value)
        .map_err(|_| RepositoryError::integrity("INVALID_DRAFT_REVISION_KIND"))
}

fn validate_structure_payload(
    request: &InsertScriptDraftRevision,
    raw_source: &[u8],
) -> Result<(), RepositoryError> {
    let payload: Value = serde_json::from_str(&request.payload_json)
        .map_err(|_| RepositoryError::serialization("draft payload", "invalid JSON"))?;
    let structure: DraftStructureV1 = validate_payload(&payload, request.schema_version)
        .map_err(|error| RepositoryError::integrity(error.code()))?;
    if structure.draft_id != request.draft_id || structure.source_id != request.source_id {
        return Err(RepositoryError::integrity("DRAFT_SOURCE_ID_MISMATCH"));
    }
    validate_structure(&structure, raw_source, request.schema_version)
        .map_err(|error| RepositoryError::integrity(error.code()))?;
    if canonical_sha256(&structure)
        .map_err(|_| RepositoryError::serialization("draft payload", "canonicalization failed"))?
        != request.payload_checksum
    {
        return Err(RepositoryError::integrity("PAYLOAD_CHECKSUM_MISMATCH"));
    }
    Ok(())
}

fn payload_revision_id(payload_json: &str) -> Result<Option<String>, RepositoryError> {
    let payload: Value = serde_json::from_str(payload_json)
        .map_err(|_| RepositoryError::serialization("draft payload", "invalid JSON"))?;
    Ok(payload
        .get("revisionId")
        .and_then(Value::as_str)
        .map(str::to_owned))
}

#[derive(FromRow)]
struct SourceForValidation {
    source_checksum: String,
    source_bytes: i64,
    source_text: String,
}

#[derive(FromRow)]
struct DraftMetadataRow {
    id: String,
    draft_id: String,
    project_id: String,
    source_id: String,
    revision: i64,
    previous_revision_id: Option<String>,
    schema_version: i64,
    revision_kind: String,
    parser_version: String,
    contract_version: i64,
    provider_kind: Option<String>,
    provider_model: Option<String>,
    provider_metadata_json: Option<String>,
    payload_checksum: String,
    summary_json: String,
    created_at: String,
}

impl DraftMetadataRow {
    fn try_into_metadata(self) -> Result<ScriptDraftRevisionMetadata, RepositoryError> {
        let id = DraftRevisionId::parse(self.id)
            .map_err(|_| RepositoryError::integrity("INVALID_DRAFT_REVISION_ID"))?;
        let draft_id = DraftId::parse(self.draft_id)
            .map_err(|_| RepositoryError::integrity("INVALID_DRAFT_ID"))?;
        let source_id = SourceId::parse(self.source_id)
            .map_err(|_| RepositoryError::integrity("INVALID_SOURCE_ID"))?;
        let revision = u64::try_from(self.revision)
            .map_err(|_| RepositoryError::integrity("INVALID_DRAFT_REVISION"))?;
        if revision == 0 {
            return Err(RepositoryError::integrity("INVALID_DRAFT_REVISION"));
        }
        let schema_version = u32::try_from(self.schema_version)
            .map_err(|_| RepositoryError::integrity("SCHEMA_VERSION_UNSUPPORTED"))?;
        let contract_version = u32::try_from(self.contract_version)
            .map_err(|_| RepositoryError::integrity("CONTRACT_VERSION_UNSUPPORTED"))?;
        if schema_version == 0 || contract_version == 0 {
            return Err(RepositoryError::integrity("SCHEMA_VERSION_UNSUPPORTED"));
        }
        let provider_metadata = self
            .provider_metadata_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|_| RepositoryError::serialization("provider metadata", "invalid JSON"))?;
        validate_checksum(&self.payload_checksum)?;
        Ok(ScriptDraftRevisionMetadata {
            id,
            draft_id,
            project_id: self.project_id,
            source_id,
            revision,
            previous_revision_id: self
                .previous_revision_id
                .map(|value| {
                    DraftRevisionId::parse(value)
                        .map_err(|_| RepositoryError::integrity("INVALID_PREVIOUS_REVISION_ID"))
                })
                .transpose()?,
            schema_version,
            revision_kind: revision_kind_from_storage(&self.revision_kind)?,
            parser_version: self.parser_version,
            contract_version,
            provider_kind: self.provider_kind,
            provider_model: self.provider_model,
            provider_metadata,
            payload_checksum: self.payload_checksum,
            summary_json: self.summary_json,
            created_at: parse_datetime("draft created_at", &self.created_at)?,
        })
    }
}

#[derive(FromRow)]
struct DraftRecordRow {
    id: String,
    draft_id: String,
    project_id: String,
    source_id: String,
    revision: i64,
    previous_revision_id: Option<String>,
    schema_version: i64,
    revision_kind: String,
    parser_version: String,
    contract_version: i64,
    provider_kind: Option<String>,
    provider_model: Option<String>,
    provider_metadata_json: Option<String>,
    payload_checksum: String,
    summary_json: String,
    payload_json: String,
    created_at: String,
}

impl DraftRecordRow {
    fn try_into_record(self) -> Result<ScriptDraftRevisionRecord, RepositoryError> {
        let metadata = DraftMetadataRow {
            id: self.id,
            draft_id: self.draft_id,
            project_id: self.project_id,
            source_id: self.source_id,
            revision: self.revision,
            previous_revision_id: self.previous_revision_id,
            schema_version: self.schema_version,
            revision_kind: self.revision_kind,
            parser_version: self.parser_version,
            contract_version: self.contract_version,
            provider_kind: self.provider_kind,
            provider_model: self.provider_model,
            provider_metadata_json: self.provider_metadata_json,
            payload_checksum: self.payload_checksum,
            summary_json: self.summary_json,
            created_at: self.created_at,
        }
        .try_into_metadata()?;
        validate_request(&InsertScriptDraftRevision {
            id: metadata.id.clone(),
            draft_id: metadata.draft_id.clone(),
            project_id: metadata.project_id.clone(),
            source_id: metadata.source_id.clone(),
            expected_revision: Some(metadata.revision.saturating_sub(1)),
            schema_version: metadata.schema_version,
            revision_kind: metadata.revision_kind,
            parser_version: metadata.parser_version.clone(),
            contract_version: metadata.contract_version,
            provider_kind: metadata.provider_kind.clone(),
            provider_model: metadata.provider_model.clone(),
            provider_metadata: metadata.provider_metadata.clone(),
            payload_checksum: metadata.payload_checksum.clone(),
            summary_json: metadata.summary_json.clone(),
            payload_json: self.payload_json.clone(),
            created_at: metadata.created_at,
        })?;
        Ok(ScriptDraftRevisionRecord {
            id: metadata.id,
            draft_id: metadata.draft_id,
            project_id: metadata.project_id,
            source_id: metadata.source_id,
            revision: metadata.revision,
            previous_revision_id: metadata.previous_revision_id,
            schema_version: metadata.schema_version,
            revision_kind: metadata.revision_kind,
            parser_version: metadata.parser_version,
            contract_version: metadata.contract_version,
            provider_kind: metadata.provider_kind,
            provider_model: metadata.provider_model,
            provider_metadata: metadata.provider_metadata,
            payload_checksum: metadata.payload_checksum,
            summary_json: metadata.summary_json,
            payload_json: self.payload_json,
            created_at: metadata.created_at,
        })
    }
}

#[derive(Clone, Debug)]
struct Cursor {
    created_at: String,
    draft_id: String,
    revision: i64,
    id: String,
}

fn page_size(requested: u32) -> u32 {
    if requested == 0 {
        DEFAULT_PAGE_SIZE
    } else {
        requested.clamp(1, MAX_PAGE_SIZE)
    }
}

fn encode_cursor(created_at: &str, draft_id: &str, revision: i64, id: &str) -> String {
    let value = format!("{created_at}\n{draft_id}\n{revision}\n{id}");
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_cursor(value: &str) -> Result<Cursor, RepositoryError> {
    if value.is_empty() || value.len() % 2 != 0 || value.len() > 4096 {
        return Err(RepositoryError::integrity("INVALID_DRAFT_CURSOR"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks_exact(2) {
        let high = hex_value(chunk[0])
            .ok_or_else(|| RepositoryError::integrity("INVALID_DRAFT_CURSOR"))?;
        let low = hex_value(chunk[1])
            .ok_or_else(|| RepositoryError::integrity("INVALID_DRAFT_CURSOR"))?;
        bytes.push((high << 4) | low);
    }
    let decoded =
        String::from_utf8(bytes).map_err(|_| RepositoryError::integrity("INVALID_DRAFT_CURSOR"))?;
    let mut parts = decoded.split('\n');
    let cursor = Cursor {
        created_at: parts.next().unwrap_or_default().to_owned(),
        draft_id: parts.next().unwrap_or_default().to_owned(),
        revision: parts
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| RepositoryError::integrity("INVALID_DRAFT_CURSOR"))?,
        id: parts.next().unwrap_or_default().to_owned(),
    };
    if parts.next().is_some()
        || cursor.created_at.is_empty()
        || cursor.draft_id.is_empty()
        || cursor.id.is_empty()
        || cursor.revision <= 0
    {
        return Err(RepositoryError::integrity("INVALID_DRAFT_CURSOR"));
    }
    Ok(cursor)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn sha256_hex(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::SqliteScriptDraftRepository;
    use crate::application::ports::{
        InsertScriptDraftRevision, ScriptDraftPageQuery, ScriptDraftRepository, ScriptSourceRecord,
        ScriptSourceRepository,
    };
    use crate::domain::script_draft::{
        canonical_json, canonical_sha256, DraftId, DraftRevisionId, DraftRevisionKind,
        DraftStructureV1, ScriptFormat, SourceId,
    };
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use tempfile::tempdir;

    async fn setup() -> (SqliteScriptDraftRepository, SourceId) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        // Keep the temporary directory alive through the test by leaking only
        // its small path guard; the pool owns the database connection.
        let _ = Box::leak(Box::new(directory));
        test_support::seed_task_dependencies(&pool).await;
        let source_id = SourceId::new();
        let raw = b"draft source".to_vec();
        let source_repo =
            crate::infrastructure::database::repositories::SqliteScriptSourceRepository::new(
                pool.clone(),
            );
        source_repo
            .insert_or_reuse(ScriptSourceRecord {
                id: source_id.clone(),
                project_id: "project-1".to_owned(),
                format: ScriptFormat::Txt,
                original_filename: Some("draft.txt".to_owned()),
                source_checksum: crate::domain::script_draft::sha256_hex(&raw),
                source_bytes: raw.len() as u64,
                source_text: raw,
                schema_version: 1,
                created_at: Utc::now(),
            })
            .await
            .expect("source fixture should insert");
        (SqliteScriptDraftRepository::new(pool), source_id)
    }

    fn request(
        draft_id: DraftId,
        source_id: SourceId,
        revision_id: DraftRevisionId,
        expected_revision: Option<u64>,
        kind: DraftRevisionKind,
        structure: &DraftStructureV1,
    ) -> InsertScriptDraftRevision {
        let payload_json = canonical_json(structure).unwrap();
        InsertScriptDraftRevision {
            id: revision_id,
            draft_id,
            project_id: "project-1".to_owned(),
            source_id,
            expected_revision,
            schema_version: 1,
            revision_kind: kind,
            parser_version: "deterministic-1".to_owned(),
            contract_version: 1,
            provider_kind: None,
            provider_model: None,
            provider_metadata: None,
            payload_checksum: canonical_sha256(structure).unwrap(),
            summary_json: r#"{"episodes":0,"scenes":0,"shots":0}"#.to_owned(),
            payload_json,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn revisions_are_immutable_chained_and_stale_writes_conflict() {
        let (repository, source_id) = setup().await;
        let draft_id = DraftId::new();
        let revision_one_id = DraftRevisionId::new();
        let structure_one =
            DraftStructureV1::new(draft_id.clone(), source_id.clone(), revision_one_id.clone());
        let first = repository
            .insert_revision(request(
                draft_id.clone(),
                source_id.clone(),
                revision_one_id,
                None,
                DraftRevisionKind::Parsed,
                &structure_one,
            ))
            .await
            .expect("first revision should insert");
        assert_eq!(first.revision, 1);
        assert!(first.previous_revision_id.is_none());

        let revision_two_id = DraftRevisionId::new();
        let structure_two =
            DraftStructureV1::new(draft_id.clone(), source_id.clone(), revision_two_id.clone());
        let second = repository
            .insert_revision(request(
                draft_id.clone(),
                source_id.clone(),
                revision_two_id,
                Some(1),
                DraftRevisionKind::UserEdit,
                &structure_two,
            ))
            .await
            .expect("second revision should insert");
        assert_eq!(second.revision, 2);
        assert_eq!(second.previous_revision_id, Some(first.id.clone()));
        let history = repository
            .list_history(
                "project-1",
                &draft_id,
                ScriptDraftPageQuery {
                    cursor: None,
                    limit: 50,
                },
            )
            .await
            .expect("history should load");
        assert_eq!(history.items.len(), 2);
        assert!(repository
            .get_latest("project-1", &draft_id)
            .await
            .unwrap()
            .unwrap()
            .summary_json
            .contains("episodes"));
        assert!(repository
            .get_revision("project-1", &draft_id, 2)
            .await
            .unwrap()
            .unwrap()
            .payload_json
            .contains("schemaVersion"));

        let stale = repository
            .insert_revision(request(
                draft_id.clone(),
                source_id.clone(),
                DraftRevisionId::new(),
                Some(1),
                DraftRevisionKind::UserEdit,
                &structure_two,
            ))
            .await
            .expect_err("stale append should conflict");
        assert!(stale.to_string().contains("DRAFT_REVISION_CONFLICT"));

        let no_op = repository
            .insert_revision(request(
                draft_id.clone(),
                source_id,
                DraftRevisionId::new(),
                Some(2),
                DraftRevisionKind::UserEdit,
                &structure_two,
            ))
            .await
            .expect("same payload should be a no-op");
        assert_eq!(no_op.id, second.id);
        assert_eq!(no_op.revision, 2);
    }
}
