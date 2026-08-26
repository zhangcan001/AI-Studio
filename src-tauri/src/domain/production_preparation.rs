//! Domain contracts for production preparation and admission.
//!
//! Preparation is deliberately a frozen evidence layer.  It owns neither a
//! queue nor a generation executor; a prepared item is still started later by
//! the existing production queue.

use crate::domain::{
    ContextDiagnostic, ProductionBatchItemStatus, PromptContext, ReadinessGateResult,
    ResolvedOutputSpec, ResolvedProfiles, ResolvedReferenceAsset, ResolvedReferenceSet,
    ResolvedShotContext, ResolvedStageInput, ResolvedStructure, ResolvedWorkflowContext,
    ShotReadiness, ShotReadinessStatus, ShotStage,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PREPARATION_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// A serializable projection of `ResolvedShotContext` for commands and the
/// immutable snapshot.  `ResolvedShotContext` itself intentionally remains a
/// Rust-only value because `ShotStage` is not a wire DTO.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedShotContextView {
    pub project_id: String,
    pub structure: ResolvedStructure,
    pub stage: String,
    pub stage_input: ResolvedStageInput,
    pub reference_pack: crate::domain::ShotReferencePack,
    pub profiles: ResolvedProfiles,
    pub reference_assets: Vec<ResolvedReferenceAsset>,
    pub prompt_context: PromptContext,
    pub workflow: ResolvedWorkflowContext,
    pub output: ResolvedOutputSpec,
    pub legacy: crate::domain::LegacyContext,
    pub diagnostics: Vec<ContextDiagnostic>,
    pub partial: bool,
    pub resolver_identity: crate::domain::ResolverIdentity,
}

impl From<&ResolvedShotContext> for ResolvedShotContextView {
    fn from(context: &ResolvedShotContext) -> Self {
        Self {
            project_id: context.project_id.clone(),
            structure: context.structure.clone(),
            stage: context.stage.as_str().to_owned(),
            stage_input: context.stage_input.clone(),
            reference_pack: context.reference_pack.clone(),
            profiles: context.profiles.clone(),
            reference_assets: context.reference_assets.clone(),
            prompt_context: context.prompt_context.clone(),
            workflow: context.workflow.clone(),
            output: context.output.clone(),
            legacy: context.legacy.clone(),
            diagnostics: context.diagnostics.clone(),
            partial: context.partial,
            resolver_identity: context.resolver_identity.clone(),
        }
    }
}

impl From<ResolvedShotContext> for ResolvedShotContextView {
    fn from(context: ResolvedShotContext) -> Self {
        Self::from(&context)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotProductionPlan {
    pub project_id: String,
    pub shot_id: String,
    pub ordinal: u32,
    pub name: String,
    pub scene_id: Option<String>,
    pub stage: String,
    pub context_hash: String,
    pub resolved_context: ResolvedShotContextView,
    pub readiness: ShotReadiness,
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub current_stage_status: Option<String>,
    pub existing_batch_ids: Vec<String>,
    pub matching_prepared_batch_ids: Vec<String>,
    pub stale_prepared_batch_ids: Vec<String>,
    pub already_prepared: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub snapshot_identity: Option<PreparationSnapshotIdentity>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotProductionPlanSummary {
    pub shot_id: String,
    pub ordinal: u32,
    pub name: String,
    pub status: ShotReadinessStatus,
    pub score: i32,
    pub warning_count: usize,
    pub incomplete_count: usize,
    pub blocker_count: usize,
    pub context_hash: String,
    pub character_names: Vec<String>,
    pub character_count: usize,
    pub scene_profile_name: Option<String>,
    pub reference_count: usize,
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub current_stage_status: Option<String>,
    pub already_prepared: bool,
    pub existing_batch_ids: Vec<String>,
    pub matching_prepared_batch_ids: Vec<String>,
    pub stale_prepared_batch_ids: Vec<String>,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub legacy: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenePreparationView {
    pub project_id: String,
    pub scene_id: String,
    pub scene_name: String,
    pub stage: String,
    pub total: usize,
    pub ready_count: usize,
    pub incomplete_count: usize,
    pub blocked_count: usize,
    pub prepared_count: usize,
    pub warning_count: usize,
    pub items: Vec<ShotProductionPlanSummary>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationSnapshotPrompt {
    pub rendered_text: String,
    pub negative_prompt: String,
    pub ordered_segments: Vec<crate::domain::PromptSegment>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationSnapshotWorkflow {
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub scalar_values: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationSnapshotReadiness {
    pub status: ShotReadinessStatus,
    pub score: i32,
    pub gates: Vec<ReadinessGateResult>,
    pub evaluated_at: DateTime<Utc>,
}

/// Small, context-specific evidence copied from the one live preflight used
/// to evaluate a preparation.  It intentionally excludes object_info and
/// other large runtime payloads.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComfyCapabilityEvidence {
    pub checked_at: Option<DateTime<Utc>>,
    pub status: Option<String>,
    pub connection: Option<String>,
    pub workflow_version_id: Option<String>,
    pub workflow_ready: bool,
    pub workflow_total: usize,
    pub runtime_busy: bool,
    pub active_task_count: usize,
    pub production_busy: bool,
    pub node_count: Option<usize>,
    pub issue_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationSnapshotV1 {
    pub schema_version: u32,
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    pub context_hash: String,
    pub resolved_at: DateTime<Utc>,
    pub prepared_at: DateTime<Utc>,
    pub structure: ResolvedStructure,
    pub profiles: ResolvedProfiles,
    pub reference_sets: Vec<ResolvedReferenceSet>,
    pub reference_assets: Vec<ResolvedReferenceAsset>,
    pub prompt: PreparationSnapshotPrompt,
    pub workflow: PreparationSnapshotWorkflow,
    pub output_spec: ResolvedOutputSpec,
    pub stage_input: ResolvedStageInput,
    pub frozen_generation_values: Value,
    pub readiness: PreparationSnapshotReadiness,
    pub comfy_capability_evidence: ComfyCapabilityEvidence,
}

impl PreparationSnapshotV1 {
    pub fn from_context(
        context: &ResolvedShotContext,
        readiness: &ShotReadiness,
        frozen_generation_values: Value,
        comfy_capability_evidence: ComfyCapabilityEvidence,
        prepared_at: DateTime<Utc>,
    ) -> Self {
        Self {
            schema_version: PREPARATION_SNAPSHOT_SCHEMA_VERSION,
            project_id: context.project_id.clone(),
            shot_id: context.structure.shot.id.clone(),
            stage: context.stage.as_str().to_owned(),
            context_hash: context.resolver_identity.context_hash.clone(),
            resolved_at: context.resolver_identity.resolved_at.unwrap_or(prepared_at),
            prepared_at,
            structure: context.structure.clone(),
            profiles: context.profiles.clone(),
            reference_sets: context.reference_pack.reference_sets.clone(),
            reference_assets: context.reference_assets.clone(),
            prompt: PreparationSnapshotPrompt {
                rendered_text: context.prompt_context.rendered_text.clone(),
                negative_prompt: context.prompt_context.negative_prompt.clone(),
                ordered_segments: context.prompt_context.segments.clone(),
            },
            workflow: PreparationSnapshotWorkflow {
                workflow_version_id: context.workflow.workflow_version_id.clone(),
                recipe_id: context.workflow.recipe_id.clone(),
                scalar_values: context.workflow.scalar_values.clone(),
            },
            output_spec: context.output.clone(),
            stage_input: context.stage_input.clone(),
            frozen_generation_values,
            readiness: PreparationSnapshotReadiness {
                status: readiness.status,
                score: readiness.score,
                gates: readiness.gates.clone(),
                evaluated_at: readiness.evaluated_at,
            },
            comfy_capability_evidence,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationSnapshotIdentity {
    pub snapshot_id: String,
    pub production_batch_id: String,
    pub production_batch_item_id: String,
    pub context_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedShotBatchRecord {
    pub shot_id: String,
    pub stage: ShotStage,
    pub context_hash: String,
    pub production_batch_id: String,
    pub production_batch_item_id: String,
    pub item_status: ProductionBatchItemStatus,
    pub snapshot_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparationSnapshotRecord {
    pub id: String,
    pub project_id: String,
    pub shot_id: String,
    pub stage: ShotStage,
    pub context_hash: String,
    pub production_batch_id: String,
    pub production_batch_item_id: String,
    pub snapshot: PreparationSnapshotV1,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPreparationAdmission {
    pub project_id: String,
    pub stage: String,
    pub requested_count: usize,
    pub created_count: usize,
    pub skipped_incomplete: usize,
    pub skipped_blocked: usize,
    pub already_prepared_count: usize,
    pub created_batch_ids: Vec<String>,
    pub matching_prepared_batch_ids: Vec<String>,
}
