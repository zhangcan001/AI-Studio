//! Backend contracts for project-scale Shot import and configuration.
//!
//! This service intentionally stops at the application/port boundary.  The
//! stage prompt columns are a migration-019 concern; keeping the service
//! independent of SQLite lets the migration and its adapter land separately
//! without falling back to the legacy shared `shots.prompt_text` field.

use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::ports::{
    Clock, GenerationDefinitionRepository, PromptLibraryRepository, RepositoryError, ShotBulkData,
    ShotBulkRepository, ShotRecord, ShotStageConfigRecord, ShotStagePromptRecord,
};
use crate::application::prompt_library_service::canonical_prompt_text;
use crate::application::shot_service::{validate_stage_config_values, ShotServiceError};
use crate::domain::{canonical_shot_name, validate_project_id, ShotStage};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};
use uuid::Uuid;

pub const MAX_BULK_SHOT_IMPORT: usize = 500;
pub const MAX_BULK_SHOT_SCOPE: usize = MAX_BULK_SHOT_IMPORT;
pub const EPISODE_BULK_SCOPE_TOO_LARGE: &str = "EPISODE_BULK_SCOPE_TOO_LARGE";
const PREVIEW_TEXT_LIMIT: usize = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotBulkInputFormat {
    Json,
    Tsv,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotBulkImportRequest {
    pub project_id: String,
    pub format: ShotBulkInputFormat,
    pub contents: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkIssue {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_number: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shot_id: Option<String>,
}

impl BulkIssue {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            row_number: None,
            shot_id: None,
        }
    }

    fn row(mut self, row_number: usize) -> Self {
        self.row_number = Some(row_number);
        self
    }

    fn shot(mut self, shot_id: &str) -> Self {
        self.shot_id = Some(shot_id.to_owned());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotBulkImportRowPreview {
    pub row_number: usize,
    pub name: String,
    pub description: String,
    pub image_prompt: Option<String>,
    pub video_prompt: Option<String>,
    pub errors: Vec<BulkIssue>,
    pub warnings: Vec<BulkIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotBulkImportPreview {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub warnings: usize,
    pub rows: Vec<ShotBulkImportRowPreview>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotBulkCreatedShot {
    pub shot_id: String,
    pub ordinal: i64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotBulkImportResult {
    pub project_id: String,
    pub created: Vec<ShotBulkCreatedShot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BulkPromptSource {
    Text(String),
    PromptLibraryVersion {
        prompt_entry_id: String,
        prompt_version_id: String,
    },
    ClearProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkPromptAssignmentRequest {
    pub project_id: String,
    pub stage: ShotStage,
    pub shot_ids: Vec<String>,
    pub source: BulkPromptSource,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BulkStageConfigRequest {
    pub project_id: String,
    pub stage: ShotStage,
    pub shot_ids: Vec<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: std::collections::BTreeMap<String, GenerationInputValue>,
    pub prompt: Option<BulkPromptSource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkAssignmentResult {
    pub project_id: String,
    pub stage: String,
    pub updated_shot_ids: Vec<String>,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkStageConfigResult {
    pub project_id: String,
    pub stage: String,
    pub configured_shot_ids: Vec<String>,
    pub prompt_updated_shot_ids: Vec<String>,
}

#[derive(Debug)]
pub enum ShotBulkServiceError {
    InvalidInput(BulkIssue),
    Validation(Vec<BulkIssue>),
    Repository(RepositoryError),
    TransactionFailed { code: &'static str, message: String },
}

impl ShotBulkServiceError {
    pub fn code(&self) -> &str {
        match self {
            Self::InvalidInput(issue) => &issue.code,
            Self::Validation(issues) => issues
                .first()
                .map(|issue| issue.code.as_str())
                .unwrap_or("BULK_VALIDATION_FAILED"),
            Self::Repository(_) => "BULK_REPOSITORY_ERROR",
            Self::TransactionFailed { code, .. } => code,
        }
    }

    pub fn issues(&self) -> Vec<BulkIssue> {
        match self {
            Self::InvalidInput(issue) => vec![issue.clone()],
            Self::Validation(issues) => issues.clone(),
            Self::Repository(error) => {
                vec![BulkIssue::new("BULK_REPOSITORY_ERROR", error.to_string())]
            }
            Self::TransactionFailed { code, message } => {
                vec![BulkIssue::new(*code, message.clone())]
            }
        }
    }
}

impl fmt::Display for ShotBulkServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(issue) => write!(formatter, "{}: {}", issue.code, issue.message),
            Self::Validation(issues) => {
                let message = issues
                    .iter()
                    .map(|issue| issue.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(formatter, "{}: {message}", self.code())
            }
            Self::Repository(error) => write!(formatter, "BULK_REPOSITORY_ERROR: {error}"),
            Self::TransactionFailed { code, message } => write!(formatter, "{code}: {message}"),
        }
    }
}

impl Error for ShotBulkServiceError {}

impl From<RepositoryError> for ShotBulkServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

pub struct ShotBulkService {
    repository: Arc<dyn ShotBulkRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    prompt_repository: Arc<dyn PromptLibraryRepository>,
    clock: Arc<dyn Clock>,
}

impl ShotBulkService {
    pub fn new(
        repository: Arc<dyn ShotBulkRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        prompt_repository: Arc<dyn PromptLibraryRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            definition_repository,
            prompt_repository,
            clock,
        }
    }

    pub async fn preview_import(
        &self,
        request: &ShotBulkImportRequest,
    ) -> Result<ShotBulkImportPreview, ShotBulkServiceError> {
        validate_project(&request.project_id)?;
        let parsed = parse_import(request.format, &request.contents)?;
        let (normalized, mut rows) = normalize_import(parsed);
        let existing = self.repository.list_bulk_data(&request.project_id).await?;
        let existing_names = existing
            .iter()
            .map(|data| canonical_name_key(&data.shot.name))
            .collect::<HashSet<_>>();
        let mut warnings = Vec::new();
        let row_indexes = rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.row_number, index))
            .collect::<HashMap<_, _>>();
        for normalized in &normalized {
            if existing_names.contains(&canonical_name_key(&normalized.name)) {
                let warning = BulkIssue::new(
                    "BULK_IMPORT_DUPLICATE_SHOT",
                    format!("项目中已有同名镜头：{}", normalized.name),
                )
                .row(normalized.row_number);
                if let Some(index) = row_indexes.get(&normalized.row_number) {
                    // Existing names are visible as warnings in the preview,
                    // but CREATE ONLY means confirmation must still be blocked.
                    rows[*index].errors.push(warning.clone());
                }
                warnings.push(warning);
            }
        }
        Ok(build_preview(rows, warnings))
    }

    pub async fn commit_import(
        &self,
        request: &ShotBulkImportRequest,
    ) -> Result<ShotBulkImportResult, ShotBulkServiceError> {
        validate_project(&request.project_id)?;
        let parsed = parse_import(request.format, &request.contents)?;
        let (normalized, rows) = normalize_import(parsed);
        let mut issues = rows
            .iter()
            .flat_map(|row| row.errors.iter().cloned())
            .collect::<Vec<_>>();
        let existing = self.repository.list_bulk_data(&request.project_id).await?;
        let existing_names = existing
            .iter()
            .map(|data| canonical_name_key(&data.shot.name))
            .collect::<HashSet<_>>();
        let mut seen_existing = HashSet::new();
        for row in &normalized {
            if existing_names.contains(&canonical_name_key(&row.name))
                && seen_existing.insert(canonical_name_key(&row.name))
            {
                issues.push(
                    BulkIssue::new(
                        "BULK_IMPORT_DUPLICATE_SHOT",
                        format!("项目中已有同名镜头：{}", row.name),
                    )
                    .row(row.row_number),
                );
            }
        }
        if !issues.is_empty() {
            return Err(ShotBulkServiceError::Validation(issues));
        }
        if normalized.is_empty() {
            return Err(ShotBulkServiceError::InvalidInput(BulkIssue::new(
                "BULK_IMPORT_EMPTY_INPUT",
                "导入至少需要一行镜头",
            )));
        }

        let max_ordinal = existing
            .iter()
            .map(|data| data.shot.ordinal)
            .max()
            .unwrap_or(-1);
        let first_ordinal = max_ordinal.checked_add(1).ok_or_else(|| {
            ShotBulkServiceError::InvalidInput(BulkIssue::new(
                "BULK_IMPORT_ORDINAL_OVERFLOW",
                "镜头序号超出范围",
            ))
        })?;
        let mut shots = Vec::with_capacity(normalized.len());
        let mut stage_prompts = Vec::new();
        let mut created = Vec::with_capacity(normalized.len());
        for (index, row) in normalized.iter().enumerate() {
            let ordinal = first_ordinal
                .checked_add(i64::try_from(index).map_err(|_| {
                    ShotBulkServiceError::InvalidInput(BulkIssue::new(
                        "BULK_IMPORT_ORDINAL_OVERFLOW",
                        "镜头序号超出范围",
                    ))
                })?)
                .ok_or_else(|| {
                    ShotBulkServiceError::InvalidInput(BulkIssue::new(
                        "BULK_IMPORT_ORDINAL_OVERFLOW",
                        "镜头序号超出范围",
                    ))
                })?;
            let shot_id = format!("sht_{}", Uuid::new_v4());
            let now = self.clock.now();
            shots.push(ShotRecord {
                id: shot_id.clone(),
                project_id: request.project_id.clone(),
                ordinal,
                name: row.name.clone(),
                // The legacy field remains the shot narrative/description;
                // stage prompts are persisted independently in migration 019.
                prompt_text: row.description.clone(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: None,
                selected_video_asset_id: None,
                created_at: now,
                updated_at: now,
            });
            if let Some(prompt) = &row.image_prompt {
                stage_prompts.push(ShotStagePromptRecord {
                    shot_id: shot_id.clone(),
                    stage: ShotStage::Image,
                    prompt_text: prompt.clone(),
                    prompt_entry_id: None,
                    prompt_version_id: None,
                    updated_at: now,
                });
            }
            if let Some(prompt) = &row.video_prompt {
                stage_prompts.push(ShotStagePromptRecord {
                    shot_id: shot_id.clone(),
                    stage: ShotStage::Video,
                    prompt_text: prompt.clone(),
                    prompt_entry_id: None,
                    prompt_version_id: None,
                    updated_at: now,
                });
            }
            created.push(ShotBulkCreatedShot {
                shot_id,
                ordinal,
                name: row.name.clone(),
            });
        }
        self.repository
            .insert_shots_atomic(&request.project_id, &shots, &stage_prompts)
            .await
            .map_err(|error| transaction_error("BULK_IMPORT_TRANSACTION_FAILED", error))?;
        Ok(ShotBulkImportResult {
            project_id: request.project_id.clone(),
            created,
        })
    }

    pub async fn assign_prompt(
        &self,
        request: BulkPromptAssignmentRequest,
    ) -> Result<BulkAssignmentResult, ShotBulkServiceError> {
        validate_project(&request.project_id)?;
        let selected = self
            .select_shots(
                &request.project_id,
                &request.shot_ids,
                "BULK_ASSIGNMENT_INVALID_SHOT",
            )
            .await?;
        let resolved = self
            .resolve_prompt_source(&request.project_id, &request.source)
            .await?;
        let now = self.clock.now();
        let updates = selected
            .iter()
            .map(|data| {
                let prompt = prompt_for_shot(&request.source, request.stage, data, &resolved);
                ShotStagePromptRecord {
                    shot_id: data.shot.id.clone(),
                    stage: request.stage,
                    prompt_text: prompt.text,
                    prompt_entry_id: prompt.prompt_entry_id,
                    prompt_version_id: prompt.prompt_version_id,
                    updated_at: now,
                }
            })
            .collect::<Vec<_>>();
        let updated_shot_ids = updates
            .iter()
            .map(|update| update.shot_id.clone())
            .collect::<Vec<_>>();
        self.repository
            .update_stage_prompts_atomic(&request.project_id, &updates)
            .await
            .map_err(|error| transaction_error("BULK_ASSIGNMENT_TRANSACTION_FAILED", error))?;
        Ok(BulkAssignmentResult {
            project_id: request.project_id,
            stage: request.stage.as_str().to_owned(),
            updated_shot_ids,
            prompt_entry_id: resolved.prompt_entry_id,
            prompt_version_id: resolved.prompt_version_id,
        })
    }

    pub async fn set_stage_config(
        &self,
        request: BulkStageConfigRequest,
    ) -> Result<BulkStageConfigResult, ShotBulkServiceError> {
        validate_project(&request.project_id)?;
        let selected = self
            .select_shots(
                &request.project_id,
                &request.shot_ids,
                "BULK_ASSIGNMENT_INVALID_SHOT",
            )
            .await?;
        let definition = self
            .definition_repository
            .find(&request.workflow_version_id, &request.recipe_id)
            .await?
            .ok_or_else(|| {
                ShotBulkServiceError::Validation(vec![BulkIssue::new(
                    "BULK_ASSIGNMENT_INVALID_RECIPE",
                    format!(
                        "工作流版本或 Recipe 当前不可用：{} / {}",
                        request.workflow_version_id, request.recipe_id
                    ),
                )])
            })?;
        let scalar_values =
            validate_stage_config_values(request.stage, &definition, &request.values)
                .map_err(|error| invalid_recipe_error(&error))?;
        let prompt_updates = if let Some(source) = &request.prompt {
            let resolved = self
                .resolve_prompt_source(&request.project_id, source)
                .await?;
            let now = self.clock.now();
            selected
                .iter()
                .map(|data| {
                    let prompt = prompt_for_shot(source, request.stage, data, &resolved);
                    ShotStagePromptRecord {
                        shot_id: data.shot.id.clone(),
                        stage: request.stage,
                        prompt_text: prompt.text,
                        prompt_entry_id: prompt.prompt_entry_id,
                        prompt_version_id: prompt.prompt_version_id,
                        updated_at: now,
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let now = self.clock.now();
        let configs = selected
            .iter()
            .map(|data| ShotStageConfigRecord {
                shot_id: data.shot.id.clone(),
                stage: request.stage,
                workflow_version_id: request.workflow_version_id.clone(),
                recipe_id: request.recipe_id.clone(),
                scalar_values: scalar_values.clone(),
                updated_at: now,
            })
            .collect::<Vec<_>>();
        let configured_shot_ids = configs
            .iter()
            .map(|config| config.shot_id.clone())
            .collect::<Vec<_>>();
        let prompt_updated_shot_ids = prompt_updates
            .iter()
            .map(|prompt| prompt.shot_id.clone())
            .collect::<Vec<_>>();
        self.repository
            .upsert_stage_configs_atomic(&request.project_id, &configs, &prompt_updates)
            .await
            .map_err(|error| transaction_error("BULK_ASSIGNMENT_TRANSACTION_FAILED", error))?;
        Ok(BulkStageConfigResult {
            project_id: request.project_id,
            stage: request.stage.as_str().to_owned(),
            configured_shot_ids,
            prompt_updated_shot_ids,
        })
    }

    /// Resolve the shots belonging to the selected Scene scopes once, then
    /// reuse the existing bulk config contract. Scene membership is supplied
    /// by the trusted production-structure caller; this service owns the
    /// project membership, dedupe, global ordinal ordering, and hard limit.
    pub async fn set_stage_config_for_scenes(
        &self,
        mut request: BulkStageConfigRequest,
        scene_shot_ids: &[Vec<String>],
    ) -> Result<BulkStageConfigResult, ShotBulkServiceError> {
        request.shot_ids = self
            .collect_scene_shot_ids(&request.project_id, scene_shot_ids)
            .await?;
        self.set_stage_config(request).await
    }

    pub async fn collect_scene_shot_ids(
        &self,
        project_id: &str,
        scene_shot_ids: &[Vec<String>],
    ) -> Result<Vec<String>, ShotBulkServiceError> {
        validate_project(project_id)?;
        let data = self.repository.list_bulk_data(project_id).await?;
        order_scene_shot_ids(scene_shot_ids, &data).map_err(scene_scope_error)
    }

    async fn select_shots(
        &self,
        project_id: &str,
        shot_ids: &[String],
        code: &str,
    ) -> Result<Vec<ShotBulkData>, ShotBulkServiceError> {
        if shot_ids.is_empty() {
            return Err(ShotBulkServiceError::InvalidInput(BulkIssue::new(
                code,
                "至少需要选择一个镜头",
            )));
        }
        let mut seen = HashSet::new();
        for shot_id in shot_ids {
            if !seen.insert(shot_id) {
                return Err(ShotBulkServiceError::InvalidInput(
                    BulkIssue::new(code, format!("镜头不能重复选择：{shot_id}")).shot(shot_id),
                ));
            }
        }
        let data = self.repository.list_bulk_data(project_id).await?;
        let by_id = data
            .into_iter()
            .map(|item| (item.shot.id.clone(), item))
            .collect::<HashMap<_, _>>();
        shot_ids
            .iter()
            .map(|shot_id| {
                by_id.get(shot_id).cloned().ok_or_else(|| {
                    ShotBulkServiceError::Validation(vec![BulkIssue::new(
                        code,
                        format!("镜头不属于当前项目：{shot_id}"),
                    )
                    .shot(shot_id)])
                })
            })
            .collect()
    }

    async fn resolve_prompt_source(
        &self,
        project_id: &str,
        source: &BulkPromptSource,
    ) -> Result<ResolvedPrompt, ShotBulkServiceError> {
        match source {
            BulkPromptSource::Text(text) => Ok(ResolvedPrompt {
                text: canonical_prompt_text(text).map_err(|message| {
                    ShotBulkServiceError::Validation(vec![BulkIssue::new(
                        "BULK_ASSIGNMENT_INVALID_PROMPT",
                        message,
                    )])
                })?,
                prompt_entry_id: None,
                prompt_version_id: None,
            }),
            BulkPromptSource::PromptLibraryVersion {
                prompt_entry_id,
                prompt_version_id,
            } => {
                let entry = self
                    .prompt_repository
                    .find_by_id(project_id, prompt_entry_id)
                    .await?
                    .ok_or_else(|| {
                        ShotBulkServiceError::Validation(vec![BulkIssue::new(
                            "BULK_ASSIGNMENT_INVALID_PROMPT",
                            format!("Prompt Library entry 不属于当前项目：{prompt_entry_id}"),
                        )])
                    })?;
                let version = self
                    .prompt_repository
                    .list_versions(project_id, &entry.id)
                    .await?
                    .into_iter()
                    .find(|version| version.id == *prompt_version_id)
                    .ok_or_else(|| {
                        ShotBulkServiceError::Validation(vec![BulkIssue::new(
                            "BULK_ASSIGNMENT_INVALID_PROMPT",
                            format!("Prompt Library version 不属于该 entry：{prompt_version_id}"),
                        )])
                    })?;
                let text = canonical_prompt_text(&version.text).map_err(|message| {
                    ShotBulkServiceError::Validation(vec![BulkIssue::new(
                        "BULK_ASSIGNMENT_INVALID_PROMPT",
                        message,
                    )])
                })?;
                Ok(ResolvedPrompt {
                    text,
                    prompt_entry_id: Some(entry.id),
                    prompt_version_id: Some(version.id),
                })
            }
            BulkPromptSource::ClearProvenance => Ok(ResolvedPrompt {
                text: String::new(),
                prompt_entry_id: None,
                prompt_version_id: None,
            }),
        }
    }
}

fn prompt_for_shot(
    source: &BulkPromptSource,
    stage: ShotStage,
    data: &ShotBulkData,
    resolved: &ResolvedPrompt,
) -> ResolvedPrompt {
    if !matches!(source, BulkPromptSource::ClearProvenance) {
        return resolved.clone();
    }
    ResolvedPrompt {
        text: data
            .stage_prompts
            .iter()
            .find(|prompt| prompt.stage == stage)
            .map(|prompt| prompt.prompt_text.clone())
            .unwrap_or_else(|| data.shot.prompt_text.clone()),
        prompt_entry_id: None,
        prompt_version_id: None,
    }
}

pub(crate) fn order_scene_shot_ids(
    scene_shot_ids: &[Vec<String>],
    data: &[ShotBulkData],
) -> Result<Vec<String>, String> {
    if scene_shot_ids.is_empty() {
        return Err(
            "EPISODE_SCENE_SELECTION_INVALID: at least one Scene scope is required".to_owned(),
        );
    }

    let by_id = data
        .iter()
        .map(|item| (item.shot.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let mut first_seen = HashMap::<String, usize>::new();
    for shot_ids in scene_shot_ids {
        for shot_id in shot_ids {
            if shot_id.trim().is_empty() {
                return Err(
                    "EPISODE_SCENE_SELECTION_INVALID: Scene scope contains an empty Shot ID"
                        .to_owned(),
                );
            }
            let position = first_seen.len();
            first_seen.entry(shot_id.clone()).or_insert(position);
        }
    }
    if first_seen.is_empty() {
        return Err("EPISODE_SCENE_SELECTION_INVALID: selected Scenes contain no Shots".to_owned());
    }
    if first_seen.len() > MAX_BULK_SHOT_SCOPE {
        return Err(format!(
            "{EPISODE_BULK_SCOPE_TOO_LARGE}: {} unique Shots exceeds the {}-Shot limit",
            first_seen.len(),
            MAX_BULK_SHOT_SCOPE
        ));
    }

    let mut ordered = first_seen
        .into_iter()
        .map(|(shot_id, first_seen)| {
            let data = by_id.get(shot_id.as_str()).ok_or_else(|| {
                format!(
                    "BULK_ASSIGNMENT_INVALID_SHOT: Shot does not belong to the current project: {shot_id}"
                )
            })?;
            Ok((shot_id, data.shot.ordinal, first_seen))
        })
        .collect::<Result<Vec<_>, String>>()?;
    ordered.sort_by_key(|(_, ordinal, first_seen)| (*ordinal, *first_seen));
    Ok(ordered.into_iter().map(|(shot_id, _, _)| shot_id).collect())
}

fn scene_scope_error(message: String) -> ShotBulkServiceError {
    let (code, message) = message.split_once(": ").map_or(
        ("EPISODE_SCENE_SELECTION_INVALID", message.as_str()),
        |parts| parts,
    );
    ShotBulkServiceError::InvalidInput(BulkIssue::new(code, message))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedPrompt {
    text: String,
    prompt_entry_id: Option<String>,
    prompt_version_id: Option<String>,
}

#[derive(Clone, Debug)]
struct ParsedImportRow {
    row_number: usize,
    name: String,
    description: String,
    image_prompt: Option<String>,
    video_prompt: Option<String>,
    errors: Vec<BulkIssue>,
}

#[derive(Clone, Debug)]
struct NormalizedImportRow {
    row_number: usize,
    name: String,
    description: String,
    image_prompt: Option<String>,
    video_prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonImportDocument {
    schema_version: u32,
    shots: Vec<JsonImportShot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonImportShot {
    name: String,
    description: String,
    #[serde(default)]
    image_prompt: Option<String>,
    #[serde(default)]
    video_prompt: Option<String>,
}

fn validate_project(project_id: &str) -> Result<(), ShotBulkServiceError> {
    validate_project_id(project_id).map_err(|error| {
        ShotBulkServiceError::InvalidInput(BulkIssue::new(
            "BULK_INVALID_PROJECT",
            error.to_string(),
        ))
    })
}

fn parse_import(
    format: ShotBulkInputFormat,
    contents: &str,
) -> Result<Vec<ParsedImportRow>, ShotBulkServiceError> {
    if contents.trim().is_empty() {
        return Err(ShotBulkServiceError::InvalidInput(BulkIssue::new(
            "BULK_IMPORT_EMPTY_INPUT",
            "导入内容不能为空",
        )));
    }
    let contents = contents.trim_start_matches('\u{feff}');
    let rows = match format {
        ShotBulkInputFormat::Json => {
            let document =
                serde_json::from_str::<JsonImportDocument>(contents).map_err(|error| {
                    ShotBulkServiceError::InvalidInput(BulkIssue::new(
                        "BULK_IMPORT_INVALID_JSON",
                        error.to_string(),
                    ))
                })?;
            if document.schema_version != 1 {
                return Err(ShotBulkServiceError::InvalidInput(BulkIssue::new(
                    "BULK_IMPORT_SCHEMA_VERSION",
                    format!("只支持 schemaVersion=1，当前为 {}", document.schema_version),
                )));
            }
            document
                .shots
                .into_iter()
                .enumerate()
                .map(|(index, shot)| ParsedImportRow {
                    row_number: index + 1,
                    name: shot.name,
                    description: shot.description,
                    image_prompt: shot.image_prompt,
                    video_prompt: shot.video_prompt,
                    errors: Vec::new(),
                })
                .collect::<Vec<_>>()
        }
        ShotBulkInputFormat::Tsv => contents
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let line = line.strip_suffix('\r').unwrap_or(line);
                if line.trim().is_empty() {
                    return None;
                }
                let fields = line.split('\t').collect::<Vec<_>>();
                if !(2..=4).contains(&fields.len()) {
                    return Some(ParsedImportRow {
                        row_number: index + 1,
                        name: String::new(),
                        description: String::new(),
                        image_prompt: None,
                        video_prompt: None,
                        errors: vec![BulkIssue::new(
                            "BULK_IMPORT_INVALID_ROW",
                            "TSV 每行必须包含 2–4 列：名称、描述、图片 Prompt、视频 Prompt",
                        )
                        .row(index + 1)],
                    });
                }
                Some(ParsedImportRow {
                    row_number: index + 1,
                    name: fields[0].to_owned(),
                    description: fields[1].to_owned(),
                    image_prompt: fields
                        .get(2)
                        .and_then(|value| (!value.trim().is_empty()).then(|| (*value).to_owned())),
                    video_prompt: fields
                        .get(3)
                        .and_then(|value| (!value.trim().is_empty()).then(|| (*value).to_owned())),
                    errors: Vec::new(),
                })
            })
            .collect::<Vec<_>>(),
    };
    if rows.is_empty() {
        return Err(ShotBulkServiceError::InvalidInput(BulkIssue::new(
            "BULK_IMPORT_EMPTY_INPUT",
            "导入至少需要一行镜头",
        )));
    }
    if rows.len() > MAX_BULK_SHOT_IMPORT {
        return Err(ShotBulkServiceError::InvalidInput(BulkIssue::new(
            "BULK_IMPORT_TOO_MANY_ROWS",
            format!(
                "一次最多导入 {} 行，当前为 {} 行",
                MAX_BULK_SHOT_IMPORT,
                rows.len()
            ),
        )));
    }
    Ok(rows)
}

fn normalize_import(
    rows: Vec<ParsedImportRow>,
) -> (Vec<NormalizedImportRow>, Vec<ShotBulkImportRowPreview>) {
    let mut normalized = Vec::new();
    let mut previews = Vec::with_capacity(rows.len());
    let mut names = HashSet::new();
    for row in rows {
        let mut errors = row.errors;
        let name = match canonical_shot_name(&row.name) {
            Ok(name) => name,
            Err(error) => {
                errors.push(
                    BulkIssue::new("BULK_IMPORT_INVALID_ROW", error.to_string())
                        .row(row.row_number),
                );
                row.name.trim().to_owned()
            }
        };
        let description = match canonical_prompt_text(&row.description) {
            Ok(description) => description,
            Err(error) => {
                errors.push(BulkIssue::new("BULK_IMPORT_INVALID_ROW", error).row(row.row_number));
                String::new()
            }
        };
        let image_prompt = normalize_optional_prompt(row.image_prompt, row.row_number, &mut errors);
        let video_prompt = normalize_optional_prompt(row.video_prompt, row.row_number, &mut errors);
        let key = canonical_name_key(&name);
        if !name.is_empty() && !names.insert(key) {
            errors.push(
                BulkIssue::new(
                    "BULK_IMPORT_DUPLICATE_SHOT",
                    format!("本次导入中重复的镜头名称：{name}"),
                )
                .row(row.row_number),
            );
        }
        if errors.is_empty() {
            normalized.push(NormalizedImportRow {
                row_number: row.row_number,
                name: name.clone(),
                description: description.clone(),
                image_prompt: image_prompt.clone(),
                video_prompt: video_prompt.clone(),
            });
        }
        previews.push(ShotBulkImportRowPreview {
            row_number: row.row_number,
            name,
            description,
            image_prompt: image_prompt.map(|value| preview_text(&value)),
            video_prompt: video_prompt.map(|value| preview_text(&value)),
            errors,
            warnings: Vec::new(),
        });
    }
    (normalized, previews)
}

fn normalize_optional_prompt(
    value: Option<String>,
    row_number: usize,
    errors: &mut Vec<BulkIssue>,
) -> Option<String> {
    let value = value?;
    match canonical_prompt_text(&value) {
        Ok(value) if value.is_empty() => None,
        Ok(value) => Some(value),
        Err(message) => {
            errors.push(BulkIssue::new("BULK_IMPORT_INVALID_ROW", message).row(row_number));
            None
        }
    }
}

fn build_preview(
    rows: Vec<ShotBulkImportRowPreview>,
    warnings: Vec<BulkIssue>,
) -> ShotBulkImportPreview {
    let invalid = rows.iter().filter(|row| !row.errors.is_empty()).count();
    ShotBulkImportPreview {
        total: rows.len(),
        valid: rows.len() - invalid,
        invalid,
        warnings: warnings.len(),
        rows,
    }
}

fn canonical_name_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn preview_text(value: &str) -> String {
    value.chars().take(PREVIEW_TEXT_LIMIT).collect()
}

fn invalid_recipe_error(error: &ShotServiceError) -> ShotBulkServiceError {
    ShotBulkServiceError::Validation(vec![BulkIssue::new(
        "BULK_ASSIGNMENT_INVALID_RECIPE",
        error.to_string(),
    )])
}

fn transaction_error(code: &'static str, error: RepositoryError) -> ShotBulkServiceError {
    ShotBulkServiceError::TransactionFailed {
        code,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        order_scene_shot_ids, parse_import, prompt_for_shot, BulkPromptAssignmentRequest,
        BulkPromptSource, BulkStageConfigRequest, ResolvedPrompt, ShotBulkData,
        ShotBulkImportRequest, ShotBulkInputFormat, ShotBulkService, MAX_BULK_SHOT_IMPORT,
        MAX_BULK_SHOT_SCOPE,
    };
    use crate::application::ports::{ShotBulkRepository, ShotRecord, ShotRepository};
    use crate::application::prompt_library_service::PromptLibraryService;
    use crate::infrastructure::database::{
        initialize,
        repositories::{
            test_support, SqliteGenerationDefinitionRepository, SqlitePromptLibraryRepository,
            SqliteShotRepository,
        },
    };
    use crate::infrastructure::time::SystemClock;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn tsv(rows: usize) -> String {
        (0..rows)
            .map(|index| format!("镜头 {index:03}\t描述 {index}\t图片 {index}\t视频 {index}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn service(repository: Arc<SqliteShotRepository>, pool: &sqlx::SqlitePool) -> ShotBulkService {
        ShotBulkService::new(
            repository,
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone())),
            Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
            Arc::new(SystemClock),
        )
    }

    fn bulk_data(id: &str, ordinal: i64) -> ShotBulkData {
        ShotBulkData {
            shot: ShotRecord {
                id: id.to_owned(),
                project_id: "prj_default".to_owned(),
                ordinal,
                name: id.to_owned(),
                prompt_text: String::new(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: None,
                selected_video_asset_id: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            stage_configs: Vec::new(),
            stage_prompts: Vec::new(),
        }
    }

    #[test]
    fn episode_scope_dedupes_and_orders_by_project_global_ordinal() {
        let data = vec![
            bulk_data("shot-3", 30),
            bulk_data("shot-1", 10),
            bulk_data("shot-2", 20),
        ];
        let ordered = order_scene_shot_ids(
            &[
                vec!["shot-3".to_owned(), "shot-1".to_owned()],
                vec!["shot-2".to_owned(), "shot-1".to_owned()],
            ],
            &data,
        )
        .unwrap();
        assert_eq!(ordered, vec!["shot-1", "shot-2", "shot-3"]);
    }

    #[test]
    fn episode_scope_rejects_more_than_500_unique_shots_without_chunking() {
        let data = (0..=MAX_BULK_SHOT_SCOPE)
            .map(|index| bulk_data(&format!("shot-{index}"), index as i64))
            .collect::<Vec<_>>();
        let ids = data
            .iter()
            .map(|item| vec![item.shot.id.clone()])
            .collect::<Vec<_>>();
        let error = order_scene_shot_ids(&ids, &data).unwrap_err();
        assert!(error.starts_with("EPISODE_BULK_SCOPE_TOO_LARGE:"));
    }

    #[test]
    fn import_parser_covers_empty_limits_and_duplicate_rows() {
        assert!(parse_import(ShotBulkInputFormat::Tsv, "").is_err());
        assert_eq!(
            parse_import(ShotBulkInputFormat::Tsv, &tsv(500))
                .unwrap()
                .len(),
            MAX_BULK_SHOT_IMPORT
        );
        assert!(parse_import(ShotBulkInputFormat::Tsv, &tsv(501)).is_err());
        let duplicate = parse_import(
            ShotBulkInputFormat::Tsv,
            "镜头 01\t描述\n镜头 01\t另一个描述",
        )
        .unwrap();
        let (_, previews) = super::normalize_import(duplicate);
        assert!(previews[1]
            .errors
            .iter()
            .any(|issue| issue.code == "BULK_IMPORT_DUPLICATE_SHOT"));
    }

    #[test]
    fn clear_provenance_falls_back_to_legacy_prompt_for_partial_stage_rows() {
        let data = ShotBulkData {
            shot: ShotRecord {
                id: "sht_partial".to_owned(),
                project_id: "prj_default".to_owned(),
                ordinal: 1,
                name: "Partial".to_owned(),
                prompt_text: "legacy prompt".to_owned(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: None,
                selected_video_asset_id: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            stage_configs: Vec::new(),
            stage_prompts: Vec::new(),
        };
        let resolved = prompt_for_shot(
            &BulkPromptSource::ClearProvenance,
            crate::domain::ShotStage::Image,
            &data,
            &ResolvedPrompt {
                text: String::new(),
                prompt_entry_id: None,
                prompt_version_id: None,
            },
        );
        assert_eq!(resolved.text, "legacy prompt");
        assert_eq!(resolved.prompt_entry_id, None);
        assert_eq!(resolved.prompt_version_id, None);
    }

    #[tokio::test]
    async fn import_is_atomic_appends_ordinals_and_freezes_prompt_provenance() {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("bulk.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET id = 'prj_default' WHERE id = 'project-1'")
            .execute(&pool)
            .await
            .expect("test project should use the production id format");
        sqlx::query(
            "INSERT INTO workflows
             (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('wfl_kera2_t2i_local_v2', 'Krea2', 'image', 't2i', 'wfv-kera2',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("Krea2 workflow fixture should insert");
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('wfv-kera2', 'wfl_kera2_t2i_local_v2', '1', '{}', 'sha',
                     '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("Krea2 workflow version fixture should insert");
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('recipe-kera2', 'wfv-kera2', '1', 1, ?, 'sha', '2026-01-01T00:00:00Z')",
        )
        .bind(
            "schema_version: 1\n\
id: kera2\n\
name: Krea2\n\
workflow:\n\
  file: workflow_api.json\n\
inputs:\n\
  prompt:\n\
    type: textarea\n\
    label: Prompt\n\
    required: true\n\
    default: \"\"\n\
bindings:\n\
  - source: prompt\n\
    target:\n\
      node: \"6\"\n\
      input: text\n\
outputs:\n\
  - id: generated_image\n\
    type: image\n\
    node: \"9\"\n\
    required: true\n",
        )
        .execute(&pool)
        .await
        .expect("Krea2 recipe fixture should insert");
        let repository = Arc::new(SqliteShotRepository::new(pool.clone()));
        repository
            .insert(&crate::application::ports::ShotRecord {
                id: "sht_existing".to_owned(),
                project_id: "prj_default".to_owned(),
                ordinal: 0,
                name: "已有镜头".to_owned(),
                prompt_text: "旧描述".to_owned(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: None,
                selected_video_asset_id: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await
            .expect("existing shot should insert");

        let service = service(repository.clone(), &pool);
        let imported = service
            .commit_import(&ShotBulkImportRequest {
                project_id: "prj_default".to_owned(),
                format: ShotBulkInputFormat::Tsv,
                contents:
                    "镜头 02\t描述 02\t图片构图\t视频动作\n镜头 03\t描述 03\t图片 03\t视频 03"
                        .to_owned(),
            })
            .await
            .expect("valid import should commit");
        assert_eq!(
            imported
                .created
                .iter()
                .map(|shot| shot.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        let data = repository
            .list_bulk_data("prj_default")
            .await
            .expect("bulk data should load");
        assert_eq!(data.len(), 3);
        let imported_shot = data
            .iter()
            .find(|item| item.shot.name == "镜头 02")
            .unwrap();
        assert_eq!(
            imported_shot
                .stage_prompts
                .iter()
                .find(|prompt| prompt.stage == crate::domain::ShotStage::Image)
                .unwrap()
                .prompt_text,
            "图片构图"
        );

        let duplicate_preview = service
            .preview_import(&ShotBulkImportRequest {
                project_id: "prj_default".to_owned(),
                format: ShotBulkInputFormat::Tsv,
                contents: "镜头 02\t重复".to_owned(),
            })
            .await
            .expect("duplicate should be previewable");
        assert_eq!(duplicate_preview.invalid, 1);
        assert_eq!(duplicate_preview.warnings, 1);

        let before = repository.list("prj_default").await.unwrap().len();
        let invalid = service
            .commit_import(&ShotBulkImportRequest {
                project_id: "prj_default".to_owned(),
                format: ShotBulkInputFormat::Tsv,
                contents: "镜头 04\t有效\n\t无名称\n镜头 05\t不会写入".to_owned(),
            })
            .await
            .expect_err("invalid middle row must abort the whole import");
        assert_eq!(invalid.code(), "BULK_IMPORT_INVALID_ROW");
        assert_eq!(repository.list("prj_default").await.unwrap().len(), before);

        let prompt_service = PromptLibraryService::new(
            Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
            Arc::new(SystemClock),
        );
        let entry = prompt_service
            .create("prj_default", "prompt", "动作库", &[], "旧动作")
            .await
            .expect("prompt entry should create");
        let old_version = entry.versions[0].clone();
        service
            .assign_prompt(BulkPromptAssignmentRequest {
                project_id: "prj_default".to_owned(),
                stage: crate::domain::ShotStage::Video,
                shot_ids: imported
                    .created
                    .iter()
                    .map(|shot| shot.shot_id.clone())
                    .collect(),
                source: BulkPromptSource::PromptLibraryVersion {
                    prompt_entry_id: entry.id.clone(),
                    prompt_version_id: old_version.id.clone(),
                },
            })
            .await
            .expect("prompt assignment should commit");
        prompt_service
            .add_version("prj_default", &entry.id, "新动作")
            .await
            .expect("new prompt version should create");
        let frozen = repository.list_bulk_data("prj_default").await.unwrap();
        for shot in imported.created {
            let item = frozen
                .iter()
                .find(|item| item.shot.id == shot.shot_id)
                .unwrap();
            let prompt = item
                .stage_prompts
                .iter()
                .find(|prompt| prompt.stage == crate::domain::ShotStage::Video)
                .unwrap();
            assert_eq!(prompt.prompt_text, "旧动作");
            assert_eq!(
                prompt.prompt_version_id.as_deref(),
                Some(old_version.id.as_str())
            );
        }
    }

    #[tokio::test]
    async fn episode_preset_scope_updates_only_stage_config_and_preserves_shot_ownership() {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("bulk.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET id = 'prj_default' WHERE id = 'project-1'")
            .execute(&pool)
            .await
            .expect("test project should use the production id format");
        sqlx::query(
            "INSERT INTO workflows
             (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('wfl_kera2_t2i_local_v2', 'Krea2', 'image', 't2i', 'wfv-kera2-scope',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("Krea2 workflow fixture should insert");
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('wfv-kera2-scope', 'wfl_kera2_t2i_local_v2', '1', '{}', 'sha',
                     '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("Krea2 workflow version fixture should insert");
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('recipe-kera2-scope', 'wfv-kera2-scope', '1', 1, ?, 'sha', '2026-01-01T00:00:00Z')",
        )
        .bind(
            r#"schema_version: 1
id: kera2_scope
name: Krea2 Scope
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
outputs:
  - id: generated_image
    type: image
    node: "9"
    required: true
"#,
        )
        .execute(&pool)
        .await
        .expect("Krea2 recipe fixture should insert");
        for asset_id in ["ast_image", "ast_video", "ast_reference"] {
            sqlx::query(
                "INSERT INTO assets
                 (id, project_id, type, category, name, original_name, storage_path,
                  sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at)
                 VALUES (?, 'prj_default', 'image', 'source_image', ?, ?, ?, 'sha', 'image/png',
                         1, 1, 1, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .bind(asset_id)
            .bind(asset_id)
            .bind(asset_id)
            .bind(format!("assets/{asset_id}.png"))
            .execute(&pool)
            .await
            .expect("asset fixture should insert");
        }
        let repository = Arc::new(SqliteShotRepository::new(pool.clone()));
        for (id, ordinal) in [("shot-a", 20), ("shot-b", 10)] {
            repository
                .insert(&ShotRecord {
                    id: id.to_owned(),
                    project_id: "prj_default".to_owned(),
                    ordinal,
                    name: id.to_owned(),
                    prompt_text: "base".to_owned(),
                    prompt_entry_id: None,
                    prompt_version_id: None,
                    selected_image_asset_id: Some("ast_image".to_owned()),
                    selected_video_asset_id: Some("ast_video".to_owned()),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                })
                .await
                .expect("shot fixture should insert");
        }
        sqlx::query(
            "INSERT INTO shot_reference_assets (shot_id, stage, asset_id, ordinal)
             VALUES ('shot-a', 'image', 'ast_reference', 0)",
        )
        .execute(&pool)
        .await
        .expect("reference fixture should insert");
        sqlx::query(
            "INSERT INTO production_series
             (id, project_id, ordinal, name, description, created_at, updated_at)
             VALUES ('series-a', 'prj_default', 0, 'Series', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("series fixture should insert");
        sqlx::query(
            "INSERT INTO production_episodes
             (id, series_id, ordinal, name, description, created_at, updated_at)
             VALUES ('episode-a', 'series-a', 0, 'Episode', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("episode fixture should insert");
        sqlx::query(
            "INSERT INTO production_scenes
             (id, episode_id, ordinal, name, description, created_at, updated_at)
             VALUES ('scene-a', 'episode-a', 0, 'Scene', '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("scene fixture should insert");
        for (shot_id, ordinal) in [("shot-a", 0), ("shot-b", 1)] {
            sqlx::query(
                "INSERT INTO shot_scene_assignments
                 (shot_id, scene_id, ordinal, created_at, updated_at)
                 VALUES (?, 'scene-a', ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .bind(shot_id)
            .bind(ordinal)
            .execute(&pool)
            .await
            .expect("scene assignment fixture should insert");
        }
        let assignments_before = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT shot_id, scene_id, ordinal FROM shot_scene_assignments ORDER BY ordinal",
        )
        .fetch_all(&pool)
        .await
        .expect("assignments should load");

        let service = service(repository.clone(), &pool);
        let result = service
            .set_stage_config_for_scenes(
                BulkStageConfigRequest {
                    project_id: "prj_default".to_owned(),
                    stage: crate::domain::ShotStage::Image,
                    shot_ids: Vec::new(),
                    workflow_version_id: "wfv-kera2-scope".to_owned(),
                    recipe_id: "recipe-kera2-scope".to_owned(),
                    values: Default::default(),
                    prompt: None,
                },
                &[
                    vec!["shot-a".to_owned(), "shot-b".to_owned()],
                    vec!["shot-a".to_owned()],
                ],
            )
            .await
            .expect("episode preset apply should use the existing atomic bulk contract");
        assert_eq!(result.configured_shot_ids, vec!["shot-b", "shot-a"]);

        let shots = repository
            .list("prj_default")
            .await
            .expect("shots should load after preset apply");
        for shot in shots {
            assert_eq!(
                shot.shot.selected_image_asset_id.as_deref(),
                Some("ast_image")
            );
            assert_eq!(
                shot.shot.selected_video_asset_id.as_deref(),
                Some("ast_video")
            );
            assert_eq!(
                shot.shot.ordinal,
                if shot.shot.id == "shot-a" { 20 } else { 10 }
            );
            assert_eq!(
                shot.reference_assets.len(),
                usize::from(shot.shot.id == "shot-a")
            );
            assert_eq!(
                shot.stage_configs.len(),
                usize::from(shot.shot.id == "shot-a" || shot.shot.id == "shot-b")
            );
        }
        let assignments_after = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT shot_id, scene_id, ordinal FROM shot_scene_assignments ORDER BY ordinal",
        )
        .fetch_all(&pool)
        .await
        .expect("assignments should remain readable");
        assert_eq!(assignments_after, assignments_before);
    }
}
