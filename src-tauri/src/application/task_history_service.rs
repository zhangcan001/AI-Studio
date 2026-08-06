use crate::application::asset_query_service::{AssetSummaryView, AssetView};
use crate::application::pagination::PageCursor;
use crate::application::ports::{
    AssetRepository, GenerationDefinitionRepository, GenerationSnapshotRepository, RepositoryError,
    TaskHistoryFilter, TaskHistoryRecord, TaskHistoryRepository, TaskOutputAssetMapping,
};
use crate::compiler::RecipeParser;
use crate::domain::{AssetType, InputDefinition, Recipe, TaskId};
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
        project_id: &str,
        filter: TaskHistoryFilter,
        cursor: Option<PageCursor>,
        limit: u32,
    ) -> Result<TaskHistoryPageView, TaskHistoryError> {
        if project_id.trim().is_empty() {
            return Err(TaskHistoryError::InvalidProjectId);
        }
        let page = self
            .history_repository
            .list_page(project_id, filter, cursor, limit.clamp(1, 100))
            .await?;
        Ok(TaskHistoryPageView {
            items: page
                .items
                .iter()
                .map(TaskHistoryItemView::from_record)
                .collect(),
            next_cursor: page.next_cursor,
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
                .map(|_| "The task did not complete successfully.".to_owned()),
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
            let asset_ids: Vec<&String> = match value {
                DraftValueView::ImageAsset { asset_id } => vec![asset_id],
                DraftValueView::ImageAssets { asset_ids } => asset_ids.iter().collect(),
                _ => Vec::new(),
            };
            for asset_id in asset_ids {
                let asset =
                    self.asset_repository
                        .find_by_id(&crate::domain::AssetId::parse(asset_id.clone()).map_err(
                            |_| TaskHistoryError::DraftUnavailable(INPUTS_UNAVAILABLE.to_owned()),
                        )?)
                        .await?;
                if !matches!(asset, Some(asset) if asset.project_id == record.task.project_id && asset.asset_type == AssetType::Image)
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
    pub workflow_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub output_assets: Vec<AssetSummaryView>,
    pub reusable_draft: ReusableDraftAvailabilityView,
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
                    | InputDefinition::Image { required: true, .. }
                    | InputDefinition::Images { required: true, .. }
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
            InputDefinition::Integer { min, max, .. } => {
                let integer = value.as_i64().ok_or("integer input must be an integer")?;
                if min.is_some_and(|min| integer < min) || max.is_some_and(|max| integer > max) {
                    return Err("integer input is outside the current recipe range");
                }
                DraftValueView::Integer { value: integer }
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
        };
        values.insert(key.clone(), parsed);
    }
    Ok(values)
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
    fn history_detail_serialization_does_not_expose_runtime_or_snapshot_payloads() {
        let detail = super::TaskDetailView {
            id: "tsk_secure".to_owned(),
            project_id: "prj_default".to_owned(),
            workflow_id: "workflow".to_owned(),
            workflow_version_id: "workflow-version".to_owned(),
            recipe_id: "recipe".to_owned(),
            workflow_name: "Demo".to_owned(),
            status: "FAILED".to_owned(),
            created_at: chrono::Utc::now(),
            queued_at: None,
            started_at: None,
            finished_at: None,
            error_code: Some("WORKFLOW_INVALID".to_owned()),
            error_message: Some("The task did not complete successfully.".to_owned()),
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
            "nodeId",
            "clientId",
            "promptId",
        ] {
            assert!(!json.contains(forbidden), "task detail leaked {forbidden}");
        }
    }
}
