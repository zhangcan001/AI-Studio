//! Application orchestration for deterministic ScriptSource imports.
//!
//! This service is intentionally limited to Script/Draft persistence.  It
//! does not depend on formal production, profiles, generation, queues, or
//! ComfyUI services.

use crate::application::script_draft_service::{
    AppendScriptDraftRevisionRequest, CreateScriptDraftRequest, ScriptDraftRevisionView,
    ScriptDraftService, ScriptSourceContentView, ScriptSourceView,
};
use crate::application::script_import_parser::{
    deterministic_diagnostic_id, parse_source, DraftParseDiffSummary, ParseCancellationToken,
    ParserError, ScriptImportStatistics, ScriptParseOptions, SCRIPT_IMPORT_PARSER_VERSION,
};
use crate::domain::script_draft::{
    canonical_json, Diagnostic, DiagnosticSeverity, DraftStructureV1, ScriptDocument, SourceId,
    DRAFT_SCHEMA_VERSION,
};
use crate::error::AppError;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct ScriptImportService {
    drafts: Arc<ScriptDraftService>,
}

#[derive(Clone)]
pub struct ScriptImportPreview {
    pub source: ScriptSourceView,
    pub document: ScriptDocument,
    pub structure: Option<DraftStructureV1>,
    pub statistics: ScriptImportStatistics,
    pub diagnostics: Vec<Diagnostic>,
    pub diff: Option<DraftParseDiffSummary>,
    pub can_persist: bool,
}

#[derive(Clone)]
pub struct ScriptImportReparseResult {
    pub revision: ScriptDraftRevisionView,
    pub diff: DraftParseDiffSummary,
}

impl ScriptImportService {
    pub fn new(drafts: Arc<ScriptDraftService>) -> Self {
        Self { drafts }
    }

    pub async fn preview_source(
        &self,
        project_id: &str,
        source_id: &SourceId,
        options: ScriptParseOptions,
        cancellation: Option<&ParseCancellationToken>,
    ) -> Result<ScriptImportPreview, AppError> {
        let started = Instant::now();
        let source = self.load_source(project_id, source_id).await?;
        let parsed = parse_source(
            source_id,
            project_id,
            source.format,
            source.original_filename.as_deref(),
            source.source_text.as_bytes(),
            &options,
            cancellation,
        )
        .map_err(map_parser_error)?;
        let mut diagnostics = parsed.diagnostics.clone();
        let mut structure = parsed.structure;
        if let Some(candidate) = structure.as_mut() {
            append_node_diagnostics(candidate, &mut diagnostics);
            candidate.metadata.insert(
                "scriptImport.options.v1".to_owned(),
                canonical_json(&options).unwrap_or_else(|_| "{}".to_owned()),
            );
        }
        if let Some(candidate) = structure.as_ref() {
            if let Err(error) =
                candidate.validate(source.source_text.as_bytes(), DRAFT_SCHEMA_VERSION)
            {
                diagnostics.push(parser_output_invalid(error.code()));
                structure = None;
            }
        }
        let document = build_document(&source, parsed.source_blocks, diagnostics.clone());
        let mut statistics = statistics(&structure, &diagnostics, document.source_blocks.len());
        statistics.parse_milliseconds = started.elapsed().as_millis();
        let can_persist = structure.is_some()
            && !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity.blocks_confirm());
        Ok(ScriptImportPreview {
            source: source_view(&source),
            document,
            structure,
            statistics,
            diagnostics,
            diff: None,
            can_persist,
        })
    }

    pub async fn create_draft_from_source(
        &self,
        project_id: &str,
        source_id: &SourceId,
        options: ScriptParseOptions,
    ) -> Result<ScriptDraftRevisionView, AppError> {
        let preview = self
            .preview_source(project_id, source_id, options, None)
            .await?;
        let structure = preview
            .structure
            .ok_or_else(|| blocker_error(&preview.diagnostics))?;
        if !preview.can_persist {
            return Err(blocker_error(&preview.diagnostics));
        }
        self.drafts
            .create_draft(CreateScriptDraftRequest {
                project_id: project_id.to_owned(),
                source_id: source_id.clone(),
                structure,
                parser_version: SCRIPT_IMPORT_PARSER_VERSION.to_owned(),
                provider_metadata: None,
            })
            .await
    }

    pub async fn reparse_draft(
        &self,
        project_id: &str,
        draft_id: &crate::domain::script_draft::DraftId,
        source_id: &SourceId,
        expected_revision: u64,
        options: ScriptParseOptions,
        cancellation: Option<&ParseCancellationToken>,
    ) -> Result<ScriptImportReparseResult, AppError> {
        let previous = self
            .drafts
            .get_latest_revision(project_id, draft_id)
            .await?
            .ok_or_else(|| AppError::invalid_input("DRAFT_NOT_FOUND: draft does not exist"))?;
        if previous.metadata.revision != expected_revision {
            return Err(AppError::invalid_input(
                "DRAFT_REVISION_CONFLICT: expected revision is stale",
            ));
        }
        let source = self.load_source(project_id, source_id).await?;
        let parsed = parse_source(
            source_id,
            project_id,
            source.format,
            source.original_filename.as_deref(),
            source.source_text.as_bytes(),
            &options,
            cancellation,
        )
        .map_err(map_parser_error)?;
        let mut parsed_diagnostics = parsed.diagnostics.clone();
        let mut next = parsed
            .structure
            .ok_or_else(|| blocker_error(&parsed_diagnostics))?;
        append_node_diagnostics(&next, &mut parsed_diagnostics);
        if parsed_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity.blocks_confirm())
        {
            return Err(blocker_error(&parsed_diagnostics));
        }
        next.draft_id = previous.structure.draft_id.clone();
        next.metadata.insert(
            "scriptImport.options.v1".to_owned(),
            canonical_json(&options).unwrap_or_else(|_| "{}".to_owned()),
        );
        if let Err(error) = next.validate(source.source_text.as_bytes(), DRAFT_SCHEMA_VERSION) {
            return Err(AppError::invalid_input(format!(
                "PARSER_OUTPUT_INVALID: {}",
                error.code()
            )));
        }
        if cancellation.is_some_and(ParseCancellationToken::is_cancelled) {
            return Err(map_parser_error(ParserError::Cancelled));
        }
        let reconciled = crate::application::script_import_parser::reconcile::reconcile(
            &previous.structure,
            next,
            options.preserve_human_edits,
        );
        if cancellation.is_some_and(ParseCancellationToken::is_cancelled) {
            return Err(map_parser_error(ParserError::Cancelled));
        }
        let diff = reconciled.diff.clone();
        let next = reconciled.structure;
        next.validate(source.source_text.as_bytes(), DRAFT_SCHEMA_VERSION)
            .map_err(|error| {
                AppError::invalid_input(format!("PARSER_OUTPUT_INVALID: {}", error.code()))
            })?;
        if previous.metadata.parser_version == SCRIPT_IMPORT_PARSER_VERSION
            && previous.metadata.source_id == *source_id
            && same_options(&previous.structure, &options)
            && semantic_structure_equal(&previous.structure, &next)
        {
            return Ok(ScriptImportReparseResult {
                revision: previous,
                diff,
            });
        }
        let revision = self
            .drafts
            .append_revision(AppendScriptDraftRevisionRequest {
                project_id: project_id.to_owned(),
                draft_id: draft_id.clone(),
                expected_revision,
                structure: next,
                revision_kind: crate::domain::script_draft::DraftRevisionKind::Reparsed,
                parser_version: SCRIPT_IMPORT_PARSER_VERSION.to_owned(),
                provider_metadata: None,
            })
            .await?;
        Ok(ScriptImportReparseResult { revision, diff })
    }

    async fn load_source(
        &self,
        project_id: &str,
        source_id: &SourceId,
    ) -> Result<ScriptSourceContentView, AppError> {
        self.drafts
            .get_source(project_id, source_id)
            .await?
            .ok_or_else(|| AppError::invalid_input("SOURCE_NOT_FOUND: source does not exist"))
    }
}

fn source_view(source: &ScriptSourceContentView) -> ScriptSourceView {
    ScriptSourceView {
        id: source.id.clone(),
        project_id: source.project_id.clone(),
        format: source.format,
        source_checksum: source.source_checksum.clone(),
        source_bytes: source.source_bytes,
        schema_version: source.schema_version,
        created_at: source.created_at,
        original_filename: source.original_filename.clone(),
    }
}

fn build_document(
    source: &ScriptSourceContentView,
    source_blocks: Vec<crate::domain::script_draft::SourceBlock>,
    diagnostics: Vec<Diagnostic>,
) -> ScriptDocument {
    ScriptDocument {
        source_id: source.id.clone(),
        project_id: Some(source.project_id.clone()),
        format: source.format,
        original_filename: source.original_filename.clone(),
        source_checksum: source.source_checksum.clone(),
        source_length: source.source_bytes,
        source_storage_ref: format!("sqlite:script_sources/{}", source.id),
        schema_version: crate::domain::script_draft::SOURCE_SCHEMA_VERSION,
        parser_version: SCRIPT_IMPORT_PARSER_VERSION.to_owned(),
        provider_metadata: None,
        source_blocks,
        diagnostics,
        imported_at: Some(source.created_at.to_rfc3339()),
    }
}

fn statistics(
    structure: &Option<DraftStructureV1>,
    diagnostics: &[Diagnostic],
    block_count: usize,
) -> ScriptImportStatistics {
    let mut result = ScriptImportStatistics {
        block_count,
        ..Default::default()
    };
    for diagnostic in diagnostics {
        count_diagnostic(&mut result, diagnostic.severity);
    }
    if let Some(structure) = structure {
        let counts = structure.counts();
        result.episode_count = counts.episodes;
        result.scene_count = counts.scenes;
        result.shot_count = counts.shots;
        result.character_mention_count = structure
            .mentions
            .iter()
            .chain(structure.episodes.iter().flat_map(|episode| {
                episode
                    .scenes
                    .iter()
                    .flat_map(|scene| scene.shots.iter().flat_map(|shot| shot.characters.iter()))
            }))
            .filter(|mention| {
                mention.entity_type == crate::domain::script_draft::EntityType::Character
            })
            .count();
    }
    result
}

fn count_diagnostic(statistics: &mut ScriptImportStatistics, severity: DiagnosticSeverity) {
    match severity {
        DiagnosticSeverity::Info => statistics.info_count += 1,
        DiagnosticSeverity::Warning => statistics.warning_count += 1,
        DiagnosticSeverity::Error => statistics.error_count += 1,
        DiagnosticSeverity::Blocker => statistics.blocker_count += 1,
    }
}

fn append_node_diagnostics(structure: &DraftStructureV1, diagnostics: &mut Vec<Diagnostic>) {
    for episode in &structure.episodes {
        append_diagnostics(&episode.diagnostics, diagnostics);
        for scene in &episode.scenes {
            append_diagnostics(&scene.diagnostics, diagnostics);
            for shot in &scene.shots {
                append_diagnostics(&shot.diagnostics, diagnostics);
            }
        }
    }
}

fn append_diagnostics(source: &[Diagnostic], target: &mut Vec<Diagnostic>) {
    for diagnostic in source {
        if !target.iter().any(|existing| {
            existing.severity == diagnostic.severity
                && existing.code == diagnostic.code
                && existing.field == diagnostic.field
                && existing.source_spans == diagnostic.source_spans
                && existing.draft_node_id == diagnostic.draft_node_id
        }) {
            target.push(diagnostic.clone());
        }
    }
}

fn parser_output_invalid(code: &str) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        DiagnosticSeverity::Blocker,
        "PARSER_OUTPUT_INVALID",
        format!("Parser output failed Draft validation ({code})"),
    );
    diagnostic.diagnostic_id = deterministic_diagnostic_id(&format!("parser-output/{code}"));
    diagnostic
}

fn same_options(structure: &DraftStructureV1, options: &ScriptParseOptions) -> bool {
    structure
        .metadata
        .get("scriptImport.options.v1")
        .map_or(true, |value| {
            value == &canonical_json(options).unwrap_or_else(|_| "{}".to_owned())
        })
}

fn semantic_structure_equal(left: &DraftStructureV1, right: &DraftStructureV1) -> bool {
    fn normalized(value: &DraftStructureV1) -> serde_json::Value {
        let mut json = serde_json::to_value(value).unwrap_or_default();
        if let Some(object) = json.as_object_mut() {
            object.remove("draftId");
            object.remove("revision");
            object.remove("revisionId");
        }
        json
    }
    normalized(left) == normalized(right)
}

fn blocker_error(diagnostics: &[Diagnostic]) -> AppError {
    let code = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity.blocks_confirm())
        .map(|diagnostic| diagnostic.code.as_str())
        .unwrap_or("PARSER_OUTPUT_INVALID");
    AppError::invalid_input(format!("{code}: import cannot be persisted"))
}

fn map_parser_error(error: ParserError) -> AppError {
    match error {
        ParserError::Cancelled => {
            AppError::invalid_input("SCRIPT_PARSE_CANCELLED: parsing was cancelled")
        }
        ParserError::InvalidUtf8 => {
            AppError::invalid_input("INVALID_SOURCE_UTF8: source must be UTF-8")
        }
        ParserError::OutputInvalid => {
            AppError::invalid_input("PARSER_OUTPUT_INVALID: parser output is invalid")
        }
    }
}

#[allow(dead_code)]
fn _canonical_document(document: &ScriptDocument) -> Result<String, AppError> {
    canonical_json(document)
        .map_err(|_| AppError::internal("Script import document serialization failed"))
}
