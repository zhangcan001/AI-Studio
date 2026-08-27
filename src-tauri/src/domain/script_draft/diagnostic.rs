use crate::domain::script_draft::{DraftNodeId, SourceSpan};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const CODE_ENCODING_OR_BOM: &str = "SOURCE_ENCODING_OR_BOM";
pub const CODE_UNKNOWN_JSON_SCHEMA: &str = "UNKNOWN_JSON_SCHEMA";
pub const CODE_EMPTY_DRAFT_NODE: &str = "EMPTY_DRAFT_NODE";
pub const CODE_DUPLICATE_SOURCE_ID: &str = "DUPLICATE_SOURCE_ID";
pub const CODE_INVALID_PARENT: &str = "INVALID_PARENT_DRAFT_NODE";
pub const CODE_MISSING_NAME: &str = "DRAFT_NAME_REQUIRED";
pub const CODE_UNRESOLVED_SPEAKER: &str = "UNRESOLVED_DIALOGUE_SPEAKER";
pub const CODE_UNCERTAIN_SCENE_BOUNDARY: &str = "UNCERTAIN_SCENE_BOUNDARY";
pub const CODE_PROVIDER_INVALID_JSON: &str = "PROVIDER_INVALID_JSON";
pub const CODE_DRAFT_SOURCE_SPAN_INVALID: &str = "DRAFT_SOURCE_SPAN_INVALID";
pub const CODE_SOURCE_SPAN_OUT_OF_BOUNDS: &str = CODE_DRAFT_SOURCE_SPAN_INVALID;
pub const CODE_DRAFT_CAPACITY_EXCEEDED: &str = "DRAFT_CAPACITY_EXCEEDED";

#[derive(Clone, Copy, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
    Blocker,
}

impl fmt::Debug for DiagnosticSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Blocker => "Blocker",
        })
    }
}

impl DiagnosticSeverity {
    pub const fn blocks_confirm(self) -> bool {
        matches!(self, Self::Error | Self::Blocker)
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub diagnostic_id: crate::domain::script_draft::DiagnosticId,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub field: Option<String>,
    #[serde(default)]
    pub source_spans: Vec<SourceSpan>,
    pub draft_node_id: Option<DraftNodeId>,
}

impl Diagnostic {
    pub fn new(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            diagnostic_id: Default::default(),
            severity,
            code: code.into(),
            message: message.into(),
            field: None,
            source_spans: Vec::new(),
            draft_node_id: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.source_spans.push(span);
        self
    }

    pub fn for_node(mut self, node_id: DraftNodeId) -> Self {
        self.draft_node_id = Some(node_id);
        self
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
}

impl fmt::Debug for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Diagnostic")
            .field("diagnostic_id", &self.diagnostic_id)
            .field("severity", &self.severity)
            .field("code", &self.code)
            .field("message", &"<redacted>")
            .field("field", &self.field)
            .field("source_spans", &self.source_spans)
            .field("draft_node_id", &self.draft_node_id)
            .finish()
    }
}
