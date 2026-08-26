//! DEV-050 readiness contracts and pure evaluator tests.
//!
//! Service wiring is owned by Agent C/Main.  These tests keep the evaluator
//! seam independent from Tauri state and use small local snapshots, so they
//! remain deterministic and never contact ComfyUI or submit generation work.

mod domain {
    pub use ai_studio_lib::domain::*;

    pub mod shot_context {
        pub use ai_studio_lib::domain::shot_context::*;
    }

    #[path = "../../src/domain/shot_readiness.rs"]
    pub mod shot_readiness;
}

mod application {
    pub mod comfy_service {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
        pub enum ComfyConnectionStatus {
            Connected,
            Offline,
            Incompatible,
        }
    }

    pub mod comfy_preflight_service {
        use super::comfy_service::ComfyConnectionStatus;

        #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
        pub enum ComfyPreflightStatus {
            Ready,
            Warning,
            Blocked,
        }

        #[derive(Clone, Debug, serde::Serialize)]
        pub struct ComfyPreflightWorkflow {
            pub workflow_version_id: Option<String>,
            pub missing_nodes: Vec<String>,
        }

        #[derive(Clone, Debug, serde::Serialize)]
        pub struct ComfyPreflightWorkflowSummary {
            pub items: Vec<ComfyPreflightWorkflow>,
        }

        #[derive(Clone, Debug, serde::Serialize)]
        pub struct ComfyPreflightReport {
            pub checked_at: String,
            pub connection: ComfyConnectionStatus,
            pub status: ComfyPreflightStatus,
            pub runtime_busy: bool,
            pub workflow_summary: ComfyPreflightWorkflowSummary,
        }
    }

    pub mod workflow_lifecycle_service {
        use super::workflow_onboarding_service::CapabilityIssueView;

        #[derive(Clone, Debug, serde::Serialize)]
        pub struct WorkflowRecipeSummaryView {
            pub recipe_id: String,
        }

        #[derive(Clone, Debug, serde::Serialize)]
        pub struct WorkflowProductionWorkspaceView {
            pub archived: bool,
            pub enabled: bool,
            pub package_status: String,
            pub workflow_version_id: Option<String>,
            pub category: Option<String>,
            pub mode: Option<String>,
            pub readiness: String,
            pub recipes: Vec<WorkflowRecipeSummaryView>,
            pub capability_issues: Vec<CapabilityIssueView>,
        }

        #[derive(Clone, Debug, serde::Serialize)]
        pub struct WorkflowProductionWorkspaceResponse {
            pub items: Vec<WorkflowProductionWorkspaceView>,
        }
    }

    pub mod workflow_onboarding_service {
        #[derive(Clone, Debug, serde::Serialize)]
        pub struct CapabilityIssueView {
            pub code: String,
            pub class_type: Option<String>,
            pub message: String,
        }
    }
}

#[path = "../src/application/shot_readiness_evaluator.rs"]
mod evaluator;

use application::comfy_preflight_service::{
    ComfyPreflightReport, ComfyPreflightStatus, ComfyPreflightWorkflow,
    ComfyPreflightWorkflowSummary,
};
use application::comfy_service::ComfyConnectionStatus;
use application::workflow_lifecycle_service::{
    WorkflowProductionWorkspaceResponse, WorkflowProductionWorkspaceView, WorkflowRecipeSummaryView,
};
use application::workflow_onboarding_service::CapabilityIssueView;
use chrono::{DateTime, Utc};
use domain::{
    ContextDiagnostic, LegacyContext, PromptContext, ResolvedOutputSpec, ResolvedProfiles,
    ResolvedReferenceAsset, ResolvedStructure, ResolvedStructureNode, ResolverIdentity,
    ShotReferencePack, ShotStage,
};
use evaluator::{evaluate_with_stage_input, ReadinessEnvironmentSnapshot, ReadinessStageInput};
use serde_json::json;
use std::fs;
use std::sync::Arc;

fn now() -> DateTime<Utc> {
    "2026-08-26T00:00:00Z".parse().unwrap()
}

fn workspace(mode: Option<&str>, ready: bool) -> WorkflowProductionWorkspaceResponse {
    WorkflowProductionWorkspaceResponse {
        items: vec![WorkflowProductionWorkspaceView {
            archived: false,
            enabled: true,
            package_status: "VALID".to_owned(),
            workflow_version_id: Some("wv-image".to_owned()),
            category: Some("IMAGE".to_owned()),
            mode: mode.map(str::to_owned),
            readiness: if ready { "READY" } else { "BLOCKED" }.to_owned(),
            recipes: vec![WorkflowRecipeSummaryView {
                recipe_id: "recipe-1".to_owned(),
            }],
            capability_issues: Vec::new(),
        }],
    }
    .with_mode(mode)
}

trait WorkspaceMode {
    fn with_mode(self, mode: Option<&str>) -> Self;
}

impl WorkspaceMode for WorkflowProductionWorkspaceResponse {
    fn with_mode(mut self, mode: Option<&str>) -> Self {
        if let Some(mode) = mode {
            self.items[0].mode = Some(mode.to_owned());
            self.items[0].category = Some(if mode == "I2V" { "VIDEO" } else { "IMAGE" }.to_owned());
        }
        self
    }
}

fn comfy(status: ComfyPreflightStatus, connection: ComfyConnectionStatus) -> ComfyPreflightReport {
    ComfyPreflightReport {
        checked_at: "2026-08-26T00:00:00Z".to_owned(),
        connection,
        status,
        runtime_busy: false,
        workflow_summary: ComfyPreflightWorkflowSummary {
            items: vec![ComfyPreflightWorkflow {
                workflow_version_id: Some("wv-image".to_owned()),
                missing_nodes: Vec::new(),
            }],
        },
    }
}

fn context(prompt: &str, stage: ShotStage) -> domain::ResolvedShotContext {
    domain::ResolvedShotContext {
        project_id: "project".to_owned(),
        structure: ResolvedStructure {
            scene: Some(ResolvedStructureNode {
                id: "scene".to_owned(),
                ordinal: 0,
                name: "Scene".to_owned(),
            }),
            shot: ResolvedStructureNode {
                id: "shot".to_owned(),
                ordinal: 0,
                name: "Shot".to_owned(),
            },
            ..ResolvedStructure::default()
        },
        stage,
        stage_input: Default::default(),
        reference_pack: ShotReferencePack {
            shot_id: "shot".to_owned(),
            prompt_context: PromptContext {
                rendered_text: prompt.to_owned(),
                ..PromptContext::default()
            },
            ..ShotReferencePack::default()
        },
        profiles: ResolvedProfiles::default(),
        reference_assets: Vec::new(),
        prompt_context: PromptContext {
            rendered_text: prompt.to_owned(),
            ..PromptContext::default()
        },
        workflow: domain::ResolvedWorkflowContext {
            workflow_version_id: Some("wv-image".to_owned()),
            recipe_id: Some("recipe-1".to_owned()),
            scalar_values: json!({"mode": "T2I"}),
        },
        output: ResolvedOutputSpec {
            width: Some(512),
            height: Some(512),
            count: Some(1),
            duration_seconds: Some(1.0),
        },
        legacy: LegacyContext {
            prompt: Some("legacy scene".to_owned()),
            ..LegacyContext::default()
        },
        diagnostics: Vec::<ContextDiagnostic>::new(),
        partial: false,
        resolver_identity: ResolverIdentity {
            context_hash: "context-hash".to_owned(),
            ..ResolverIdentity::default()
        },
    }
}

fn evaluate(
    context: &domain::ResolvedShotContext,
    report: Option<ComfyPreflightReport>,
    workspace: WorkflowProductionWorkspaceResponse,
    input: Option<ReadinessStageInput>,
) -> domain::shot_readiness::ShotReadiness {
    let environment = ReadinessEnvironmentSnapshot::new(report, workspace);
    evaluate_with_stage_input(context, &environment, input.as_ref(), now(), true)
}

fn shot_context(index: usize, prompt: &str, stage: ShotStage) -> domain::ResolvedShotContext {
    let mut context = context(prompt, stage);
    context.structure.shot.id = format!("shot-{index:03}");
    context.resolver_identity.context_hash = format!("hash-{index:03}");
    context
}

#[test]
fn evaluator_emits_exactly_seven_gates_and_ready_for_all_passes() {
    let result = evaluate(
        &context("a valid prompt", ShotStage::Image),
        Some(comfy(
            ComfyPreflightStatus::Ready,
            ComfyConnectionStatus::Connected,
        )),
        workspace(Some("T2I"), true),
        None,
    );
    assert_eq!(result.gates.len(), 7);
    assert_eq!(
        result.status,
        domain::shot_readiness::ShotReadinessStatus::Ready
    );
    assert_eq!(result.score, 100);
    assert_eq!(
        result
            .gates
            .iter()
            .map(|gate| gate.key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "CHARACTER",
            "SCENE",
            "REFERENCE",
            "PROMPT",
            "WORKFLOW",
            "OUTPUT",
            "COMFY_CAPABILITY"
        ]
    );
}

#[test]
fn warning_keeps_ready_but_incomplete_and_blocker_change_status() {
    let mut busy = comfy(
        ComfyPreflightStatus::Ready,
        ComfyConnectionStatus::Connected,
    );
    busy.runtime_busy = true;
    let warning = evaluate(
        &context("prompt", ShotStage::Image),
        Some(busy),
        workspace(Some("T2I"), true),
        None,
    );
    assert_eq!(
        warning.status,
        domain::shot_readiness::ShotReadinessStatus::Ready
    );
    assert_eq!(
        warning
            .gate(domain::shot_readiness::ReadinessGateKey::ComfyCapability)
            .unwrap()
            .state,
        domain::shot_readiness::ReadinessCheckState::Warning
    );

    let incomplete = evaluate(
        &context("", ShotStage::Image),
        None,
        workspace(Some("T2I"), true),
        None,
    );
    assert_eq!(
        incomplete.status,
        domain::shot_readiness::ShotReadinessStatus::Incomplete
    );

    let blocked = evaluate(
        &context("prompt", ShotStage::Image),
        Some(comfy(
            ComfyPreflightStatus::Blocked,
            ComfyConnectionStatus::Offline,
        )),
        workspace(Some("T2I"), true),
        None,
    );
    assert_eq!(
        blocked.status,
        domain::shot_readiness::ShotReadinessStatus::Blocked
    );
    assert!(blocked.score < incomplete.score);
}

#[test]
fn video_i2v_requires_selected_image_and_ref2va_requires_two_references() {
    let mut video = context("video prompt", ShotStage::Video);
    video.workflow.scalar_values = json!({"mode": "I2V"});
    video.workflow.workflow_version_id = Some("wv-video".to_owned());
    let mut workflow = workspace(Some("I2V"), true);
    workflow.items[0].workflow_version_id = Some("wv-video".to_owned());
    let missing = evaluate(
        &video,
        Some(comfy(
            ComfyPreflightStatus::Ready,
            ComfyConnectionStatus::Connected,
        )),
        workflow.clone(),
        Some(ReadinessStageInput::default()),
    );
    let reference = missing
        .gate(domain::shot_readiness::ReadinessGateKey::Reference)
        .unwrap();
    assert_eq!(
        reference.state,
        domain::shot_readiness::ReadinessCheckState::Incomplete
    );
    assert!(reference
        .checks
        .iter()
        .any(|check| check.code == "VIDEO_KEYFRAME_REQUIRED"));

    video.workflow.scalar_values = json!({"mode": "REF2VA"});
    let ref2va = evaluate(
        &video,
        Some(comfy(
            ComfyPreflightStatus::Ready,
            ComfyConnectionStatus::Connected,
        )),
        workspace(Some("REF2VA"), true),
        None,
    );
    assert!(ref2va
        .gate(domain::shot_readiness::ReadinessGateKey::Reference)
        .unwrap()
        .checks
        .iter()
        .any(|check| check.code == "REF2VA_REFERENCES_REQUIRED"));

    video.reference_assets.push(ResolvedReferenceAsset {
        asset_id: "ast_keyframe".to_owned(),
        sha256: "sha-keyframe".to_owned(),
        role: domain::BindingRole::ShotReference,
        ordinal: 0,
        source_reference_set_id: "legacy:shot".to_owned(),
        source_profile_id: None,
        source_scope: domain::ContextSourceScope::Legacy,
    });
    video.workflow.scalar_values = json!({"mode": "I2V"});
    let valid = evaluate(
        &video,
        Some(comfy(
            ComfyPreflightStatus::Ready,
            ComfyConnectionStatus::Connected,
        )),
        workspace(Some("I2V"), true),
        Some(ReadinessStageInput {
            selected_image_asset_id: Some("ast_keyframe".to_owned()),
            selected_image_sha256: Some("sha-keyframe".to_owned()),
        }),
    );
    assert!(!valid
        .gate(domain::shot_readiness::ReadinessGateKey::Reference)
        .unwrap()
        .checks
        .iter()
        .any(|check| check.code == "VIDEO_KEYFRAME_REQUIRED"));
}

#[test]
fn output_error_and_partial_context_are_never_silent() {
    let mut invalid = context("prompt", ShotStage::Image);
    invalid.output.width = Some(0);
    let result = evaluate(&invalid, None, workspace(Some("T2I"), true), None);
    assert_eq!(
        result
            .gate(domain::shot_readiness::ReadinessGateKey::Output)
            .unwrap()
            .state,
        domain::shot_readiness::ReadinessCheckState::Blocker
    );

    invalid.partial = true;
    let result = evaluate(&invalid, None, workspace(Some("T2I"), true), None);
    assert_eq!(
        result.status,
        domain::shot_readiness::ShotReadinessStatus::Blocked
    );
    assert!(result
        .gate(domain::shot_readiness::ReadinessGateKey::Prompt)
        .unwrap()
        .checks
        .iter()
        .any(|check| check.code == "CONTEXT_PARTIAL"));
}

#[test]
fn missing_comfy_nodes_block_the_selected_workflow() {
    let mut workflow = workspace(Some("T2I"), true);
    workflow.items[0]
        .capability_issues
        .push(CapabilityIssueView {
            code: "MISSING_NODE".to_owned(),
            class_type: Some("KSampler".to_owned()),
            message: "node is unavailable".to_owned(),
        });
    let result = evaluate(
        &context("prompt", ShotStage::Image),
        Some(comfy(
            ComfyPreflightStatus::Ready,
            ComfyConnectionStatus::Connected,
        )),
        workflow,
        None,
    );
    let gate = result
        .gate(domain::shot_readiness::ReadinessGateKey::ComfyCapability)
        .unwrap();
    assert_eq!(
        gate.state,
        domain::shot_readiness::ReadinessCheckState::Blocker
    );
    assert!(gate
        .checks
        .iter()
        .any(|check| check.code == "COMFY_MISSING_NODES"));
}

#[test]
fn command_and_service_are_read_only_camel_case_boundaries() {
    let command = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/commands/shot_readiness.rs"
    ))
    .unwrap();
    for name in [
        "shot_readiness_cached",
        "shot_preflight",
        "scene_readiness_cached",
        "scene_preflight",
    ] {
        assert!(command.contains(name), "missing command {name}");
    }
    assert!(command.contains("rename_all = \"camelCase\""));
    assert!(!command.contains("GenerationService"));
    assert!(!command.contains("ProductionQueueService"));
    assert!(!command.contains("TaskRepository"));

    let service = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/application/shot_readiness_service.rs"
    ))
    .unwrap();
    assert!(service.contains("READINESS_BATCH_LIMIT"));
    assert!(service.contains("preflight_many"));
    assert!(service.contains("scene_readiness_cached"));
    assert!(service.contains("scene_preflight"));
    assert!(service.contains("resolve_many_draft"));
    assert!(service.contains("list_workspace()"));
    assert!(service.contains("current()"));
    assert!(!service.contains("generation_service"));
    assert!(!service.contains("production_queue_service"));
}

#[test]
fn twenty_shot_summary_counts_ready_incomplete_and_blocked() {
    let ready_environment = ReadinessEnvironmentSnapshot::new(
        Some(comfy(
            ComfyPreflightStatus::Ready,
            ComfyConnectionStatus::Connected,
        )),
        workspace(Some("T2I"), true),
    );
    let blocked_environment = ReadinessEnvironmentSnapshot::new(
        Some(comfy(
            ComfyPreflightStatus::Blocked,
            ComfyConnectionStatus::Offline,
        )),
        workspace(Some("T2I"), true),
    );
    let mut counts = [0usize; 3];

    for index in 0..20 {
        let (context, environment) = match index {
            0..=11 => (
                shot_context(index, "ready prompt", ShotStage::Image),
                &ready_environment,
            ),
            12..=16 => (
                shot_context(index, "", ShotStage::Image),
                &ready_environment,
            ),
            _ => (
                shot_context(index, "blocked prompt", ShotStage::Image),
                &blocked_environment,
            ),
        };
        let readiness = evaluate_with_stage_input(&context, environment, None, now(), true);
        let bucket = match readiness.status {
            domain::shot_readiness::ShotReadinessStatus::Ready => 0,
            domain::shot_readiness::ShotReadinessStatus::Incomplete => 1,
            domain::shot_readiness::ShotReadinessStatus::Blocked => 2,
        };
        counts[bucket] += 1;
    }

    assert_eq!(counts, [12, 5, 3]);
}

#[test]
fn five_hundred_shots_reuse_one_environment_snapshot_without_comfy_refresh() {
    let environment = Arc::new(ReadinessEnvironmentSnapshot::new(
        Some(comfy(
            ComfyPreflightStatus::Ready,
            ComfyConnectionStatus::Connected,
        )),
        workspace(Some("T2I"), true),
    ));
    let snapshot_address = Arc::as_ptr(&environment);
    let checked_at = environment
        .comfy_report
        .as_ref()
        .map(|report| report.checked_at.clone());
    let results = (0..500)
        .map(|index| {
            assert_eq!(Arc::as_ptr(&environment), snapshot_address);
            evaluate_with_stage_input(
                &shot_context(index, "batch prompt", ShotStage::Image),
                environment.as_ref(),
                None,
                now(),
                true,
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(results.len(), 500);
    assert!(results
        .iter()
        .all(|result| { result.status == domain::shot_readiness::ShotReadinessStatus::Ready }));
    assert_eq!(Arc::strong_count(&environment), 1);
    assert_eq!(
        environment
            .comfy_report
            .as_ref()
            .map(|report| report.checked_at.clone()),
        checked_at
    );
}

#[test]
fn readiness_and_preflight_boundaries_never_dispatch_generation_work() {
    let command = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/commands/shot_readiness.rs"
    ))
    .unwrap();
    let service = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/application/shot_readiness_service.rs"
    ))
    .unwrap();

    for source in [&command, &service] {
        for forbidden in [
            "GenerationService",
            "ProductionQueueService",
            "TaskRepository",
            "enqueue_generation",
            "submit_generation",
            "create_task",
        ] {
            assert!(
                !source.contains(forbidden),
                "unexpected dispatch API: {forbidden}"
            );
        }
    }
}
