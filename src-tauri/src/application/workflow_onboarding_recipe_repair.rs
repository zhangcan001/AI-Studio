//! Recipe repair transforms used by the app-level repair jobs (PR-A step 11).
//!
//! Recipes are immutable: a repair republishes the current recipe of a user
//! workflow version as a new patch recipe version on the same workflow version
//! (the workflow JSON and its sha are unchanged). Built-in PRODUCT packages
//! are never touched. Anything that cannot be repaired unambiguously is
//! reported as "needs review" instead of being guessed.

use super::*;
use crate::application::workflow_analysis_service::{
    is_animated_save_class, sampler_conditioning_roles, EvidenceKind, PromptRole,
};
use crate::application::workflow_recognition_service::RecognitionConfidence;
use crate::application::workflow_semantic_graph::{
    canonical_semantic_hint, normalize_input_name, CanonicalSemantic,
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeRepairKind {
    /// W-01: defaults truncated/trimmed by the display summary.
    DefaultTruncation,
    /// W-02: a text field bound to a COMBO/model-name input.
    TextareaBoundToCombo,
    /// R-03: a prompt bound to a string-join delimiter.
    PromptOnDelimiter,
    /// W-02/R-04/R-05: prompt/negative bound against the sampler role or to a
    /// system prompt.
    PromptRoleAlias,
    /// R-01: a frame count exposed as duration_seconds.
    FramesAsSeconds,
    /// W-08: an animated saver declared as an image output.
    AnimatedOutputType,
    /// W-07: report-only recheck of the publish gate for current recipes.
    PublishGateRecheck,
}

impl RecipeRepairKind {
    pub const ALL: [RecipeRepairKind; 7] = [
        Self::DefaultTruncation,
        Self::TextareaBoundToCombo,
        Self::PromptOnDelimiter,
        Self::PromptRoleAlias,
        Self::FramesAsSeconds,
        Self::AnimatedOutputType,
        Self::PublishGateRecheck,
    ];

    pub fn job_id(self) -> &'static str {
        match self {
            Self::DefaultTruncation => "recipe_default_truncation_v1",
            Self::TextareaBoundToCombo => "recipe_textarea_bound_to_combo_v1",
            Self::PromptOnDelimiter => "recipe_prompt_on_delimiter_v1",
            Self::PromptRoleAlias => "recipe_prompt_role_alias_v1",
            Self::FramesAsSeconds => "recipe_frames_as_seconds_v1",
            Self::AnimatedOutputType => "recipe_animated_output_type_v1",
            Self::PublishGateRecheck => "recipe_publish_gate_recheck_v1",
        }
    }

    pub fn is_report_only(self) -> bool {
        self == Self::PublishGateRecheck
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeRepairCandidate {
    pub workflow_id: String,
    pub workflow_version: String,
    pub recipe_version: String,
    pub recipe_id: String,
    pub package_name: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeRepairOutcome {
    Published {
        workflow_version_id: Option<String>,
        /// Database recipe id of the repaired source recipe, when registered.
        old_recipe_id: Option<String>,
        new_recipe_id: String,
    },
    NeedsReview(String),
    Skipped(String),
}

#[derive(Debug, Eq, PartialEq)]
enum RepairDecision {
    Unchanged,
    Changed(String),
    NeedsReview(String),
}

struct RepairContext<'a> {
    workflow: &'a WorkflowDocument,
    analysis: &'a WorkflowAnalysisReport,
}

impl WorkflowOnboardingService {
    /// Read-only planning pass. `promoted_recipe_ids` are repaired in addition
    /// to the highest recipe version of each user workflow version.
    pub async fn plan_recipe_repairs(
        &self,
        kind: RecipeRepairKind,
        promoted_recipe_ids: &BTreeSet<String>,
    ) -> Result<Vec<RecipeRepairCandidate>, String> {
        let packages = self
            .source
            .load_packages()
            .await
            .map_err(|error| error.to_string())?;
        let mut loaded = Vec::new();
        for package in packages {
            let WorkflowPackageLoad::Loaded(files) = package else {
                continue;
            };
            if crate::application::builtin_runtime_packages::is_builtin_package_name(
                &files.package_name,
            ) {
                continue;
            }
            let (Ok(manifest), Ok(recipe)) = (
                WorkflowManifest::parse(&files.manifest_yaml),
                RecipeParser::parse(&files.recipe_yaml),
            ) else {
                continue;
            };
            loaded.push((files, manifest, recipe));
        }
        let mut highest = BTreeMap::<(String, String), String>::new();
        for (_, manifest, _) in &loaded {
            let key = (manifest.id.clone(), manifest.workflow_version.clone());
            let replace = highest
                .get(&key)
                .is_none_or(|current| compare_semver(&manifest.recipe_version, current).is_gt());
            if replace {
                highest.insert(key, manifest.recipe_version.clone());
            }
        }
        let mut candidates = Vec::new();
        for (files, mut manifest, recipe) in loaded {
            let is_highest = highest
                .get(&(manifest.id.clone(), manifest.workflow_version.clone()))
                .is_some_and(|version| version == &manifest.recipe_version);
            if !is_highest && !promoted_recipe_ids.contains(&recipe.id) {
                continue;
            }
            let Ok(workflow) = parse_api_workflow_string(&files.workflow_json) else {
                continue;
            };
            let Ok(mut inputs) = input_mappings_from_recipe(&recipe) else {
                continue;
            };
            let mut outputs = recipe
                .outputs
                .iter()
                .map(output_mapping_from_recipe)
                .collect::<Vec<_>>();
            let analysis = WorkflowAnalysisService::analyze_workflow(
                &workflow,
                files.workflow_json.as_bytes(),
            );
            let context = RepairContext {
                workflow: &workflow,
                analysis: &analysis,
            };
            let reason = match repair_recipe_mappings(
                kind,
                &context,
                &mut inputs,
                &mut outputs,
                &mut manifest,
            ) {
                RepairDecision::Unchanged => continue,
                RepairDecision::Changed(reason) | RepairDecision::NeedsReview(reason) => reason,
            };
            candidates.push(RecipeRepairCandidate {
                workflow_id: manifest.id,
                workflow_version: manifest.workflow_version,
                recipe_version: files_recipe_version(&files).unwrap_or_default(),
                recipe_id: recipe.id,
                package_name: files.package_name,
                reason,
            });
        }
        Ok(candidates)
    }

    /// Publish the repaired recipe as a new patch version, or report why it
    /// needs a human decision. Never mutates the source package.
    pub async fn apply_recipe_repair(
        &self,
        kind: RecipeRepairKind,
        candidate: &RecipeRepairCandidate,
    ) -> Result<RecipeRepairOutcome, String> {
        if kind.is_report_only() {
            return Ok(RecipeRepairOutcome::NeedsReview(candidate.reason.clone()));
        }
        let _commit_guard = self.commit_gate.lock().await;
        let next_version = increment_semver(
            &self
                .latest_recipe_version(&candidate.workflow_id, &candidate.workflow_version)
                .await
                .map_err(|error| error.to_string())?,
        );
        let view = self
            .duplicate_recipe_draft(
                &candidate.workflow_id,
                &candidate.workflow_version,
                Some(&candidate.recipe_version),
                Some(next_version),
            )
            .await
            .map_err(|error| error.to_string())?;
        let draft_id = view.draft_id.clone();
        let old_recipe_id = self
            .registered_recipe_id(
                &candidate.workflow_id,
                &candidate.workflow_version,
                &candidate.recipe_version,
            )
            .await;
        let result = self
            .apply_recipe_repair_to_draft(kind, &draft_id)
            .await
            .map(|outcome| match outcome {
                RecipeRepairOutcome::Published {
                    workflow_version_id,
                    new_recipe_id,
                    ..
                } => RecipeRepairOutcome::Published {
                    workflow_version_id,
                    old_recipe_id,
                    new_recipe_id,
                },
                other => other,
            });
        let _ = self.discard(&draft_id);
        result
    }

    async fn apply_recipe_repair_to_draft(
        &self,
        kind: RecipeRepairKind,
        draft_id: &str,
    ) -> Result<RecipeRepairOutcome, String> {
        let mut draft = self
            .with_registry(|registry| registry.get(draft_id))
            .and_then(|result| result)
            .map_err(|error| error.to_string())?;
        let workflow = draft
            .normalized_workflow()
            .map_err(|error| error.to_string())?
            .clone();
        let analysis = WorkflowAnalysisService::analyze_workflow(&workflow, &draft.raw_bytes);
        let context = RepairContext {
            workflow: &workflow,
            analysis: &analysis,
        };
        let mut inputs = draft.input_mappings.clone();
        let mut outputs = draft.output_mappings.clone();
        let mut manifest = draft.manifest.clone();
        match repair_recipe_mappings(kind, &context, &mut inputs, &mut outputs, &mut manifest) {
            RepairDecision::Unchanged => {
                return Ok(RecipeRepairOutcome::Skipped(
                    "recipe no longer needs this repair".to_owned(),
                ))
            }
            RepairDecision::NeedsReview(reason) => {
                return Ok(RecipeRepairOutcome::NeedsReview(reason))
            }
            RepairDecision::Changed(_) => {}
        }
        for mapping in &mut inputs {
            mapping.source = InputMappingSource::ReusedRecipe;
        }
        draft.input_mappings = inputs;
        draft.output_mappings = outputs;
        draft.manifest = manifest;
        let draft_for_registry = draft.clone();
        self.with_registry(|registry| {
            registry.insert(draft_for_registry);
            Ok(())
        })
        .and_then(|result| result)
        .map_err(|error| error.to_string())?;
        match self.publish_internal(draft_id, true, false).await {
            Ok(published) => Ok(RecipeRepairOutcome::Published {
                workflow_version_id: published.workflow_version_id,
                old_recipe_id: None,
                new_recipe_id: published.recipe_id,
            }),
            Err(error)
                if matches!(
                    error.code,
                    "WORKFLOW_REQUIRED_INPUT_MAPPING_UNRESOLVED"
                        | "WORKFLOW_ONBOARDING_NOT_READY"
                        | "WORKFLOW_LINK_INVALID"
                ) =>
            {
                Ok(RecipeRepairOutcome::NeedsReview(error.to_string()))
            }
            Err(error) => Err(error.to_string()),
        }
    }
}

impl WorkflowOnboardingService {
    async fn registered_recipe_id(
        &self,
        workflow_id: &str,
        workflow_version: &str,
        recipe_version: &str,
    ) -> Option<String> {
        let repository = self.runtime_repository.as_ref()?;
        repository
            .list_versions()
            .await
            .ok()?
            .into_iter()
            .find(|version| {
                version.workflow_id == workflow_id && version.workflow_version == workflow_version
            })?
            .recipes
            .into_iter()
            .find(|recipe| recipe.version == recipe_version)
            .map(|recipe| recipe.recipe_id)
    }
}

fn files_recipe_version(files: &WorkflowPackageFiles) -> Option<String> {
    WorkflowManifest::parse(&files.manifest_yaml)
        .ok()
        .map(|manifest| manifest.recipe_version)
}

fn repair_recipe_mappings(
    kind: RecipeRepairKind,
    context: &RepairContext<'_>,
    inputs: &mut Vec<InputMapping>,
    outputs: &mut [OutputMapping],
    manifest: &mut WorkflowManifest,
) -> RepairDecision {
    match kind {
        RecipeRepairKind::DefaultTruncation => repair_default_truncation(context, inputs),
        RecipeRepairKind::TextareaBoundToCombo => retarget_text_mappings(
            context,
            inputs,
            "text field bound to a combo input",
            |_, input, value| is_combo_like_target(input, value),
        ),
        RecipeRepairKind::PromptOnDelimiter => retarget_text_mappings(
            context,
            inputs,
            "prompt bound to a string delimiter",
            |_, input, _| is_delimiter_target(input),
        ),
        RecipeRepairKind::PromptRoleAlias => repair_prompt_roles(context, inputs),
        RecipeRepairKind::FramesAsSeconds => repair_frames_as_seconds(context, inputs),
        RecipeRepairKind::AnimatedOutputType => repair_animated_outputs(context, outputs, manifest),
        RecipeRepairKind::PublishGateRecheck => recheck_publish_gate(context, inputs, outputs),
    }
}

fn literal_at<'a>(context: &'a RepairContext<'_>, node: &str, input: &str) -> Option<&'a Value> {
    context
        .workflow
        .inputs(node)
        .and_then(|inputs| inputs.get(input))
}

fn repair_default_truncation(
    context: &RepairContext<'_>,
    inputs: &mut [InputMapping],
) -> RepairDecision {
    let mut repaired = Vec::new();
    for mapping in inputs.iter_mut().filter(|mapping| {
        mapping.item_index.is_none()
            && matches!(
                mapping.field_type,
                SemanticFieldType::Textarea
                    | SemanticFieldType::Integer
                    | SemanticFieldType::Number
            )
    }) {
        let Some(literal) = literal_at(context, &mapping.target_node, &mapping.target_input) else {
            continue;
        };
        let Some(default) = mapping.default_value.clone() else {
            continue;
        };
        if possible_link(literal).is_some() {
            if default.starts_with("linked to node") {
                mapping.default_value = None;
                repaired.push(mapping.semantic_key.clone());
            }
            continue;
        }
        let Some(raw) = raw_default_value(literal) else {
            continue;
        };
        if raw != default && current_value_summary(literal) == default {
            mapping.default_value = Some(raw);
            repaired.push(mapping.semantic_key.clone());
        }
    }
    if repaired.is_empty() {
        RepairDecision::Unchanged
    } else {
        RepairDecision::Changed(format!("restored full default for {}", repaired.join(", ")))
    }
}

fn is_combo_like_target(input: &str, value: Option<&Value>) -> bool {
    let name = normalize_input_name(input);
    if name.ends_with("_name") || matches!(name.as_str(), "scheduler" | "weight_dtype" | "device") {
        return true;
    }
    value.and_then(Value::as_str).is_some_and(|value| {
        let lower = value.trim().to_ascii_lowercase();
        [".safetensors", ".ckpt", ".pt", ".pth", ".gguf", ".bin"]
            .iter()
            .any(|extension| lower.ends_with(extension))
    })
}

fn is_delimiter_target(input: &str) -> bool {
    matches!(
        normalize_input_name(input).as_str(),
        "delimiter" | "separator" | "joiner" | "sep"
    )
}

fn confident_analysis_target(
    context: &RepairContext<'_>,
    semantic_key: &str,
    item_index: Option<usize>,
) -> Option<(String, String)> {
    context
        .analysis
        .inputs
        .iter()
        .find(|input| {
            input.semantic_key == semantic_key
                && input.item_index == item_index
                && input.confidence == RecognitionConfidence::High
        })
        .map(|input| (input.node_id.clone(), input.input_name.clone()))
}

fn retarget_text_mappings(
    context: &RepairContext<'_>,
    inputs: &mut Vec<InputMapping>,
    label: &str,
    is_defective: impl Fn(&str, &str, Option<&Value>) -> bool,
) -> RepairDecision {
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let mut review = Vec::new();
    for mapping in inputs.iter_mut() {
        if mapping.field_type != SemanticFieldType::Textarea {
            continue;
        }
        let value = literal_at(context, &mapping.target_node, &mapping.target_input);
        if !is_defective(&mapping.target_node, &mapping.target_input, value) {
            continue;
        }
        match confident_analysis_target(context, &mapping.semantic_key, mapping.item_index) {
            Some((node, input))
                if !is_defective(&node, &input, literal_at(context, &node, &input)) =>
            {
                mapping.default_value =
                    literal_at(context, &node, &input).and_then(raw_default_value);
                mapping.target_node = node;
                mapping.target_input = input;
                changed.push(mapping.semantic_key.clone());
            }
            _ if !mapping.required => removed.push(mapping.semantic_key.clone()),
            _ => review.push(mapping.semantic_key.clone()),
        }
    }
    if !review.is_empty() {
        return RepairDecision::NeedsReview(format!(
            "{label}: no unambiguous replacement for {}",
            review.join(", ")
        ));
    }
    inputs.retain(|mapping| {
        !(removed.contains(&mapping.semantic_key)
            && mapping.field_type == SemanticFieldType::Textarea)
    });
    if changed.is_empty() && removed.is_empty() {
        return RepairDecision::Unchanged;
    }
    RepairDecision::Changed(format!(
        "{label}: retargeted [{}], removed optional [{}]",
        changed.join(", "),
        removed.join(", ")
    ))
}

fn is_system_prompt_target(context: &RepairContext<'_>, node: &str, input: &str) -> bool {
    let title = context
        .workflow
        .node(node)
        .and_then(|node| node.get("_meta"))
        .and_then(|meta| meta.get("title"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    title.contains("system") || normalize_input_name(input).contains("system")
}

fn repair_prompt_roles(
    context: &RepairContext<'_>,
    inputs: &mut Vec<InputMapping>,
) -> RepairDecision {
    let Ok(graph) = WorkflowGraph::from_document(context.workflow) else {
        return RepairDecision::Unchanged;
    };
    let roles = sampler_conditioning_roles(context.workflow, &graph, None);
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let mut review = Vec::new();
    for mapping in inputs.iter_mut() {
        let expected = match mapping.semantic_key.as_str() {
            "prompt" => PromptRole::Positive,
            "negative_prompt" => PromptRole::Negative,
            _ => continue,
        };
        if mapping.field_type != SemanticFieldType::Textarea || mapping.item_index.is_some() {
            continue;
        }
        let leaf_roles = roles.get(&(mapping.target_node.clone(), mapping.target_input.clone()));
        let role_conflict = leaf_roles
            .is_some_and(|leaf_roles| leaf_roles.len() == 1 && !leaf_roles.contains(&expected));
        let system_prompt =
            is_system_prompt_target(context, &mapping.target_node, &mapping.target_input);
        if !role_conflict && !system_prompt {
            continue;
        }
        let replacement = context
            .analysis
            .inputs
            .iter()
            .find(|input| {
                input.semantic_key == mapping.semantic_key
                    && input.item_index.is_none()
                    && input.confidence == RecognitionConfidence::High
                    && input
                        .evidence
                        .iter()
                        .any(|item| item.kind == EvidenceKind::GRAPH_CONDITIONING_ROLE)
            })
            .map(|input| (input.node_id.clone(), input.input_name.clone()));
        match replacement {
            Some((node, input))
                if (node.as_str(), input.as_str())
                    != (mapping.target_node.as_str(), mapping.target_input.as_str()) =>
            {
                mapping.default_value =
                    literal_at(context, &node, &input).and_then(raw_default_value);
                mapping.target_node = node;
                mapping.target_input = input;
                changed.push(mapping.semantic_key.clone());
            }
            _ if !mapping.required => removed.push(mapping.semantic_key.clone()),
            _ => review.push(mapping.semantic_key.clone()),
        }
    }
    if !review.is_empty() {
        return RepairDecision::NeedsReview(format!(
            "prompt role conflict without an unambiguous replacement for {}",
            review.join(", ")
        ));
    }
    inputs.retain(|mapping| !removed.contains(&mapping.semantic_key));
    if changed.is_empty() && removed.is_empty() {
        return RepairDecision::Unchanged;
    }
    RepairDecision::Changed(format!(
        "prompt role repaired: retargeted [{}], removed optional [{}]",
        changed.join(", "),
        removed.join(", ")
    ))
}

fn repair_frames_as_seconds(
    context: &RepairContext<'_>,
    inputs: &mut [InputMapping],
) -> RepairDecision {
    if inputs
        .iter()
        .any(|mapping| mapping.semantic_key == "frame_count")
    {
        return if inputs.iter().any(|mapping| {
            mapping.semantic_key == "duration_seconds" && is_frame_input(&mapping.target_input)
        }) {
            RepairDecision::NeedsReview(
                "duration_seconds targets a frame count but frame_count already exists".to_owned(),
            )
        } else {
            RepairDecision::Unchanged
        };
    }
    let mut changed = false;
    for mapping in inputs.iter_mut().filter(|mapping| {
        mapping.semantic_key == "duration_seconds"
            && mapping.item_index.is_none()
            && is_frame_input(&mapping.target_input)
    }) {
        let Some(literal) = literal_at(context, &mapping.target_node, &mapping.target_input) else {
            continue;
        };
        if !literal.is_number() {
            continue;
        }
        mapping.semantic_key = "frame_count".to_owned();
        mapping.label = "Frame Count".to_owned();
        mapping.field_type = SemanticFieldType::Integer;
        mapping.default_value = raw_default_value(literal);
        mapping.min_value = None;
        mapping.max_value = None;
        mapping.step = None;
        changed = true;
    }
    if changed {
        RepairDecision::Changed(
            "duration_seconds bound to a frame count renamed to frame_count".to_owned(),
        )
    } else {
        RepairDecision::Unchanged
    }
}

fn is_frame_input(input: &str) -> bool {
    canonical_semantic_hint(input).is_some_and(|hint| hint.semantic == CanonicalSemantic::Frames)
}

fn animated_mode(mode: &str) -> Option<&'static str> {
    match mode {
        "text_to_image" => Some("text_to_video"),
        "image_to_image" => Some("image_to_video"),
        _ => None,
    }
}

fn repair_animated_outputs(
    context: &RepairContext<'_>,
    outputs: &mut [OutputMapping],
    manifest: &mut WorkflowManifest,
) -> RepairDecision {
    let animated = outputs
        .iter()
        .filter(|output| {
            output.output_type == OutputType::Image
                && context
                    .workflow
                    .class_type(&output.node_id)
                    .is_some_and(is_animated_save_class)
        })
        .count();
    if animated == 0 {
        return RepairDecision::Unchanged;
    }
    if animated != outputs.len() {
        return RepairDecision::NeedsReview(
            "recipe mixes animated and still image outputs".to_owned(),
        );
    }
    if manifest.category == "image" {
        let Some(mode) = animated_mode(&manifest.mode) else {
            return RepairDecision::NeedsReview(format!(
                "animated outputs in image mode {} have no video equivalent",
                manifest.mode
            ));
        };
        manifest.category = "video".to_owned();
        manifest.mode = mode.to_owned();
    }
    for output in outputs.iter_mut() {
        output.output_type = OutputType::Video;
    }
    RepairDecision::Changed("animated saver outputs declared as video".to_owned())
}

fn recheck_publish_gate(
    context: &RepairContext<'_>,
    inputs: &mut [InputMapping],
    outputs: &mut [OutputMapping],
) -> RepairDecision {
    let mut reasons = Vec::new();
    for mapping in inputs.iter() {
        let value = literal_at(context, &mapping.target_node, &mapping.target_input);
        if context.workflow.node(&mapping.target_node).is_none() {
            reasons.push(format!(
                "{} targets missing node {}",
                mapping.semantic_key, mapping.target_node
            ));
            continue;
        }
        if mapping.field_type == SemanticFieldType::Textarea
            && (is_combo_like_target(&mapping.target_input, value)
                || is_delimiter_target(&mapping.target_input))
        {
            reasons.push(format!(
                "{} is a text field on {}.{}",
                mapping.semantic_key, mapping.target_node, mapping.target_input
            ));
        }
    }
    for issue in context
        .analysis
        .issues
        .iter()
        .filter(|issue| is_structural_link_issue(&issue.code))
    {
        reasons.push(format!("{}: {}", issue.code, issue.message));
    }
    for output in outputs.iter() {
        if context.workflow.node(&output.node_id).is_none() {
            reasons.push(format!(
                "output {} targets missing node {}",
                output.output_id, output.node_id
            ));
        }
    }
    if reasons.is_empty() {
        RepairDecision::Unchanged
    } else {
        RepairDecision::NeedsReview(reasons.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(value: Value) -> WorkflowDocument {
        validate_api_workflow(value).unwrap()
    }

    fn mapping(key: &str, field_type: SemanticFieldType, node: &str, input: &str) -> InputMapping {
        InputMapping {
            semantic_key: key.to_owned(),
            field_type,
            label: key.to_owned(),
            required: key == "prompt",
            default_value: None,
            min_value: None,
            max_value: None,
            step: None,
            min_items: None,
            max_items: None,
            target_node: node.to_owned(),
            target_input: input.to_owned(),
            item_index: None,
            source: InputMappingSource::ReusedRecipe,
        }
    }

    fn t2i() -> Value {
        json!({
            "4": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": "model.safetensors"}},
            "5": {"class_type": "EmptyLatentImage", "inputs": {"width": 512, "height": 512, "batch_size": 1}},
            "6": {"class_type": "CLIPTextEncode", "inputs": {"text": "a cat", "clip": ["4", 1]}},
            "7": {"class_type": "CLIPTextEncode", "inputs": {"text": "blurry", "clip": ["4", 1]}},
            "3": {"class_type": "KSampler", "inputs": {
                "seed": 1, "steps": 20, "cfg": 7.0, "sampler_name": "euler", "scheduler": "normal",
                "denoise": 1.0, "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0],
                "latent_image": ["5", 0]}},
            "8": {"class_type": "VAEDecode", "inputs": {"samples": ["3", 0], "vae": ["4", 2]}},
            "9": {"class_type": "SaveImage", "inputs": {"filename_prefix": "out", "images": ["8", 0]}}
        })
    }

    fn run(
        kind: RecipeRepairKind,
        workflow: &WorkflowDocument,
        inputs: &mut Vec<InputMapping>,
        outputs: &mut Vec<OutputMapping>,
        manifest: &mut WorkflowManifest,
    ) -> RepairDecision {
        let bytes = serde_json::to_vec(workflow.value()).unwrap();
        let analysis = WorkflowAnalysisService::analyze_workflow(workflow, &bytes);
        let context = RepairContext {
            workflow,
            analysis: &analysis,
        };
        repair_recipe_mappings(kind, &context, inputs, outputs, manifest)
    }

    fn manifest(category: &str, mode: &str) -> WorkflowManifest {
        WorkflowManifest {
            schema_version: 1,
            id: "wfl_test".to_owned(),
            name: "Test".to_owned(),
            workflow_version: "1.0.0".to_owned(),
            recipe_version: "1.0.0".to_owned(),
            category: category.to_owned(),
            mode: mode.to_owned(),
        }
    }

    fn image_output(node: &str) -> OutputMapping {
        OutputMapping {
            output_id: "image".to_owned(),
            label: "image".to_owned(),
            output_type: OutputType::Image,
            node_id: node.to_owned(),
            required: true,
        }
    }

    #[test]
    fn repair_job_ids_are_versioned_and_unique() {
        let ids = RecipeRepairKind::ALL
            .iter()
            .map(|kind| kind.job_id())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), 7);
        assert!(ids.iter().all(|id| id.ends_with("_v1")));
        assert!(RecipeRepairKind::PublishGateRecheck.is_report_only());
    }

    #[test]
    fn default_truncation_restores_full_prompt() {
        let long = format!("{} end", "very long prompt ".repeat(12));
        let mut value = t2i();
        value["6"]["inputs"]["text"] = json!(long);
        let workflow = document(value);
        let mut inputs = vec![mapping("prompt", SemanticFieldType::Textarea, "6", "text")];
        inputs[0].default_value = Some(current_value_summary(&json!(long)));
        let decision = run(
            RecipeRepairKind::DefaultTruncation,
            &workflow,
            &mut inputs,
            &mut Vec::new(),
            &mut manifest("image", "text_to_image"),
        );
        assert!(
            matches!(decision, RepairDecision::Changed(_)),
            "{decision:?}"
        );
        assert_eq!(inputs[0].default_value.as_deref(), Some(long.as_str()));
        // Idempotent: a repaired recipe is unchanged by a second pass.
        let again = run(
            RecipeRepairKind::DefaultTruncation,
            &workflow,
            &mut inputs,
            &mut Vec::new(),
            &mut manifest("image", "text_to_image"),
        );
        assert_eq!(again, RepairDecision::Unchanged);
    }

    #[test]
    fn negative_prompt_on_checkpoint_combo_is_retargeted() {
        let workflow = document(t2i());
        let mut inputs = vec![
            mapping("prompt", SemanticFieldType::Textarea, "6", "text"),
            mapping(
                "negative_prompt",
                SemanticFieldType::Textarea,
                "4",
                "ckpt_name",
            ),
        ];
        let decision = run(
            RecipeRepairKind::TextareaBoundToCombo,
            &workflow,
            &mut inputs,
            &mut Vec::new(),
            &mut manifest("image", "text_to_image"),
        );
        assert!(
            matches!(decision, RepairDecision::Changed(_)),
            "{decision:?}"
        );
        let negative = inputs
            .iter()
            .find(|mapping| mapping.semantic_key == "negative_prompt")
            .unwrap();
        assert_eq!(
            (
                negative.target_node.as_str(),
                negative.target_input.as_str()
            ),
            ("7", "text")
        );
        assert_eq!(negative.default_value.as_deref(), Some("blurry"));
    }

    #[test]
    fn prompt_on_delimiter_without_replacement_needs_review() {
        let workflow = document(json!({
            "1": {"class_type": "StringConcatenate", "inputs": {"string_a": "a", "string_b": "b", "delimiter": ", "}},
            "2": {"class_type": "SaveImage", "inputs": {"images": ["1", 0]}}
        }));
        let mut inputs = vec![mapping(
            "prompt",
            SemanticFieldType::Textarea,
            "1",
            "delimiter",
        )];
        let decision = run(
            RecipeRepairKind::PromptOnDelimiter,
            &workflow,
            &mut inputs,
            &mut Vec::new(),
            &mut manifest("image", "text_to_image"),
        );
        assert!(
            matches!(decision, RepairDecision::NeedsReview(_)),
            "{decision:?}"
        );
    }

    #[test]
    fn swapped_prompt_roles_are_repaired() {
        let workflow = document(t2i());
        let mut inputs = vec![
            mapping("prompt", SemanticFieldType::Textarea, "7", "text"),
            mapping("negative_prompt", SemanticFieldType::Textarea, "6", "text"),
        ];
        let decision = run(
            RecipeRepairKind::PromptRoleAlias,
            &workflow,
            &mut inputs,
            &mut Vec::new(),
            &mut manifest("image", "text_to_image"),
        );
        assert!(
            matches!(decision, RepairDecision::Changed(_)),
            "{decision:?}"
        );
        let target = |key: &str| {
            let mapping = inputs
                .iter()
                .find(|mapping| mapping.semantic_key == key)
                .unwrap();
            (mapping.target_node.clone(), mapping.target_input.clone())
        };
        assert_eq!(target("prompt"), ("6".to_owned(), "text".to_owned()));
        assert_eq!(
            target("negative_prompt"),
            ("7".to_owned(), "text".to_owned())
        );
    }

    #[test]
    fn frames_bound_as_seconds_become_frame_count() {
        let mut value = t2i();
        value["5"] = json!({"class_type": "EmptyHunyuanLatentVideo", "inputs": {
            "width": 848, "height": 480, "length": 81, "batch_size": 1}});
        let workflow = document(value);
        let mut inputs = vec![mapping(
            "duration_seconds",
            SemanticFieldType::Integer,
            "5",
            "length",
        )];
        inputs[0].min_value = Some("1".to_owned());
        inputs[0].max_value = Some("15".to_owned());
        inputs[0].default_value = Some("5".to_owned());
        let decision = run(
            RecipeRepairKind::FramesAsSeconds,
            &workflow,
            &mut inputs,
            &mut Vec::new(),
            &mut manifest("video", "text_to_video"),
        );
        assert!(
            matches!(decision, RepairDecision::Changed(_)),
            "{decision:?}"
        );
        assert_eq!(inputs[0].semantic_key, "frame_count");
        assert_eq!(inputs[0].default_value.as_deref(), Some("81"));
        assert_eq!(inputs[0].max_value, None);
    }

    #[test]
    fn h3_seconds_leaf_is_not_treated_as_frames() {
        let workflow = document(json!({
            "22": {"class_type": "PrimitiveInt", "inputs": {"value": 5}},
            "2": {"class_type": "SaveVideo", "inputs": {"video": ["22", 0]}}
        }));
        let mut inputs = vec![mapping(
            "duration_seconds",
            SemanticFieldType::Integer,
            "22",
            "value",
        )];
        let decision = run(
            RecipeRepairKind::FramesAsSeconds,
            &workflow,
            &mut inputs,
            &mut Vec::new(),
            &mut manifest("video", "text_to_video"),
        );
        assert_eq!(decision, RepairDecision::Unchanged);
    }

    #[test]
    fn animated_webp_output_becomes_video() {
        let mut value = t2i();
        value["9"] = json!({"class_type": "SaveAnimatedWEBP", "inputs": {"filename_prefix": "out", "images": ["8", 0], "fps": 8.0}});
        let workflow = document(value);
        let mut outputs = vec![image_output("9")];
        let mut manifest = manifest("image", "text_to_image");
        let decision = run(
            RecipeRepairKind::AnimatedOutputType,
            &workflow,
            &mut Vec::new(),
            &mut outputs,
            &mut manifest,
        );
        assert!(
            matches!(decision, RepairDecision::Changed(_)),
            "{decision:?}"
        );
        assert_eq!(outputs[0].output_type, OutputType::Video);
        assert_eq!(
            (manifest.category.as_str(), manifest.mode.as_str()),
            ("video", "text_to_video")
        );
    }

    #[test]
    fn publish_gate_recheck_reports_text_on_combo() {
        let workflow = document(t2i());
        let mut inputs = vec![mapping(
            "negative_prompt",
            SemanticFieldType::Textarea,
            "4",
            "ckpt_name",
        )];
        let decision = run(
            RecipeRepairKind::PublishGateRecheck,
            &workflow,
            &mut inputs,
            &mut vec![image_output("9")],
            &mut manifest("image", "text_to_image"),
        );
        assert!(
            matches!(decision, RepairDecision::NeedsReview(_)),
            "{decision:?}"
        );
    }
}
