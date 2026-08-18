use crate::application::ports::{
    Clock, ProjectRecord, ProjectRepository, PromptEntryRecord, PromptLibraryRepository,
    RepositoryError, ShotBulkData, ShotBulkRepository, ShotStagePromptRecord,
};
use crate::application::production_structure_service::{
    ProductionEpisodeTreeView, ProductionSceneTreeView, ProductionSeriesTreeView,
    ProductionStructureError, ProductionStructureService, ProductionStructureTreeView,
};
use crate::application::prompt_library_service::canonical_prompt_text;
use crate::application::prompt_template_service::{PromptTemplateError, PromptTemplateService};
use crate::application::reference_anchor_service::{
    ReferenceAnchorError, ReferenceAnchorService, ReferenceAnchorView,
};
use crate::domain::{
    PromptAnchor, PromptAnchorContext, PromptAnchorKind, PromptProjectContext, PromptShotContext,
    PromptStructureContext, PromptTemplateContext, ReferenceAnchorKind, ShotStage,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};

const MAX_SHOTS: usize = 500;
const MAX_ANCHORS: usize = 20;
const MAX_CUSTOM_VALUES: usize = 50;
const MAX_CUSTOM_KEY_CHARS: usize = 64;
const MAX_CUSTOM_VALUE_BYTES: usize = 4096;
const MAX_CUSTOM_TOTAL_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptTemplatePreviewInput {
    pub project_id: String,
    pub prompt_entry_id: String,
    pub prompt_version_id: String,
    pub shot_id: String,
    pub context_anchor_ids: Vec<String>,
    pub custom_values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptTemplateBulkPreviewInput {
    pub project_id: String,
    pub prompt_entry_id: String,
    pub prompt_version_id: String,
    pub shot_ids: Vec<String>,
    pub context_anchor_ids: Vec<String>,
    pub custom_values: BTreeMap<String, String>,
    pub preview_limit: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptTemplateApplyInput {
    pub project_id: String,
    pub prompt_entry_id: String,
    pub prompt_version_id: String,
    pub stage: ShotStage,
    pub shot_ids: Vec<String>,
    pub context_anchor_ids: Vec<String>,
    pub custom_values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplatePreview {
    pub shot_id: String,
    pub shot_name: String,
    pub template_text: String,
    pub rendered_text: String,
    pub variables: Vec<String>,
    pub context: Value,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateBulkPreviewEntry {
    pub shot_id: String,
    pub shot_name: String,
    pub rendered_text: String,
    pub variables: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateIssue {
    pub shot_id: Option<String>,
    pub shot_name: Option<String>,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateBulkPreview {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub preview_entries: Vec<PromptTemplateBulkPreviewEntry>,
    pub issues: Vec<PromptTemplateIssue>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateApplyResult {
    pub stage: String,
    pub updated_count: usize,
    pub shot_ids: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

pub struct PromptTemplateBulkService {
    project_repository: Arc<dyn ProjectRepository>,
    prompt_library_repository: Arc<dyn PromptLibraryRepository>,
    shot_bulk_repository: Arc<dyn ShotBulkRepository>,
    production_structure_service: Arc<ProductionStructureService>,
    reference_anchor_service: Arc<ReferenceAnchorService>,
    template_service: Arc<PromptTemplateService>,
    clock: Arc<dyn Clock>,
}

impl PromptTemplateBulkService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_repository: Arc<dyn ProjectRepository>,
        prompt_library_repository: Arc<dyn PromptLibraryRepository>,
        shot_bulk_repository: Arc<dyn ShotBulkRepository>,
        production_structure_service: Arc<ProductionStructureService>,
        reference_anchor_service: Arc<ReferenceAnchorService>,
        template_service: Arc<PromptTemplateService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            project_repository,
            prompt_library_repository,
            shot_bulk_repository,
            production_structure_service,
            reference_anchor_service,
            template_service,
            clock,
        }
    }

    pub async fn preview(
        &self,
        request: PromptTemplatePreviewInput,
    ) -> Result<PromptTemplatePreview, PromptTemplateBulkError> {
        validate_project_id(&request.project_id)?;
        validate_anchor_ids(&request.context_anchor_ids)?;
        validate_custom_values(&request.custom_values)?;
        let template = self
            .load_template(
                &request.project_id,
                &request.prompt_entry_id,
                &request.prompt_version_id,
            )
            .await?;
        let loaded = self
            .load_context(&request.project_id, &request.context_anchor_ids)
            .await?;
        let shot = loaded.shots_by_id.get(&request.shot_id).ok_or_else(|| {
            PromptTemplateBulkError::NotFound(format!("SHOT_NOT_FOUND: {}", request.shot_id))
        })?;
        let rendered = self
            .render_shot(&template.text, shot, &loaded, &request.custom_values)
            .map_err(|error| PromptTemplateBulkError::Validation(vec![error]))?;
        Ok(PromptTemplatePreview {
            shot_id: shot.shot.shot.id.clone(),
            shot_name: shot.shot.shot.name.clone(),
            template_text: template.text,
            rendered_text: rendered.rendered_text,
            variables: rendered.variables,
            context: rendered.context,
            warnings: Vec::new(),
        })
    }

    pub async fn preview_bulk(
        &self,
        request: PromptTemplateBulkPreviewInput,
    ) -> Result<PromptTemplateBulkPreview, PromptTemplateBulkError> {
        validate_project_id(&request.project_id)?;
        validate_shot_ids(&request.shot_ids)?;
        validate_anchor_ids(&request.context_anchor_ids)?;
        validate_custom_values(&request.custom_values)?;
        let preview_limit = request.preview_limit.unwrap_or(20).min(50);
        let template = self
            .load_template(
                &request.project_id,
                &request.prompt_entry_id,
                &request.prompt_version_id,
            )
            .await?;
        let loaded = self
            .load_context(&request.project_id, &request.context_anchor_ids)
            .await?;

        let mut valid = 0;
        let mut preview_entries = Vec::new();
        let mut issues = Vec::new();
        for shot_id in &request.shot_ids {
            let Some(shot) = loaded.shots_by_id.get(shot_id) else {
                issues.push(issue(
                    Some(shot_id.clone()),
                    None,
                    "SHOT_NOT_FOUND",
                    format!("镜头 {shot_id} 不属于当前项目。"),
                ));
                continue;
            };
            match self.render_shot(&template.text, shot, &loaded, &request.custom_values) {
                Ok(rendered) => {
                    valid += 1;
                    if preview_entries.len() < preview_limit {
                        preview_entries.push(PromptTemplateBulkPreviewEntry {
                            shot_id: shot.shot.shot.id.clone(),
                            shot_name: shot.shot.shot.name.clone(),
                            rendered_text: rendered.rendered_text,
                            variables: rendered.variables,
                        });
                    }
                }
                Err(error) => issues.push(error),
            }
        }
        Ok(PromptTemplateBulkPreview {
            total: request.shot_ids.len(),
            valid,
            invalid: issues.len(),
            preview_entries,
            issues,
        })
    }

    pub async fn apply(
        &self,
        request: PromptTemplateApplyInput,
    ) -> Result<PromptTemplateApplyResult, PromptTemplateBulkError> {
        validate_project_id(&request.project_id)?;
        validate_shot_ids(&request.shot_ids)?;
        validate_anchor_ids(&request.context_anchor_ids)?;
        validate_custom_values(&request.custom_values)?;
        let template = self
            .load_template(
                &request.project_id,
                &request.prompt_entry_id,
                &request.prompt_version_id,
            )
            .await?;
        let loaded = self
            .load_context(&request.project_id, &request.context_anchor_ids)
            .await?;

        let mut issues = Vec::new();
        let mut rendered_prompts = Vec::with_capacity(request.shot_ids.len());
        for shot_id in &request.shot_ids {
            let Some(shot) = loaded.shots_by_id.get(shot_id) else {
                issues.push(issue(
                    Some(shot_id.clone()),
                    None,
                    "SHOT_NOT_FOUND",
                    format!("镜头 {shot_id} 不属于当前项目。"),
                ));
                continue;
            };
            match self.render_shot(&template.text, shot, &loaded, &request.custom_values) {
                Ok(rendered) => rendered_prompts.push((shot, rendered)),
                Err(error) => issues.push(error),
            }
        }
        if !issues.is_empty() {
            return Err(PromptTemplateBulkError::Validation(issues));
        }

        // The same instant is deliberately used for every stage snapshot.
        // The repository performs the complete upsert in one transaction.
        let updated_at = self.clock.now();
        let updates = rendered_prompts
            .into_iter()
            .map(|(shot, rendered)| ShotStagePromptRecord {
                shot_id: shot.shot.shot.id.clone(),
                stage: request.stage,
                prompt_text: rendered.rendered_text,
                prompt_entry_id: Some(request.prompt_entry_id.clone()),
                prompt_version_id: Some(request.prompt_version_id.clone()),
                updated_at,
            })
            .collect::<Vec<_>>();
        self.shot_bulk_repository
            .update_stage_prompts_atomic(&request.project_id, &updates)
            .await?;
        Ok(PromptTemplateApplyResult {
            stage: request.stage.as_str().to_owned(),
            updated_count: updates.len(),
            shot_ids: request.shot_ids,
            updated_at,
        })
    }

    async fn load_template(
        &self,
        project_id: &str,
        prompt_entry_id: &str,
        prompt_version_id: &str,
    ) -> Result<PromptVersion, PromptTemplateBulkError> {
        if prompt_entry_id.trim().is_empty() || prompt_version_id.trim().is_empty() {
            return Err(PromptTemplateBulkError::InvalidInput(
                "PROMPT_TEMPLATE_VERSION_REQUIRED: prompt entry and version are required"
                    .to_owned(),
            ));
        }
        let entry = self
            .prompt_library_repository
            .find_by_id(project_id, prompt_entry_id)
            .await?
            .ok_or_else(|| {
                PromptTemplateBulkError::NotFound(format!("PROMPT_NOT_FOUND: {prompt_entry_id}"))
            })?;
        if entry.project_id != project_id {
            return Err(PromptTemplateBulkError::InvalidInput(
                "PROMPT_TEMPLATE_PROJECT_MISMATCH: prompt entry does not belong to project"
                    .to_owned(),
            ));
        }
        if entry.kind != "prompt" {
            return Err(PromptTemplateBulkError::InvalidInput(
                "PROMPT_TEMPLATE_KIND_INVALID: only prompt entries can be rendered as templates"
                    .to_owned(),
            ));
        }
        let version = self
            .prompt_library_repository
            .list_versions(project_id, prompt_entry_id)
            .await?
            .into_iter()
            .find(|version| version.id == prompt_version_id)
            .ok_or_else(|| {
                PromptTemplateBulkError::NotFound(format!(
                    "PROMPT_TEMPLATE_VERSION_NOT_FOUND: {prompt_version_id}"
                ))
            })?;
        if version.prompt_id != entry.id {
            return Err(PromptTemplateBulkError::InvalidInput(
                "PROMPT_TEMPLATE_PROVENANCE_INVALID: version does not belong to entry".to_owned(),
            ));
        }
        Ok(PromptVersion {
            entry,
            text: version.text,
        })
    }

    async fn load_context(
        &self,
        project_id: &str,
        anchor_ids: &[String],
    ) -> Result<LoadedContext, PromptTemplateBulkError> {
        let project = self
            .project_repository
            .find_by_id(project_id)
            .await?
            .ok_or_else(|| {
                PromptTemplateBulkError::NotFound(format!("PROJECT_NOT_FOUND: {project_id}"))
            })?;
        // Each service call is a set-based load. No call below depends on the
        // number of requested shots, which keeps 500-shot apply predictable.
        let structure = self
            .production_structure_service
            .tree(project_id)
            .await
            .map_err(PromptTemplateBulkError::from_structure)?;
        let shots = self.shot_bulk_repository.list_bulk_data(project_id).await?;
        let anchors = self
            .reference_anchor_service
            .list(project_id)
            .await
            .map_err(PromptTemplateBulkError::from_anchor)?;
        let selected_anchors = select_anchors(&anchors, anchor_ids)?;
        let structure_by_shot = index_structure(&structure);
        let shots_by_id = shots
            .into_iter()
            .map(|shot| {
                let shot_id = shot.shot.id.clone();
                (
                    shot_id.clone(),
                    ResolvedShot {
                        shot,
                        structure: structure_by_shot.get(&shot_id).cloned(),
                    },
                )
            })
            .collect();
        Ok(LoadedContext {
            project,
            selected_anchors,
            shots_by_id,
        })
    }

    fn render_shot(
        &self,
        template_text: &str,
        shot: &ResolvedShot,
        loaded: &LoadedContext,
        custom_values: &BTreeMap<String, String>,
    ) -> Result<RenderedShot, PromptTemplateIssue> {
        let context = build_context(
            &loaded.project,
            &shot.shot.shot,
            shot.structure.as_ref(),
            &loaded.selected_anchors,
            custom_values,
        );
        let analysis = self
            .template_service
            .analyze(template_text)
            .map_err(|error| render_issue(&shot.shot.shot, error))?;
        let rendered_text = self
            .template_service
            .render(template_text, &context.template)
            .map_err(|error| render_issue(&shot.shot.shot, error))?;
        let rendered_text = canonical_prompt_text(&rendered_text).map_err(|message| {
            issue(
                Some(shot.shot.shot.id.clone()),
                Some(shot.shot.shot.name.clone()),
                "PROMPT_TEMPLATE_RESULT_TOO_LARGE",
                message,
            )
        })?;
        Ok(RenderedShot {
            rendered_text,
            variables: analysis.variables,
            context: context.view,
        })
    }
}

#[derive(Clone, Debug)]
struct PromptVersion {
    #[allow(dead_code)]
    entry: PromptEntryRecord,
    text: String,
}

#[derive(Clone, Debug)]
struct LoadedContext {
    project: ProjectRecord,
    selected_anchors: Vec<SelectedAnchor>,
    shots_by_id: HashMap<String, ResolvedShot>,
}

#[derive(Clone, Debug)]
struct ResolvedShot {
    shot: ShotBulkData,
    structure: Option<StructureContext>,
}

#[derive(Clone, Debug)]
struct StructureContext {
    series: ContextEntity,
    episode: ContextEntity,
    scene: ContextEntity,
}

#[derive(Clone, Debug)]
struct ContextEntity {
    id: String,
    name: String,
    description: String,
    ordinal: u32,
}

#[derive(Clone, Debug)]
struct SelectedAnchor {
    id: String,
    kind: ReferenceAnchorKind,
    name: String,
    description: String,
}

#[derive(Clone, Debug)]
struct RenderedShot {
    rendered_text: String,
    variables: Vec<String>,
    context: Value,
}

struct BuiltContext {
    template: PromptTemplateContext,
    view: Value,
}

fn index_structure(tree: &ProductionStructureTreeView) -> HashMap<String, StructureContext> {
    let mut index = HashMap::new();
    for series in &tree.series {
        for episode in &series.episodes {
            for scene in &episode.scenes {
                let structure = StructureContext {
                    series: series_entity(series),
                    episode: episode_entity(episode),
                    scene: scene_entity(scene),
                };
                for shot_id in &scene.shot_ids {
                    index.insert(shot_id.clone(), structure.clone());
                }
            }
        }
    }
    index
}

fn series_entity(series: &ProductionSeriesTreeView) -> ContextEntity {
    ContextEntity {
        id: series.series.id.clone(),
        name: series.series.name.clone(),
        description: series.series.description.clone(),
        ordinal: series.series.ordinal,
    }
}

fn episode_entity(episode: &ProductionEpisodeTreeView) -> ContextEntity {
    ContextEntity {
        id: episode.episode.id.clone(),
        name: episode.episode.name.clone(),
        description: episode.episode.description.clone(),
        ordinal: episode.episode.ordinal,
    }
}

fn scene_entity(scene: &ProductionSceneTreeView) -> ContextEntity {
    ContextEntity {
        id: scene.scene.id.clone(),
        name: scene.scene.name.clone(),
        description: scene.scene.description.clone(),
        ordinal: scene.scene.ordinal,
    }
}

fn build_context(
    project: &ProjectRecord,
    shot: &crate::application::ports::ShotRecord,
    structure: Option<&StructureContext>,
    anchors: &[SelectedAnchor],
    custom_values: &BTreeMap<String, String>,
) -> BuiltContext {
    let structure_context = structure.map(|value| PromptStructureContext {
        id: value.series.id.clone(),
        name: value.series.name.clone(),
        description: value.series.description.clone(),
        number: value.series.ordinal + 1,
    });
    let episode_context = structure.map(|value| PromptStructureContext {
        id: value.episode.id.clone(),
        name: value.episode.name.clone(),
        description: value.episode.description.clone(),
        number: value.episode.ordinal + 1,
    });
    let scene_context = structure.map(|value| PromptStructureContext {
        id: value.scene.id.clone(),
        name: value.scene.name.clone(),
        description: value.scene.description.clone(),
        number: value.scene.ordinal + 1,
    });
    let anchor_context = anchors
        .iter()
        .map(|anchor| {
            (
                match anchor.kind {
                    ReferenceAnchorKind::Character => PromptAnchorKind::Character,
                    ReferenceAnchorKind::Scene => PromptAnchorKind::Scene,
                    ReferenceAnchorKind::Prop => PromptAnchorKind::Prop,
                    ReferenceAnchorKind::Style => PromptAnchorKind::Style,
                },
                PromptAnchor {
                    id: anchor.id.clone(),
                    name: anchor.name.clone(),
                    description: anchor.description.clone(),
                },
            )
        })
        .collect::<Vec<_>>();
    let template = PromptTemplateContext::new(
        PromptProjectContext {
            id: project.id.clone(),
            name: project.name.clone(),
            description: project.description.clone(),
        },
        PromptShotContext {
            id: shot.id.clone(),
            name: shot.name.clone(),
            number: shot.ordinal as u32 + 1,
            base_prompt: shot.prompt_text.clone(),
        },
    )
    .with_custom_values(custom_values.clone())
    .with_anchors(PromptAnchorContext::from_selected(anchor_context));
    let template = if let Some(value) = structure_context {
        template.with_series(value)
    } else {
        template
    };
    let template = if let Some(value) = episode_context {
        template.with_episode(value)
    } else {
        template
    };
    let template = if let Some(value) = scene_context {
        template.with_scene(value)
    } else {
        template
    };
    BuiltContext {
        view: context_view(&template),
        template,
    }
}

fn context_view(context: &PromptTemplateContext) -> Value {
    let structure = |value: &Option<PromptStructureContext>| {
        value.as_ref().map_or(Value::Null, |value| {
            json!({
                "id": value.id,
                "name": value.name,
                "description": value.description,
                "number": value.number,
            })
        })
    };
    let anchors = |values: &[PromptAnchor]| {
        json!({
            "names": values.iter().map(|value| value.name.clone()).collect::<Vec<_>>().join("、"),
            "context": values.iter().map(|value| {
                if value.description.is_empty() { value.name.clone() } else { format!("{}：{}", value.name, value.description) }
            }).collect::<Vec<_>>().join("\n"),
        })
    };
    json!({
        "project": {
            "id": context.project.id,
            "name": context.project.name,
            "description": context.project.description.clone().unwrap_or_default(),
        },
        "series": structure(&context.series),
        "episode": structure(&context.episode),
        "scene": structure(&context.scene),
        "shot": {
            "id": context.shot.id,
            "name": context.shot.name,
            "number": context.shot.number,
            "basePrompt": context.shot.base_prompt,
        },
        "anchors": {
            "character": anchors(&context.anchors.character),
            "scene": anchors(&context.anchors.scene),
            "prop": anchors(&context.anchors.prop),
            "style": anchors(&context.anchors.style),
            "all": anchors(&context.anchors.all),
        },
        "custom": context.custom_values,
    })
}

#[cfg(test)]
fn anchor_context_value(anchors: &[SelectedAnchor]) -> Value {
    let groups = [
        (ReferenceAnchorKind::Character, "character"),
        (ReferenceAnchorKind::Scene, "scene"),
        (ReferenceAnchorKind::Prop, "prop"),
        (ReferenceAnchorKind::Style, "style"),
    ];
    let mut result = serde_json::Map::new();
    for (kind, key) in groups {
        let matching = anchors.iter().filter(|anchor| anchor.kind == kind);
        let names = matching
            .clone()
            .map(|anchor| anchor.name.clone())
            .collect::<Vec<_>>();
        let contexts = matching
            .map(|anchor| format_anchor_context(&anchor.name, &anchor.description))
            .collect::<Vec<_>>();
        result.insert(
            key.to_owned(),
            json!({
                "names": names.join("、"),
                "context": contexts.join("\n"),
            }),
        );
    }
    // `all` follows the caller's selection order, not the kind grouping order.
    result.insert(
        "all".to_owned(),
        json!({
            "names": anchors.iter().map(|anchor| anchor.name.clone()).collect::<Vec<_>>().join("、"),
            "context": anchors.iter().map(|anchor| format_anchor_context(&anchor.name, &anchor.description)).filter(|value| !value.is_empty()).collect::<Vec<_>>().join("\n"),
        }),
    );
    Value::Object(result)
}

#[cfg(test)]
fn format_anchor_context(name: &str, description: &str) -> String {
    if description.is_empty() {
        name.to_owned()
    } else {
        format!("{name}：{description}")
    }
}

fn select_anchors(
    available: &[ReferenceAnchorView],
    ids: &[String],
) -> Result<Vec<SelectedAnchor>, PromptTemplateBulkError> {
    let by_id = available
        .iter()
        .map(|anchor| (anchor.id.as_str(), anchor))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    ids.iter()
        .map(|id| {
            if !seen.insert(id) {
                return Err(PromptTemplateBulkError::InvalidInput(
                    "PROMPT_TEMPLATE_ANCHOR_DUPLICATE: context anchors must be unique".to_owned(),
                ));
            }
            let anchor = by_id.get(id.as_str()).ok_or_else(|| {
                PromptTemplateBulkError::InvalidInput(format!(
                    "PROMPT_TEMPLATE_ANCHOR_PROJECT_MISMATCH: anchor {id} is not in the project"
                ))
            })?;
            Ok(SelectedAnchor {
                id: anchor.id.clone(),
                kind: anchor.kind,
                name: anchor.name.clone(),
                description: anchor.description.clone(),
            })
        })
        .collect()
}

fn validate_project_id(project_id: &str) -> Result<(), PromptTemplateBulkError> {
    if project_id.trim().is_empty() {
        return Err(PromptTemplateBulkError::InvalidInput(
            "PROJECT_ID_REQUIRED: project id must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn validate_shot_ids(shot_ids: &[String]) -> Result<(), PromptTemplateBulkError> {
    if shot_ids.is_empty() || shot_ids.len() > MAX_SHOTS {
        return Err(PromptTemplateBulkError::InvalidInput(
            "PROMPT_TEMPLATE_SHOT_LIMIT: shot_ids must contain 1..500 shots".to_owned(),
        ));
    }
    if shot_ids.iter().collect::<HashSet<_>>().len() != shot_ids.len() {
        return Err(PromptTemplateBulkError::InvalidInput(
            "PROMPT_TEMPLATE_SHOT_DUPLICATE: shot_ids must be unique".to_owned(),
        ));
    }
    if shot_ids.iter().any(|id| id.trim().is_empty()) {
        return Err(PromptTemplateBulkError::InvalidInput(
            "PROMPT_TEMPLATE_SHOT_ID_REQUIRED: shot ids must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn validate_anchor_ids(ids: &[String]) -> Result<(), PromptTemplateBulkError> {
    if ids.len() > MAX_ANCHORS {
        return Err(PromptTemplateBulkError::InvalidInput(
            "PROMPT_TEMPLATE_ANCHOR_LIMIT: at most 20 context anchors are allowed".to_owned(),
        ));
    }
    if ids.iter().collect::<HashSet<_>>().len() != ids.len() {
        return Err(PromptTemplateBulkError::InvalidInput(
            "PROMPT_TEMPLATE_ANCHOR_DUPLICATE: context anchors must be unique".to_owned(),
        ));
    }
    Ok(())
}

fn validate_custom_values(
    values: &BTreeMap<String, String>,
) -> Result<(), PromptTemplateBulkError> {
    if values.len() > MAX_CUSTOM_VALUES {
        return Err(PromptTemplateBulkError::InvalidInput(
            "PROMPT_TEMPLATE_CUSTOM_VALUE_LIMIT: at most 50 custom variables are allowed"
                .to_owned(),
        ));
    }
    let mut total_bytes = 0;
    for (key, value) in values {
        let key_length = key.chars().count();
        if key_length == 0 || key_length > MAX_CUSTOM_KEY_CHARS {
            return Err(PromptTemplateBulkError::InvalidInput(
                "PROMPT_TEMPLATE_CUSTOM_KEY_INVALID: custom keys must be 1..64 characters"
                    .to_owned(),
            ));
        }
        if value.as_bytes().len() > MAX_CUSTOM_VALUE_BYTES {
            return Err(PromptTemplateBulkError::InvalidInput(
                "PROMPT_TEMPLATE_CUSTOM_VALUE_TOO_LARGE: each custom value must be at most 4096 bytes".to_owned(),
            ));
        }
        total_bytes += value.as_bytes().len();
    }
    if total_bytes > MAX_CUSTOM_TOTAL_BYTES {
        return Err(PromptTemplateBulkError::InvalidInput(
            "PROMPT_TEMPLATE_CUSTOM_TOTAL_TOO_LARGE: custom values must total at most 32 KiB"
                .to_owned(),
        ));
    }
    Ok(())
}

fn render_issue(
    shot: &crate::application::ports::ShotRecord,
    error: PromptTemplateError,
) -> PromptTemplateIssue {
    issue(
        Some(shot.id.clone()),
        Some(shot.name.clone()),
        error.code(),
        error.to_string(),
    )
}

fn issue(
    shot_id: Option<String>,
    shot_name: Option<String>,
    code: impl Into<String>,
    message: impl Into<String>,
) -> PromptTemplateIssue {
    PromptTemplateIssue {
        shot_id,
        shot_name,
        code: code.into(),
        message: message.into(),
    }
}

#[derive(Debug)]
pub enum PromptTemplateBulkError {
    InvalidInput(String),
    NotFound(String),
    Validation(Vec<PromptTemplateIssue>),
    Repository(RepositoryError),
}

impl PromptTemplateBulkError {
    fn from_structure(error: ProductionStructureError) -> Self {
        match error {
            ProductionStructureError::InvalidInput(message)
            | ProductionStructureError::NotFound(message) => Self::InvalidInput(message),
            ProductionStructureError::Repository(error) => Self::Repository(error),
        }
    }

    fn from_anchor(error: ReferenceAnchorError) -> Self {
        match error {
            ReferenceAnchorError::InvalidInput(message) => Self::InvalidInput(message),
            ReferenceAnchorError::NotFound(message) => Self::NotFound(message),
            ReferenceAnchorError::AssetNotFound(asset_id) => {
                Self::InvalidInput(format!("REFERENCE_ANCHOR_ASSET_NOT_FOUND: {asset_id}"))
            }
            ReferenceAnchorError::AssetProjectMismatch {
                asset_id,
                project_id,
            } => Self::InvalidInput(format!(
                "REFERENCE_ANCHOR_ASSET_PROJECT_MISMATCH: {asset_id} / {project_id}"
            )),
            ReferenceAnchorError::ImageRequired(asset_id) => {
                Self::InvalidInput(format!("REFERENCE_ANCHOR_IMAGE_REQUIRED: {asset_id}"))
            }
            ReferenceAnchorError::Repository(error) => Self::Repository(error),
        }
    }
}

impl fmt::Display for PromptTemplateBulkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) => formatter.write_str(message),
            Self::Validation(issues) => {
                let message = issues
                    .iter()
                    .map(|issue| format!("{}: {}", issue.code, issue.message))
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(
                    formatter,
                    "PROMPT_TEMPLATE_APPLY_VALIDATION_FAILED: {message}"
                )
            }
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PromptTemplateBulkError {}

impl From<RepositoryError> for PromptTemplateBulkError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        anchor_context_value, validate_custom_values, PromptTemplateApplyInput,
        PromptTemplateBulkError, PromptTemplateBulkService, PromptTemplatePreviewInput,
    };
    use crate::application::ports::{
        Clock, ProductionStructureRepository, ProjectRepository, PromptLibraryRepository,
        ReferenceAnchorRepository, ShotBulkRepository, ShotRecord,
    };
    use crate::application::production_structure_service::{
        CreateEpisodeRequest, CreateSceneRequest, CreateSeriesRequest, ProductionStructureService,
    };
    use crate::application::prompt_library_service::PromptLibraryService;
    use crate::application::prompt_template_service::PromptTemplateService;
    use crate::application::reference_anchor_service::ReferenceAnchorService;
    use crate::domain::{ReferenceAnchorKind, ShotStage};
    use crate::infrastructure::database::repositories::test_support;
    use crate::infrastructure::database::{
        initialize, SqliteAssetRepository, SqliteProductionStructureRepository,
        SqliteProjectRepository, SqlitePromptLibraryRepository, SqliteReferenceAnchorRepository,
        SqliteShotRepository,
    };
    use crate::infrastructure::time::SystemClock;
    use chrono::Utc;
    use serde_json::json;
    use std::{collections::BTreeMap, sync::Arc};
    use tempfile::{tempdir, TempDir};

    #[test]
    fn custom_limits_are_enforced_before_database_work() {
        let mut values = BTreeMap::new();
        values.insert("camera".to_owned(), "x".repeat(4097));
        assert!(validate_custom_values(&values).is_err());
    }

    #[test]
    fn anchor_context_keeps_input_order_and_metadata_only() {
        let value = anchor_context_value(&[
            super::SelectedAnchor {
                id: "anc_a".to_owned(),
                kind: ReferenceAnchorKind::Character,
                name: "甲".to_owned(),
                description: "角色设定".to_owned(),
            },
            super::SelectedAnchor {
                id: "anc_b".to_owned(),
                kind: ReferenceAnchorKind::Style,
                name: "乙".to_owned(),
                description: String::new(),
            },
        ]);
        assert_eq!(value["character"]["names"], json!("甲"));
        assert_eq!(value["character"]["context"], json!("甲：角色设定"));
        assert_eq!(value["all"]["names"], json!("甲、乙"));
        assert_eq!(value["all"]["context"], json!("甲：角色设定\n乙"));
        assert!(value.to_string().contains("anc_a") == false);
    }

    #[test]
    fn stage_prompt_contract_uses_existing_stage_type() {
        assert_eq!(ShotStage::Image.as_str(), "image");
        let shot = ShotRecord {
            id: "s".to_owned(),
            project_id: "p".to_owned(),
            ordinal: 0,
            name: "Shot".to_owned(),
            prompt_text: "base".to_owned(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(shot.prompt_text, "base");
    }

    async fn setup_service() -> (
        TempDir,
        sqlx::SqlitePool,
        PromptTemplateBulkService,
        String,
        String,
        String,
    ) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let now = "2026-01-01T00:00:00Z";
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES ('prj_default', '地藏经', '项目描述', ?, ?, ?)",
        )
        .bind(directory.path().to_string_lossy().to_string())
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
        for (id, ordinal, name) in [("shot-a", 0, "佛陀端坐"), ("shot-b", 1, "大众听法")] {
            sqlx::query(
                "INSERT INTO shots (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
                 VALUES (?, 'prj_default', ?, ?, '基础画面', ?, ?)",
            )
            .bind(id)
            .bind(ordinal)
            .bind(name)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO reference_anchors
             (id, project_id, kind, name, normalized_name, description, created_at, updated_at)
              VALUES ('anc_character', 'prj_default', 'CHARACTER', '释迦牟尼佛', '释迦牟尼佛', '成熟庄严佛相', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let project_repository: Arc<dyn ProjectRepository> =
            Arc::new(SqliteProjectRepository::new(pool.clone()));
        let prompt_repository: Arc<dyn PromptLibraryRepository> =
            Arc::new(SqlitePromptLibraryRepository::new(pool.clone()));
        let shot_bulk_repository: Arc<dyn ShotBulkRepository> =
            Arc::new(SqliteShotRepository::new(pool.clone()));
        let structure_repository: Arc<dyn ProductionStructureRepository> =
            Arc::new(SqliteProductionStructureRepository::new(pool.clone()));
        let production_structure_service = Arc::new(ProductionStructureService::new(
            structure_repository,
            clock.clone(),
        ));
        let anchor_repository: Arc<dyn ReferenceAnchorRepository> =
            Arc::new(SqliteReferenceAnchorRepository::new(pool.clone()));
        let reference_anchor_service = Arc::new(ReferenceAnchorService::new(
            anchor_repository,
            Arc::new(SqliteAssetRepository::new(pool.clone())),
            clock.clone(),
        ));
        let prompt_library_service =
            PromptLibraryService::new(prompt_repository.clone(), clock.clone());
        let prompt = prompt_library_service
            .create(
                "prj_default",
                "prompt",
                "场景模板",
                &[],
                "{{project.name}}/{{series.name}}/{{episode.number}}/{{scene.name}}/{{shot.name}}/{{anchors.character.context}}/{{custom.camera}}",
            )
            .await
            .unwrap();
        let series = production_structure_service
            .create_series(CreateSeriesRequest {
                project_id: "prj_default".to_owned(),
                name: "第一季".to_owned(),
                description: String::new(),
            })
            .await
            .unwrap();
        let episode = production_structure_service
            .create_episode(CreateEpisodeRequest {
                project_id: "prj_default".to_owned(),
                series_id: series.id,
                name: "第一集".to_owned(),
                description: String::new(),
            })
            .await
            .unwrap();
        let scene = production_structure_service
            .create_scene(CreateSceneRequest {
                project_id: "prj_default".to_owned(),
                episode_id: episode.id,
                name: "忉利天宫".to_owned(),
                description: "佛陀于忉利天为母说法".to_owned(),
            })
            .await
            .unwrap();
        production_structure_service
            .assign_shots("prj_default", &scene.id, &["shot-a".to_owned()])
            .await
            .unwrap();
        let service = PromptTemplateBulkService::new(
            project_repository,
            prompt_repository,
            shot_bulk_repository,
            production_structure_service,
            reference_anchor_service,
            Arc::new(PromptTemplateService::new()),
            clock,
        );
        (
            directory,
            pool,
            service,
            prompt.id,
            prompt.versions[0].id.clone(),
            scene.id,
        )
    }

    #[tokio::test]
    async fn preview_and_apply_load_context_once_and_freeze_atomically() {
        let (_directory, pool, service, prompt_id, version_id, scene_id) = setup_service().await;
        let mut custom_values = BTreeMap::new();
        custom_values.insert("camera".to_owned(), "中景缓慢推进".to_owned());
        let failed = service
            .apply(PromptTemplateApplyInput {
                project_id: "prj_default".to_owned(),
                prompt_entry_id: prompt_id.clone(),
                prompt_version_id: version_id.clone(),
                stage: ShotStage::Image,
                shot_ids: vec!["shot-a".to_owned(), "shot-b".to_owned()],
                context_anchor_ids: vec!["anc_character".to_owned()],
                custom_values: custom_values.clone(),
            })
            .await
            .unwrap_err();
        assert!(
            matches!(failed, PromptTemplateBulkError::Validation(_)),
            "unexpected atomic validation error: {failed:?}"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shot_stage_prompts")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );

        let preview = service
            .preview(PromptTemplatePreviewInput {
                project_id: "prj_default".to_owned(),
                prompt_entry_id: prompt_id.clone(),
                prompt_version_id: version_id.clone(),
                shot_id: "shot-a".to_owned(),
                context_anchor_ids: vec!["anc_character".to_owned()],
                custom_values: custom_values.clone(),
            })
            .await
            .unwrap();
        assert!(preview
            .rendered_text
            .contains("地藏经/第一季/1/忉利天宫/佛陀端坐"));
        assert!(preview.rendered_text.contains("释迦牟尼佛：成熟庄严佛相"));
        assert!(preview.rendered_text.contains("中景缓慢推进"));

        service
            .apply(PromptTemplateApplyInput {
                project_id: "prj_default".to_owned(),
                prompt_entry_id: prompt_id,
                prompt_version_id: version_id,
                stage: ShotStage::Image,
                shot_ids: vec!["shot-a".to_owned()],
                context_anchor_ids: vec!["anc_character".to_owned()],
                custom_values,
            })
            .await
            .unwrap();
        let frozen: String = sqlx::query_scalar(
            "SELECT prompt_text FROM shot_stage_prompts WHERE shot_id = 'shot-a' AND stage = 'image'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE production_scenes SET name = '忉利天宫·新版' WHERE id = ?")
            .bind(scene_id)
            .execute(&pool)
            .await
            .unwrap();
        let after_edit: String = sqlx::query_scalar(
            "SELECT prompt_text FROM shot_stage_prompts WHERE shot_id = 'shot-a' AND stage = 'image'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(after_edit, frozen);
        assert!(!after_edit.contains("新版"));
    }

    #[test]
    fn input_limits_match_bulk_contract() {
        assert!(super::validate_shot_ids(
            &(0..501)
                .map(|index| format!("shot-{index}"))
                .collect::<Vec<_>>()
        )
        .is_err());
        assert!(super::validate_anchor_ids(
            &(0..21)
                .map(|index| format!("anc-{index}"))
                .collect::<Vec<_>>()
        )
        .is_err());
    }
}
