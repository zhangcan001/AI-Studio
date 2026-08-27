use crate::domain::script_draft::diagnostic::Diagnostic;
use crate::domain::script_draft::ids::SourceId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, fmt};

pub const SOURCE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScriptFormat {
    Txt,
    Markdown,
    Json,
}

impl ScriptFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Txt => "TXT",
            Self::Markdown => "MARKDOWN",
            Self::Json => "JSON",
        }
    }

    pub fn try_from_storage(value: &str) -> Result<Self, SourceDomainError> {
        match value {
            "TXT" => Ok(Self::Txt),
            "MARKDOWN" => Ok(Self::Markdown),
            "JSON" => Ok(Self::Json),
            _ => Err(SourceDomainError::InvalidFormat),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpan {
    pub start_byte: u64,
    pub end_byte: u64,
    #[serde(default)]
    pub start_line: u32,
    #[serde(default)]
    pub end_line: u32,
    #[serde(default)]
    pub start_character: u32,
    #[serde(default)]
    pub end_character: u32,
}

impl SourceSpan {
    pub const fn new(start_byte: u64, end_byte: u64) -> Self {
        Self {
            start_byte,
            end_byte,
            start_line: 0,
            end_line: 0,
            start_character: 0,
            end_character: 0,
        }
    }

    pub const fn with_positions(
        start_byte: u64,
        end_byte: u64,
        start_line: u32,
        end_line: u32,
        start_character: u32,
        end_character: u32,
    ) -> Self {
        Self {
            start_byte,
            end_byte,
            start_line,
            end_line,
            start_character,
            end_character,
        }
    }

    pub fn validate(&self, source: &[u8]) -> Result<(), SourceSpanError> {
        if self.end_byte < self.start_byte {
            return Err(SourceSpanError::InvalidRange);
        }
        if self.end_byte > source.len() as u64 {
            return Err(SourceSpanError::OutOfBounds);
        }
        let text = std::str::from_utf8(source).map_err(|_| SourceSpanError::InvalidUtf8)?;
        let start = self.start_byte as usize;
        let end = self.end_byte as usize;
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            return Err(SourceSpanError::NotUtf8Boundary);
        }
        if self.end_line < self.start_line {
            return Err(SourceSpanError::InvalidLineRange);
        }
        if self.end_character < self.start_character {
            return Err(SourceSpanError::InvalidCharacterRange);
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.start_byte == self.end_byte
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpanError {
    InvalidRange,
    OutOfBounds,
    InvalidUtf8,
    NotUtf8Boundary,
    InvalidLineRange,
    IncompleteLineRange,
    InvalidCharacterRange,
    IncompleteCharacterRange,
}

impl SourceSpanError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRange => "DRAFT_SOURCE_SPAN_INVALID",
            Self::OutOfBounds => "DRAFT_SOURCE_SPAN_INVALID",
            Self::InvalidUtf8 => "INVALID_SOURCE_UTF8",
            Self::NotUtf8Boundary => "DRAFT_SOURCE_SPAN_INVALID",
            Self::InvalidLineRange => "DRAFT_SOURCE_SPAN_INVALID",
            Self::IncompleteLineRange => "DRAFT_SOURCE_SPAN_INVALID",
            Self::InvalidCharacterRange => "DRAFT_SOURCE_SPAN_INVALID",
            Self::IncompleteCharacterRange => "DRAFT_SOURCE_SPAN_INVALID",
        }
    }
}

impl fmt::Display for SourceSpanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for SourceSpanError {}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceBlockKind {
    Unknown,
    Heading,
    Paragraph,
    Dialogue,
    Narration,
    List,
    Quote,
    Code,
    Table,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBlock {
    pub block_id: String,
    pub span: SourceSpan,
    pub preview: Option<String>,
    pub kind: SourceBlockKind,
    pub parent_hint: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMetadata {
    pub provider_kind: String,
    pub model_label: Option<String>,
    pub prompt_contract_version: Option<u32>,
    pub input_checksum: Option<String>,
    pub output_checksum: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptDocument {
    pub source_id: SourceId,
    pub project_id: Option<String>,
    pub format: ScriptFormat,
    pub original_filename: Option<String>,
    pub source_checksum: String,
    pub source_length: u64,
    pub source_storage_ref: String,
    pub schema_version: u32,
    pub parser_version: String,
    pub provider_metadata: Option<ProviderMetadata>,
    #[serde(default)]
    pub source_blocks: Vec<SourceBlock>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub imported_at: Option<String>,
}

pub type ScriptSource = ScriptDocument;
pub type SourceSpanV1 = SourceSpan;

impl ScriptDocument {
    pub fn from_raw_bytes(
        source_id: SourceId,
        raw: &[u8],
        format: ScriptFormat,
        source_storage_ref: impl Into<String>,
    ) -> Result<Self, SourceDomainError> {
        std::str::from_utf8(raw).map_err(|_| SourceDomainError::InvalidUtf8)?;
        Ok(Self {
            source_id,
            project_id: None,
            format,
            original_filename: None,
            source_checksum: sha256_hex(raw),
            source_length: raw.len() as u64,
            source_storage_ref: source_storage_ref.into(),
            schema_version: SOURCE_SCHEMA_VERSION,
            parser_version: "unparsed".to_owned(),
            provider_metadata: None,
            source_blocks: Vec::new(),
            diagnostics: Vec::new(),
            imported_at: None,
        })
    }

    pub fn validate_raw_bytes(&self, raw: &[u8]) -> Result<(), SourceDomainError> {
        std::str::from_utf8(raw).map_err(|_| SourceDomainError::InvalidUtf8)?;
        if self.source_length != raw.len() as u64 {
            return Err(SourceDomainError::SourceLengthMismatch);
        }
        if self.source_checksum != sha256_hex(raw) {
            return Err(SourceDomainError::SourceChecksumMismatch);
        }
        for block in &self.source_blocks {
            for diagnostic in &block.diagnostics {
                for span in &diagnostic.source_spans {
                    span.validate(raw).map_err(SourceDomainError::Span)?;
                }
            }
            block.span.validate(raw).map_err(SourceDomainError::Span)?;
        }
        for diagnostic in &self.diagnostics {
            for span in &diagnostic.source_spans {
                span.validate(raw).map_err(SourceDomainError::Span)?;
            }
        }
        Ok(())
    }

    pub fn validate(
        &self,
        raw: &[u8],
    ) -> Result<(), crate::domain::script_draft::DraftValidationError> {
        crate::domain::script_draft::validate_source(self, raw)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceDomainError {
    InvalidUtf8,
    SourceLengthMismatch,
    SourceChecksumMismatch,
    InvalidFormat,
    Span(SourceSpanError),
}

impl SourceDomainError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "INVALID_SOURCE_UTF8",
            Self::SourceLengthMismatch => "SOURCE_LENGTH_MISMATCH",
            Self::SourceChecksumMismatch => "SOURCE_CHECKSUM_MISMATCH",
            Self::InvalidFormat => "INVALID_SCRIPT_FORMAT",
            Self::Span(error) => error.code(),
        }
    }
}

impl fmt::Display for SourceDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for SourceDomainError {}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn source_checksum(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}
