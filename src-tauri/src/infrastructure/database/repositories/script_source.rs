use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    RepositoryError, ScriptSourceMetadata, ScriptSourceRecord, ScriptSourceRepository,
};
use crate::domain::script_draft::{ScriptFormat, SourceId};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};

const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct SqliteScriptSourceRepository {
    pool: SqlitePool,
}

impl SqliteScriptSourceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ScriptSourceRepository for SqliteScriptSourceRepository {
    async fn insert_or_reuse(
        &self,
        source: ScriptSourceRecord,
    ) -> Result<ScriptSourceRecord, RepositoryError> {
        validate_source(&source)?;

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "INSERT INTO script_sources
             (id, project_id, format, original_filename, source_checksum, source_bytes, source_text, schema_version, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(project_id, source_checksum, format) DO NOTHING",
        )
        .bind(source.id.as_str())
        .bind(&source.project_id)
        .bind(source.format.as_str())
        .bind(&source.original_filename)
        .bind(&source.source_checksum)
        .bind(i64::try_from(source.source_bytes).map_err(|_| {
            RepositoryError::integrity("SOURCE_BYTES_OUT_OF_RANGE")
        })?)
        .bind(std::str::from_utf8(&source.source_text).map_err(|_| {
            RepositoryError::integrity("INVALID_SOURCE_UTF8")
        })?)
        .bind(i64::from(source.schema_version))
        .bind(format_datetime(source.created_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let row = sqlx::query_as::<_, ScriptSourceRow>(
            "SELECT id, project_id, format, original_filename, source_checksum, source_bytes, source_text,
                    schema_version, created_at
             FROM script_sources
             WHERE project_id = ? AND source_checksum = ? AND format = ?",
        )
        .bind(&source.project_id)
        .bind(&source.source_checksum)
        .bind(source.format.as_str())
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        transaction.commit().await.map_err(map_sqlx_error)?;
        row.try_into_record()
    }

    async fn find_by_id(
        &self,
        project_id: &str,
        source_id: &SourceId,
    ) -> Result<Option<ScriptSourceRecord>, RepositoryError> {
        sqlx::query_as::<_, ScriptSourceRow>(
            "SELECT id, project_id, format, original_filename, source_checksum, source_bytes, source_text,
                    schema_version, created_at
             FROM script_sources
             WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(source_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(ScriptSourceRow::try_into_record)
        .transpose()
    }

    async fn find_by_checksum(
        &self,
        project_id: &str,
        format: ScriptFormat,
        source_checksum: &str,
    ) -> Result<Option<ScriptSourceMetadata>, RepositoryError> {
        validate_checksum(source_checksum)?;
        sqlx::query_as::<_, ScriptSourceMetadataRow>(
            "SELECT id, project_id, format, original_filename, source_checksum, source_bytes, schema_version, created_at
             FROM script_sources
             WHERE project_id = ? AND source_checksum = ? AND format = ?",
        )
        .bind(project_id)
        .bind(source_checksum)
        .bind(format.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(ScriptSourceMetadataRow::try_into_metadata)
        .transpose()
    }

    async fn list_metadata(
        &self,
        project_id: &str,
    ) -> Result<Vec<ScriptSourceMetadata>, RepositoryError> {
        let rows = sqlx::query_as::<_, ScriptSourceMetadataRow>(
            "SELECT id, project_id, format, original_filename, source_checksum, source_bytes, schema_version, created_at
             FROM script_sources
             WHERE project_id = ?
             ORDER BY created_at DESC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ScriptSourceMetadataRow::try_into_metadata)
            .collect()
    }
}

fn validate_source(source: &ScriptSourceRecord) -> Result<(), RepositoryError> {
    if source.project_id.trim().is_empty() {
        return Err(RepositoryError::integrity("PROJECT_MISMATCH"));
    }
    if source.source_bytes != source.source_text.len() as u64 {
        return Err(RepositoryError::integrity("SOURCE_LENGTH_MISMATCH"));
    }
    if source.source_bytes > MAX_SOURCE_BYTES {
        return Err(RepositoryError::integrity("PAYLOAD_TOO_LARGE"));
    }
    std::str::from_utf8(&source.source_text)
        .map_err(|_| RepositoryError::integrity("INVALID_SOURCE_UTF8"))?;
    validate_checksum(&source.source_checksum)?;
    if sha256_hex(&source.source_text) != source.source_checksum {
        return Err(RepositoryError::integrity("SOURCE_CHECKSUM_MISMATCH"));
    }
    if source.schema_version == 0 {
        return Err(RepositoryError::integrity("SCHEMA_VERSION_UNSUPPORTED"));
    }
    Ok(())
}

fn validate_checksum(checksum: &str) -> Result<(), RepositoryError> {
    if checksum.len() != 64
        || checksum
            .as_bytes()
            .iter()
            .any(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(RepositoryError::integrity("INVALID_SOURCE_CHECKSUM"));
    }
    Ok(())
}

#[derive(FromRow)]
struct ScriptSourceRow {
    id: String,
    project_id: String,
    format: String,
    original_filename: Option<String>,
    source_checksum: String,
    source_bytes: i64,
    source_text: String,
    schema_version: i64,
    created_at: String,
}

impl ScriptSourceRow {
    fn try_into_record(self) -> Result<ScriptSourceRecord, RepositoryError> {
        let source_text = self.source_text.into_bytes();
        let id = SourceId::parse(self.id)
            .map_err(|_| RepositoryError::integrity("INVALID_SOURCE_ID"))?;
        let format = ScriptFormat::try_from_storage(&self.format)
            .map_err(|_| RepositoryError::integrity("INVALID_SCRIPT_FORMAT"))?;
        let source_bytes = u64::try_from(self.source_bytes)
            .map_err(|_| RepositoryError::serialization("source_bytes", "negative value"))?;
        if source_bytes != source_text.len() as u64 {
            return Err(RepositoryError::integrity("SOURCE_LENGTH_MISMATCH"));
        }
        std::str::from_utf8(&source_text)
            .map_err(|_| RepositoryError::integrity("INVALID_SOURCE_UTF8"))?;
        if sha256_hex(&source_text) != self.source_checksum {
            return Err(RepositoryError::integrity("SOURCE_CHECKSUM_MISMATCH"));
        }
        validate_checksum(&self.source_checksum)?;
        let schema_version = u32::try_from(self.schema_version)
            .map_err(|_| RepositoryError::serialization("source schema_version", "out of range"))?;
        if schema_version == 0 {
            return Err(RepositoryError::integrity("SCHEMA_VERSION_UNSUPPORTED"));
        }
        Ok(ScriptSourceRecord {
            id,
            project_id: self.project_id,
            format,
            original_filename: self.original_filename,
            source_checksum: self.source_checksum,
            source_bytes,
            source_text,
            schema_version,
            created_at: parse_datetime("source created_at", &self.created_at)?,
        })
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(FromRow)]
struct ScriptSourceMetadataRow {
    id: String,
    project_id: String,
    format: String,
    original_filename: Option<String>,
    source_checksum: String,
    source_bytes: i64,
    schema_version: i64,
    created_at: String,
}

impl ScriptSourceMetadataRow {
    fn try_into_metadata(self) -> Result<ScriptSourceMetadata, RepositoryError> {
        // Metadata conversion must not read or expose the source TEXT. This row does
        // not select it and never allocates source-sized memory.
        let id = SourceId::parse(self.id)
            .map_err(|_| RepositoryError::integrity("INVALID_SOURCE_ID"))?;
        let format = ScriptFormat::try_from_storage(&self.format)
            .map_err(|_| RepositoryError::integrity("INVALID_SCRIPT_FORMAT"))?;
        let source_bytes = u64::try_from(self.source_bytes)
            .map_err(|_| RepositoryError::serialization("source_bytes", "negative value"))?;
        validate_checksum(&self.source_checksum)?;
        let schema_version = u32::try_from(self.schema_version)
            .map_err(|_| RepositoryError::serialization("source schema_version", "out of range"))?;
        if schema_version == 0 {
            return Err(RepositoryError::integrity("SCHEMA_VERSION_UNSUPPORTED"));
        }
        Ok(ScriptSourceMetadata {
            id,
            project_id: self.project_id,
            format,
            original_filename: self.original_filename,
            source_checksum: self.source_checksum,
            source_bytes,
            schema_version,
            created_at: parse_datetime("source created_at", &self.created_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteScriptSourceRepository;
    use crate::application::ports::{ScriptSourceRecord, ScriptSourceRepository};
    use crate::domain::script_draft::{sha256_hex, ScriptFormat, SourceId};
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn source_identity_is_project_scoped_and_preserves_raw_utf8_text() {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Project 2', 'C:/project-2', ?, ?)",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("second project should insert");

        let raw = b"\xEF\xBB\xBFfirst\r\nsecond\n".to_vec();
        let checksum = sha256_hex(&raw);
        let repository = SqliteScriptSourceRepository::new(pool.clone());
        let first_id = SourceId::new();
        let stored = repository
            .insert_or_reuse(ScriptSourceRecord {
                id: first_id.clone(),
                project_id: "project-1".to_owned(),
                format: ScriptFormat::Txt,
                original_filename: Some("story.txt".to_owned()),
                source_checksum: checksum.clone(),
                source_bytes: raw.len() as u64,
                source_text: raw.clone(),
                schema_version: 1,
                created_at: Utc::now(),
            })
            .await
            .expect("source should insert");
        assert_eq!(stored.id, first_id);
        assert_eq!(
            repository
                .find_by_id("project-1", &stored.id)
                .await
                .unwrap()
                .unwrap()
                .source_text,
            raw
        );

        let reused = repository
            .insert_or_reuse(ScriptSourceRecord {
                id: SourceId::new(),
                project_id: "project-1".to_owned(),
                format: ScriptFormat::Txt,
                original_filename: Some("renamed.txt".to_owned()),
                source_checksum: checksum.clone(),
                source_bytes: stored.source_bytes,
                source_text: stored.source_text.clone(),
                schema_version: 1,
                created_at: Utc::now(),
            })
            .await;
        assert_eq!(reused.unwrap().id, stored.id);

        let isolated = repository
            .insert_or_reuse(ScriptSourceRecord {
                id: SourceId::new(),
                project_id: "project-2".to_owned(),
                format: ScriptFormat::Txt,
                original_filename: Some("story.txt".to_owned()),
                source_checksum: checksum,
                source_bytes: raw.len() as u64,
                source_text: raw,
                schema_version: 1,
                created_at: Utc::now(),
            })
            .await
            .expect("same content in another project should be isolated");
        assert_ne!(isolated.id, stored.id);
        let metadata = repository.list_metadata("project-1").await.unwrap();
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].original_filename.as_deref(), Some("story.txt"));
        pool.close().await;
    }
}
