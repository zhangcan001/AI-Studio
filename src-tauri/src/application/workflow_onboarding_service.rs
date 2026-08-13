use crate::application::{
    ports::{
        Clock, ComfyAdapter, ComfyAdapterError, WorkflowLibrarySource, WorkflowPackageBytes,
        WorkflowPackageLoad, WorkflowPackageStore, WorkflowRunRepository,
        WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
    },
    workflow_library_service::{WorkflowLibraryService, WorkflowSyncReport},
    workflow_manifest::WorkflowManifest,
};
use crate::compiler::{
    BindingValidator, RecipeParser, RecipeValidator, WorkflowCompiler, WorkflowValidator,
};
use crate::domain::{
    Binding, BindingTarget, CompileRequest, InputDefinition, InputValue, OutputDefinition,
    OutputType, Recipe, SeedDefault, SeedValue, WorkflowDocument,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    error::Error,
    fmt,
    path::Path,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

pub const MAX_WORKFLOW_IMPORT_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_ONBOARDING_DRAFTS: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowOnboardingError {
    code: &'static str,
    message: String,
}

impl WorkflowOnboardingError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for WorkflowOnboardingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for WorkflowOnboardingError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SemanticFieldType {
    Textarea,
    Integer,
    Seed,
    Image,
    Images,
    Video,
    Videos,
    Audio,
    Audios,
}

impl SemanticFieldType {
    fn parse(value: &str) -> Result<Self, WorkflowOnboardingError> {
        match value {
            "textarea" => Ok(Self::Textarea),
            "integer" => Ok(Self::Integer),
            "seed" => Ok(Self::Seed),
            "image" => Ok(Self::Image),
            "images" => Ok(Self::Images),
            "video" => Ok(Self::Video),
            "videos" => Ok(Self::Videos),
            "audio" => Ok(Self::Audio),
            "audios" => Ok(Self::Audios),
            _ => Err(WorkflowOnboardingError::new(
                "MAPPING_UNSUPPORTED",
                format!("unsupported semantic field type {value}"),
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Textarea => "textarea",
            Self::Integer => "integer",
            Self::Seed => "seed",
            Self::Image => "image",
            Self::Images => "images",
            Self::Video => "video",
            Self::Videos => "videos",
            Self::Audio => "audio",
            Self::Audios => "audios",
        }
    }

    fn is_plural(self) -> bool {
        matches!(self, Self::Images | Self::Videos | Self::Audios)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CapabilityState {
    NotChecked,
    Ready,
    MissingNodes,
    IncompatibleInputValues,
    ComfyOffline,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowAutoOnboardingState {
    AutoPublished,
    NeedsReview,
    WaitingForComfyUi,
    AlreadyExists,
    AlreadyExistsArchived,
    Blocked,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InferenceConfidence {
    Certain,
    High,
    Ambiguous,
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOnboardingMetadataRequest {
    #[serde(default)]
    pub workflow_id: Option<String>,
    pub name: String,
    pub workflow_version: String,
    pub recipe_version: String,
    pub category: String,
    pub mode: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOnboardingInputMappingRequest {
    pub semantic_key: String,
    pub field_type: String,
    pub label: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub min_value: Option<String>,
    #[serde(default)]
    pub max_value: Option<String>,
    #[serde(default)]
    pub step: Option<String>,
    #[serde(default)]
    pub min_items: Option<usize>,
    #[serde(default)]
    pub max_items: Option<usize>,
    pub target_node: String,
    pub target_input: String,
    #[serde(default)]
    pub item_index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOnboardingRemoveInputMappingRequest {
    pub semantic_key: String,
    #[serde(default)]
    pub item_index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOnboardingOutputMappingRequest {
    pub output_id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub output_type: String,
    pub node_id: String,
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool {
    true
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOnboardingDraftView {
    pub draft_id: String,
    pub workflow_sha256: String,
    pub original_filename: String,
    pub node_count: usize,
    pub unique_class_count: usize,
    pub nodes: Vec<WorkflowNodeView>,
    pub capability: CapabilityCheckView,
    pub input_mappings: Vec<WorkflowInputMappingView>,
    pub output_mappings: Vec<WorkflowOutputMappingView>,
    pub manifest: WorkflowManifestView,
    pub recipe: RecipeDraftView,
    pub validation: WorkflowOnboardingValidationView,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowNodeView {
    pub node_id: String,
    pub class_type: String,
    pub title: String,
    pub is_output_node: bool,
    pub inputs: Vec<WorkflowInputView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInputView {
    pub name: String,
    pub kind: String,
    pub current_value_summary: String,
    pub is_linked: bool,
    pub bindable: bool,
    pub suggested_type: Option<String>,
    pub suggested_semantic_key: Option<String>,
    pub numeric_min: Option<String>,
    pub numeric_max: Option<String>,
    pub numeric_step: Option<String>,
    pub allowed_options: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityCheckView {
    pub state: CapabilityState,
    pub checked_at: Option<String>,
    pub issues: Vec<CapabilityIssueView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityIssueView {
    pub code: String,
    pub class_type: Option<String>,
    pub node_id: Option<String>,
    pub affected_node_ids: Vec<String>,
    pub input_name: Option<String>,
    pub current_value: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInputMappingView {
    pub semantic_key: String,
    pub field_type: String,
    pub label: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub step: Option<String>,
    pub min_items: Option<usize>,
    pub max_items: Option<usize>,
    pub target_node: String,
    pub target_input: String,
    pub item_index: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOutputMappingView {
    pub output_id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub output_type: String,
    pub node_id: String,
    pub required: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifestView {
    pub workflow_id: String,
    pub name: String,
    pub workflow_version: String,
    pub recipe_version: String,
    pub category: String,
    pub mode: String,
    pub recipe_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeDraftView {
    pub inputs: Vec<RecipeInputView>,
    pub bindings: Vec<RecipeBindingView>,
    pub outputs: Vec<WorkflowOutputMappingView>,
    pub yaml: Option<String>,
    pub valid: bool,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeInputView {
    pub key: String,
    pub field_type: String,
    pub label: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub step: Option<String>,
    pub min_items: Option<usize>,
    pub max_items: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeBindingView {
    pub semantic_key: String,
    pub target_node: String,
    pub target_input: String,
    pub item_index: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOnboardingValidationView {
    pub api_format: bool,
    pub recipe: bool,
    pub bindings: bool,
    pub outputs: bool,
    pub manifest: bool,
    pub capability: bool,
    pub dry_run: bool,
    pub ready_to_publish: bool,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOnboardingPublishView {
    pub workflow_id: String,
    pub workflow_version: String,
    pub recipe_id: String,
    pub package_name: String,
    pub workflow_sha256: String,
    pub refreshed: WorkflowSyncReport,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAutoInferenceView {
    pub field: String,
    pub value: Option<String>,
    pub confidence: InferenceConfidence,
    pub source: String,
    pub alternatives: Vec<String>,
    pub node_id: Option<String>,
    pub input_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAutoIssueCandidateView {
    pub label: String,
    pub node_id: Option<String>,
    pub input_name: Option<String>,
    pub output_id: Option<String>,
    pub output_type: Option<String>,
    pub field_type: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAutoIssueView {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
    pub candidates: Vec<WorkflowAutoIssueCandidateView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAutoOnboardingPlanView {
    pub draft_id: String,
    pub state: WorkflowAutoOnboardingState,
    pub workflow_kind: String,
    pub workflow_sha256: String,
    pub original_filename: String,
    pub node_count: usize,
    pub unique_class_count: usize,
    pub metadata: WorkflowManifestView,
    pub capability: CapabilityCheckView,
    pub input_mappings: Vec<WorkflowInputMappingView>,
    pub output_mappings: Vec<WorkflowOutputMappingView>,
    pub validation: WorkflowOnboardingValidationView,
    pub inferences: Vec<WorkflowAutoInferenceView>,
    pub issues: Vec<WorkflowAutoIssueView>,
    pub auto_publishable: bool,
    pub published: Option<WorkflowOnboardingPublishView>,
    pub existing_workflow_id: Option<String>,
    pub existing_workflow_version: Option<String>,
    pub existing_package_name: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWorkspaceView {
    pub workflow_id: String,
    pub name: String,
    pub workflow_version: String,
    pub mode: String,
    pub package_name: String,
    pub package_status: String,
    pub workflow_sha256: String,
    pub node_count: usize,
    pub unique_class_count: usize,
    pub capability: CapabilityState,
    pub capability_issues: Vec<CapabilityIssueView>,
    pub input_mappings: Vec<WorkflowInputMappingView>,
    pub outputs: Vec<WorkflowOutputMappingView>,
    pub has_successful_run: bool,
}

#[derive(Clone, Debug)]
struct InputMapping {
    semantic_key: String,
    field_type: SemanticFieldType,
    label: String,
    required: bool,
    default_value: Option<String>,
    min_value: Option<String>,
    max_value: Option<String>,
    step: Option<String>,
    min_items: Option<usize>,
    max_items: Option<usize>,
    target_node: String,
    target_input: String,
    item_index: Option<usize>,
}

#[derive(Clone, Debug)]
struct OutputMapping {
    output_id: String,
    label: String,
    output_type: OutputType,
    node_id: String,
    required: bool,
}

#[derive(Clone, Debug)]
struct WorkflowOnboardingDraft {
    draft_id: String,
    raw_bytes: Vec<u8>,
    workflow: WorkflowDocument,
    workflow_sha256: String,
    original_filename: String,
    nodes: Vec<WorkflowNodeView>,
    manifest: WorkflowManifest,
    recipe_id: String,
    allow_existing_workflow_sha: bool,
    capability: CapabilityCheckView,
    input_mappings: Vec<InputMapping>,
    output_mappings: Vec<OutputMapping>,
}

#[derive(Default)]
struct WorkflowOnboardingRegistry {
    order: VecDeque<String>,
    drafts: BTreeMap<String, WorkflowOnboardingDraft>,
}

impl WorkflowOnboardingRegistry {
    fn insert(&mut self, draft: WorkflowOnboardingDraft) {
        let id = draft.draft_id.clone();
        self.drafts.insert(id.clone(), draft);
        self.order.retain(|candidate| candidate != &id);
        self.order.push_back(id);
        while self.order.len() > MAX_ONBOARDING_DRAFTS {
            if let Some(oldest) = self.order.pop_front() {
                self.drafts.remove(&oldest);
            }
        }
    }

    fn get(&self, draft_id: &str) -> Result<WorkflowOnboardingDraft, WorkflowOnboardingError> {
        self.drafts.get(draft_id).cloned().ok_or_else(|| {
            WorkflowOnboardingError::new(
                "WORKFLOW_ONBOARDING_DRAFT_NOT_FOUND",
                format!("draft {draft_id} was not found"),
            )
        })
    }

    fn get_mut(
        &mut self,
        draft_id: &str,
    ) -> Result<&mut WorkflowOnboardingDraft, WorkflowOnboardingError> {
        self.drafts.get_mut(draft_id).ok_or_else(|| {
            WorkflowOnboardingError::new(
                "WORKFLOW_ONBOARDING_DRAFT_NOT_FOUND",
                format!("draft {draft_id} was not found"),
            )
        })
    }

    fn remove(&mut self, draft_id: &str) -> Result<(), WorkflowOnboardingError> {
        if self.drafts.remove(draft_id).is_none() {
            return Err(WorkflowOnboardingError::new(
                "WORKFLOW_ONBOARDING_DRAFT_NOT_FOUND",
                format!("draft {draft_id} was not found"),
            ));
        }
        self.order.retain(|candidate| candidate != draft_id);
        Ok(())
    }
}

pub struct WorkflowOnboardingService {
    source: Arc<dyn WorkflowLibrarySource>,
    comfy_adapter: Arc<dyn ComfyAdapter>,
    workflow_library_service: Arc<WorkflowLibraryService>,
    workflow_run_repository: Arc<dyn WorkflowRunRepository>,
    package_store: Arc<dyn WorkflowPackageStore>,
    clock: Arc<dyn Clock>,
    runtime_repository: Option<Arc<dyn WorkflowRuntimeRepository>>,
    state_repository: Option<Arc<dyn WorkflowRuntimeStateRepository>>,
    registry: Mutex<WorkflowOnboardingRegistry>,
}

impl WorkflowOnboardingService {
    pub fn new(
        source: Arc<dyn WorkflowLibrarySource>,
        comfy_adapter: Arc<dyn ComfyAdapter>,
        workflow_library_service: Arc<WorkflowLibraryService>,
        workflow_run_repository: Arc<dyn WorkflowRunRepository>,
        package_store: Arc<dyn WorkflowPackageStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            source,
            comfy_adapter,
            workflow_library_service,
            workflow_run_repository,
            package_store,
            clock,
            runtime_repository: None,
            state_repository: None,
            registry: Mutex::new(WorkflowOnboardingRegistry::default()),
        }
    }

    pub fn with_runtime_state(
        mut self,
        runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
        state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
    ) -> Self {
        self.runtime_repository = Some(runtime_repository);
        self.state_repository = Some(state_repository);
        self
    }

    pub async fn import_bytes(
        &self,
        bytes: Vec<u8>,
        original_filename: String,
        existing_workflow_id: Option<String>,
    ) -> Result<WorkflowOnboardingDraftView, WorkflowOnboardingError> {
        if bytes.len() as u64 > MAX_WORKFLOW_IMPORT_BYTES {
            return Err(WorkflowOnboardingError::new(
                "WORKFLOW_FILE_TOO_LARGE",
                format!(
                    "workflow import is {} bytes; maximum is {} bytes",
                    bytes.len(),
                    MAX_WORKFLOW_IMPORT_BYTES
                ),
            ));
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
            WorkflowOnboardingError::new(
                "WORKFLOW_NOT_API_FORMAT",
                format!("workflow is not valid JSON: {error}"),
            )
        })?;
        let workflow = validate_api_workflow(value)?;
        let nodes = inspect_workflow(&workflow)?;
        let workflow_sha256 = sha256(&bytes);
        let is_new_version = existing_workflow_id.is_some();
        let workflow_id = match existing_workflow_id {
            Some(value) => validate_workflow_id(&value)?,
            None => format!("wfl_{}", slugify(&filename_stem(&original_filename))),
        };
        let workflow_version = if is_new_version {
            self.next_workflow_version(&workflow_id).await
        } else {
            "1.0.0".to_owned()
        };
        let name = filename_stem(&original_filename);
        let draft_id = format!("onb_{}", Uuid::new_v4());
        let recipe_id = format!("rcp_{}", Uuid::new_v4());
        let draft = WorkflowOnboardingDraft {
            draft_id,
            raw_bytes: bytes,
            workflow,
            workflow_sha256,
            original_filename: safe_filename(&original_filename),
            nodes,
            manifest: WorkflowManifest {
                schema_version: 1,
                id: workflow_id,
                name: if name.trim().is_empty() {
                    "Imported Workflow".to_owned()
                } else {
                    name
                },
                workflow_version,
                recipe_version: "1.0.0".to_owned(),
                category: "image".to_owned(),
                mode: "text_to_image".to_owned(),
            },
            recipe_id,
            allow_existing_workflow_sha: false,
            capability: CapabilityCheckView {
                state: CapabilityState::NotChecked,
                checked_at: None,
                issues: Vec::new(),
            },
            input_mappings: Vec::new(),
            output_mappings: Vec::new(),
        };
        let draft_id = draft.draft_id.clone();
        self.with_registry(|registry| {
            registry.insert(draft);
            Ok(())
        })??;
        self.get(&draft_id)
    }

    pub async fn auto_onboard_bytes(
        &self,
        bytes: Vec<u8>,
        original_filename: String,
        existing_workflow_id: Option<String>,
    ) -> Result<WorkflowAutoOnboardingPlanView, WorkflowOnboardingError> {
        let draft = self
            .import_bytes(bytes, original_filename, existing_workflow_id)
            .await?;
        self.auto_confirm(&draft.draft_id).await
    }

    pub async fn auto_confirm(
        &self,
        draft_id: &str,
    ) -> Result<WorkflowAutoOnboardingPlanView, WorkflowOnboardingError> {
        let initial = self.with_registry(|registry| registry.get(draft_id))??;
        if let Some((manifest, package_name)) = self
            .existing_package_for_sha(&initial.workflow_sha256)
            .await?
        {
            let archived = self
                .is_archived_package(&manifest.id, &manifest.workflow_version)
                .await?;
            return Ok(auto_plan_for_draft(
                &initial,
                if archived {
                    WorkflowAutoOnboardingState::AlreadyExistsArchived
                } else {
                    WorkflowAutoOnboardingState::AlreadyExists
                },
                &[],
                &[],
                false,
                None,
                Some(manifest.id),
                Some(manifest.workflow_version),
                Some(package_name),
                if archived {
                    "该工作流已归档，可在工作流管理中恢复。".to_owned()
                } else {
                    "该工作流已经导入。".to_owned()
                },
            ));
        }

        let (capability, enriched_nodes) = self.check_capability_for_workflow(&initial).await;
        self.with_registry(|registry| {
            let draft = registry.get_mut(draft_id)?;
            draft.capability = capability.clone();
            if let Some(nodes) = enriched_nodes {
                draft.nodes = nodes;
            }
            Ok(())
        })??;

        let current = self.with_registry(|registry| registry.get(draft_id))??;
        let inference = infer_auto_onboarding(&current);
        self.with_registry(|registry| {
            let draft = registry.get_mut(draft_id)?;
            for mapping in &inference.input_mappings {
                let already_mapped = draft.input_mappings.iter().any(|existing| {
                    existing.target_node == mapping.target_node
                        && existing.target_input == mapping.target_input
                }) || draft.input_mappings.iter().any(|existing| {
                    existing.semantic_key == mapping.semantic_key
                        && existing.item_index == mapping.item_index
                });
                if !already_mapped {
                    draft.input_mappings.push(mapping.clone());
                }
            }
            if draft.output_mappings.is_empty() {
                draft.output_mappings = inference.output_mappings.clone();
            }
            draft.input_mappings.sort_by(|left, right| {
                left.semantic_key
                    .cmp(&right.semantic_key)
                    .then(left.item_index.cmp(&right.item_index))
            });
            draft
                .output_mappings
                .sort_by(|left, right| left.output_id.cmp(&right.output_id));
            draft.manifest.name = inference.name.clone();
            draft.manifest.category = inference.category.clone();
            draft.manifest.mode = inference.mode.clone();
            Ok(())
        })??;

        let current = self.with_registry(|registry| registry.get(draft_id))??;
        let validation = validation_for_draft(&current);
        let mut issues = inference.issues.clone();
        issues.extend(auto_issues_from_capability(&current.capability));
        let has_ambiguous_required = issues.iter().any(|issue| {
            matches!(
                issue.code.as_str(),
                "AMBIGUOUS_INPUT"
                    | "UNKNOWN_INPUT"
                    | "AMBIGUOUS_OUTPUT"
                    | "UNKNOWN_OUTPUT"
                    | "FLOAT_INPUT_NEEDS_REVIEW"
            )
        });
        let auto_publishable =
            validation.ready_to_publish && !has_ambiguous_required && issues.is_empty();

        if auto_publishable {
            let published = self.publish(draft_id).await?;
            let draft = self.with_registry(|registry| registry.get(draft_id))??;
            return Ok(auto_plan_for_draft(
                &draft,
                WorkflowAutoOnboardingState::AutoPublished,
                &inference.inferences,
                &[],
                true,
                Some(published),
                None,
                None,
                None,
                "工作流导入成功，已自动生成 Recipe 并启用。".to_owned(),
            ));
        }

        let (state, message) = match current.capability.state {
            CapabilityState::ComfyOffline => (
                WorkflowAutoOnboardingState::WaitingForComfyUi,
                "工作流已解析，连接 ComfyUI 后即可完成自动确认。".to_owned(),
            ),
            CapabilityState::MissingNodes => (
                WorkflowAutoOnboardingState::Blocked,
                "工作流需要确认：当前 ComfyUI 缺少工作流节点。".to_owned(),
            ),
            _ if issues.is_empty() => (
                WorkflowAutoOnboardingState::Blocked,
                "工作流需要确认：自动校验尚未通过。".to_owned(),
            ),
            _ => (
                WorkflowAutoOnboardingState::NeedsReview,
                format!("工作流需要确认，发现{}个问题。", issues.len()),
            ),
        };
        Ok(auto_plan_for_draft(
            &current,
            state,
            &inference.inferences,
            &issues,
            auto_publishable,
            None,
            None,
            None,
            None,
            message,
        ))
    }

    async fn existing_package_for_sha(
        &self,
        workflow_sha256: &str,
    ) -> Result<Option<(WorkflowManifest, String)>, WorkflowOnboardingError> {
        let packages = self.source.load_packages().await.map_err(|error| {
            WorkflowOnboardingError::new("WORKFLOW_LIBRARY_ERROR", error.to_string())
        })?;
        Ok(packages.into_iter().find_map(|package| {
            let WorkflowPackageLoad::Loaded(files) = package else {
                return None;
            };
            (sha256(files.workflow_json.as_bytes()) == workflow_sha256)
                .then(|| {
                    WorkflowManifest::parse(&files.manifest_yaml)
                        .ok()
                        .map(|manifest| (manifest, files.package_name))
                })
                .flatten()
        }))
    }

    async fn is_archived_package(
        &self,
        workflow_id: &str,
        workflow_version: &str,
    ) -> Result<bool, WorkflowOnboardingError> {
        let (Some(runtime_repository), Some(state_repository)) =
            (&self.runtime_repository, &self.state_repository)
        else {
            return Ok(false);
        };
        let versions = runtime_repository
            .list_versions()
            .await
            .map_err(|error| WorkflowOnboardingError::new("DATABASE_ERROR", error.to_string()))?;
        let Some(version) = versions.into_iter().find(|version| {
            version.workflow_id == workflow_id && version.workflow_version == workflow_version
        }) else {
            return Ok(false);
        };
        Ok(state_repository
            .find_state(&version.workflow_version_id)
            .await
            .map_err(|error| WorkflowOnboardingError::new("DATABASE_ERROR", error.to_string()))?
            .is_some_and(|state| state.archived))
    }

    pub fn get(
        &self,
        draft_id: &str,
    ) -> Result<WorkflowOnboardingDraftView, WorkflowOnboardingError> {
        let draft = self.with_registry(|registry| registry.get(draft_id))??;
        Ok(view_for_draft(&draft))
    }

    pub fn is_draft_active(&self, draft_id: &str) -> bool {
        self.registry
            .lock()
            .map(|registry| registry.drafts.contains_key(draft_id))
            .unwrap_or(true)
    }

    pub async fn duplicate_recipe_draft(
        &self,
        workflow_id: &str,
        workflow_version: &str,
        source_recipe_version: Option<&str>,
        recipe_version: Option<String>,
    ) -> Result<WorkflowOnboardingDraftView, WorkflowOnboardingError> {
        let packages = self.source.load_packages().await.map_err(|error| {
            WorkflowOnboardingError::new("WORKFLOW_LIBRARY_ERROR", error.to_string())
        })?;
        let files = packages
            .into_iter()
            .find_map(|package| match package {
                WorkflowPackageLoad::Loaded(files) => {
                    let manifest = WorkflowManifest::parse(&files.manifest_yaml).ok()?;
                    (manifest.id == workflow_id
                        && manifest.workflow_version == workflow_version
                        && source_recipe_version
                            .map(|version| manifest.recipe_version == version)
                            .unwrap_or(true))
                    .then_some((files, manifest))
                }
                WorkflowPackageLoad::Invalid { .. } => None,
            })
            .ok_or_else(|| {
                WorkflowOnboardingError::new(
                    "RUNTIME_PACKAGE_MISSING",
                    "the requested runtime package is not available",
                )
            })?;
        let (files, manifest) = files;
        let workflow_value: Value =
            serde_json::from_str(&files.workflow_json).map_err(|error| {
                WorkflowOnboardingError::new("WORKFLOW_NOT_API_FORMAT", error.to_string())
            })?;
        let workflow = validate_api_workflow(workflow_value)?;
        let recipe = RecipeParser::parse(&files.recipe_yaml)
            .map_err(|error| WorkflowOnboardingError::new("RECIPE_INVALID", error.to_string()))?;
        let mut next_manifest = manifest.clone();
        next_manifest.recipe_version = recipe_version
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| increment_semver(&manifest.recipe_version));
        validate_metadata_values(
            &next_manifest.name,
            &next_manifest.workflow_version,
            &next_manifest.recipe_version,
            &next_manifest.category,
            &next_manifest.mode,
        )?;
        let raw_bytes = files.workflow_json.into_bytes();
        let draft = WorkflowOnboardingDraft {
            draft_id: format!("onb_{}", Uuid::new_v4()),
            workflow_sha256: sha256(&raw_bytes),
            original_filename: safe_filename(&format!("{}.json", next_manifest.name)),
            nodes: inspect_workflow(&workflow)?,
            workflow,
            raw_bytes,
            manifest: next_manifest,
            recipe_id: format!("rcp_{}", Uuid::new_v4()),
            allow_existing_workflow_sha: true,
            capability: CapabilityCheckView {
                state: CapabilityState::NotChecked,
                checked_at: None,
                issues: Vec::new(),
            },
            input_mappings: input_mappings_from_recipe(&recipe)?,
            output_mappings: recipe
                .outputs
                .iter()
                .map(output_mapping_from_recipe)
                .collect(),
        };
        let draft_id = draft.draft_id.clone();
        self.with_registry(|registry| {
            registry.insert(draft);
            Ok(())
        })??;
        self.get(&draft_id)
    }

    pub fn set_metadata(
        &self,
        draft_id: &str,
        request: WorkflowOnboardingMetadataRequest,
    ) -> Result<WorkflowOnboardingDraftView, WorkflowOnboardingError> {
        let view = self.with_registry(|registry| {
            let draft = registry.get_mut(draft_id)?;
            if let Some(workflow_id) = request.workflow_id {
                draft.manifest.id = validate_workflow_id(&workflow_id)?;
            }
            validate_metadata_values(
                &request.name,
                &request.workflow_version,
                &request.recipe_version,
                &request.category,
                &request.mode,
            )?;
            draft.manifest.name = request.name;
            draft.manifest.workflow_version = request.workflow_version;
            draft.manifest.recipe_version = request.recipe_version;
            draft.manifest.category = request.category;
            draft.manifest.mode = request.mode;
            Ok(view_for_draft(draft))
        })??;
        Ok(view)
    }

    pub fn set_input_mapping(
        &self,
        draft_id: &str,
        request: WorkflowOnboardingInputMappingRequest,
    ) -> Result<WorkflowOnboardingDraftView, WorkflowOnboardingError> {
        let field_type = SemanticFieldType::parse(&request.field_type)?;
        if !is_safe_key(&request.semantic_key) {
            return Err(WorkflowOnboardingError::new(
                "MAPPING_INVALID",
                "semantic_key must match [a-z][a-z0-9_]{0,63}",
            ));
        }
        if request.label.trim().is_empty() {
            return Err(WorkflowOnboardingError::new(
                "MAPPING_INVALID",
                "mapping label must not be empty",
            ));
        }
        if field_type.is_plural() && request.max_items.unwrap_or(8) == 0 {
            return Err(WorkflowOnboardingError::new(
                "MAPPING_INVALID",
                "plural mapping max_items must be positive",
            ));
        }
        if !field_type.is_plural() && request.item_index.is_some() {
            return Err(WorkflowOnboardingError::new(
                "MAPPING_INVALID",
                "item_index is only valid for plural media mappings",
            ));
        }
        let view = self.with_registry(|registry| {
            let draft = registry.get_mut(draft_id)?;
            let node = draft.workflow.node(&request.target_node).ok_or_else(|| {
                WorkflowOnboardingError::new(
                    "MAPPING_INVALID",
                    format!("target node {} does not exist", request.target_node),
                )
            })?;
            let inputs = node
                .as_object()
                .and_then(|node| node.get("inputs"))
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    WorkflowOnboardingError::new(
                        "MAPPING_INVALID",
                        format!("target node {} has no inputs", request.target_node),
                    )
                })?;
            let current = inputs.get(&request.target_input).ok_or_else(|| {
                WorkflowOnboardingError::new(
                    "MAPPING_INVALID",
                    format!("target input {} does not exist", request.target_input),
                )
            })?;
            if is_workflow_link(current, &draft.workflow)?.is_some() {
                return Err(WorkflowOnboardingError::new(
                    "LINKED_INPUT_NOT_BINDABLE",
                    "Connected input — not directly bindable",
                ));
            }
            if is_dangerous_input_name(&request.target_input) {
                return Err(WorkflowOnboardingError::new(
                    "MAPPING_DANGEROUS_INPUT",
                    format!(
                        "{} is an internal or unsafe input and cannot be exposed",
                        request.target_input
                    ),
                ));
            }
            validate_mapping_value(field_type, current)?;
            let input_view = draft
                .nodes
                .iter()
                .find(|candidate| candidate.node_id == request.target_node)
                .and_then(|candidate| {
                    candidate
                        .inputs
                        .iter()
                        .find(|input| input.name == request.target_input)
                });
            validate_mapping_bounds(field_type, input_view, &request)?;
            let key = (request.semantic_key.clone(), request.item_index);
            if draft.input_mappings.iter().any(|existing| {
                existing.semantic_key == request.semantic_key
                    && existing.item_index != request.item_index
                    && existing.field_type != field_type
            }) {
                return Err(WorkflowOnboardingError::new(
                    "MAPPING_INVALID",
                    "one semantic key cannot use multiple field types",
                ));
            }
            if draft.input_mappings.iter().any(|existing| {
                existing.semantic_key == request.semantic_key
                    && existing.item_index == request.item_index
                    && (existing.target_node != request.target_node
                        || existing.target_input != request.target_input)
            }) {
                return Err(WorkflowOnboardingError::new(
                    "MAPPING_DUPLICATE",
                    "semantic key is already mapped for this item",
                ));
            }
            let min_items = if field_type.is_plural() {
                request
                    .min_items
                    .or_else(|| request.item_index.map(|index| index.saturating_add(1)))
            } else {
                None
            };
            let max_items = if field_type.is_plural() {
                request.max_items.or(Some(8))
            } else {
                None
            };
            if let Some(item_index) = request.item_index {
                if max_items.is_some_and(|max| item_index >= max)
                    || min_items.is_some_and(|min| item_index >= min)
                {
                    return Err(WorkflowOnboardingError::new(
                        "MAPPING_INVALID",
                        "item_index must be within the configured plural input slots",
                    ));
                }
            }
            let mapping = InputMapping {
                semantic_key: request.semantic_key,
                field_type,
                label: request.label,
                required: request.required,
                default_value: request.default_value,
                min_value: request.min_value,
                max_value: request.max_value,
                step: request.step,
                min_items,
                max_items,
                target_node: request.target_node,
                target_input: request.target_input,
                item_index: request.item_index,
            };
            draft.input_mappings.retain(|existing| {
                (existing.semantic_key.clone(), existing.item_index) != key
                    && (existing.target_node != mapping.target_node
                        || existing.target_input != mapping.target_input)
            });
            draft.input_mappings.push(mapping);
            draft.input_mappings.sort_by(|left, right| {
                left.semantic_key
                    .cmp(&right.semantic_key)
                    .then(left.item_index.cmp(&right.item_index))
            });
            Ok(view_for_draft(draft))
        })??;
        Ok(view)
    }

    pub fn remove_input_mapping(
        &self,
        draft_id: &str,
        request: WorkflowOnboardingRemoveInputMappingRequest,
    ) -> Result<WorkflowOnboardingDraftView, WorkflowOnboardingError> {
        let view = self.with_registry(|registry| {
            let draft = registry.get_mut(draft_id)?;
            draft.input_mappings.retain(|mapping| {
                !(mapping.semantic_key == request.semantic_key
                    && mapping.item_index == request.item_index)
            });
            Ok(view_for_draft(draft))
        })??;
        Ok(view)
    }

    pub fn set_output_mapping(
        &self,
        draft_id: &str,
        request: WorkflowOnboardingOutputMappingRequest,
    ) -> Result<WorkflowOnboardingDraftView, WorkflowOnboardingError> {
        if !is_safe_key(&request.output_id) {
            return Err(WorkflowOnboardingError::new(
                "OUTPUT_INVALID",
                "output_id must match [a-z][a-z0-9_]{0,63}",
            ));
        }
        if request.label.trim().is_empty() {
            return Err(WorkflowOnboardingError::new(
                "OUTPUT_INVALID",
                "output label must not be empty",
            ));
        }
        let output_type = match request.output_type.as_str() {
            "image" => OutputType::Image,
            "video" => OutputType::Video,
            _ => {
                return Err(WorkflowOnboardingError::new(
                    "OUTPUT_INVALID",
                    "output type must be image or video",
                ))
            }
        };
        let view = self.with_registry(|registry| {
            let draft = registry.get_mut(draft_id)?;
            if draft.workflow.node(&request.node_id).is_none() {
                return Err(WorkflowOnboardingError::new(
                    "OUTPUT_INVALID",
                    format!("output node {} does not exist", request.node_id),
                ));
            }
            let output = OutputMapping {
                output_id: request.output_id.clone(),
                label: request.label,
                output_type,
                node_id: request.node_id,
                required: request.required,
            };
            if draft
                .output_mappings
                .iter()
                .any(|existing| existing.output_id == request.output_id)
            {
                return Err(WorkflowOnboardingError::new(
                    "OUTPUT_DUPLICATE",
                    format!("output id {} is already mapped", request.output_id),
                ));
            }
            draft.output_mappings.push(output);
            draft
                .output_mappings
                .sort_by(|left, right| left.output_id.cmp(&right.output_id));
            Ok(view_for_draft(draft))
        })??;
        Ok(view)
    }

    pub async fn check_capability(
        &self,
        draft_id: &str,
    ) -> Result<CapabilityCheckView, WorkflowOnboardingError> {
        let draft = self.with_registry(|registry| registry.get(draft_id))??;
        let (capability, enriched_nodes) = self.check_capability_for_workflow(&draft).await;
        self.with_registry(|registry| {
            let draft = registry.get_mut(draft_id)?;
            draft.capability = capability.clone();
            if let Some(nodes) = enriched_nodes {
                draft.nodes = nodes;
            }
            Ok(())
        })??;
        Ok(capability)
    }

    pub async fn check_runtime_workflow(
        &self,
        workflow_json: &str,
    ) -> Result<CapabilityCheckView, WorkflowOnboardingError> {
        let raw_bytes = workflow_json.as_bytes().to_vec();
        let workflow =
            validate_api_workflow(serde_json::from_str(workflow_json).map_err(|error| {
                WorkflowOnboardingError::new("WORKFLOW_NOT_API_FORMAT", error.to_string())
            })?)?;
        let draft = WorkflowOnboardingDraft {
            draft_id: "onb_runtime_check".to_owned(),
            workflow_sha256: sha256(&raw_bytes),
            original_filename: "runtime.json".to_owned(),
            nodes: inspect_workflow(&workflow)?,
            workflow,
            raw_bytes,
            manifest: WorkflowManifest {
                schema_version: 1,
                id: "wfl_runtime_check".to_owned(),
                name: "Runtime Check".to_owned(),
                workflow_version: "1.0.0".to_owned(),
                recipe_version: "1.0.0".to_owned(),
                category: "diagnostic".to_owned(),
                mode: "diagnostic".to_owned(),
            },
            recipe_id: "rcp_runtime_check".to_owned(),
            allow_existing_workflow_sha: true,
            capability: CapabilityCheckView {
                state: CapabilityState::NotChecked,
                checked_at: None,
                issues: Vec::new(),
            },
            input_mappings: Vec::new(),
            output_mappings: Vec::new(),
        };
        Ok(self.check_capability_for_workflow(&draft).await.0)
    }

    pub fn check_runtime_workflow_with_object_info(
        &self,
        workflow_json: &str,
        object_info: &Value,
    ) -> Result<CapabilityCheckView, WorkflowOnboardingError> {
        let workflow =
            validate_api_workflow(serde_json::from_str(workflow_json).map_err(|error| {
                WorkflowOnboardingError::new("WORKFLOW_NOT_API_FORMAT", error.to_string())
            })?)?;
        let nodes = inspect_workflow(&workflow)?;
        Ok(evaluate_capability(&workflow, &nodes, object_info))
    }

    /// Check several runtime graphs against one already fetched ComfyUI
    /// object_info document. This is the only batch capability path and keeps
    /// the network request count at one.
    pub async fn check_runtime_workflows(
        &self,
        workflows: &[(String, String)],
    ) -> Result<Vec<(String, CapabilityCheckView)>, WorkflowOnboardingError> {
        if workflows.is_empty() {
            return Ok(Vec::new());
        }
        let object_info = self.comfy_adapter.get_object_info().await;
        workflows
            .iter()
            .map(|(workflow_version_id, workflow_json)| {
                let capability = match &object_info {
                    Ok(object) if object.is_object() => {
                        self.check_runtime_workflow_with_object_info(workflow_json, object)?
                    }
                    Ok(_) => CapabilityCheckView {
                        state: CapabilityState::IncompatibleInputValues,
                        checked_at: Some(self.clock.now().to_rfc3339()),
                        issues: vec![CapabilityIssueView {
                            code: "COMFY_PROTOCOL_ERROR".to_owned(),
                            class_type: None,
                            node_id: None,
                            affected_node_ids: Vec::new(),
                            input_name: None,
                            current_value: None,
                            message: "ComfyUI object_info response is not an object".to_owned(),
                        }],
                    },
                    Err(ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_)) => {
                        CapabilityCheckView {
                            state: CapabilityState::ComfyOffline,
                            checked_at: Some(self.clock.now().to_rfc3339()),
                            issues: Vec::new(),
                        }
                    }
                    Err(error) => CapabilityCheckView {
                        state: CapabilityState::IncompatibleInputValues,
                        checked_at: Some(self.clock.now().to_rfc3339()),
                        issues: vec![CapabilityIssueView {
                            code: "COMFY_PROTOCOL_ERROR".to_owned(),
                            class_type: None,
                            node_id: None,
                            affected_node_ids: Vec::new(),
                            input_name: None,
                            current_value: None,
                            message: error.to_string(),
                        }],
                    },
                };
                Ok((workflow_version_id.clone(), capability))
            })
            .collect()
    }

    pub fn validate(
        &self,
        draft_id: &str,
    ) -> Result<WorkflowOnboardingValidationView, WorkflowOnboardingError> {
        let draft = self.with_registry(|registry| registry.get(draft_id))??;
        Ok(validation_for_draft(&draft))
    }

    pub async fn publish(
        &self,
        draft_id: &str,
    ) -> Result<WorkflowOnboardingPublishView, WorkflowOnboardingError> {
        // Always recheck the live capability before publishing. A previous
        // READY result is only a snapshot and cannot authorize a stale draft.
        self.check_capability(draft_id).await?;
        let draft = self.with_registry(|registry| registry.get(draft_id))??;
        let validation = validation_for_draft(&draft);
        if !validation.ready_to_publish {
            return Err(WorkflowOnboardingError::new(
                "WORKFLOW_ONBOARDING_NOT_READY",
                validation.issues.join("; "),
            ));
        }

        let existing_packages = self.source.load_packages().await.map_err(|error| {
            WorkflowOnboardingError::new("WORKFLOW_LIBRARY_ERROR", error.to_string())
        })?;
        if !draft.allow_existing_workflow_sha
            && existing_packages.iter().any(|package| {
                if let WorkflowPackageLoad::Loaded(files) = package {
                    sha256(files.workflow_json.as_bytes()) == draft.workflow_sha256
                } else {
                    false
                }
            })
        {
            return Err(WorkflowOnboardingError::new(
                "IDENTICAL_WORKFLOW_ALREADY_EXISTS",
                "Identical workflow already exists",
            ));
        }

        let recipe = build_recipe(&draft)?;
        let recipe_yaml = recipe_to_yaml(&recipe)?;
        let manifest_yaml = draft
            .manifest
            .to_yaml()
            .map_err(|message| WorkflowOnboardingError::new("MANIFEST_INVALID", message))?;
        let package_name = if draft.allow_existing_workflow_sha {
            package_directory_name_with_recipe(
                &draft.manifest,
                &draft.workflow_sha256,
                &recipe_yaml,
            )
        } else {
            package_directory_name(&draft.manifest, &draft.workflow_sha256)
        };
        let staging_name = draft.draft_id.clone();
        let package = WorkflowPackageBytes::new(
            manifest_yaml.into_bytes(),
            recipe_yaml.into_bytes(),
            draft.raw_bytes.clone(),
        );
        self.package_store
            .stage(&staging_name, &package)
            .await
            .map_err(|error| {
                WorkflowOnboardingError::new("WORKFLOW_PACKAGE_PUBLISH_FAILED", error.to_string())
            })?;
        let staged = match self.package_store.read_staging(&staging_name).await {
            Ok(package) => package,
            Err(error) => {
                let _ = self.package_store.remove_staging(&staging_name).await;
                return Err(WorkflowOnboardingError::new(
                    "WORKFLOW_PACKAGE_PUBLISH_FAILED",
                    error.to_string(),
                ));
            }
        };
        if let Err(error) = read_back_and_validate_package(&staged) {
            let _ = self.package_store.remove_staging(&staging_name).await;
            return Err(error);
        }
        if let Err(error) = self
            .package_store
            .publish_atomic(&staging_name, &package_name)
            .await
        {
            let _ = self.package_store.remove_staging(&staging_name).await;
            return Err(WorkflowOnboardingError::new(
                "WORKFLOW_PACKAGE_PUBLISH_FAILED",
                error.to_string(),
            ));
        }

        let refreshed = match self.workflow_library_service.sync().await {
            Ok(report)
                if report
                    .errors
                    .iter()
                    .all(|error| error.package != package_name) =>
            {
                report
            }
            Ok(report) => {
                let _ = self.package_store.remove_published(&package_name).await;
                return Err(WorkflowOnboardingError::new(
                    "WORKFLOW_PACKAGE_PUBLISH_FAILED",
                    report
                        .errors
                        .iter()
                        .find(|error| error.package == package_name)
                        .map(|error| error.message.clone())
                        .unwrap_or_else(|| "published package failed library refresh".to_owned()),
                ));
            }
            Err(error) => {
                let _ = self.package_store.remove_published(&package_name).await;
                return Err(WorkflowOnboardingError::new(
                    "WORKFLOW_PACKAGE_PUBLISH_FAILED",
                    error.to_string(),
                ));
            }
        };

        Ok(WorkflowOnboardingPublishView {
            workflow_id: draft.manifest.id,
            workflow_version: draft.manifest.workflow_version,
            recipe_id: draft.recipe_id,
            package_name,
            workflow_sha256: draft.workflow_sha256,
            refreshed,
        })
    }

    pub fn discard(&self, draft_id: &str) -> Result<(), WorkflowOnboardingError> {
        self.with_registry(|registry| registry.remove(draft_id))?
    }

    pub async fn list_workspace(
        &self,
    ) -> Result<Vec<WorkflowWorkspaceView>, WorkflowOnboardingError> {
        let packages = self.source.load_packages().await.map_err(|error| {
            WorkflowOnboardingError::new("WORKFLOW_LIBRARY_ERROR", error.to_string())
        })?;
        let capability_source = self.comfy_adapter.get_object_info().await;
        let (capability_object, offline) = match capability_source {
            Ok(value) if value.is_object() => (Some(value), false),
            Ok(_) => (None, false),
            Err(ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_)) => (None, true),
            Err(_) => (None, false),
        };
        let mut views = Vec::new();
        for package in packages {
            let WorkflowPackageLoad::Loaded(files) = package else {
                continue;
            };
            let manifest = match WorkflowManifest::parse(&files.manifest_yaml) {
                Ok(manifest) => manifest,
                Err(_) => continue,
            };
            if self
                .is_archived_package(&manifest.id, &manifest.workflow_version)
                .await?
            {
                continue;
            }
            let workflow = match parse_api_workflow_string(&files.workflow_json) {
                Ok(workflow) => workflow,
                Err(_) => continue,
            };
            let mut nodes = inspect_workflow(&workflow)?;
            let capability = if offline {
                CapabilityCheckView {
                    state: CapabilityState::ComfyOffline,
                    checked_at: None,
                    issues: Vec::new(),
                }
            } else if let Some(object) = &capability_object {
                enrich_nodes_with_capability(&mut nodes, object);
                evaluate_capability(&workflow, &nodes, object)
            } else {
                CapabilityCheckView {
                    state: CapabilityState::IncompatibleInputValues,
                    checked_at: None,
                    issues: vec![CapabilityIssueView {
                        code: "COMFY_PROTOCOL_ERROR".to_owned(),
                        class_type: None,
                        node_id: None,
                        affected_node_ids: Vec::new(),
                        input_name: None,
                        current_value: None,
                        message: "ComfyUI object_info was not a JSON object".to_owned(),
                    }],
                }
            };
            let (input_mappings, outputs) = match RecipeParser::parse(&files.recipe_yaml) {
                Ok(recipe) => (
                    recipe
                        .bindings
                        .iter()
                        .map(|binding| WorkflowInputMappingView {
                            semantic_key: binding.source.clone(),
                            field_type: recipe
                                .inputs
                                .get(&binding.source)
                                .map(InputDefinition::kind)
                                .unwrap_or("unknown")
                                .to_owned(),
                            label: recipe
                                .inputs
                                .get(&binding.source)
                                .map(InputDefinition::label)
                                .unwrap_or(&binding.source)
                                .to_owned(),
                            required: recipe
                                .inputs
                                .get(&binding.source)
                                .map(input_required)
                                .unwrap_or(false),
                            default_value: None,
                            min_value: None,
                            max_value: None,
                            step: recipe.inputs.get(&binding.source).and_then(input_step),
                            min_items: None,
                            max_items: None,
                            target_node: binding.target.node.clone(),
                            target_input: binding.target.input.clone(),
                            item_index: binding.item_index,
                        })
                        .collect(),
                    recipe.outputs.iter().map(output_view).collect(),
                ),
                Err(_) => (Vec::new(), Vec::new()),
            };
            let has_successful_run = self
                .workflow_run_repository
                .has_successful_run(&manifest.id, &manifest.workflow_version)
                .await
                .map_err(|error| {
                    WorkflowOnboardingError::new("DATABASE_ERROR", error.to_string())
                })?;
            views.push(WorkflowWorkspaceView {
                workflow_id: manifest.id,
                name: manifest.name,
                workflow_version: manifest.workflow_version,
                mode: manifest.mode,
                package_name: files.package_name,
                package_status: "VALID".to_owned(),
                workflow_sha256: sha256(files.workflow_json.as_bytes()),
                node_count: nodes.len(),
                unique_class_count: nodes
                    .iter()
                    .map(|node| node.class_type.clone())
                    .collect::<HashSet<_>>()
                    .len(),
                capability: capability.state,
                capability_issues: capability.issues,
                input_mappings,
                outputs,
                has_successful_run,
            });
        }
        views.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.workflow_version.cmp(&right.workflow_version))
        });
        Ok(views)
    }

    async fn next_workflow_version(&self, workflow_id: &str) -> String {
        let Ok(packages) = self.source.load_packages().await else {
            return "1.0.0".to_owned();
        };
        let mut latest: Option<String> = None;
        for package in packages {
            let WorkflowPackageLoad::Loaded(files) = package else {
                continue;
            };
            if let Ok(manifest) = WorkflowManifest::parse(&files.manifest_yaml) {
                if manifest.id == workflow_id {
                    let candidate = manifest.workflow_version;
                    latest = Some(match latest {
                        None => candidate,
                        Some(current) => {
                            if compare_semver(&candidate, &current).is_gt() {
                                candidate
                            } else {
                                current
                            }
                        }
                    });
                }
            }
        }
        latest
            .map(|value| increment_semver(&value))
            .unwrap_or_else(|| "1.0.0".to_owned())
    }

    async fn check_capability_for_workflow(
        &self,
        draft: &WorkflowOnboardingDraft,
    ) -> (CapabilityCheckView, Option<Vec<WorkflowNodeView>>) {
        match self.comfy_adapter.get_object_info().await {
            Ok(object) if object.is_object() => {
                let mut nodes = draft.nodes.clone();
                enrich_nodes_with_capability(&mut nodes, &object);
                (
                    evaluate_capability(&draft.workflow, &nodes, &object),
                    Some(nodes),
                )
            }
            Ok(_) => (
                CapabilityCheckView {
                    state: CapabilityState::IncompatibleInputValues,
                    checked_at: Some(self.clock.now().to_rfc3339()),
                    issues: vec![CapabilityIssueView {
                        code: "COMFY_PROTOCOL_ERROR".to_owned(),
                        class_type: None,
                        node_id: None,
                        affected_node_ids: Vec::new(),
                        input_name: None,
                        current_value: None,
                        message: "ComfyUI object_info response is not an object".to_owned(),
                    }],
                },
                None,
            ),
            Err(ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_)) => (
                CapabilityCheckView {
                    state: CapabilityState::ComfyOffline,
                    checked_at: Some(self.clock.now().to_rfc3339()),
                    issues: Vec::new(),
                },
                None,
            ),
            Err(error) => (
                CapabilityCheckView {
                    state: CapabilityState::IncompatibleInputValues,
                    checked_at: Some(self.clock.now().to_rfc3339()),
                    issues: vec![CapabilityIssueView {
                        code: "COMFY_PROTOCOL_ERROR".to_owned(),
                        class_type: None,
                        node_id: None,
                        affected_node_ids: Vec::new(),
                        input_name: None,
                        current_value: None,
                        message: error.to_string(),
                    }],
                },
                None,
            ),
        }
    }

    fn with_registry<T>(
        &self,
        action: impl FnOnce(&mut WorkflowOnboardingRegistry) -> Result<T, WorkflowOnboardingError>,
    ) -> Result<Result<T, WorkflowOnboardingError>, WorkflowOnboardingError> {
        let mut registry = self.registry.lock().map_err(|_| {
            WorkflowOnboardingError::new(
                "WORKFLOW_ONBOARDING_REGISTRY_ERROR",
                "workflow onboarding registry lock was poisoned",
            )
        })?;
        Ok(action(&mut registry))
    }
}

#[derive(Default)]
struct AutoInferenceResult {
    name: String,
    category: String,
    mode: String,
    input_mappings: Vec<InputMapping>,
    output_mappings: Vec<OutputMapping>,
    inferences: Vec<WorkflowAutoInferenceView>,
    issues: Vec<WorkflowAutoIssueView>,
}

#[derive(Clone)]
struct AutoInputCandidate {
    mapping: InputMapping,
    inference: WorkflowAutoInferenceView,
    issue_candidate: WorkflowAutoIssueCandidateView,
    confidence: InferenceConfidence,
}

#[derive(Clone, Copy)]
struct AutoInputGuess {
    semantic_key: &'static str,
    field_type: SemanticFieldType,
    confidence: InferenceConfidence,
    source: &'static str,
    required: bool,
}

fn auto_plan_for_draft(
    draft: &WorkflowOnboardingDraft,
    state: WorkflowAutoOnboardingState,
    inferences: &[WorkflowAutoInferenceView],
    issues: &[WorkflowAutoIssueView],
    auto_publishable: bool,
    published: Option<WorkflowOnboardingPublishView>,
    existing_workflow_id: Option<String>,
    existing_workflow_version: Option<String>,
    existing_package_name: Option<String>,
    message: String,
) -> WorkflowAutoOnboardingPlanView {
    let view = view_for_draft(draft);
    let workflow_kind = workflow_kind_for_outputs(&draft.output_mappings);
    WorkflowAutoOnboardingPlanView {
        draft_id: draft.draft_id.clone(),
        state,
        workflow_kind,
        workflow_sha256: draft.workflow_sha256.clone(),
        original_filename: draft.original_filename.clone(),
        node_count: view.node_count,
        unique_class_count: view.unique_class_count,
        metadata: view.manifest,
        capability: view.capability,
        input_mappings: view.input_mappings,
        output_mappings: view.output_mappings,
        validation: view.validation,
        inferences: inferences.to_vec(),
        issues: issues.to_vec(),
        auto_publishable,
        published,
        existing_workflow_id,
        existing_workflow_version,
        existing_package_name,
        message,
    }
}

fn infer_auto_onboarding(draft: &WorkflowOnboardingDraft) -> AutoInferenceResult {
    let mut result = AutoInferenceResult {
        name: infer_workflow_name(&draft.workflow, &draft.original_filename),
        ..AutoInferenceResult::default()
    };
    let mut candidates_by_key: BTreeMap<String, Vec<AutoInputCandidate>> = BTreeMap::new();

    for node in &draft.nodes {
        for input in &node.inputs {
            if !input.bindable || input.is_linked {
                continue;
            }
            let Some(value) = draft
                .workflow
                .inputs(&node.node_id)
                .and_then(|inputs| inputs.get(&input.name))
            else {
                continue;
            };
            let lower_name = input.name.to_ascii_lowercase();
            if value.is_number() && is_float_input_name(&lower_name) {
                result.inferences.push(WorkflowAutoInferenceView {
                    field: lower_name.clone(),
                    value: Some(current_value_summary(value)),
                    confidence: InferenceConfidence::Ambiguous,
                    source: "INPUT_NAME_WITHOUT_FLOAT_RECIPE_TYPE".to_owned(),
                    alternatives: vec!["integer（不安全）".to_owned(), "手动字段类型".to_owned()],
                    node_id: Some(node.node_id.clone()),
                    input_name: Some(input.name.clone()),
                });
                result.issues.push(WorkflowAutoIssueView {
                    code: "FLOAT_INPUT_NEEDS_REVIEW".to_owned(),
                    message: format!(
                        "{} 是数字输入，但当前 Recipe 字段体系不支持浮点数，不能自动转换。",
                        input.name
                    ),
                    field: Some(lower_name),
                    candidates: Vec::new(),
                });
                continue;
            }
            let Some(guess) = auto_input_guess(&node.class_type, &input.name, value) else {
                continue;
            };
            if let Some(suggested_type) = input
                .suggested_type
                .as_deref()
                .and_then(|value| SemanticFieldType::parse(value).ok())
            {
                let explicit_frame = matches!(guess.semantic_key, "first_frame" | "last_frame");
                if suggested_type != guess.field_type && !explicit_frame {
                    continue;
                }
            }
            let Ok(mapping) = auto_input_mapping(&node.node_id, input, value, guess) else {
                continue;
            };
            let inference = WorkflowAutoInferenceView {
                field: guess.semantic_key.to_owned(),
                value: Some(guess.field_type.as_str().to_owned()),
                confidence: guess.confidence,
                source: guess.source.to_owned(),
                alternatives: Vec::new(),
                node_id: Some(node.node_id.clone()),
                input_name: Some(input.name.clone()),
            };
            let issue_candidate = WorkflowAutoIssueCandidateView {
                label: format!("节点 {} · {}", node.node_id, input.name),
                node_id: Some(node.node_id.clone()),
                input_name: Some(input.name.clone()),
                output_id: None,
                output_type: None,
                field_type: Some(guess.field_type.as_str().to_owned()),
            };
            candidates_by_key
                .entry(guess.semantic_key.to_owned())
                .or_default()
                .push(AutoInputCandidate {
                    mapping,
                    inference,
                    issue_candidate,
                    confidence: guess.confidence,
                });
        }
    }

    for (semantic_key, mut candidates) in candidates_by_key {
        if draft
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == semantic_key)
        {
            continue;
        }
        candidates.sort_by(|left, right| {
            left.mapping
                .target_node
                .cmp(&right.mapping.target_node)
                .then(left.mapping.target_input.cmp(&right.mapping.target_input))
        });
        let certain = candidates
            .iter()
            .filter(|candidate| candidate.confidence == InferenceConfidence::Certain)
            .cloned()
            .collect::<Vec<_>>();
        let selected = if certain.len() == 1 {
            Some(certain[0].clone())
        } else if candidates.len() == 1 {
            Some(candidates[0].clone())
        } else {
            None
        };
        if let Some(mut selected) = selected {
            if candidates.len() == 1 && selected.confidence == InferenceConfidence::Ambiguous {
                selected.confidence = InferenceConfidence::High;
                selected.inference.confidence = InferenceConfidence::High;
            }
            result.input_mappings.push(selected.mapping);
            result.inferences.push(selected.inference);
        } else {
            let alternatives = candidates
                .iter()
                .map(|candidate| candidate.issue_candidate.label.clone())
                .collect::<Vec<_>>();
            let issue_candidates = candidates
                .iter()
                .map(|candidate| candidate.issue_candidate.clone())
                .collect::<Vec<_>>();
            for candidate in candidates {
                let mut inference = candidate.inference;
                inference.confidence = InferenceConfidence::Ambiguous;
                inference.alternatives = alternatives.clone();
                result.inferences.push(inference);
            }
            result.issues.push(WorkflowAutoIssueView {
                code: "AMBIGUOUS_INPUT".to_owned(),
                message: format!("无法唯一判断 {semantic_key} 输入，请选择一个节点字段。"),
                field: Some(semantic_key),
                candidates: issue_candidates,
            });
        }
    }

    let (output_mappings, output_inferences, output_issues) = infer_auto_outputs(draft);
    result.output_mappings = output_mappings;
    result.inferences.extend(output_inferences);
    result.issues.extend(output_issues);
    let has_first_frame = result
        .input_mappings
        .iter()
        .any(|mapping| mapping.semantic_key == "first_frame")
        || draft
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == "first_frame");
    let has_last_frame = result
        .input_mappings
        .iter()
        .any(|mapping| mapping.semantic_key == "last_frame")
        || draft
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == "last_frame");
    let has_reference_media = result
        .input_mappings
        .iter()
        .any(|mapping| mapping.semantic_key.starts_with("reference_"))
        || draft
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key.starts_with("reference_"));
    let outputs = if result.output_mappings.is_empty() {
        &draft.output_mappings
    } else {
        &result.output_mappings
    };
    let (category, mode) = infer_manifest_media(
        outputs,
        &result.input_mappings,
        has_first_frame,
        has_last_frame,
        has_reference_media,
    );
    result.category = category;
    result.mode = mode;
    result
}

fn auto_input_guess(class_type: &str, name: &str, value: &Value) -> Option<AutoInputGuess> {
    let name = name.to_ascii_lowercase();
    let is_text = value.is_string();
    let is_number = value.is_number();
    let is_media = value.is_string() || value.is_array();
    if is_text
        && (name == "negative" || name == "negative_prompt" || name.contains("negative_prompt"))
    {
        return Some(AutoInputGuess {
            semantic_key: "negative_prompt",
            field_type: SemanticFieldType::Textarea,
            confidence: if name == "negative_prompt" {
                InferenceConfidence::Certain
            } else {
                InferenceConfidence::High
            },
            source: "INPUT_NAME_NEGATIVE_PROMPT",
            required: false,
        });
    }
    if is_text && name == "prompt" {
        return Some(AutoInputGuess {
            semantic_key: "prompt",
            field_type: SemanticFieldType::Textarea,
            confidence: InferenceConfidence::Certain,
            source: "INPUT_NAME_EXACT",
            required: true,
        });
    }
    if is_text && ["text", "positive", "positive_prompt"].contains(&name.as_str()) {
        return Some(AutoInputGuess {
            semantic_key: "prompt",
            field_type: SemanticFieldType::Textarea,
            confidence: InferenceConfidence::High,
            source: "INPUT_NAME_PROMPT_ALIAS",
            required: true,
        });
    }
    if is_text && (name.starts_with("prompt_") || name.ends_with("_prompt")) {
        return Some(AutoInputGuess {
            semantic_key: "prompt",
            field_type: SemanticFieldType::Textarea,
            confidence: InferenceConfidence::Ambiguous,
            source: "INPUT_NAME_PROMPT_HEURISTIC",
            required: true,
        });
    }
    if is_number && ["seed", "noise_seed", "random_seed"].contains(&name.as_str()) {
        return Some(AutoInputGuess {
            semantic_key: "seed",
            field_type: SemanticFieldType::Seed,
            confidence: if name == "seed" {
                InferenceConfidence::Certain
            } else {
                InferenceConfidence::High
            },
            source: "INPUT_NAME_SEED_AND_INTEGER_LITERAL",
            required: true,
        });
    }
    if is_number && name == "width" {
        return Some(integer_guess("width"));
    }
    if is_number && name == "height" {
        return Some(integer_guess("height"));
    }
    if is_number && ["steps", "num_steps", "sampling_steps"].contains(&name.as_str()) {
        return Some(AutoInputGuess {
            semantic_key: "steps",
            field_type: SemanticFieldType::Integer,
            confidence: if name == "steps" {
                InferenceConfidence::Certain
            } else {
                InferenceConfidence::High
            },
            source: "INPUT_NAME_INTEGER_PARAMETER",
            required: true,
        });
    }
    if is_number && ["duration", "duration_seconds", "seconds"].contains(&name.as_str()) {
        return Some(AutoInputGuess {
            semantic_key: "duration_seconds",
            field_type: SemanticFieldType::Integer,
            confidence: InferenceConfidence::High,
            source: "INPUT_NAME_DURATION_PARAMETER",
            required: true,
        });
    }
    if is_text && (name == "first_frame" || name == "start_frame") {
        return Some(AutoInputGuess {
            semantic_key: "first_frame",
            field_type: SemanticFieldType::Image,
            confidence: InferenceConfidence::Certain,
            source: "INPUT_NAME_FIRST_FRAME",
            required: true,
        });
    }
    if is_text && (name == "last_frame" || name == "end_frame") {
        return Some(AutoInputGuess {
            semantic_key: "last_frame",
            field_type: SemanticFieldType::Image,
            confidence: InferenceConfidence::Certain,
            source: "INPUT_NAME_LAST_FRAME",
            required: true,
        });
    }
    if is_media && name.contains("image") {
        let plural = name.contains("images") || name.contains("reference_images");
        return Some(AutoInputGuess {
            semantic_key: if plural {
                "reference_images"
            } else {
                "reference_image"
            },
            field_type: if plural {
                SemanticFieldType::Images
            } else {
                SemanticFieldType::Image
            },
            confidence: if name == "image" || name == "input_image" {
                InferenceConfidence::High
            } else {
                InferenceConfidence::Certain
            },
            source: if class_type.to_ascii_lowercase().contains("loadimage") {
                "INPUT_NAME_AND_NODE_CLASS"
            } else {
                "INPUT_NAME_MEDIA_SEMANTICS"
            },
            required: true,
        });
    }
    if is_media && name.contains("video") {
        let plural = name.contains("videos") || name.contains("reference_videos");
        return Some(AutoInputGuess {
            semantic_key: if plural {
                "reference_videos"
            } else {
                "reference_video"
            },
            field_type: if plural {
                SemanticFieldType::Videos
            } else {
                SemanticFieldType::Video
            },
            confidence: InferenceConfidence::High,
            source: "INPUT_NAME_MEDIA_SEMANTICS",
            required: true,
        });
    }
    if is_media && name.contains("audio") {
        let plural = name.contains("audios") || name.contains("reference_audios");
        return Some(AutoInputGuess {
            semantic_key: if plural {
                "reference_audios"
            } else {
                "reference_audio"
            },
            field_type: if plural {
                SemanticFieldType::Audios
            } else {
                SemanticFieldType::Audio
            },
            confidence: InferenceConfidence::High,
            source: "INPUT_NAME_MEDIA_SEMANTICS",
            required: true,
        });
    }
    None
}

fn integer_guess(key: &'static str) -> AutoInputGuess {
    AutoInputGuess {
        semantic_key: key,
        field_type: SemanticFieldType::Integer,
        confidence: InferenceConfidence::Certain,
        source: "INPUT_NAME_INTEGER_PARAMETER",
        required: true,
    }
}

fn auto_input_mapping(
    node_id: &str,
    input: &WorkflowInputView,
    value: &Value,
    guess: AutoInputGuess,
) -> Result<InputMapping, WorkflowOnboardingError> {
    validate_mapping_value(guess.field_type, value)?;
    let default_value = match guess.field_type {
        SemanticFieldType::Textarea | SemanticFieldType::Integer | SemanticFieldType::Seed => {
            Some(current_value_summary(value))
        }
        _ => None,
    };
    let step = if guess.field_type == SemanticFieldType::Integer {
        input
            .numeric_step
            .clone()
            .filter(|value| value.parse::<i64>().ok().is_some_and(|step| step > 0))
    } else {
        None
    };
    Ok(InputMapping {
        semantic_key: guess.semantic_key.to_owned(),
        field_type: guess.field_type,
        label: humanize_field_key(guess.semantic_key),
        required: guess.required,
        default_value,
        min_value: if matches!(
            guess.field_type,
            SemanticFieldType::Integer | SemanticFieldType::Seed
        ) {
            input.numeric_min.clone()
        } else {
            None
        },
        max_value: if matches!(
            guess.field_type,
            SemanticFieldType::Integer | SemanticFieldType::Seed
        ) {
            input.numeric_max.clone()
        } else {
            None
        },
        step,
        min_items: guess.field_type.is_plural().then_some(0),
        max_items: guess.field_type.is_plural().then_some(8),
        target_node: node_id.to_owned(),
        target_input: input.name.clone(),
        item_index: None,
    })
}

fn infer_auto_outputs(
    draft: &WorkflowOnboardingDraft,
) -> (
    Vec<OutputMapping>,
    Vec<WorkflowAutoInferenceView>,
    Vec<WorkflowAutoIssueView>,
) {
    if !draft.output_mappings.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    let mut candidates = draft
        .nodes
        .iter()
        .filter(|node| is_probable_output_node(node))
        .map(|node| {
            let output_type = output_type_for_node(draft, node);
            let label = if node.title.trim().is_empty() {
                format!("节点 {}", node.node_id)
            } else {
                node.title.clone()
            };
            (
                node.node_id.clone(),
                label,
                output_type,
                node.class_type.to_ascii_lowercase().contains("save"),
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    candidates.dedup_by(|left, right| left.0 == right.0);
    if candidates.is_empty() {
        return (
            Vec::new(),
            vec![WorkflowAutoInferenceView {
                field: "output_1".to_owned(),
                value: None,
                confidence: InferenceConfidence::Unknown,
                source: "OUTPUT_NODE_SEMANTICS".to_owned(),
                alternatives: Vec::new(),
                node_id: None,
                input_name: None,
            }],
            vec![WorkflowAutoIssueView {
                code: "UNKNOWN_OUTPUT".to_owned(),
                message: "未能识别唯一的最终输出节点。".to_owned(),
                field: Some("output_1".to_owned()),
                candidates: Vec::new(),
            }],
        );
    }
    let saved = candidates
        .iter()
        .filter(|candidate| candidate.3)
        .cloned()
        .collect::<Vec<_>>();
    let selected = if saved.len() == 1 {
        Some(saved[0].clone())
    } else if candidates.len() == 1 {
        Some(candidates[0].clone())
    } else {
        None
    };
    if let Some((node_id, label, output_type, _)) = selected {
        let output_name = match output_type {
            OutputType::Image => "图片",
            OutputType::Video => "视频",
        };
        return (
            vec![OutputMapping {
                output_id: "output_1".to_owned(),
                label: if label.trim().is_empty() {
                    format!("输出{output_name}")
                } else {
                    label.clone()
                },
                output_type,
                node_id: node_id.clone(),
                required: true,
            }],
            vec![WorkflowAutoInferenceView {
                field: "output_1".to_owned(),
                value: Some(match output_type {
                    OutputType::Image => "image".to_owned(),
                    OutputType::Video => "video".to_owned(),
                }),
                confidence: if candidates.len() == 1 {
                    InferenceConfidence::Certain
                } else {
                    InferenceConfidence::High
                },
                source: "OUTPUT_NODE_AND_SAVE_NODE_PRIORITY".to_owned(),
                alternatives: Vec::new(),
                node_id: Some(node_id),
                input_name: None,
            }],
            Vec::new(),
        );
    }
    let issue_candidates = candidates
        .iter()
        .map(
            |(node_id, label, output_type, _)| WorkflowAutoIssueCandidateView {
                label: format!("节点 {node_id} · {label}"),
                node_id: Some(node_id.clone()),
                input_name: None,
                output_id: Some("output_1".to_owned()),
                output_type: Some(match output_type {
                    OutputType::Image => "image".to_owned(),
                    OutputType::Video => "video".to_owned(),
                }),
                field_type: None,
            },
        )
        .collect::<Vec<_>>();
    (
        Vec::new(),
        candidates
            .iter()
            .map(|(node_id, _, output_type, _)| WorkflowAutoInferenceView {
                field: "output_1".to_owned(),
                value: Some(match output_type {
                    OutputType::Image => "image".to_owned(),
                    OutputType::Video => "video".to_owned(),
                }),
                confidence: InferenceConfidence::Ambiguous,
                source: "OUTPUT_NODE_SEMANTICS".to_owned(),
                alternatives: issue_candidates
                    .iter()
                    .map(|candidate| candidate.label.clone())
                    .collect(),
                node_id: Some(node_id.clone()),
                input_name: None,
            })
            .collect(),
        vec![WorkflowAutoIssueView {
            code: "AMBIGUOUS_OUTPUT".to_owned(),
            message: "检测到多个可能的最终输出节点，请选择要发布的输出。".to_owned(),
            field: Some("output_1".to_owned()),
            candidates: issue_candidates,
        }],
    )
}

fn auto_issues_from_capability(capability: &CapabilityCheckView) -> Vec<WorkflowAutoIssueView> {
    capability
        .issues
        .iter()
        .map(|issue| WorkflowAutoIssueView {
            code: if issue.code == "MISSING_NODE" {
                "MISSING_NODES".to_owned()
            } else {
                issue.code.clone()
            },
            message: issue.message.clone(),
            field: issue.input_name.clone(),
            candidates: Vec::new(),
        })
        .collect()
}

fn infer_manifest_media(
    outputs: &[OutputMapping],
    input_mappings: &[InputMapping],
    has_first_frame: bool,
    has_last_frame: bool,
    has_reference_media: bool,
) -> (String, String) {
    let has_video = outputs
        .iter()
        .any(|output| output.output_type == OutputType::Video);
    let has_image = outputs
        .iter()
        .any(|output| output.output_type == OutputType::Image);
    if has_video {
        let mode = if has_first_frame && has_last_frame {
            "image_to_video"
        } else if has_reference_media {
            "reference_to_video"
        } else if input_mappings.iter().any(|mapping| {
            matches!(
                mapping.field_type,
                SemanticFieldType::Image
                    | SemanticFieldType::Images
                    | SemanticFieldType::Video
                    | SemanticFieldType::Videos
                    | SemanticFieldType::Audio
                    | SemanticFieldType::Audios
            )
        }) {
            "image_to_video"
        } else {
            "text_to_video"
        };
        ("video".to_owned(), mode.to_owned())
    } else if has_image {
        let mode = if input_mappings.iter().any(|mapping| {
            matches!(
                mapping.field_type,
                SemanticFieldType::Image
                    | SemanticFieldType::Images
                    | SemanticFieldType::Video
                    | SemanticFieldType::Videos
                    | SemanticFieldType::Audio
                    | SemanticFieldType::Audios
            )
        }) {
            "image_to_image"
        } else {
            "text_to_image"
        };
        ("image".to_owned(), mode.to_owned())
    } else {
        ("image".to_owned(), "text_to_image".to_owned())
    }
}

fn workflow_kind_for_outputs(outputs: &[OutputMapping]) -> String {
    let has_image = outputs
        .iter()
        .any(|output| output.output_type == OutputType::Image);
    let has_video = outputs
        .iter()
        .any(|output| output.output_type == OutputType::Video);
    match (has_image, has_video) {
        (true, true) => "MIXED".to_owned(),
        (false, true) => "VIDEO".to_owned(),
        (true, false) => "IMAGE".to_owned(),
        (false, false) => "UNKNOWN".to_owned(),
    }
}

fn is_float_input_name(name: &str) -> bool {
    ["cfg", "cfg_scale", "guidance"]
        .iter()
        .any(|key| name == *key || name.contains(key))
}

fn is_probable_output_node(node: &WorkflowNodeView) -> bool {
    if node.is_output_node {
        return true;
    }
    let text = format!("{} {}", node.class_type, node.title).to_ascii_lowercase();
    [
        "saveimage",
        "savevideo",
        "previewimage",
        "previewvideo",
        "output",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn output_type_for_node(draft: &WorkflowOnboardingDraft, node: &WorkflowNodeView) -> OutputType {
    let text = format!("{} {}", node.class_type, node.title).to_ascii_lowercase();
    if text.contains("video")
        || text.contains("animated")
        || text.contains("webm")
        || node.inputs.iter().any(|input| {
            let name = input.name.to_ascii_lowercase();
            name.contains("video") || name.contains("frames")
        })
        || draft.workflow.inputs(&node.node_id).is_some_and(|inputs| {
            inputs
                .keys()
                .any(|name| name.to_ascii_lowercase().contains("video"))
        })
    {
        OutputType::Video
    } else {
        OutputType::Image
    }
}

fn infer_workflow_name(workflow: &WorkflowDocument, filename: &str) -> String {
    let mut nodes = workflow
        .value()
        .as_object()
        .into_iter()
        .flat_map(|object| object.iter())
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.0.cmp(right.0));
    for (_, node) in nodes {
        let Some(object) = node.as_object() else {
            continue;
        };
        let Some(title) = object
            .get("_meta")
            .and_then(Value::as_object)
            .and_then(|meta| meta.get("title"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let lower = title.to_ascii_lowercase();
        if !title.trim().is_empty()
            && !["save image", "save video", "preview image", "preview video"]
                .iter()
                .any(|generic| lower == *generic)
        {
            return title.trim().to_owned();
        }
    }
    let name = filename_stem(filename);
    if name.trim().is_empty() {
        "Imported Workflow".to_owned()
    } else {
        name
    }
}

fn humanize_field_key(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn view_for_draft(draft: &WorkflowOnboardingDraft) -> WorkflowOnboardingDraftView {
    let recipe_result = build_recipe(draft);
    let recipe = match &recipe_result {
        Ok(recipe) => RecipeDraftView {
            inputs: recipe_inputs_view(recipe),
            bindings: recipe
                .bindings
                .iter()
                .map(|binding| RecipeBindingView {
                    semantic_key: binding.source.clone(),
                    target_node: binding.target.node.clone(),
                    target_input: binding.target.input.clone(),
                    item_index: binding.item_index,
                })
                .collect(),
            outputs: draft
                .output_mappings
                .iter()
                .map(output_mapping_view)
                .collect(),
            yaml: recipe_to_yaml(recipe).ok(),
            valid: RecipeValidator::validate(recipe).is_ok(),
            issues: recipe_validation_issues(recipe),
        },
        Err(error) => RecipeDraftView {
            inputs: Vec::new(),
            bindings: Vec::new(),
            outputs: draft
                .output_mappings
                .iter()
                .map(output_mapping_view)
                .collect(),
            yaml: None,
            valid: false,
            issues: vec![error.to_string()],
        },
    };
    let validation = validation_for_draft(draft);
    WorkflowOnboardingDraftView {
        draft_id: draft.draft_id.clone(),
        workflow_sha256: draft.workflow_sha256.clone(),
        original_filename: draft.original_filename.clone(),
        node_count: draft.nodes.len(),
        unique_class_count: draft
            .nodes
            .iter()
            .map(|node| node.class_type.clone())
            .collect::<HashSet<_>>()
            .len(),
        nodes: draft.nodes.clone(),
        capability: draft.capability.clone(),
        input_mappings: draft
            .input_mappings
            .iter()
            .map(input_mapping_view)
            .collect(),
        output_mappings: draft
            .output_mappings
            .iter()
            .map(output_mapping_view)
            .collect(),
        manifest: WorkflowManifestView {
            workflow_id: draft.manifest.id.clone(),
            name: draft.manifest.name.clone(),
            workflow_version: draft.manifest.workflow_version.clone(),
            recipe_version: draft.manifest.recipe_version.clone(),
            category: draft.manifest.category.clone(),
            mode: draft.manifest.mode.clone(),
            recipe_id: draft.recipe_id.clone(),
        },
        recipe,
        validation,
    }
}

fn validation_for_draft(draft: &WorkflowOnboardingDraft) -> WorkflowOnboardingValidationView {
    let mut issues = Vec::new();
    let manifest = draft
        .manifest
        .validate()
        .map_err(|error| WorkflowOnboardingError::new("MANIFEST_INVALID", error))
        .and_then(|_| {
            validate_metadata_values(
                &draft.manifest.name,
                &draft.manifest.workflow_version,
                &draft.manifest.recipe_version,
                &draft.manifest.category,
                &draft.manifest.mode,
            )
        });
    if let Err(error) = &manifest {
        issues.push(error.to_string());
    }
    let recipe_result = build_recipe(draft);
    let recipe_valid = match &recipe_result {
        Ok(recipe) => match RecipeValidator::validate(recipe) {
            Ok(()) => true,
            Err(error) => {
                issues.push(error.to_string());
                false
            }
        },
        Err(error) => {
            issues.push(error.to_string());
            false
        }
    };
    let outputs_valid = !draft.output_mappings.is_empty()
        && draft
            .output_mappings
            .iter()
            .all(|output| draft.workflow.node(&output.node_id).is_some());
    if !outputs_valid {
        issues.push("OUTPUT_INVALID: at least one valid output is required".to_owned());
    }
    let bindings_valid = recipe_result
        .as_ref()
        .is_ok_and(|recipe| BindingValidator::validate(recipe, &draft.workflow).is_ok());
    if !bindings_valid {
        issues.push("BINDING_INVALID: one or more bindings are invalid".to_owned());
    }
    let mut mapping_bounds_valid = true;
    for mapping in &draft.input_mappings {
        let input = draft
            .nodes
            .iter()
            .find(|node| node.node_id == mapping.target_node)
            .and_then(|node| {
                node.inputs
                    .iter()
                    .find(|input| input.name == mapping.target_input)
            });
        if let Err(error) = validate_numeric_mapping_bounds(
            mapping.field_type,
            input,
            mapping.min_value.as_ref(),
            mapping.max_value.as_ref(),
        ) {
            mapping_bounds_valid = false;
            issues.push(error.to_string());
        }
    }
    let dry_run = recipe_result
        .as_ref()
        .is_ok_and(|recipe| dry_run_compile(recipe, &draft.workflow).is_ok());
    if !dry_run {
        issues.push("DRY_RUN_FAILED: recipe cannot compile without side effects".to_owned());
    }
    let capability = draft.capability.state == CapabilityState::Ready;
    if !capability {
        issues.push(format!(
            "CAPABILITY_NOT_READY: {:?}",
            draft.capability.state
        ));
    }
    let ready_to_publish = manifest.is_ok()
        && recipe_valid
        && bindings_valid
        && mapping_bounds_valid
        && outputs_valid
        && dry_run
        && capability;
    WorkflowOnboardingValidationView {
        api_format: true,
        recipe: recipe_valid,
        bindings: bindings_valid,
        outputs: outputs_valid,
        manifest: manifest.is_ok(),
        capability,
        dry_run,
        ready_to_publish,
        issues,
    }
}

fn build_recipe(draft: &WorkflowOnboardingDraft) -> Result<Recipe, WorkflowOnboardingError> {
    let mut inputs = BTreeMap::new();
    let mut bindings = Vec::new();
    for mapping in &draft.input_mappings {
        let definition = mapping.to_definition()?;
        if let Some(existing) = inputs.get(&mapping.semantic_key) {
            if existing != &definition {
                return Err(WorkflowOnboardingError::new(
                    "MAPPING_INVALID",
                    format!(
                        "semantic key {} has inconsistent definitions",
                        mapping.semantic_key
                    ),
                ));
            }
        } else {
            inputs.insert(mapping.semantic_key.clone(), definition);
        }
        bindings.push(Binding {
            source: mapping.semantic_key.clone(),
            item_index: mapping.item_index,
            target: BindingTarget {
                node: mapping.target_node.clone(),
                input: mapping.target_input.clone(),
            },
            clear_targets: Vec::new(),
        });
    }
    Ok(Recipe {
        schema_version: 1,
        id: draft.recipe_id.clone(),
        name: draft.manifest.name.clone(),
        workflow: crate::domain::WorkflowRef {
            file: "workflow_api.json".to_owned(),
        },
        inputs,
        bindings,
        outputs: draft
            .output_mappings
            .iter()
            .map(|output| OutputDefinition {
                id: output.output_id.clone(),
                output_type: output.output_type,
                node: output.node_id.clone(),
                required: output.required,
            })
            .collect(),
    })
}

impl InputMapping {
    fn to_definition(&self) -> Result<InputDefinition, WorkflowOnboardingError> {
        let default = self.default_value.clone();
        match self.field_type {
            SemanticFieldType::Textarea => Ok(InputDefinition::TextArea {
                label: self.label.clone(),
                required: self.required,
                default,
            }),
            SemanticFieldType::Integer => Ok(InputDefinition::Integer {
                label: self.label.clone(),
                required: self.required,
                default: parse_optional_i64(default, "default_value")?,
                min: parse_optional_i64(self.min_value.clone(), "min_value")?,
                max: parse_optional_i64(self.max_value.clone(), "max_value")?,
                step: parse_optional_i64(self.step.clone(), "step")?,
            }),
            SemanticFieldType::Seed => Ok(InputDefinition::Seed {
                label: self.label.clone(),
                default: match default.as_deref() {
                    None | Some("") | Some("random") => SeedDefault::Random,
                    Some(value) => SeedDefault::Fixed(value.parse().map_err(|_| {
                        WorkflowOnboardingError::new(
                            "MAPPING_INVALID",
                            format!("seed default {value} is not an unsigned integer"),
                        )
                    })?),
                },
                min: parse_optional_u64(self.min_value.clone(), "min_value")?,
                max: parse_optional_u64(self.max_value.clone(), "max_value")?,
            }),
            SemanticFieldType::Image => Ok(InputDefinition::Image {
                label: self.label.clone(),
                required: self.required,
            }),
            SemanticFieldType::Images => Ok(InputDefinition::Images {
                label: self.label.clone(),
                required: self.required,
                min_items: self.min_items.unwrap_or(0),
                max_items: self.max_items.unwrap_or(8),
            }),
            SemanticFieldType::Video => Ok(InputDefinition::Video {
                label: self.label.clone(),
                required: self.required,
            }),
            SemanticFieldType::Videos => Ok(InputDefinition::Videos {
                label: self.label.clone(),
                required: self.required,
                min_items: self.min_items.unwrap_or(0),
                max_items: self.max_items.unwrap_or(8),
            }),
            SemanticFieldType::Audio => Ok(InputDefinition::Audio {
                label: self.label.clone(),
                required: self.required,
            }),
            SemanticFieldType::Audios => Ok(InputDefinition::Audios {
                label: self.label.clone(),
                required: self.required,
                min_items: self.min_items.unwrap_or(0),
                max_items: self.max_items.unwrap_or(8),
            }),
        }
    }
}

pub struct RecipeYamlWriter;

impl RecipeYamlWriter {
    pub fn write(recipe: &Recipe) -> Result<String, WorkflowOnboardingError> {
        let mut input_values = Map::new();
        for (key, definition) in &recipe.inputs {
            input_values.insert(key.clone(), input_definition_json(definition));
        }
        let bindings = recipe
            .bindings
            .iter()
            .map(|binding| {
                let mut target = Map::new();
                target.insert(
                    "node".to_owned(),
                    Value::String(binding.target.node.clone()),
                );
                target.insert(
                    "input".to_owned(),
                    Value::String(binding.target.input.clone()),
                );
                let mut item = Map::new();
                item.insert("source".to_owned(), Value::String(binding.source.clone()));
                if let Some(item_index) = binding.item_index {
                    item.insert("item".to_owned(), json!(item_index));
                }
                item.insert("target".to_owned(), Value::Object(target));
                if !binding.clear_targets.is_empty() {
                    item.insert(
                        "clear_targets".to_owned(),
                        Value::Array(
                            binding
                                .clear_targets
                                .iter()
                                .map(|target| {
                                    json!({
                                        "node": target.node,
                                        "input": target.input,
                                    })
                                })
                                .collect(),
                        ),
                    );
                }
                Value::Object(item)
            })
            .collect::<Vec<_>>();
        let outputs = recipe
        .outputs
        .iter()
        .map(|output| {
            json!({
                "id": output.id,
                "type": match output.output_type { OutputType::Image => "image", OutputType::Video => "video" },
                "node": output.node,
                "required": output.required,
            })
        })
        .collect::<Vec<_>>();
        let value = json!({
            "schema_version": recipe.schema_version,
            "id": recipe.id,
            "name": recipe.name,
            "workflow": {"file": recipe.workflow.file},
            "inputs": Value::Object(input_values),
            "bindings": bindings,
            "outputs": outputs,
        });
        yaml_serde::to_string(&value).map_err(|error| {
            WorkflowOnboardingError::new("RECIPE_SERIALIZE_ERROR", error.to_string())
        })
    }
}

fn recipe_to_yaml(recipe: &Recipe) -> Result<String, WorkflowOnboardingError> {
    RecipeYamlWriter::write(recipe)
}

fn input_definition_json(definition: &InputDefinition) -> Value {
    match definition {
        InputDefinition::TextArea {
            label,
            required,
            default,
        } => json!({"type":"textarea","label":label,"required":required,"default":default}),
        InputDefinition::Integer {
            label,
            required,
            default,
            min,
            max,
            step,
        } => {
            json!({"type":"integer","label":label,"required":required,"default":default,"min":min,"max":max,"step":step})
        }
        InputDefinition::Seed {
            label,
            default,
            min,
            max,
        } => {
            json!({"type":"seed","label":label,"default":match default { SeedDefault::Random => Value::String("random".to_owned()), SeedDefault::Fixed(value) => json!(value) },"min":min,"max":max})
        }
        InputDefinition::Image { label, required } => {
            json!({"type":"image","label":label,"required":required})
        }
        InputDefinition::Images {
            label,
            required,
            min_items,
            max_items,
        } => {
            json!({"type":"images","label":label,"required":required,"min_items":min_items,"max_items":max_items})
        }
        InputDefinition::Video { label, required } => {
            json!({"type":"video","label":label,"required":required})
        }
        InputDefinition::Audio { label, required } => {
            json!({"type":"audio","label":label,"required":required})
        }
        InputDefinition::Videos {
            label,
            required,
            min_items,
            max_items,
        } => {
            json!({"type":"videos","label":label,"required":required,"min_items":min_items,"max_items":max_items})
        }
        InputDefinition::Audios {
            label,
            required,
            min_items,
            max_items,
        } => {
            json!({"type":"audios","label":label,"required":required,"min_items":min_items,"max_items":max_items})
        }
    }
}

fn dry_run_compile(
    recipe: &Recipe,
    workflow: &WorkflowDocument,
) -> Result<(), WorkflowOnboardingError> {
    let mut values = BTreeMap::new();
    for (key, definition) in &recipe.inputs {
        let value = match definition {
            InputDefinition::TextArea { default, .. } => InputValue::String(
                default
                    .clone()
                    .unwrap_or_else(|| "onboarding placeholder".to_owned()),
            ),
            InputDefinition::Integer { default, min, .. } => {
                InputValue::Integer(default.unwrap_or(min.unwrap_or(0)))
            }
            InputDefinition::Seed { default, min, .. } => {
                let seed = match default {
                    SeedDefault::Fixed(value) => *value,
                    SeedDefault::Random => min.unwrap_or(0),
                };
                InputValue::Seed(SeedValue::Fixed(seed))
            }
            InputDefinition::Image { .. } => {
                InputValue::Image("__onboarding_image__.png".to_owned())
            }
            InputDefinition::Images { min_items, .. } => InputValue::Images(vec![
                    "__onboarding_image__.png".to_owned();
                    (*min_items).max(1)
                ]),
            InputDefinition::Video { .. } => {
                InputValue::Video("__onboarding_video__.mp4".to_owned())
            }
            InputDefinition::Videos { min_items, .. } => InputValue::Videos(vec![
                    "__onboarding_video__.mp4".to_owned();
                    (*min_items).max(1)
                ]),
            InputDefinition::Audio { .. } => {
                InputValue::Audio("__onboarding_audio__.wav".to_owned())
            }
            InputDefinition::Audios { min_items, .. } => InputValue::Audios(vec![
                    "__onboarding_audio__.wav".to_owned();
                    (*min_items).max(1)
                ]),
        };
        values.insert(key.clone(), value);
    }
    WorkflowCompiler
        .compile(workflow, recipe, &CompileRequest::new(values))
        .map(|_| ())
        .map_err(|error| WorkflowOnboardingError::new("DRY_RUN_FAILED", error.to_string()))
}

fn recipe_inputs_view(recipe: &Recipe) -> Vec<RecipeInputView> {
    recipe
        .inputs
        .iter()
        .map(|(key, definition)| RecipeInputView {
            key: key.clone(),
            field_type: definition.kind().to_owned(),
            label: definition.label().to_owned(),
            required: input_required(definition),
            default_value: input_default(definition),
            min_value: input_min(definition),
            max_value: input_max(definition),
            step: input_step(definition),
            min_items: input_min_items(definition),
            max_items: input_max_items(definition),
        })
        .collect()
}

fn recipe_validation_issues(recipe: &Recipe) -> Vec<String> {
    RecipeValidator::validate(recipe)
        .err()
        .map(|error| vec![error.to_string()])
        .unwrap_or_default()
}

fn input_mapping_view(mapping: &InputMapping) -> WorkflowInputMappingView {
    WorkflowInputMappingView {
        semantic_key: mapping.semantic_key.clone(),
        field_type: mapping.field_type.as_str().to_owned(),
        label: mapping.label.clone(),
        required: mapping.required,
        default_value: mapping.default_value.clone(),
        min_value: mapping.min_value.clone(),
        max_value: mapping.max_value.clone(),
        step: mapping.step.clone(),
        min_items: mapping.min_items,
        max_items: mapping.max_items,
        target_node: mapping.target_node.clone(),
        target_input: mapping.target_input.clone(),
        item_index: mapping.item_index,
    }
}

fn output_mapping_view(output: &OutputMapping) -> WorkflowOutputMappingView {
    WorkflowOutputMappingView {
        output_id: output.output_id.clone(),
        label: output.label.clone(),
        output_type: match output.output_type {
            OutputType::Image => "image",
            OutputType::Video => "video",
        }
        .to_owned(),
        node_id: output.node_id.clone(),
        required: output.required,
    }
}

fn output_view(output: &OutputDefinition) -> WorkflowOutputMappingView {
    WorkflowOutputMappingView {
        output_id: output.id.clone(),
        label: output.id.clone(),
        output_type: match output.output_type {
            OutputType::Image => "image",
            OutputType::Video => "video",
        }
        .to_owned(),
        node_id: output.node.clone(),
        required: output.required,
    }
}

fn input_mappings_from_recipe(
    recipe: &Recipe,
) -> Result<Vec<InputMapping>, WorkflowOnboardingError> {
    recipe
        .bindings
        .iter()
        .map(|binding| {
            let definition = recipe.inputs.get(&binding.source).ok_or_else(|| {
                WorkflowOnboardingError::new(
                    "BINDING_INVALID",
                    format!("recipe binding references unknown input {}", binding.source),
                )
            })?;
            Ok(InputMapping {
                semantic_key: binding.source.clone(),
                field_type: SemanticFieldType::parse(definition.kind())?,
                label: definition.label().to_owned(),
                required: input_required(definition),
                default_value: input_default(definition),
                min_value: input_min(definition),
                max_value: input_max(definition),
                step: input_step(definition),
                min_items: input_min_items(definition),
                max_items: input_max_items(definition),
                target_node: binding.target.node.clone(),
                target_input: binding.target.input.clone(),
                item_index: binding.item_index,
            })
        })
        .collect()
}

fn output_mapping_from_recipe(output: &OutputDefinition) -> OutputMapping {
    OutputMapping {
        output_id: output.id.clone(),
        label: output.id.clone(),
        output_type: output.output_type,
        node_id: output.node.clone(),
        required: output.required,
    }
}

fn input_required(definition: &InputDefinition) -> bool {
    match definition {
        InputDefinition::TextArea { required, .. }
        | InputDefinition::Integer { required, .. }
        | InputDefinition::Image { required, .. }
        | InputDefinition::Images { required, .. }
        | InputDefinition::Video { required, .. }
        | InputDefinition::Audio { required, .. }
        | InputDefinition::Videos { required, .. }
        | InputDefinition::Audios { required, .. } => *required,
        InputDefinition::Seed { .. } => true,
    }
}

fn input_default(definition: &InputDefinition) -> Option<String> {
    match definition {
        InputDefinition::TextArea { default, .. } => default.clone(),
        InputDefinition::Integer { default, .. } => default.map(|value| value.to_string()),
        InputDefinition::Seed { default, .. } => Some(match default {
            SeedDefault::Random => "random".to_owned(),
            SeedDefault::Fixed(value) => value.to_string(),
        }),
        _ => None,
    }
}

fn input_min(definition: &InputDefinition) -> Option<String> {
    match definition {
        InputDefinition::Integer { min, .. } => min.map(|value| value.to_string()),
        InputDefinition::Seed { min, .. } => min.map(|value| value.to_string()),
        _ => None,
    }
}

fn input_max(definition: &InputDefinition) -> Option<String> {
    match definition {
        InputDefinition::Integer { max, .. } => max.map(|value| value.to_string()),
        InputDefinition::Seed { max, .. } => max.map(|value| value.to_string()),
        _ => None,
    }
}

fn input_step(definition: &InputDefinition) -> Option<String> {
    match definition {
        InputDefinition::Integer { step, .. } => step.map(|value| value.to_string()),
        _ => None,
    }
}

fn input_min_items(definition: &InputDefinition) -> Option<usize> {
    match definition {
        InputDefinition::Images { min_items, .. }
        | InputDefinition::Videos { min_items, .. }
        | InputDefinition::Audios { min_items, .. } => Some(*min_items),
        _ => None,
    }
}

fn input_max_items(definition: &InputDefinition) -> Option<usize> {
    match definition {
        InputDefinition::Images { max_items, .. }
        | InputDefinition::Videos { max_items, .. }
        | InputDefinition::Audios { max_items, .. } => Some(*max_items),
        _ => None,
    }
}

fn validate_api_workflow(value: Value) -> Result<WorkflowDocument, WorkflowOnboardingError> {
    let object = value.as_object().cloned().ok_or_else(|| {
        WorkflowOnboardingError::new(
            "WORKFLOW_NOT_API_FORMAT",
            "Please export the workflow in ComfyUI API Format.",
        )
    })?;
    if object.is_empty() {
        return Err(WorkflowOnboardingError::new(
            "WORKFLOW_EMPTY",
            "API workflow must contain at least one node",
        ));
    }
    let visual_markers = ["nodes", "links", "groups", "last_node_id", "version"];
    if object.keys().any(|key| !is_numeric_node_id(key)) {
        let visual = visual_markers
            .iter()
            .any(|marker| object.contains_key(*marker));
        return Err(WorkflowOnboardingError::new(
            if visual {
                "WORKFLOW_NOT_API_FORMAT"
            } else {
                "WORKFLOW_NODE_ID_INVALID"
            },
            "Please export the workflow in ComfyUI API Format.",
        ));
    }
    let workflow = WorkflowDocument::parse(value).map_err(|error| {
        WorkflowOnboardingError::new("WORKFLOW_NOT_API_FORMAT", error.to_string())
    })?;
    WorkflowValidator::validate(&workflow).map_err(|error| {
        let code = if error.to_string().contains("class_type") {
            "WORKFLOW_CLASS_TYPE_MISSING"
        } else if error.to_string().contains("inputs") {
            "WORKFLOW_INPUTS_MISSING"
        } else {
            "WORKFLOW_NOT_API_FORMAT"
        };
        WorkflowOnboardingError::new(code, error.to_string())
    })?;
    let node_ids = object.keys().cloned().collect::<HashSet<_>>();
    for (node_id, node) in &object {
        let inputs = node
            .get("inputs")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                WorkflowOnboardingError::new(
                    "WORKFLOW_INPUTS_MISSING",
                    format!("node {node_id} is missing inputs"),
                )
            })?;
        for (input_name, value) in inputs {
            if let Some((source, _)) = possible_link(value) {
                if !node_ids.contains(source) {
                    return Err(WorkflowOnboardingError::new(
                        "WORKFLOW_BROKEN_LINK",
                        format!("source node {source}, target node {node_id}, target input {input_name}"),
                    ));
                }
            }
        }
    }
    Ok(workflow)
}

fn parse_api_workflow_string(value: &str) -> Result<WorkflowDocument, WorkflowOnboardingError> {
    let json = serde_json::from_str(value).map_err(|error| {
        WorkflowOnboardingError::new("WORKFLOW_NOT_API_FORMAT", error.to_string())
    })?;
    validate_api_workflow(json)
}

fn inspect_workflow(
    workflow: &WorkflowDocument,
) -> Result<Vec<WorkflowNodeView>, WorkflowOnboardingError> {
    let mut nodes = Vec::new();
    let object = workflow.value().as_object().ok_or_else(|| {
        WorkflowOnboardingError::new("WORKFLOW_NOT_API_FORMAT", "workflow root is not an object")
    })?;
    for (node_id, node) in object {
        let node_object = node.as_object().ok_or_else(|| {
            WorkflowOnboardingError::new("WORKFLOW_NOT_API_FORMAT", "node is not an object")
        })?;
        let class_type = node_object
            .get("class_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let title = node_object
            .get("_meta")
            .and_then(Value::as_object)
            .and_then(|meta| meta.get("title"))
            .and_then(Value::as_str)
            .filter(|title| !title.trim().is_empty())
            .unwrap_or(&class_type)
            .to_owned();
        let is_output_node = node_object
            .get("output_node")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut inputs: Vec<WorkflowInputView> = node_object
            .get("inputs")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                WorkflowOnboardingError::new("WORKFLOW_INPUTS_MISSING", "inputs is not an object")
            })?
            .iter()
            .map(|(name, value)| {
                let linked = possible_link(value).is_some();
                WorkflowInputView {
                    name: name.clone(),
                    kind: value_kind(value, linked).to_owned(),
                    current_value_summary: current_value_summary(value),
                    is_linked: linked,
                    bindable: !linked,
                    suggested_type: suggestion_for_input(name, value, linked),
                    suggested_semantic_key: suggestion_for_semantic_key(name, value, linked),
                    numeric_min: None,
                    numeric_max: None,
                    numeric_step: None,
                    allowed_options: Vec::new(),
                }
            })
            .collect();
        inputs.sort_by(|left: &WorkflowInputView, right: &WorkflowInputView| {
            left.name.cmp(&right.name)
        });
        nodes.push(WorkflowNodeView {
            node_id: node_id.clone(),
            class_type,
            title,
            is_output_node,
            inputs,
        });
    }
    nodes.sort_by(|left, right| {
        left.node_id
            .parse::<u64>()
            .unwrap_or(u64::MAX)
            .cmp(&right.node_id.parse::<u64>().unwrap_or(u64::MAX))
            .then(left.node_id.cmp(&right.node_id))
    });
    Ok(nodes)
}

fn evaluate_capability(
    workflow: &WorkflowDocument,
    nodes: &[WorkflowNodeView],
    object_info: &Value,
) -> CapabilityCheckView {
    let Some(object) = object_info.as_object() else {
        return CapabilityCheckView {
            state: CapabilityState::IncompatibleInputValues,
            checked_at: Some(chrono::Utc::now().to_rfc3339()),
            issues: vec![CapabilityIssueView {
                code: "COMFY_PROTOCOL_ERROR".to_owned(),
                class_type: None,
                node_id: None,
                affected_node_ids: Vec::new(),
                input_name: None,
                current_value: None,
                message: "ComfyUI object_info was not a JSON object".to_owned(),
            }],
        };
    };
    let mut issues = Vec::new();
    let mut missing_by_class: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in nodes {
        if !object.contains_key(&node.class_type) {
            missing_by_class
                .entry(node.class_type.clone())
                .or_default()
                .push(node.node_id.clone());
            continue;
        }
        let capability = object.get(&node.class_type).and_then(Value::as_object);
        let input_meta = capability
            .and_then(|node| node.get("input"))
            .and_then(Value::as_object);
        let Some(workflow_inputs) = workflow.inputs(&node.node_id) else {
            continue;
        };
        for (input_name, value) in workflow_inputs {
            if possible_link(value).is_some() {
                continue;
            }
            let Some(spec) = find_input_spec(input_meta, input_name) else {
                continue;
            };
            let Some(spec_array) = spec.as_array() else {
                continue;
            };
            let options = spec_array.first().and_then(Value::as_array);
            if let Some(options) = options {
                if let Some(current) = value.as_str() {
                    let available = options
                        .iter()
                        .any(|option| option.as_str() == Some(current));
                    if !available {
                        issues.push(CapabilityIssueView {
                            code: "INPUT_OPTION_UNAVAILABLE".to_owned(),
                            class_type: Some(node.class_type.clone()),
                            node_id: Some(node.node_id.clone()),
                            affected_node_ids: Vec::new(),
                            input_name: Some(input_name.clone()),
                            current_value: Some(current.to_owned()),
                            message: "Current ComfyUI does not offer this workflow value."
                                .to_owned(),
                        });
                    }
                }
            }
            if let Some(constraints) = spec_array.get(1).and_then(Value::as_object) {
                let current = value.as_f64();
                if let Some(current) = current {
                    let min = constraints.get("min").and_then(Value::as_f64);
                    let max = constraints.get("max").and_then(Value::as_f64);
                    if min.is_some_and(|min| current < min) || max.is_some_and(|max| current > max)
                    {
                        issues.push(CapabilityIssueView {
                            code: "INPUT_VALUE_OUT_OF_RANGE".to_owned(),
                            class_type: Some(node.class_type.clone()),
                            node_id: Some(node.node_id.clone()),
                            affected_node_ids: Vec::new(),
                            input_name: Some(input_name.clone()),
                            current_value: Some(current_value_summary(value)),
                            message: format!(
                                "Workflow value is outside the current ComfyUI range."
                            ),
                        });
                    }
                }
            }
        }
    }
    for (class_type, node_ids) in missing_by_class {
        issues.push(CapabilityIssueView {
            code: "MISSING_NODE".to_owned(),
            class_type: Some(class_type.clone()),
            node_id: None,
            affected_node_ids: node_ids.clone(),
            input_name: None,
            current_value: None,
            message: format!("Missing ComfyUI node class {class_type}"),
        });
    }
    let state = if issues.iter().any(|issue| issue.code == "MISSING_NODE") {
        CapabilityState::MissingNodes
    } else if !issues.is_empty() {
        CapabilityState::IncompatibleInputValues
    } else {
        CapabilityState::Ready
    };
    CapabilityCheckView {
        state,
        checked_at: Some(chrono::Utc::now().to_rfc3339()),
        issues,
    }
}

fn enrich_nodes_with_capability(nodes: &mut [WorkflowNodeView], object_info: &Value) {
    let Some(object) = object_info.as_object() else {
        return;
    };
    for node in nodes {
        let Some(capability) = object.get(&node.class_type).and_then(Value::as_object) else {
            continue;
        };
        if capability
            .get("output_node")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            node.is_output_node = true;
        }
        let input_meta = capability.get("input").and_then(Value::as_object);
        for input in &mut node.inputs {
            let Some(spec) = find_input_spec(input_meta, &input.name) else {
                continue;
            };
            let Some(spec_array) = spec.as_array() else {
                continue;
            };
            if let Some(options) = spec_array.first().and_then(Value::as_array) {
                input.allowed_options = options
                    .iter()
                    .filter_map(Value::as_str)
                    .map(sanitize_display_value)
                    .take(128)
                    .collect();
            }
            if let Some(constraints) = spec_array.get(1).and_then(Value::as_object) {
                input.numeric_min = constraints
                    .get("min")
                    .and_then(number_or_string)
                    .map(|value| value);
                input.numeric_max = constraints
                    .get("max")
                    .and_then(number_or_string)
                    .map(|value| value);
                input.numeric_step = constraints
                    .get("step")
                    .and_then(number_or_string)
                    .map(|value| value);
            }
        }
    }
}

fn number_or_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn validate_mapping_bounds(
    field_type: SemanticFieldType,
    input: Option<&WorkflowInputView>,
    request: &WorkflowOnboardingInputMappingRequest,
) -> Result<(), WorkflowOnboardingError> {
    let Some(input) = input else {
        return Ok(());
    };
    if !matches!(
        field_type,
        SemanticFieldType::Integer | SemanticFieldType::Seed
    ) {
        return Ok(());
    }
    validate_numeric_mapping_bounds(
        field_type,
        Some(input),
        request.min_value.as_ref(),
        request.max_value.as_ref(),
    )
}

fn validate_numeric_mapping_bounds(
    field_type: SemanticFieldType,
    input: Option<&WorkflowInputView>,
    min_value: Option<&String>,
    max_value: Option<&String>,
) -> Result<(), WorkflowOnboardingError> {
    let Some(input) = input else {
        return Ok(());
    };
    if !matches!(
        field_type,
        SemanticFieldType::Integer | SemanticFieldType::Seed
    ) {
        return Ok(());
    }
    let parse = |value: Option<&String>, label: &str| {
        value
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                value.parse::<i128>().map_err(|_| {
                    WorkflowOnboardingError::new(
                        "MAPPING_INVALID",
                        format!("{label} must be an integer"),
                    )
                })
            })
            .transpose()
    };
    let requested_min = parse(min_value, "min_value")?;
    let requested_max = parse(max_value, "max_value")?;
    if requested_min.is_some_and(|min| requested_max.is_some_and(|max| min > max)) {
        return Err(WorkflowOnboardingError::new(
            "MAPPING_INVALID",
            "min_value must be less than or equal to max_value",
        ));
    }
    let actual_min = input
        .numeric_min
        .as_ref()
        .and_then(|value| value.parse::<i128>().ok());
    let actual_max = input
        .numeric_max
        .as_ref()
        .and_then(|value| value.parse::<i128>().ok());
    if requested_min.is_some_and(|min| actual_min.is_some_and(|actual| min < actual))
        || requested_max.is_some_and(|max| actual_max.is_some_and(|actual| max > actual))
    {
        return Err(WorkflowOnboardingError::new(
            "MAPPING_OUT_OF_RANGE",
            "mapping bounds cannot exceed the current ComfyUI input range",
        ));
    }
    Ok(())
}

fn find_input_spec<'a>(
    input_meta: Option<&'a Map<String, Value>>,
    name: &str,
) -> Option<&'a Value> {
    input_meta.and_then(|meta| {
        ["required", "optional", "hidden"]
            .iter()
            .find_map(|section| {
                meta.get(*section)
                    .and_then(Value::as_object)
                    .and_then(|values| values.get(name))
            })
    })
}

fn validate_mapping_value(
    field_type: SemanticFieldType,
    value: &Value,
) -> Result<(), WorkflowOnboardingError> {
    let compatible = match field_type {
        SemanticFieldType::Textarea => value.is_string(),
        SemanticFieldType::Integer | SemanticFieldType::Seed => {
            value.as_i64().is_some() || value.as_u64().is_some()
        }
        SemanticFieldType::Image | SemanticFieldType::Video | SemanticFieldType::Audio => {
            value.is_string()
        }
        SemanticFieldType::Images | SemanticFieldType::Videos | SemanticFieldType::Audios => {
            value.is_string() || value.is_array()
        }
    };
    if compatible {
        Ok(())
    } else {
        Err(WorkflowOnboardingError::new(
            "MAPPING_UNSUPPORTED",
            format!(
                "workflow literal cannot be mapped to {}",
                field_type.as_str()
            ),
        ))
    }
}

fn possible_link(value: &Value) -> Option<(&str, u64)> {
    let array = value.as_array()?;
    if array.len() != 2 {
        return None;
    }
    let source = array.first()?.as_str()?;
    if !is_numeric_node_id(source) {
        return None;
    }
    Some((source, array.get(1)?.as_u64()?))
}

fn is_workflow_link(
    value: &Value,
    workflow: &WorkflowDocument,
) -> Result<Option<(String, u64)>, WorkflowOnboardingError> {
    let Some((source, index)) = possible_link(value) else {
        return Ok(None);
    };
    if workflow.node(source).is_none() {
        return Err(WorkflowOnboardingError::new(
            "WORKFLOW_BROKEN_LINK",
            format!("source node {source} does not exist"),
        ));
    }
    Ok(Some((source.to_owned(), index)))
}

fn value_kind(value: &Value, linked: bool) -> &'static str {
    if linked {
        "link"
    } else if value.is_string() {
        "string"
    } else if value.is_number() {
        "number"
    } else if value.is_boolean() {
        "boolean"
    } else if value.is_array() {
        "list"
    } else if value.is_object() {
        "object"
    } else {
        "null"
    }
}

fn current_value_summary(value: &Value) -> String {
    if let Some((node_id, _)) = possible_link(value) {
        return format!("linked to node {node_id}");
    }
    match value {
        Value::String(value) => sanitize_display_value(value),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Array(values) => format!("list ({})", values.len()),
        Value::Object(_) => "object".to_owned(),
        Value::Null => "null".to_owned(),
    }
}

fn suggestion_for_input(name: &str, value: &Value, linked: bool) -> Option<String> {
    if linked {
        return None;
    }
    let name = name.to_ascii_lowercase();
    if value.is_string()
        && (["prompt", "text", "positive", "negative"]
            .iter()
            .any(|key| name == *key || name.contains(key)))
    {
        return Some("textarea".to_owned());
    }
    if value.is_number()
        && ["seed", "noise_seed", "random_seed"]
            .iter()
            .any(|key| name == *key || name.contains(key))
    {
        return (value.as_i64().is_some() || value.as_u64().is_some()).then_some("seed".to_owned());
    }
    if value.is_number()
        && ["cfg", "cfg_scale", "guidance"]
            .iter()
            .any(|key| name == *key || name.contains(key))
    {
        return value.as_i64().map(|_| "integer".to_owned());
    }
    if value.is_number() {
        return value.as_i64().map(|_| "integer".to_owned());
    }
    if (value.is_string() || value.is_array()) && name.contains("image") {
        if value.is_array() || name.contains("images") || name.contains("references") {
            return Some("images".to_owned());
        }
        return Some("image".to_owned());
    }
    if (value.is_string() || value.is_array()) && name.contains("video") {
        if value.is_array() || name.contains("videos") || name.contains("references") {
            return Some("videos".to_owned());
        }
        return Some("video".to_owned());
    }
    if (value.is_string() || value.is_array()) && name.contains("audio") {
        if value.is_array() || name.contains("audios") || name.contains("references") {
            return Some("audios".to_owned());
        }
        return Some("audio".to_owned());
    }
    if value.is_string()
        && ["first_frame", "start_frame", "last_frame", "end_frame"]
            .iter()
            .any(|key| name == *key)
    {
        return Some("image".to_owned());
    }
    None
}

fn suggestion_for_semantic_key(name: &str, value: &Value, linked: bool) -> Option<String> {
    if linked || value.is_object() || value.is_boolean() || value.is_null() {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    let semantic = [
        "prompt",
        "seed",
        "first_frame",
        "last_frame",
        "width",
        "height",
        "steps",
        "cfg",
        "guidance",
        "denoise",
        "fps",
        "duration",
        "frame_count",
        "strength",
        "images",
        "image",
        "videos",
        "video",
        "audios",
        "audio",
    ];
    semantic
        .iter()
        .find(|candidate| lower == **candidate || lower.contains(**candidate))
        .map(|candidate| (*candidate).to_owned())
        .or_else(|| is_safe_key(&lower).then_some(lower))
}

fn is_dangerous_input_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "model_path",
        "filename_prefix",
        "output_directory",
        "output_dir",
        "filesystem_path",
        "file_path",
        "custom_python",
        "python_path",
        "backend_endpoint",
        "endpoint",
        "filename",
        "directory",
        "folder",
        "path",
        "prefix",
        "python",
        "device",
        "provider",
        "checkpoint",
        "ckpt",
        "unet",
        "vae",
        "clip",
        "lora",
        "model",
    ]
    .iter()
    .any(|token| lower == *token || lower.contains(token))
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    value.chars().take(limit).collect::<String>() + "…"
}

fn sanitize_display_value(value: &str) -> String {
    let trimmed = value.trim();
    let looks_absolute = trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || (trimmed.as_bytes().get(1) == Some(&b':'));
    let display = if looks_absolute {
        trimmed
            .rsplit(|character: char| character == '/' || character == '\\')
            .find(|part| !part.is_empty())
            .unwrap_or(trimmed)
    } else {
        trimmed
    };
    truncate(display, 120)
}

fn is_numeric_node_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok()
}

fn is_safe_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn validate_workflow_id(value: &str) -> Result<String, WorkflowOnboardingError> {
    let suffix = value.strip_prefix("wfl_").unwrap_or("");
    if suffix.is_empty() || !is_safe_key(suffix) {
        return Err(WorkflowOnboardingError::new(
            "MANIFEST_INVALID",
            "workflow id must match wfl_[a-z][a-z0-9_]{0,63}",
        ));
    }
    Ok(value.to_owned())
}

fn validate_metadata_values(
    name: &str,
    workflow_version: &str,
    recipe_version: &str,
    category: &str,
    mode: &str,
) -> Result<(), WorkflowOnboardingError> {
    if name.trim().is_empty() || category.trim().is_empty() || mode.trim().is_empty() {
        return Err(WorkflowOnboardingError::new(
            "MANIFEST_INVALID",
            "name, category, and mode must not be empty",
        ));
    }
    for (label, version) in [
        ("workflow_version", workflow_version),
        ("recipe_version", recipe_version),
    ] {
        if !is_semver(version) {
            return Err(WorkflowOnboardingError::new(
                "MANIFEST_INVALID",
                format!("{label} must be valid SemVer"),
            ));
        }
    }
    Ok(())
}

fn is_semver(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

fn compare_semver(left: &str, right: &str) -> std::cmp::Ordering {
    let parse = |value: &str| {
        value
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parse(left).cmp(&parse(right))
}

fn increment_semver(value: &str) -> String {
    let mut parts = value
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    while parts.len() < 3 {
        parts.push(0);
    }
    parts[2] = parts[2].saturating_add(1);
    format!("{}.{}.{}", parts[0], parts[1], parts[2])
}

fn parse_optional_i64(
    value: Option<String>,
    field: &str,
) -> Result<Option<i64>, WorkflowOnboardingError> {
    value
        .map(|value| {
            value.parse().map_err(|_| {
                WorkflowOnboardingError::new(
                    "MAPPING_INVALID",
                    format!("{field} must be an integer"),
                )
            })
        })
        .transpose()
}

fn parse_optional_u64(
    value: Option<String>,
    field: &str,
) -> Result<Option<u64>, WorkflowOnboardingError> {
    value
        .map(|value| {
            value.parse().map_err(|_| {
                WorkflowOnboardingError::new(
                    "MAPPING_INVALID",
                    format!("{field} must be an unsigned integer"),
                )
            })
        })
        .transpose()
}

pub(crate) fn read_back_and_validate_package(
    package: &WorkflowPackageBytes,
) -> Result<(), WorkflowOnboardingError> {
    let manifest = String::from_utf8(package.manifest_yaml.clone())
        .map_err(|error| WorkflowOnboardingError::new("MANIFEST_INVALID", error.to_string()))?;
    let recipe = String::from_utf8(package.recipe_yaml.clone())
        .map_err(|error| WorkflowOnboardingError::new("RECIPE_INVALID", error.to_string()))?;
    let manifest = WorkflowManifest::parse(&manifest)
        .map_err(|error| WorkflowOnboardingError::new("MANIFEST_INVALID", error))?;
    manifest
        .validate()
        .map_err(|error| WorkflowOnboardingError::new("MANIFEST_INVALID", error))?;
    let recipe = RecipeParser::parse(&recipe)
        .map_err(|error| WorkflowOnboardingError::new("RECIPE_INVALID", error.to_string()))?;
    RecipeValidator::validate(&recipe)
        .map_err(|error| WorkflowOnboardingError::new("RECIPE_INVALID", error.to_string()))?;
    let workflow =
        validate_api_workflow(serde_json::from_slice(&package.workflow_api_json).map_err(
            |error| WorkflowOnboardingError::new("WORKFLOW_NOT_API_FORMAT", error.to_string()),
        )?)?;
    BindingValidator::validate(&recipe, &workflow)
        .map_err(|error| WorkflowOnboardingError::new("BINDING_INVALID", error.to_string()))?;
    dry_run_compile(&recipe, &workflow)?;
    Ok(())
}

fn package_directory_name(manifest: &WorkflowManifest, workflow_sha256: &str) -> String {
    format!(
        "{}_{}_{}",
        slugify(&manifest.id.trim_start_matches("wfl_")),
        manifest.workflow_version.replace('.', "_"),
        &workflow_sha256[..8.min(workflow_sha256.len())]
    )
}

fn package_directory_name_with_recipe(
    manifest: &WorkflowManifest,
    workflow_sha256: &str,
    recipe_yaml: &str,
) -> String {
    format!(
        "{}_{}_{}_{}_{}",
        slugify(&manifest.id.trim_start_matches("wfl_")),
        manifest.workflow_version.replace('.', "_"),
        slugify(&manifest.recipe_version),
        &workflow_sha256[..8.min(workflow_sha256.len())],
        &sha256(recipe_yaml.as_bytes())[..8],
    )
}

fn slugify(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            result.push(character);
        } else if character.is_ascii_uppercase() {
            result.push(character.to_ascii_lowercase());
        } else if !result.ends_with('_') {
            result.push('_');
        }
    }
    let result = result.trim_matches('_').to_owned();
    if result.is_empty() {
        "workflow".to_owned()
    } else {
        result
    }
}

fn filename_stem(value: &str) -> String {
    Path::new(value)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "Imported Workflow".to_owned())
}

fn safe_filename(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("workflow.json")
        .to_owned()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::ports::{
            ComfyEventSubscription, ComfyHealth, ComfyHistory, ComfyInputUpload, ComfyOutputData,
            ComfyOutputFile, ComfyQueueState, ComfyUploadedInput, PromptSubmission,
            RepositoryError, SystemStats, WorkflowRunRepository,
        },
        compiler::RecipeParser,
        domain::OutputType,
        infrastructure::{
            database::{initialize, SqliteWorkflowLibraryRepository},
            filesystem::{FileSystemWorkflowLibrarySource, FileSystemWorkflowPackageStore},
        },
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn validates_api_shape_and_broken_links_without_confusing_literal_arrays() {
        let valid = validate_api_workflow(json!({
            "1": {"inputs": {"literal": ["not_a_node", 0]}, "class_type": "Literal"},
            "2": {"inputs": {"connected": ["1", 0]}, "class_type": "Consumer"}
        }));
        assert!(valid.is_ok());

        let broken = validate_api_workflow(json!({
            "2": {"inputs": {"connected": ["999", 0]}, "class_type": "Consumer"}
        }))
        .expect_err("broken link must fail");
        assert_eq!(broken.code(), "WORKFLOW_BROKEN_LINK");
    }

    #[test]
    fn rejects_visual_and_empty_workflows() {
        assert_eq!(
            validate_api_workflow(json!({})).unwrap_err().code(),
            "WORKFLOW_EMPTY"
        );
        assert_eq!(
            validate_api_workflow(json!({"nodes": [], "links": []}))
                .unwrap_err()
                .code(),
            "WORKFLOW_NOT_API_FORMAT"
        );
    }

    #[test]
    fn rejects_invalid_nodes_but_tolerates_future_node_fields() {
        assert_eq!(
            validate_api_workflow(json!({"node": {"inputs": {}, "class_type": "Node"}}))
                .unwrap_err()
                .code(),
            "WORKFLOW_NODE_ID_INVALID"
        );
        assert_eq!(
            validate_api_workflow(json!({"1": {"inputs": {}}}))
                .unwrap_err()
                .code(),
            "WORKFLOW_CLASS_TYPE_MISSING"
        );
        assert_eq!(
            validate_api_workflow(json!({"1": {"class_type": "Node"}}))
                .unwrap_err()
                .code(),
            "WORKFLOW_INPUTS_MISSING"
        );
        assert!(validate_api_workflow(json!({
            "1": {
                "inputs": {},
                "class_type": "Node",
                "_meta": {"title": "Safe"},
                "future_field": {"accepted": true}
            }
        }))
        .is_ok());
    }

    #[test]
    fn capability_is_generic_and_groups_missing_classes() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"inputs": {"sampler": "euler", "steps": 12}, "class_type": "Sampler"},
            "2": {"inputs": {}, "class_type": "MissingNode"},
            "3": {"inputs": {}, "class_type": "MissingNode"}
        }))
        .unwrap();
        let nodes = inspect_workflow(&workflow).unwrap();
        let object_info = json!({
            "Sampler": {"input": {"required": {
                "sampler": [["euler", "ddim"], {}],
                "steps": ["INT", {"min": 1, "max": 10}]
            }}}
        });
        let report = evaluate_capability(&workflow, &nodes, &object_info);
        assert_eq!(report.state, CapabilityState::MissingNodes);
        let missing = report
            .issues
            .iter()
            .find(|issue| issue.code == "MISSING_NODE")
            .expect("missing class should be reported");
        assert_eq!(missing.class_type.as_deref(), Some("MissingNode"));
        assert_eq!(missing.affected_node_ids, vec!["2", "3"]);

        let available = json!({
            "Sampler": {"input": {"required": {
                "sampler": [["euler", "ddim"], {}],
                "steps": ["INT", {"min": 1, "max": 10}]
            }}},
            "MissingNode": {"input": {"required": {}}}
        });
        let report = evaluate_capability(&workflow, &nodes, &available);
        assert_eq!(report.state, CapabilityState::IncompatibleInputValues);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "INPUT_VALUE_OUT_OF_RANGE"));
        assert!(!report
            .issues
            .iter()
            .any(|issue| issue.code == "MISSING_NODE"));
    }

    #[test]
    fn capability_detects_unavailable_combo_and_enriches_safe_inspector_data() {
        let workflow = WorkflowDocument::parse(json!({
            "1": {"inputs": {"sampler": "not_available", "steps": 4}, "class_type": "Sampler"}
        }))
        .unwrap();
        let mut nodes = inspect_workflow(&workflow).unwrap();
        let object_info = json!({
            "Sampler": {
                "output_node": true,
                "input": {"required": {
                    "sampler": [["euler", "ddim"], {}],
                    "steps": ["INT", {"min": 1, "max": 10}]
                }}
            }
        });
        enrich_nodes_with_capability(&mut nodes, &object_info);
        assert!(nodes[0].is_output_node);
        assert_eq!(nodes[0].inputs[0].allowed_options, vec!["euler", "ddim"]);
        assert_eq!(nodes[0].inputs[1].numeric_min.as_deref(), Some("1"));
        let report = evaluate_capability(&workflow, &nodes, &object_info);
        assert_eq!(report.state, CapabilityState::IncompatibleInputValues);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "INPUT_OPTION_UNAVAILABLE"
                && issue.node_id.as_deref() == Some("1")));
    }

    #[test]
    fn mapping_supports_recipe_types_and_rejects_unsafe_or_expanded_ranges() {
        assert!(validate_mapping_value(SemanticFieldType::Textarea, &json!("text")).is_ok());
        assert!(validate_mapping_value(SemanticFieldType::Integer, &json!(7)).is_ok());
        assert!(validate_mapping_value(SemanticFieldType::Seed, &json!(7)).is_ok());
        assert!(validate_mapping_value(SemanticFieldType::Image, &json!("image.png")).is_ok());
        assert!(validate_mapping_value(SemanticFieldType::Images, &json!("image.png")).is_ok());
        assert!(validate_mapping_value(SemanticFieldType::Video, &json!("video.mp4")).is_ok());
        assert!(validate_mapping_value(SemanticFieldType::Videos, &json!("video.mp4")).is_ok());
        assert!(validate_mapping_value(SemanticFieldType::Audio, &json!("audio.wav")).is_ok());
        assert!(validate_mapping_value(SemanticFieldType::Audios, &json!("audio.wav")).is_ok());
        assert_eq!(
            validate_mapping_value(SemanticFieldType::Image, &json!({}))
                .unwrap_err()
                .code(),
            "MAPPING_UNSUPPORTED"
        );
        assert!(is_safe_key("generated_image"));
        assert!(!is_safe_key("GeneratedImage"));
        assert!(is_dangerous_input_name("filename_prefix"));
        assert!(is_dangerous_input_name("model_path"));
        assert!(!is_dangerous_input_name("steps"));

        let input = WorkflowInputView {
            name: "steps".to_owned(),
            kind: "number".to_owned(),
            current_value_summary: "4".to_owned(),
            is_linked: false,
            bindable: true,
            suggested_type: Some("integer".to_owned()),
            suggested_semantic_key: Some("steps".to_owned()),
            numeric_min: Some("1".to_owned()),
            numeric_max: Some("10".to_owned()),
            numeric_step: None,
            allowed_options: Vec::new(),
        };
        let request = WorkflowOnboardingInputMappingRequest {
            semantic_key: "steps".to_owned(),
            field_type: "integer".to_owned(),
            label: "Steps".to_owned(),
            required: true,
            default_value: None,
            min_value: Some("0".to_owned()),
            max_value: Some("20".to_owned()),
            step: None,
            min_items: None,
            max_items: None,
            target_node: "1".to_owned(),
            target_input: "steps".to_owned(),
            item_index: None,
        };
        assert_eq!(
            validate_mapping_bounds(SemanticFieldType::Integer, Some(&input), &request)
                .unwrap_err()
                .code(),
            "MAPPING_OUT_OF_RANGE"
        );
    }

    #[test]
    fn inspector_suggests_semantic_keys_without_exposing_links_or_unsupported_values() {
        assert_eq!(
            suggestion_for_semantic_key("positive_prompt", &json!("hello"), false).as_deref(),
            Some("prompt")
        );
        assert_eq!(
            suggestion_for_semantic_key("steps", &json!(20), false).as_deref(),
            Some("steps")
        );
        assert!(suggestion_for_input("denoise", &json!(0.5), false).is_none());
        assert!(suggestion_for_input("steps", &json!(20.5), false).is_none());
        assert!(suggestion_for_semantic_key("connected", &json!(["1", 0]), true).is_none());
        assert!(suggestion_for_semantic_key("enabled", &json!(true), false).is_none());
    }

    #[test]
    fn recipe_yaml_round_trip_preserves_domain_semantics_and_outputs() {
        let draft = sample_draft();
        let recipe = build_recipe(&draft).unwrap();
        let yaml = recipe_to_yaml(&recipe).unwrap();
        let parsed = RecipeParser::parse(&yaml).unwrap();
        assert_eq!(parsed, recipe);
        assert_eq!(recipe.outputs[0].output_type, OutputType::Image);
        assert!(yaml.contains("schema_version: 1"));
    }

    #[tokio::test]
    async fn staging_readback_runs_manifest_recipe_workflow_and_dry_run_validation() {
        let directory = tempdir().unwrap();
        let library_root = directory.path().join("library");
        let staging_root = directory.path().join("staging");
        tokio::fs::create_dir_all(&library_root).await.unwrap();
        tokio::fs::create_dir_all(&staging_root).await.unwrap();
        let draft = sample_draft();
        let recipe = build_recipe(&draft).unwrap();
        let manifest = draft.manifest.to_yaml().unwrap();
        let recipe_yaml = recipe_to_yaml(&recipe).unwrap();
        let store = FileSystemWorkflowPackageStore::new(library_root, staging_root);
        let package = WorkflowPackageBytes::new(
            manifest.into_bytes(),
            recipe_yaml.into_bytes(),
            draft.raw_bytes.clone(),
        );
        store
            .stage("onb_00000000-0000-4000-8000-000000000001", &package)
            .await
            .unwrap();
        let staged = store
            .read_staging("onb_00000000-0000-4000-8000-000000000001")
            .await
            .unwrap();
        read_back_and_validate_package(&staged).unwrap();
        assert_eq!(staged, package);
    }

    #[tokio::test]
    async fn publish_creates_new_package_refreshes_catalog_and_keeps_dry_run_offline() {
        let directory = tempdir().unwrap();
        let data_root = directory.path().join("AIStudioData");
        let library_root = data_root.join("workflow_library");
        let staging_root = data_root.join("workflow_staging");
        tokio::fs::create_dir_all(&library_root).await.unwrap();
        tokio::fs::create_dir_all(&staging_root).await.unwrap();
        let pool = initialize(&data_root.join("app.db")).await.unwrap();
        let source = Arc::new(FileSystemWorkflowLibrarySource::new(library_root.clone()));
        let library_service = Arc::new(WorkflowLibraryService::new(
            source.clone(),
            Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone())),
            Arc::new(TestClock),
        ));
        let adapter = Arc::new(StubComfyAdapter::ready());
        let service = WorkflowOnboardingService::new(
            source,
            adapter.clone(),
            library_service,
            Arc::new(StubRunRepository),
            Arc::new(FileSystemWorkflowPackageStore::new(
                library_root.clone(),
                staging_root.clone(),
            )),
            Arc::new(TestClock),
        );
        let raw = br#"{"1":{"inputs":{"prompt":"hello"},"class_type":"Sampler"},"2":{"inputs":{"image":["1",0],"filename_prefix":"ComfyUI"},"class_type":"SaveImage"}}"#.to_vec();
        let draft = service
            .import_bytes(raw, "onboarded_t2i.json".to_owned(), None)
            .await
            .unwrap();
        let capability = service.check_capability(&draft.draft_id).await.unwrap();
        assert_eq!(capability.state, CapabilityState::Ready);
        let linked_error = service
            .set_input_mapping(
                &draft.draft_id,
                WorkflowOnboardingInputMappingRequest {
                    semantic_key: "reference_image".to_owned(),
                    field_type: "image".to_owned(),
                    label: "Reference image".to_owned(),
                    required: false,
                    default_value: None,
                    min_value: None,
                    max_value: None,
                    step: None,
                    min_items: None,
                    max_items: None,
                    target_node: "2".to_owned(),
                    target_input: "image".to_owned(),
                    item_index: None,
                },
            )
            .expect_err("linked inputs must remain internal");
        assert_eq!(linked_error.code(), "LINKED_INPUT_NOT_BINDABLE");
        let dangerous_error = service
            .set_input_mapping(
                &draft.draft_id,
                WorkflowOnboardingInputMappingRequest {
                    semantic_key: "output_prefix".to_owned(),
                    field_type: "textarea".to_owned(),
                    label: "Output prefix".to_owned(),
                    required: false,
                    default_value: Some("ComfyUI".to_owned()),
                    min_value: None,
                    max_value: None,
                    step: None,
                    min_items: None,
                    max_items: None,
                    target_node: "2".to_owned(),
                    target_input: "filename_prefix".to_owned(),
                    item_index: None,
                },
            )
            .expect_err("dangerous path-like inputs must remain internal");
        assert_eq!(dangerous_error.code(), "MAPPING_DANGEROUS_INPUT");
        let mapped = service
            .set_input_mapping(
                &draft.draft_id,
                WorkflowOnboardingInputMappingRequest {
                    semantic_key: "prompt".to_owned(),
                    field_type: "textarea".to_owned(),
                    label: "Prompt".to_owned(),
                    required: true,
                    default_value: Some("hello".to_owned()),
                    min_value: None,
                    max_value: None,
                    step: None,
                    min_items: None,
                    max_items: None,
                    target_node: "1".to_owned(),
                    target_input: "prompt".to_owned(),
                    item_index: None,
                },
            )
            .unwrap();
        assert!(mapped.recipe.valid);
        service
            .set_output_mapping(
                &draft.draft_id,
                WorkflowOnboardingOutputMappingRequest {
                    output_id: "generated_image".to_owned(),
                    label: "Generated image".to_owned(),
                    output_type: "image".to_owned(),
                    node_id: "2".to_owned(),
                    required: true,
                },
            )
            .unwrap();
        let validation = service.validate(&draft.draft_id).unwrap();
        assert!(validation.ready_to_publish);
        let published = service.publish(&draft.draft_id).await.unwrap();
        assert!(published.package_name.starts_with("onboarded_t2i_1_0_0_"));
        assert!(library_root.join(&published.package_name).is_dir());
        assert!(!staging_root.join(&draft.draft_id).exists());
        assert_eq!(adapter.submit_calls(), 0);
        assert_eq!(adapter.upload_calls(), 0);
        let definitions =
            crate::application::ports::GenerationDefinitionRepository::list_available(
                &crate::infrastructure::database::SqliteGenerationDefinitionRepository::new(pool),
            )
            .await
            .unwrap();
        assert_eq!(definitions.len(), 1);

        let duplicate = service
            .duplicate_recipe_draft("wfl_onboarded_t2i", "1.0.0", Some("1.0.0"), None)
            .await
            .unwrap();
        assert_eq!(duplicate.manifest.recipe_version, "1.0.1");
        assert_eq!(duplicate.workflow_sha256, draft.workflow_sha256);
        assert!(!duplicate.input_mappings.is_empty());
        service.check_capability(&duplicate.draft_id).await.unwrap();
        assert!(
            service
                .validate(&duplicate.draft_id)
                .unwrap()
                .ready_to_publish
        );
        let duplicate_published = service.publish(&duplicate.draft_id).await.unwrap();
        assert_ne!(duplicate_published.package_name, published.package_name);
    }

    #[tokio::test]
    async fn auto_onboarding_publishes_t2i_without_gpu_submission() {
        let directory = tempdir().unwrap();
        let data_root = directory.path().join("AIStudioData");
        let library_root = data_root.join("workflow_library");
        let staging_root = data_root.join("workflow_staging");
        tokio::fs::create_dir_all(&library_root).await.unwrap();
        tokio::fs::create_dir_all(&staging_root).await.unwrap();
        let pool = initialize(&data_root.join("app.db")).await.unwrap();
        let source = Arc::new(FileSystemWorkflowLibrarySource::new(library_root.clone()));
        let library_service = Arc::new(WorkflowLibraryService::new(
            source.clone(),
            Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone())),
            Arc::new(TestClock),
        ));
        let adapter = Arc::new(StubComfyAdapter {
            object_info: Ok(json!({
                "Sampler": {"input": {"required": {
                    "prompt": ["STRING", {}],
                    "seed": ["INT", {"min": 0, "max": 999999, "step": 1}],
                    "width": ["INT", {"min": 64, "max": 2048, "step": 64}],
                    "height": ["INT", {"min": 64, "max": 2048, "step": 64}]
                }}},
                "SaveImage": {"output_node": true, "input": {"required": {}}}
            })),
            object_info_calls: std::sync::atomic::AtomicUsize::new(0),
            submit_calls: std::sync::atomic::AtomicUsize::new(0),
            upload_calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let service = WorkflowOnboardingService::new(
            source,
            adapter.clone(),
            library_service,
            Arc::new(StubRunRepository),
            Arc::new(FileSystemWorkflowPackageStore::new(
                library_root.clone(),
                staging_root,
            )),
            Arc::new(TestClock),
        );
        let raw = br#"{
            "1":{"inputs":{"prompt":"hello","seed":7,"width":512,"height":512},"class_type":"Sampler"},
            "2":{"inputs":{"images":["1",0]},"class_type":"SaveImage"}
        }"#
        .to_vec();

        let plan = service
            .auto_onboard_bytes(raw.clone(), "simple_t2i.json".to_owned(), None)
            .await
            .unwrap();

        assert_eq!(plan.state, WorkflowAutoOnboardingState::AutoPublished);
        assert_eq!(plan.workflow_kind, "IMAGE");
        assert_eq!(plan.metadata.mode, "text_to_image");
        assert!(plan.auto_publishable);
        assert!(plan.published.is_some());
        assert!(plan
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == "prompt"));
        assert!(plan
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == "seed"));
        assert!(plan.input_mappings.iter().any(
            |mapping| mapping.semantic_key == "width" && mapping.step.as_deref() == Some("64")
        ));
        assert_eq!(plan.output_mappings[0].output_type, "image");
        assert_eq!(adapter.submit_calls(), 0);
        assert_eq!(adapter.upload_calls(), 0);

        let duplicate = service
            .auto_onboard_bytes(raw, "renamed_copy.json".to_owned(), None)
            .await
            .unwrap();
        assert_eq!(duplicate.state, WorkflowAutoOnboardingState::AlreadyExists);
        assert_eq!(
            duplicate.existing_workflow_id.as_deref(),
            Some("wfl_simple_t2i")
        );
        assert!(duplicate.published.is_none());

        let next_raw = br#"{
            "1":{"inputs":{"prompt":"hello v2","seed":7,"width":512,"height":512},"class_type":"Sampler"},
            "2":{"inputs":{"images":["1",0]},"class_type":"SaveImage"}
        }"#
        .to_vec();
        let next_version = service
            .auto_onboard_bytes(
                next_raw,
                "renamed_copy.json".to_owned(),
                Some("wfl_simple_t2i".to_owned()),
            )
            .await
            .unwrap();
        assert_eq!(
            next_version.state,
            WorkflowAutoOnboardingState::AutoPublished
        );
        assert_eq!(next_version.metadata.workflow_version, "1.0.1");
    }

    #[tokio::test]
    async fn batch_capability_check_fetches_object_info_once() {
        let directory = tempdir().unwrap();
        let library_root = directory.path().join("library");
        let staging_root = directory.path().join("staging");
        tokio::fs::create_dir_all(&library_root).await.unwrap();
        tokio::fs::create_dir_all(&staging_root).await.unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        let source = Arc::new(FileSystemWorkflowLibrarySource::new(library_root.clone()));
        let adapter = Arc::new(StubComfyAdapter::ready());
        let service = WorkflowOnboardingService::new(
            source.clone(),
            adapter.clone(),
            Arc::new(WorkflowLibraryService::new(
                source,
                Arc::new(SqliteWorkflowLibraryRepository::new(pool)),
                Arc::new(TestClock),
            )),
            Arc::new(StubRunRepository),
            Arc::new(FileSystemWorkflowPackageStore::new(
                library_root,
                staging_root,
            )),
            Arc::new(TestClock),
        );
        let workflow = r#"{
            "1":{"inputs":{"prompt":"hello"},"class_type":"Sampler"},
            "2":{"inputs":{"image":["1",0]},"class_type":"SaveImage"}
        }"#
        .to_owned();
        let workflows = (0..10)
            .map(|index| (format!("version_{index}"), workflow.clone()))
            .collect::<Vec<_>>();

        let checked = service.check_runtime_workflows(&workflows).await.unwrap();

        assert_eq!(checked.len(), 10);
        assert_eq!(adapter.object_info_calls(), 1);
    }

    #[test]
    fn auto_inference_covers_video_modes_linked_inputs_and_prompt_ambiguity() {
        let video = br#"{
            "1":{"inputs":{"prompt":"motion","seed":7,"first_frame":"first.png","last_frame":"last.png","duration":5},"class_type":"VideoSampler"},
            "2":{"inputs":{"video":["1",0]},"class_type":"SaveVideo"}
        }"#;
        let workflow = validate_api_workflow(serde_json::from_slice(video).unwrap()).unwrap();
        let draft = WorkflowOnboardingDraft {
            draft_id: "onb_video".to_owned(),
            raw_bytes: video.to_vec(),
            workflow_sha256: sha256(video),
            original_filename: "video.json".to_owned(),
            nodes: inspect_workflow(&workflow).unwrap(),
            workflow,
            manifest: WorkflowManifest {
                schema_version: 1,
                id: "wfl_video".to_owned(),
                name: "Video".to_owned(),
                workflow_version: "1.0.0".to_owned(),
                recipe_version: "1.0.0".to_owned(),
                category: "image".to_owned(),
                mode: "text_to_image".to_owned(),
            },
            recipe_id: "rcp_video".to_owned(),
            allow_existing_workflow_sha: false,
            capability: CapabilityCheckView {
                state: CapabilityState::Ready,
                checked_at: None,
                issues: Vec::new(),
            },
            input_mappings: Vec::new(),
            output_mappings: Vec::new(),
        };
        let result = infer_auto_onboarding(&draft);
        assert_eq!(result.category, "video");
        assert_eq!(result.mode, "image_to_video");
        assert!(result
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == "first_frame"));
        assert!(result
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == "last_frame"));
        assert!(result
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == "duration_seconds"));
        assert!(result
            .output_mappings
            .iter()
            .any(|mapping| mapping.output_type == OutputType::Video));
        assert!(!result
            .input_mappings
            .iter()
            .any(|mapping| mapping.target_input == "video"));

        let ambiguous = br#"{
            "1":{"inputs":{"prompt_a":"a","prompt_b":"b"},"class_type":"TextNode"},
            "2":{"inputs":{"images":["1",0]},"class_type":"SaveImage","output_node":true}
        }"#;
        let workflow = validate_api_workflow(serde_json::from_slice(ambiguous).unwrap()).unwrap();
        let mut ambiguous_draft = draft.clone();
        ambiguous_draft.raw_bytes = ambiguous.to_vec();
        ambiguous_draft.workflow_sha256 = sha256(ambiguous);
        ambiguous_draft.original_filename = "ambiguous.json".to_owned();
        ambiguous_draft.workflow = workflow.clone();
        ambiguous_draft.nodes = inspect_workflow(&workflow).unwrap();
        ambiguous_draft.input_mappings.clear();
        ambiguous_draft.output_mappings.clear();
        let result = infer_auto_onboarding(&ambiguous_draft);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "AMBIGUOUS_INPUT"));
        assert!(result.input_mappings.is_empty());
    }

    #[tokio::test]
    async fn offline_capability_keeps_mapping_available_but_blocks_publish() {
        let directory = tempdir().unwrap();
        let library_root = directory.path().join("library");
        let staging_root = directory.path().join("staging");
        tokio::fs::create_dir_all(&library_root).await.unwrap();
        tokio::fs::create_dir_all(&staging_root).await.unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        let source = Arc::new(FileSystemWorkflowLibrarySource::new(library_root.clone()));
        let service = WorkflowOnboardingService::new(
            source.clone(),
            Arc::new(StubComfyAdapter::offline()),
            Arc::new(WorkflowLibraryService::new(
                source,
                Arc::new(SqliteWorkflowLibraryRepository::new(pool)),
                Arc::new(TestClock),
            )),
            Arc::new(StubRunRepository),
            Arc::new(FileSystemWorkflowPackageStore::new(
                library_root.clone(),
                staging_root.clone(),
            )),
            Arc::new(TestClock),
        );
        let draft = service
            .import_bytes(
                br#"{"1":{"inputs":{"prompt":"hello"},"class_type":"Sampler"}}"#.to_vec(),
                "offline.json".to_owned(),
                None,
            )
            .await
            .unwrap();
        let capability = service.check_capability(&draft.draft_id).await.unwrap();
        assert_eq!(capability.state, CapabilityState::ComfyOffline);
        let error = service.publish(&draft.draft_id).await.unwrap_err();
        assert_eq!(error.code(), "WORKFLOW_ONBOARDING_NOT_READY");
        assert!(tokio::fs::read_dir(&library_root)
            .await
            .unwrap()
            .next_entry()
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn auto_onboarding_waits_when_comfy_is_offline_and_deduplicates_exact_sha() {
        let directory = tempdir().unwrap();
        let library_root = directory.path().join("library");
        let staging_root = directory.path().join("staging");
        tokio::fs::create_dir_all(&library_root).await.unwrap();
        tokio::fs::create_dir_all(&staging_root).await.unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        let source = Arc::new(FileSystemWorkflowLibrarySource::new(library_root.clone()));
        let service = WorkflowOnboardingService::new(
            source.clone(),
            Arc::new(StubComfyAdapter::offline()),
            Arc::new(WorkflowLibraryService::new(
                source,
                Arc::new(SqliteWorkflowLibraryRepository::new(pool)),
                Arc::new(TestClock),
            )),
            Arc::new(StubRunRepository),
            Arc::new(FileSystemWorkflowPackageStore::new(
                library_root.clone(),
                staging_root,
            )),
            Arc::new(TestClock),
        );
        let raw = br#"{
            "1":{"inputs":{"prompt":"offline"},"class_type":"Sampler"},
            "2":{"inputs":{"images":["1",0]},"class_type":"SaveImage"}
        }"#
        .to_vec();
        let waiting = service
            .auto_onboard_bytes(raw.clone(), "offline_auto.json".to_owned(), None)
            .await
            .unwrap();
        assert_eq!(
            waiting.state,
            WorkflowAutoOnboardingState::WaitingForComfyUi
        );
        assert!(waiting
            .input_mappings
            .iter()
            .any(|mapping| mapping.semantic_key == "prompt"));
        assert!(waiting.published.is_none());
        assert!(waiting.message.contains("连接 ComfyUI"));
    }

    #[test]
    fn registry_evicts_oldest_draft_at_sixteen() {
        let mut registry = WorkflowOnboardingRegistry::default();
        for index in 0..(MAX_ONBOARDING_DRAFTS + 1) {
            let id = format!("onb_{index}");
            let workflow = WorkflowDocument::parse(json!({
                "1": {"inputs": {}, "class_type": "Test"}
            }))
            .unwrap();
            registry.insert(WorkflowOnboardingDraft {
                draft_id: id,
                raw_bytes: br#"{"1":{"inputs":{},"class_type":"Test"}}"#.to_vec(),
                workflow,
                workflow_sha256: "sha".to_owned(),
                original_filename: "test.json".to_owned(),
                nodes: Vec::new(),
                manifest: WorkflowManifest {
                    schema_version: 1,
                    id: "wfl_test".to_owned(),
                    name: "Test".to_owned(),
                    workflow_version: "1.0.0".to_owned(),
                    recipe_version: "1.0.0".to_owned(),
                    category: "image".to_owned(),
                    mode: "text_to_image".to_owned(),
                },
                recipe_id: "rcp_test".to_owned(),
                allow_existing_workflow_sha: false,
                capability: CapabilityCheckView {
                    state: CapabilityState::NotChecked,
                    checked_at: None,
                    issues: Vec::new(),
                },
                input_mappings: Vec::new(),
                output_mappings: Vec::new(),
            });
        }
        assert_eq!(registry.order.len(), MAX_ONBOARDING_DRAFTS);
        assert!(!registry.drafts.contains_key("onb_0"));
    }

    #[test]
    fn semver_increment_and_value_summaries_are_stable() {
        assert!(compare_semver("1.0.1", "1.0.0").is_gt());
        assert_eq!(increment_semver("1.2.9"), "1.2.10");
        assert_eq!(current_value_summary(&json!("a")), "a");
        assert_eq!(current_value_summary(&json!([1, 2])), "list (2)");
        assert_eq!(
            current_value_summary(&json!("C:\\\\ComfyUI\\models\\foo.safetensors")),
            "foo.safetensors"
        );
    }

    fn sample_draft() -> WorkflowOnboardingDraft {
        let raw_bytes = br#"{"1":{"inputs":{"prompt":"hello","seed":7},"class_type":"Sampler"},"2":{"inputs":{"image":["1",0]},"class_type":"SaveImage","output_node":true}}"#.to_vec();
        let workflow = validate_api_workflow(serde_json::from_slice(&raw_bytes).unwrap()).unwrap();
        let nodes = inspect_workflow(&workflow).unwrap();
        WorkflowOnboardingDraft {
            draft_id: "onb_test".to_owned(),
            raw_bytes,
            workflow,
            workflow_sha256: sha256(br#"sample"#),
            original_filename: "sample.json".to_owned(),
            nodes,
            manifest: WorkflowManifest {
                schema_version: 1,
                id: "wfl_sample".to_owned(),
                name: "Sample".to_owned(),
                workflow_version: "1.0.0".to_owned(),
                recipe_version: "1.0.0".to_owned(),
                category: "image".to_owned(),
                mode: "text_to_image".to_owned(),
            },
            recipe_id: "rcp_sample".to_owned(),
            allow_existing_workflow_sha: false,
            capability: CapabilityCheckView {
                state: CapabilityState::Ready,
                checked_at: None,
                issues: Vec::new(),
            },
            input_mappings: vec![
                InputMapping {
                    semantic_key: "prompt".to_owned(),
                    field_type: SemanticFieldType::Textarea,
                    label: "Prompt".to_owned(),
                    required: true,
                    default_value: Some("hello".to_owned()),
                    min_value: None,
                    max_value: None,
                    step: None,
                    min_items: None,
                    max_items: None,
                    target_node: "1".to_owned(),
                    target_input: "prompt".to_owned(),
                    item_index: None,
                },
                InputMapping {
                    semantic_key: "seed".to_owned(),
                    field_type: SemanticFieldType::Seed,
                    label: "Seed".to_owned(),
                    required: true,
                    default_value: Some("7".to_owned()),
                    min_value: Some("0".to_owned()),
                    max_value: Some("10".to_owned()),
                    step: None,
                    min_items: None,
                    max_items: None,
                    target_node: "1".to_owned(),
                    target_input: "seed".to_owned(),
                    item_index: None,
                },
            ],
            output_mappings: vec![OutputMapping {
                output_id: "generated_image".to_owned(),
                label: "Generated image".to_owned(),
                output_type: OutputType::Image,
                node_id: "2".to_owned(),
                required: true,
            }],
        }
    }

    #[derive(Clone)]
    struct TestClock;

    impl Clock for TestClock {
        fn now(&self) -> chrono::DateTime<chrono::Utc> {
            chrono::Utc::now()
        }
    }

    struct StubRunRepository;

    #[async_trait]
    impl WorkflowRunRepository for StubRunRepository {
        async fn has_successful_run(
            &self,
            _workflow_id: &str,
            _workflow_version: &str,
        ) -> Result<bool, RepositoryError> {
            Ok(false)
        }
    }

    struct StubComfyAdapter {
        object_info: Result<serde_json::Value, ComfyAdapterError>,
        object_info_calls: std::sync::atomic::AtomicUsize,
        submit_calls: std::sync::atomic::AtomicUsize,
        upload_calls: std::sync::atomic::AtomicUsize,
    }

    impl StubComfyAdapter {
        fn ready() -> Self {
            Self {
                object_info: Ok(json!({
                    "Sampler": {"input": {"required": {"prompt": ["STRING", {}]} }},
                    "SaveImage": {"output_node": true, "input": {"required": {}}}
                })),
                object_info_calls: std::sync::atomic::AtomicUsize::new(0),
                submit_calls: std::sync::atomic::AtomicUsize::new(0),
                upload_calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn offline() -> Self {
            Self {
                object_info: Err(ComfyAdapterError::Offline("test offline".to_owned())),
                object_info_calls: std::sync::atomic::AtomicUsize::new(0),
                submit_calls: std::sync::atomic::AtomicUsize::new(0),
                upload_calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn object_info_calls(&self) -> usize {
            self.object_info_calls
                .load(std::sync::atomic::Ordering::SeqCst)
        }

        fn submit_calls(&self) -> usize {
            self.submit_calls.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn upload_calls(&self) -> usize {
            self.upload_calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ComfyAdapter for StubComfyAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Offline("test".to_owned()))
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Err(ComfyAdapterError::Offline("test".to_owned()))
        }

        async fn get_object_info(&self) -> Result<serde_json::Value, ComfyAdapterError> {
            self.object_info_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.object_info.clone()
        }

        async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("test".to_owned()))
        }

        async fn download_output(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<ComfyOutputData, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("test".to_owned()))
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            _prompt_id: &str,
            _workflow: serde_json::Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            self.submit_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(ComfyAdapterError::Incompatible("test".to_owned()))
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("test".to_owned()))
        }

        async fn get_queue_state(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("test".to_owned()))
        }

        async fn upload_input_file(
            &self,
            _upload: ComfyInputUpload,
        ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
            self.upload_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(ComfyAdapterError::Incompatible("test".to_owned()))
        }
    }
}
