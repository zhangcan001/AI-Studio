//! Domain types and rules live here.
//!
//! This module intentionally has no dependency on Tauri, SQLx, HTTP clients,
//! or other infrastructure concerns.

pub mod asset;
pub mod consistency;
pub mod generation_snapshot;
pub mod preset;
pub mod production_item_review;
pub mod production_queue;
pub mod production_run;
pub mod production_structure;
pub mod project_id;
pub mod prompt_template;
pub mod recipe;
pub mod reference_anchor;
pub mod shot;
pub mod task;
pub mod workflow;

pub use asset::{
    Asset, AssetDomainError, AssetId, AssetType, GENERATED_IMAGE_CATEGORY,
    GENERATED_VIDEO_CATEGORY, SOURCE_AUDIO_CATEGORY, SOURCE_IMAGE_CATEGORY, SOURCE_VIDEO_CATEGORY,
};
pub use consistency::{
    generate_consistency_id, validate_consistency_id, validate_metadata_json,
    validate_optional_text, validate_profile_binding, validate_profile_name,
    validate_prompt_fragment, validate_reference_set, validate_reference_set_binding,
    validate_reference_set_items, BindingDomainError, BindingRole, CharacterProfile,
    ConsistencyIdKind, ConsistencyIdValidationError, ConsistencyProfileRecord,
    ConsistencyValidationError, CostumeVariant, InheritanceMode, ProfileDomainError,
    ProfileRevision, ProfileRevisionStatus, ProfileType, PropProfile, ReferenceSet,
    ReferenceSetItem, ReferenceSetPurpose, ReferenceSetPurposeError, SceneProfile,
    ShotProfileBinding, ShotReferenceSetBinding, StyleProfile, ValidationError,
};
pub use generation_snapshot::{GenerationSnapshot, SnapshotDomainError, SnapshotId};
pub use preset::{Preset, PresetDomainError, PresetId};
pub use production_item_review::{ProductionReviewDomainError, ProductionReviewStatus};
pub use production_queue::{
    ProductionBatch, ProductionBatchDetail, ProductionBatchId, ProductionBatchItem,
    ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus,
    ProductionQueueDomainError,
};
pub use production_run::{ProductionRunStatus, ProductionStageStatus, ProductionStageType};
pub use production_structure::{
    ProductionEpisode, ProductionEpisodeId, ProductionScene, ProductionSceneId, ProductionSeries,
    ProductionSeriesId, ProductionStructureDomainError, ShotSceneAssignment,
};
pub use project_id::{validate_project_id, ProjectIdValidationError};
pub use prompt_template::{
    ParsedPromptTemplate, PromptAnchor, PromptAnchorContext, PromptAnchorKind,
    PromptProjectContext, PromptShotContext, PromptStructureContext, PromptTemplateAnalysis,
    PromptTemplateContext, PromptTemplateSegment,
};
pub use recipe::{
    Binding, BindingTarget, CompileRequest, InputDefinition, InputValue, OutputDefinition,
    OutputType, Recipe, RecipeError, ResolvedInputValue, SeedDefault, SeedValue, WorkflowRef,
};
pub use reference_anchor::{
    ReferenceAnchor, ReferenceAnchorAsset, ReferenceAnchorDomainError, ReferenceAnchorId,
    ReferenceAnchorKind,
};
pub use shot::{
    canonical_shot_name, derive_stage_status, validate_scalar_values, ShotDomainError, ShotStage,
    ShotViewStatus,
};
pub use task::{
    NewTaskEvent, RuntimeProvenance, StoredTaskEvent, Task, TaskDomainError, TaskError,
    TaskEventType, TaskId, TaskProgress, TaskStateMachine, TaskStatus, TaskTelemetry,
    TaskTelemetryDurations, TaskTelemetryPatch,
};
pub use workflow::{WorkflowDocument, WorkflowError};
