use crate::application::ports::RepositoryError;
use crate::domain::script_draft::{ScriptFormat, SourceId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::fmt;

/// The complete source row. `source_text` is intentionally only exposed by
/// the explicit source lookup; list methods return `ScriptSourceMetadata`.
#[derive(Clone, PartialEq, Eq)]
pub struct ScriptSourceRecord {
    pub id: SourceId,
    pub project_id: String,
    pub format: ScriptFormat,
    pub original_filename: Option<String>,
    pub source_checksum: String,
    pub source_bytes: u64,
    pub source_text: Vec<u8>,
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
}

impl fmt::Debug for ScriptSourceRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScriptSourceRecord")
            .field("id", &self.id)
            .field("project_id", &self.project_id)
            .field("format", &self.format)
            .field("original_filename", &self.original_filename)
            .field("source_checksum", &self.source_checksum)
            .field("source_bytes", &self.source_bytes)
            .field("source_text", &"<redacted>")
            .field("schema_version", &self.schema_version)
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptSourceMetadata {
    pub id: SourceId,
    pub project_id: String,
    pub format: ScriptFormat,
    pub original_filename: Option<String>,
    pub source_checksum: String,
    pub source_bytes: u64,
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait ScriptSourceRepository: Send + Sync {
    /// Insert an immutable source, or return the existing row for the same
    /// project/checksum/format identity. A source from another project is
    /// never reused.
    async fn insert_or_reuse(
        &self,
        source: ScriptSourceRecord,
    ) -> Result<ScriptSourceRecord, RepositoryError>;

    async fn find_by_id(
        &self,
        project_id: &str,
        source_id: &SourceId,
    ) -> Result<Option<ScriptSourceRecord>, RepositoryError>;

    async fn find_by_checksum(
        &self,
        project_id: &str,
        format: ScriptFormat,
        source_checksum: &str,
    ) -> Result<Option<ScriptSourceMetadata>, RepositoryError>;

    async fn list_metadata(
        &self,
        project_id: &str,
    ) -> Result<Vec<ScriptSourceMetadata>, RepositoryError>;
}
