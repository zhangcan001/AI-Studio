use crate::application::asset_query_service::{AssetSummaryView, AssetView};
use crate::application::pagination::PageCursor;
use crate::application::ports::{
    AssetRepository, GenerationDefinitionRepository, GenerationSnapshotRepository, RepositoryError,
    TaskHistoryQuery, TaskHistoryRecord, TaskHistoryRepository, TaskOutputAssetMapping,
};
use crate::compiler::RecipeParser;
use crate::domain::{
    AssetType, InputDefinition, Recipe, RuntimeProvenance, TaskId, GENERATED_VIDEO_CATEGORY,
    SOURCE_AUDIO_CATEGORY, SOURCE_VIDEO_CATEGORY,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

const WORKFLOW_UNAVAILABLE: &str =
    "This workflow version is no longer available in the runtime catalog.";
const INPUTS_UNAVAILABLE: &str = "Inputs unavailable for reuse.";

pub struct TaskHistoryService {
    history_repository: Arc<dyn TaskHistoryRepository>,
    snapshot_repository: Arc<dyn GenerationSnapshotRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    asset_repository: Arc<dyn AssetRepository>,
}

impl TaskHistoryService {
    pub fn new(
        history_repository: Arc<dyn TaskHistoryRepository>,
        snapshot_repository: Arc<dyn GenerationSnapshotRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        asset_repository: Arc<dyn AssetRepository>,
    ) -> Self {
        Self {
            history_repository,
            snapshot_repository,
            definition_repository,
            asset_repository,
        }
    }

    pub async fn list_page(
        &self,
        mut query: TaskHistoryQuery,
    ) -> Result<TaskHistoryPageView, TaskHistoryError> {
        if query.project_id.trim().is_empty() {
            return Err(TaskHistoryError::InvalidProjectId);
        }
        query.project_id = query.project_id.trim().to_owned();
        query.workflow_id = query.workflow_id.and_then(|workflow_id| {
            let workflow_id = workflow_id.trim().to_owned();
            (!workflow_id.is_empty()).then_some(workflow_id)
        });
        query.keyword = query.keyword.and_then(|keyword| {
            let keyword = keyword.trim().to_owned();
            (!keyword.is_empty()).then_some(keyword)
        });
        query.limit = query.limit.clamp(1, 100);
        let page = self.history_repository.list_page(query.clone()).await?;
        let workflow_options = self
            .history_repository
            .list_workflow_options(&query.project_id)
            .await?
            .into_iter()
            .map(TaskHistoryWorkflowOptionView::from)
            .collect();
        Ok(TaskHistoryPageView {
            items: page
                .items
                .iter()
                .map(TaskHistoryItemView::from_record)
                .collect(),
            next_cursor: page.next_cursor,
            workflow_options,
        })
    }

    pub async fn get_detail(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<TaskDetailView, TaskHistoryError> {
        validate_project_id(project_id)?;
        let task_id = parse_task_id(task_id)?;
        let record = self
            .history_repository
            .find_detail(project_id, &task_id)
            .await?
            .ok_or_else(|| TaskHistoryError::NotFound(task_id.as_str().to_owned()))?;
        let output_assets = self.list_output_assets(&record).await?;
        let reusable_draft = match self.build_draft(&record).await {
            Ok(draft) => ReusableDraftAvailabilityView {
                available: true,
                reason: None,
                missing_asset_ids: draft.missing_asset_ids,
            },
            Err(TaskHistoryError::DraftUnavailable(reason)) => ReusableDraftAvailabilityView {
                available: false,
                reason: Some(reason),
                missing_asset_ids: Vec::new(),
            },
            Err(error) => return Err(error),
        };

        Ok(TaskDetailView {
            id: record.task.id.as_str().to_owned(),
            project_id: record.task.project_id.clone(),
            workflow_id: record.task.workflow_id.clone(),
            workflow_version_id: record.task.workflow_version_id.clone(),
            recipe_id: record.task.recipe_id.clone(),
            runtime_provenance: record
                .task
                .runtime_provenance
                .as_ref()
                .map(RuntimeProvenanceView::from),
            telemetry: TaskTelemetryView::from_task(&record.task),
            workflow_name: record.workflow_name,
            status: record.task.status.as_str().to_owned(),
            created_at: record.task.created_at,
            queued_at: record.task.queued_at,
            started_at: record.task.started_at,
            finished_at: record.task.finished_at,
            error_code: record.task.error.as_ref().map(|error| error.code.clone()),
            error_message: record
                .task
                .error
                .as_ref()
                .map(|error| error.message.clone()),
            node_errors: record
                .task
                .error
                .as_ref()
                .and_then(|error| (error.code == "WORKFLOW_VALIDATION_FAILED").then_some(error))
                .map(|error| structured_node_errors(error.raw.as_ref()))
                .unwrap_or_default(),
            raw_error: record
                .task
                .error
                .as_ref()
                .and_then(|error| error.raw.clone()),
            output_assets,
            reusable_draft,
        })
    }

    async fn list_output_assets(
        &self,
        record: &TaskHistoryRecord,
    ) -> Result<Vec<AssetSummaryView>, TaskHistoryError> {
        let mut mapped = self
            .asset_repository
            .list_mapped_assets(&record.task.id)
            .await?;
        if mapped.is_empty() {
            return Ok(self
                .asset_repository
                .list_by_source_task(&record.task.id)
                .await?
                .into_iter()
                .filter(|asset| asset.project_id == record.task.project_id)
                .map(AssetView::from)
                .collect());
        }

        let output_order = self
            .definition_repository
            .find(&record.task.workflow_version_id, &record.task.recipe_id)
            .await?
            .and_then(|definition| RecipeParser::parse(&definition.recipe_yaml).ok())
            .map(|recipe| {
                recipe
                    .outputs
                    .into_iter()
                    .map(|output| output.id)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        mapped.sort_by(|(left, _), (right, _)| compare_mappings(left, right, &output_order));
        Ok(mapped
            .into_iter()
            .filter(|(_, asset)| asset.project_id == record.task.project_id)
            .map(|(_, asset)| AssetView::from(asset))
            .collect())
    }

    pub async fn get_reusable_draft(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<ReusableGenerationDraftView, TaskHistoryError> {
        validate_project_id(project_id)?;
        let task_id = parse_task_id(task_id)?;
        let record = self
            .history_repository
            .find_detail(project_id, &task_id)
            .await?
            .ok_or_else(|| TaskHistoryError::NotFound(task_id.as_str().to_owned()))?;
        let draft = self.build_draft(&record).await?;
        Ok(ReusableGenerationDraftView {
            project_id: record.task.project_id,
            workflow_version_id: record.task.workflow_version_id,
            recipe_id: record.task.recipe_id,
            workflow_name: record.workflow_name,
            created_at: record.task.created_at,
            values: draft.values,
            missing_asset_ids: draft.missing_asset_ids,
        })
    }

    async fn build_draft(
        &self,
        record: &TaskHistoryRecord,
    ) -> Result<ReusableDraft, TaskHistoryError> {
        let available = self.definition_repository.list_available().await?;
        if !available.iter().any(|definition| {
            definition.workflow_version_id == record.task.workflow_version_id
                && definition.recipe_id == record.task.recipe_id
        }) {
            return Err(TaskHistoryError::DraftUnavailable(
                WORKFLOW_UNAVAILABLE.to_owned(),
            ));
        }
        let definition = self
            .definition_repository
            .find(&record.task.workflow_version_id, &record.task.recipe_id)
            .await?
            .ok_or_else(|| TaskHistoryError::DraftUnavailable(WORKFLOW_UNAVAILABLE.to_owned()))?;

        let snapshot = match self
            .snapshot_repository
            .find_by_task_id(&record.task.id)
            .await
        {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                return Err(TaskHistoryError::DraftUnavailable(
                    INPUTS_UNAVAILABLE.to_owned(),
                ))
            }
            Err(RepositoryError::Serialization { .. }) => {
                return Err(TaskHistoryError::DraftUnavailable(
                    INPUTS_UNAVAILABLE.to_owned(),
                ))
            }
            Err(error) => return Err(TaskHistoryError::Repository(error)),
        };

        let recipe = RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|_| TaskHistoryError::DraftUnavailable(INPUTS_UNAVAILABLE.to_owned()))?;
        let values = parse_snapshot_values(&recipe, &snapshot.user_inputs_json)
            .map_err(|_| TaskHistoryError::DraftUnavailable(INPUTS_UNAVAILABLE.to_owned()))?;
        let mut missing_asset_ids = Vec::new();
        for value in values.values() {
            let asset_refs: Vec<(&String, DraftAssetKind)> = match value {
                DraftValueView::ImageAsset { asset_id } => {
                    vec![(asset_id, DraftAssetKind::Image)]
                }
                DraftValueView::ImageAssets { asset_ids } => asset_ids
                    .iter()
                    .map(|asset_id| (asset_id, DraftAssetKind::Image))
                    .collect(),
                DraftValueView::VideoAsset { asset_id } => {
                    vec![(asset_id, DraftAssetKind::Video)]
                }
                DraftValueView::VideoAssets { asset_ids } => asset_ids
                    .iter()
                    .map(|asset_id| (asset_id, DraftAssetKind::Video))
                    .collect(),
                DraftValueView::AudioAsset { asset_id } => {
                    vec![(asset_id, DraftAssetKind::Audio)]
                }
                DraftValueView::AudioAssets { asset_ids } => asset_ids
                    .iter()
                    .map(|asset_id| (asset_id, DraftAssetKind::Audio))
                    .collect(),
                _ => Vec::new(),
            };
            for (asset_id, expected_kind) in asset_refs {
                let asset =
                    self.asset_repository
                        .find_by_id(&crate::domain::AssetId::parse(asset_id.clone()).map_err(
                            |_| TaskHistoryError::DraftUnavailable(INPUTS_UNAVAILABLE.to_owned()),
                        )?)
                        .await?;
                if !asset_matches_draft_kind(asset.as_ref(), &record.task.project_id, expected_kind)
                {
                    if !missing_asset_ids.contains(asset_id) {
                        missing_asset_ids.push(asset_id.clone());
                    }
                }
            }
        }

        Ok(ReusableDraft {
            values,
            missing_asset_ids,
        })
    }
}

fn compare_mappings(
    left: &TaskOutputAssetMapping,
    right: &TaskOutputAssetMapping,
    output_order: &[String],
) -> std::cmp::Ordering {
    let left_rank = output_order
        .iter()
        .position(|output_id| output_id == &left.output_id)
        .unwrap_or(usize::MAX);
    let right_rank = output_order
        .iter()
        .position(|output_id| output_id == &right.output_id)
        .unwrap_or(usize::MAX);
    left_rank
        .cmp(&right_rank)
        .then_with(|| left.ordinal.cmp(&right.ordinal))
        .then_with(|| left.output_id.cmp(&right.output_id))
}

fn parse_task_id(value: &str) -> Result<TaskId, TaskHistoryError> {
    TaskId::parse(value.to_owned())
        .map_err(|error| TaskHistoryError::InvalidTaskId(error.to_string()))
}

fn validate_project_id(value: &str) -> Result<(), TaskHistoryError> {
    if value.trim().is_empty() {
        Err(TaskHistoryError::InvalidProjectId)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskHistoryPageView {
    pub items: Vec<TaskHistoryItemView>,
    pub next_cursor: Option<PageCursor>,
    pub workflow_options: Vec<TaskHistoryWorkflowOptionView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskHistoryWorkflowOptionView {
    pub workflow_id: String,
    pub workflow_name: String,
}

impl From<crate::application::ports::TaskHistoryWorkflowOption> for TaskHistoryWorkflowOptionView {
    fn from(value: crate::application::ports::TaskHistoryWorkflowOption) -> Self {
        Self {
            workflow_id: value.workflow_id,
            workflow_name: value.workflow_name,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskHistoryItemView {
    pub id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub workflow_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>,
    pub output_count: u32,
}

impl TaskHistoryItemView {
    fn from_record(record: &TaskHistoryRecord) -> Self {
        Self {
            id: record.task.id.as_str().to_owned(),
            workflow_id: record.task.workflow_id.clone(),
            workflow_version_id: record.task.workflow_version_id.clone(),
            recipe_id: record.task.recipe_id.clone(),
            workflow_name: record.workflow_name.clone(),
            status: record.task.status.as_str().to_owned(),
            created_at: record.task.created_at,
            queued_at: record.task.queued_at,
            started_at: record.task.started_at,
            finished_at: record.task.finished_at,
            error_code: record.task.error.as_ref().map(|error| error.code.clone()),
            output_count: record.output_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetailView {
    pub id: String,
    pub project_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub runtime_provenance: Option<RuntimeProvenanceView>,
    pub telemetry: TaskTelemetryView,
    pub workflow_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub node_errors: Vec<TaskNodeErrorView>,
    pub raw_error: Option<Value>,
    pub output_assets: Vec<AssetSummaryView>,
    pub reusable_draft: ReusableDraftAvailabilityView,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskTelemetryView {
    pub generation_execution_id: Option<String>,
    pub compiled_workflow_sha256: Option<String>,
    pub runtime_profile: Option<String>,
    pub concurrency_class: Option<String>,
    pub prepare_started_at: Option<DateTime<Utc>>,
    pub prepared_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub execution_started_at: Option<DateTime<Utc>>,
    pub execution_finished_at: Option<DateTime<Utc>>,
    pub collection_finished_at: Option<DateTime<Utc>>,
    pub queue_wait_ms: Option<i64>,
    pub prepare_ms: Option<i64>,
    pub submit_ms: Option<i64>,
    pub comfy_execution_ms: Option<i64>,
    pub collection_ms: Option<i64>,
    pub total_ms: Option<i64>,
}

impl TaskTelemetryView {
    fn from_task(task: &crate::domain::Task) -> Self {
        let durations = task.telemetry.durations(task.created_at, task.queued_at);
        Self {
            generation_execution_id: task.telemetry.generation_execution_id.clone(),
            compiled_workflow_sha256: task.telemetry.compiled_workflow_sha256.clone(),
            runtime_profile: task.telemetry.runtime_profile.clone(),
            concurrency_class: task.telemetry.concurrency_class.clone(),
            prepare_started_at: task.telemetry.prepare_started_at,
            prepared_at: task.telemetry.prepared_at,
            submitted_at: task.telemetry.submitted_at,
            execution_started_at: task.telemetry.execution_started_at,
            execution_finished_at: task.telemetry.execution_finished_at,
            collection_finished_at: task.telemetry.collection_finished_at,
            queue_wait_ms: durations.queue_wait_ms,
            prepare_ms: durations.prepare_ms,
            submit_ms: durations.submit_ms,
            comfy_execution_ms: durations.comfy_execution_ms,
            collection_ms: durations.collection_ms,
            total_ms: durations.total_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProvenanceView {
    pub app_version: String,
    pub build_commit: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub workflow_version: String,
    pub workflow_sha256: String,
    pub recipe_id: String,
    pub recipe_version: String,
    pub recipe_sha256: String,
    pub package_name: Option<String>,
    pub package_source_path: Option<String>,
    pub dynamic_binding_targets: Vec<String>,
}

impl From<&RuntimeProvenance> for RuntimeProvenanceView {
    fn from(value: &RuntimeProvenance) -> Self {
        Self {
            app_version: value.app_version.clone(),
            build_commit: value.build_commit.clone(),
            workflow_id: value.workflow_id.clone(),
            workflow_version_id: value.workflow_version_id.clone(),
            workflow_version: value.workflow_version.clone(),
            workflow_sha256: value.workflow_sha256.clone(),
            recipe_id: value.recipe_id.clone(),
            recipe_version: value.recipe_version.clone(),
            recipe_sha256: value.recipe_sha256.clone(),
            package_name: value.package_name.clone(),
            package_source_path: value.package_source_path.clone(),
            dynamic_binding_targets: value.dynamic_binding_targets.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskNodeErrorView {
    pub node_id: String,
    pub node_type: Option<String>,
    pub input: Option<String>,
    pub error_type: Option<String>,
    pub message: String,
    pub details: Option<String>,
    pub received_value: Option<Value>,
    pub expected_config: Option<Value>,
}

fn structured_node_errors(raw: Option<&Value>) -> Vec<TaskNodeErrorView> {
    let Some(nodes) = raw.and_then(|value| value.get("nodes").unwrap_or(value).as_object()) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for (node_id, node) in nodes {
        let Some(node) = node.as_object() else {
            continue;
        };
        let node_type = first_string(node, &["class_type", "classType"]);
        if let Some(errors) = node.get("errors").and_then(Value::as_array) {
            for error in errors {
                result.push(parse_node_error(node_id, node_type.clone(), error));
            }
        } else if node.get("message").is_some() || node.get("details").is_some() {
            result.push(parse_node_error(
                node_id,
                node_type,
                &Value::Object(node.clone()),
            ));
        }
    }
    result
}

fn parse_node_error(node_id: &str, node_type: Option<String>, error: &Value) -> TaskNodeErrorView {
    let error_object = error.as_object();
    let extra_info = error_object
        .and_then(|object| object.get("extra_info"))
        .and_then(Value::as_object);
    let input = error_object
        .and_then(|object| first_string(object, &["input_name", "inputName"]))
        .or_else(|| {
            extra_info.and_then(|object| first_string(object, &["input_name", "inputName"]))
        });
    let error_type =
        error_object.and_then(|object| first_string(object, &["type", "error_type", "errorType"]));
    let details = error_object.and_then(|object| first_string(object, &["details"]));
    let message = error_object
        .and_then(|object| first_string(object, &["message"]))
        .or_else(|| details.clone())
        .unwrap_or_else(|| error.to_string());
    let received_value = error_object
        .and_then(|object| {
            object
                .get("received_value")
                .or_else(|| object.get("receivedValue"))
        })
        .cloned()
        .or_else(|| {
            extra_info
                .and_then(|object| {
                    object
                        .get("received_value")
                        .or_else(|| object.get("receivedValue"))
                })
                .cloned()
        });
    let expected_config = error_object
        .and_then(|object| {
            object
                .get("expected_config")
                .or_else(|| object.get("expectedConfig"))
        })
        .cloned()
        .or_else(|| {
            extra_info
                .and_then(|object| {
                    object
                        .get("input_config")
                        .or_else(|| object.get("inputConfig"))
                })
                .cloned()
        });

    TaskNodeErrorView {
        node_id: node_id.to_owned(),
        node_type,
        input,
        error_type,
        message,
        details,
        received_value,
        expected_config,
    }
}

fn first_string(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str).map(str::to_owned))
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReusableDraftAvailabilityView {
    pub available: bool,
    pub reason: Option<String>,
    pub missing_asset_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReusableGenerationDraftView {
    pub project_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub workflow_name: String,
    pub created_at: DateTime<Utc>,
    pub values: BTreeMap<String, DraftValueView>,
    pub missing_asset_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DraftValueView {
    String {
        value: String,
    },
    Integer {
        value: i64,
    },
    Number {
        value: f64,
    },
    SeedRandom,
    SeedFixed {
        value: String,
    },
    ImageAsset {
        #[serde(rename = "assetId")]
        asset_id: String,
    },
    ImageAssets {
        #[serde(rename = "assetIds")]
        asset_ids: Vec<String>,
    },
    VideoAsset {
        #[serde(rename = "assetId")]
        asset_id: String,
    },
    AudioAsset {
        #[serde(rename = "assetId")]
        asset_id: String,
    },
    VideoAssets {
        #[serde(rename = "assetIds")]
        asset_ids: Vec<String>,
    },
    AudioAssets {
        #[serde(rename = "assetIds")]
        asset_ids: Vec<String>,
    },
}

#[derive(Clone, Copy)]
enum DraftAssetKind {
    Image,
    Video,
    Audio,
}

fn asset_matches_draft_kind(
    asset: Option<&crate::domain::Asset>,
    project_id: &str,
    kind: DraftAssetKind,
) -> bool {
    let Some(asset) = asset else {
        return false;
    };
    if asset.project_id != project_id {
        return false;
    }
    match kind {
        DraftAssetKind::Image => asset.asset_type == AssetType::Image,
        DraftAssetKind::Video => {
            asset.asset_type == AssetType::Video
                && matches!(
                    asset.category.as_str(),
                    SOURCE_VIDEO_CATEGORY | GENERATED_VIDEO_CATEGORY
                )
        }
        DraftAssetKind::Audio => {
            asset.asset_type == AssetType::Audio && asset.category == SOURCE_AUDIO_CATEGORY
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ReusableDraft {
    values: BTreeMap<String, DraftValueView>,
    missing_asset_ids: Vec<String>,
}

fn parse_snapshot_values(
    recipe: &Recipe,
    snapshot: &Value,
) -> Result<BTreeMap<String, DraftValueView>, &'static str> {
    let object = snapshot
        .as_object()
        .ok_or("snapshot inputs must be an object")?;
    if object.keys().any(|key| !recipe.inputs.contains_key(key)) {
        return Err("snapshot contains an unknown input");
    }

    let mut values = BTreeMap::new();
    for (key, definition) in &recipe.inputs {
        let Some(value) = object.get(key) else {
            if matches!(
                definition,
                InputDefinition::TextArea { required: true, .. }
                    | InputDefinition::Integer { required: true, .. }
                    | InputDefinition::Number { required: true, .. }
                    | InputDefinition::Image { required: true, .. }
                    | InputDefinition::Images { required: true, .. }
                    | InputDefinition::Video { required: true, .. }
                    | InputDefinition::Audio { required: true, .. }
                    | InputDefinition::Videos { required: true, .. }
                    | InputDefinition::Audios { required: true, .. }
            ) {
                return Err("snapshot is missing a required input");
            }
            continue;
        };
        let parsed = match definition {
            InputDefinition::TextArea { .. } => DraftValueView::String {
                value: value
                    .as_str()
                    .ok_or("text input must be a string")?
                    .to_owned(),
            },
            InputDefinition::Integer { min, max, step, .. } => {
                let integer = value.as_i64().ok_or("integer input must be an integer")?;
                if min.is_some_and(|min| integer < min) || max.is_some_and(|max| integer > max) {
                    return Err("integer input is outside the current recipe range");
                }
                if step.is_some_and(|step| integer % step != 0) {
                    return Err("integer input is not aligned to the current recipe step");
                }
                DraftValueView::Integer { value: integer }
            }
            InputDefinition::Number { min, max, step, .. } => {
                let number = value.as_f64().ok_or("number input must be a number")?;
                if !number.is_finite()
                    || min.is_some_and(|min| number < min)
                    || max.is_some_and(|max| number > max)
                {
                    return Err("number input is outside the current recipe range");
                }
                if step.is_some_and(|step| {
                    !crate::compiler::number_is_aligned_to_step(number, min.unwrap_or(0.0), step)
                }) {
                    return Err("number input is not aligned to the current recipe step");
                }
                DraftValueView::Number { value: number }
            }
            InputDefinition::Seed { min, max, .. } => {
                if value.as_str() == Some("random") {
                    DraftValueView::SeedRandom
                } else {
                    let seed = value
                        .as_u64()
                        .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
                        .ok_or("seed input must be random or an unsigned decimal")?;
                    if min.is_some_and(|min| seed < min) || max.is_some_and(|max| seed > max) {
                        return Err("seed input is outside the current recipe range");
                    }
                    DraftValueView::SeedFixed {
                        value: seed.to_string(),
                    }
                }
            }
            InputDefinition::Image { .. } => {
                let image = value.as_object().ok_or("image input must be an object")?;
                if image.len() != 2
                    || image.get("type").and_then(Value::as_str) != Some("image_asset")
                {
                    return Err("image input has an invalid shape");
                }
                let asset_id = image
                    .get("assetId")
                    .and_then(Value::as_str)
                    .ok_or("image input must contain assetId")?;
                crate::domain::AssetId::parse(asset_id.to_owned())
                    .map_err(|_| "image input contains an invalid asset id")?;
                DraftValueView::ImageAsset {
                    asset_id: asset_id.to_owned(),
                }
            }
            InputDefinition::Images {
                min_items,
                max_items,
                required,
                ..
            } => {
                let images = value.as_object().ok_or("images input must be an object")?;
                if images.get("type").and_then(Value::as_str) != Some("image_assets") {
                    return Err("images input has an invalid shape");
                }
                let asset_ids = images
                    .get("assetIds")
                    .and_then(Value::as_array)
                    .ok_or("images input must contain assetIds")?
                    .iter()
                    .map(|asset_id| {
                        let asset_id =
                            asset_id.as_str().ok_or("image asset id must be a string")?;
                        crate::domain::AssetId::parse(asset_id.to_owned())
                            .map_err(|_| "images input contains an invalid asset id")?;
                        Ok(asset_id.to_owned())
                    })
                    .collect::<Result<Vec<_>, &'static str>>()?;
                if asset_ids.len() > *max_items
                    || (*required && asset_ids.len() < *min_items)
                    || (!asset_ids.is_empty() && asset_ids.len() < *min_items)
                {
                    return Err("images input count is outside the current recipe range");
                }
                DraftValueView::ImageAssets { asset_ids }
            }
            InputDefinition::Video { .. } => {
                parse_single_media_snapshot(value, "video_asset", |asset_id| {
                    DraftValueView::VideoAsset { asset_id }
                })?
            }
            InputDefinition::Audio { .. } => {
                parse_single_media_snapshot(value, "audio_asset", |asset_id| {
                    DraftValueView::AudioAsset { asset_id }
                })?
            }
            InputDefinition::Videos {
                min_items,
                max_items,
                required,
                ..
            } => parse_plural_media_snapshot(
                value,
                "video_assets",
                *min_items,
                *max_items,
                *required,
                |asset_ids| DraftValueView::VideoAssets { asset_ids },
            )?,
            InputDefinition::Audios {
                min_items,
                max_items,
                required,
                ..
            } => parse_plural_media_snapshot(
                value,
                "audio_assets",
                *min_items,
                *max_items,
                *required,
                |asset_ids| DraftValueView::AudioAssets { asset_ids },
            )?,
        };
        values.insert(key.clone(), parsed);
    }
    Ok(values)
}

fn parse_single_media_snapshot<F>(
    value: &Value,
    expected_type: &str,
    build: F,
) -> Result<DraftValueView, &'static str>
where
    F: FnOnce(String) -> DraftValueView,
{
    let media = value.as_object().ok_or("media input must be an object")?;
    if media.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err("media input has an invalid shape");
    }
    let asset_id = media
        .get("assetId")
        .and_then(Value::as_str)
        .ok_or("media input must contain assetId")?;
    crate::domain::AssetId::parse(asset_id.to_owned())
        .map_err(|_| "media input contains an invalid asset id")?;
    Ok(build(asset_id.to_owned()))
}

fn parse_plural_media_snapshot<F>(
    value: &Value,
    expected_type: &str,
    min_items: usize,
    max_items: usize,
    required: bool,
    build: F,
) -> Result<DraftValueView, &'static str>
where
    F: FnOnce(Vec<String>) -> DraftValueView,
{
    let media = value
        .as_object()
        .ok_or("media list input must be an object")?;
    if media.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err("media list input has an invalid shape");
    }
    let asset_ids = media
        .get("assetIds")
        .and_then(Value::as_array)
        .ok_or("media list input must contain assetIds")?
        .iter()
        .map(|value| {
            let asset_id = value.as_str().ok_or("media asset id must be a string")?;
            crate::domain::AssetId::parse(asset_id.to_owned())
                .map_err(|_| "media list input contains an invalid asset id")?;
            Ok(asset_id.to_owned())
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    if asset_ids.len() > max_items
        || (required && asset_ids.len() < min_items)
        || (!asset_ids.is_empty() && asset_ids.len() < min_items)
    {
        return Err("media list input count is outside the current recipe range");
    }
    Ok(build(asset_ids))
}

#[derive(Debug)]
pub enum TaskHistoryError {
    InvalidProjectId,
    InvalidTaskId(String),
    NotFound(String),
    DraftUnavailable(String),
    Repository(RepositoryError),
}

impl fmt::Display for TaskHistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectId => {
                formatter.write_str("INVALID_PROJECT_ID: project id must not be empty")
            }
            Self::InvalidTaskId(message) => write!(formatter, "INVALID_TASK_ID: {message}"),
            Self::NotFound(id) => write!(formatter, "TASK_NOT_FOUND: task {id} was not found"),
            Self::DraftUnavailable(message) => {
                write!(formatter, "REUSABLE_DRAFT_UNAVAILABLE: {message}")
            }
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TaskHistoryError {}

impl From<RepositoryError> for TaskHistoryError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_snapshot_values, DraftValueView};
    use crate::compiler::RecipeParser;
    use serde_json::json;

    fn recipe() -> crate::domain::Recipe {
        RecipeParser::parse(
            "schema_version: 1\nid: demo\nname: Demo\nworkflow:\n  file: workflow_api.json\ninputs:\n  prompt:\n    type: textarea\n    label: Prompt\n    required: true\n  steps:\n    type: integer\n    label: Steps\n    required: true\n    min: 1\n    max: 50\n  seed:\n    type: seed\n    label: Seed\n    default: random\n  image:\n    type: image\n    label: Image\n    required: false\nbindings: []\noutputs: []\n",
        )
        .unwrap()
    }

    fn multi_recipe() -> crate::domain::Recipe {
        RecipeParser::parse(
            "schema_version: 1\nid: multi\nname: Multi\nworkflow:\n  file: workflow_api.json\ninputs:\n  references:\n    type: images\n    label: References\n    required: false\n    min_items: 1\n    max_items: 3\nbindings: []\noutputs: []\n",
        )
        .unwrap()
    }

    fn media_recipe() -> crate::domain::Recipe {
        RecipeParser::parse(
            "schema_version: 1\nid: media\nname: Media\nworkflow:\n  file: workflow_api.json\ninputs:\n  video:\n    type: video\n    label: Video\n    required: true\n  audio:\n    type: audio\n    label: Audio\n    required: false\n  videos:\n    type: videos\n    label: Videos\n    required: false\n    min_items: 0\n    max_items: 3\n  audios:\n    type: audios\n    label: Audios\n    required: false\n    min_items: 0\n    max_items: 3\nbindings: []\noutputs: []\n",
        )
        .unwrap()
    }

    fn number_recipe() -> crate::domain::Recipe {
        RecipeParser::parse(
            r#"
schema_version: 1
id: number_history
name: Number History
workflow:
  file: workflow_api.json
inputs:
  strength:
    type: number
    label: Strength
    required: true
    min: 0.0
    max: 1.0
    step: 0.1
bindings: []
outputs: []
"#,
        )
        .unwrap()
    }

    #[test]
    fn parses_random_fixed_u64_max_and_image_values_without_exact_retry_payload() {
        let values = parse_snapshot_values(
            &recipe(),
            &json!({
                "prompt": "hello",
                "steps": 20,
                "seed": 18446744073709551615u64,
                "image": {"type": "image_asset", "assetId": "ast_history"}
            }),
        )
        .unwrap();
        assert_eq!(
            values["seed"],
            DraftValueView::SeedFixed {
                value: "18446744073709551615".to_owned()
            }
        );
        assert_eq!(
            values["image"],
            DraftValueView::ImageAsset {
                asset_id: "ast_history".to_owned()
            }
        );
        assert_eq!(
            values["prompt"],
            DraftValueView::String {
                value: "hello".to_owned()
            }
        );
    }

    #[test]
    fn parses_number_from_historical_task_snapshot() {
        let values = parse_snapshot_values(&number_recipe(), &json!({"strength": 0.3})).unwrap();
        assert_eq!(values["strength"], DraftValueView::Number { value: 0.3 });
    }

    #[test]
    fn rejects_unknown_or_wrongly_typed_snapshot_inputs() {
        assert!(parse_snapshot_values(
            &recipe(),
            &json!({"prompt": "hello", "steps": 20, "seed": "random", "extra": true})
        )
        .is_err());
        assert!(parse_snapshot_values(
            &recipe(),
            &json!({"prompt": "hello", "steps": "20", "seed": "random"})
        )
        .is_err());
    }

    #[test]
    fn parses_multi_image_snapshot_in_original_order() {
        let values = parse_snapshot_values(
            &multi_recipe(),
            &json!({
                "references": {
                    "type": "image_assets",
                    "assetIds": ["ast_first", "ast_second"]
                }
            }),
        )
        .unwrap();
        assert_eq!(
            values["references"],
            DraftValueView::ImageAssets {
                asset_ids: vec!["ast_first".to_owned(), "ast_second".to_owned()]
            }
        );
    }

    #[test]
    fn parses_media_snapshot_values_in_original_order() {
        let values = parse_snapshot_values(
            &media_recipe(),
            &json!({
                "video": {"type": "video_asset", "assetId": "ast_video"},
                "audio": {"type": "audio_asset", "assetId": "ast_audio"},
                "videos": {"type": "video_assets", "assetIds": ["ast_video_a", "ast_video_b"]},
                "audios": {"type": "audio_assets", "assetIds": ["ast_audio_a", "ast_audio_b"]}
            }),
        )
        .unwrap();
        assert_eq!(
            values["video"],
            DraftValueView::VideoAsset {
                asset_id: "ast_video".to_owned()
            }
        );
        assert_eq!(
            values["audio"],
            DraftValueView::AudioAsset {
                asset_id: "ast_audio".to_owned()
            }
        );
        assert_eq!(
            values["videos"],
            DraftValueView::VideoAssets {
                asset_ids: vec!["ast_video_a".to_owned(), "ast_video_b".to_owned()]
            }
        );
        assert_eq!(
            values["audios"],
            DraftValueView::AudioAssets {
                asset_ids: vec!["ast_audio_a".to_owned(), "ast_audio_b".to_owned()]
            }
        );
    }

    #[test]
    fn history_detail_serialization_does_not_expose_runtime_or_snapshot_payloads() {
        let detail = super::TaskDetailView {
            id: "tsk_secure".to_owned(),
            project_id: "prj_default".to_owned(),
            workflow_id: "workflow".to_owned(),
            workflow_version_id: "workflow-version".to_owned(),
            recipe_id: "recipe".to_owned(),
            runtime_provenance: None,
            telemetry: super::TaskTelemetryView {
                generation_execution_id: None,
                compiled_workflow_sha256: None,
                runtime_profile: None,
                concurrency_class: None,
                prepare_started_at: None,
                prepared_at: None,
                submitted_at: None,
                execution_started_at: None,
                execution_finished_at: None,
                collection_finished_at: None,
                queue_wait_ms: None,
                prepare_ms: None,
                submit_ms: None,
                comfy_execution_ms: None,
                collection_ms: None,
                total_ms: None,
            },
            workflow_name: "Demo".to_owned(),
            status: "FAILED".to_owned(),
            created_at: chrono::Utc::now(),
            queued_at: None,
            started_at: None,
            finished_at: None,
            error_code: Some("WORKFLOW_INVALID".to_owned()),
            error_message: Some("The task did not complete successfully.".to_owned()),
            node_errors: Vec::new(),
            raw_error: None,
            output_assets: Vec::new(),
            reusable_draft: super::ReusableDraftAvailabilityView {
                available: false,
                reason: Some(super::INPUTS_UNAVAILABLE.to_owned()),
                missing_asset_ids: Vec::new(),
            },
        };
        let json = serde_json::to_string(&detail).unwrap();
        for forbidden in [
            "workflowJson",
            "recipeYaml",
            "rawErrorJson",
            "clientId",
            "promptId",
        ] {
            assert!(!json.contains(forbidden), "task detail leaked {forbidden}");
        }
    }

    #[test]
    fn structured_node_errors_preserve_comfy_validation_context() {
        let raw = serde_json::json!({
            "26": {
                "class_type": "NBH3HyperStepSimple",
                "errors": [{
                    "type": "value_not_in_list",
                    "message": "Value not in list: Middle-36",
                    "details": "The selected mode is not available.",
                    "extra_info": {
                        "input_name": "mode",
                        "received_value": "Middle-36",
                        "input_config": [["Middle-20", "Middle-28"]]
                    }
                }]
            }
        });

        let errors = super::structured_node_errors(Some(&raw));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].node_id, "26");
        assert_eq!(errors[0].node_type.as_deref(), Some("NBH3HyperStepSimple"));
        assert_eq!(errors[0].input.as_deref(), Some("mode"));
        assert_eq!(errors[0].error_type.as_deref(), Some("value_not_in_list"));
        assert_eq!(errors[0].message, "Value not in list: Middle-36");
        assert_eq!(
            errors[0].received_value,
            Some(serde_json::json!("Middle-36"))
        );
        assert_eq!(
            errors[0].expected_config,
            Some(serde_json::json!([["Middle-20", "Middle-28"]]))
        );
    }
}
