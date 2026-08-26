use crate::application::comfy_preflight_service::{ComfyPreflightReport, ComfyPreflightStatus};
use crate::application::comfy_service::ComfyConnectionStatus;
use crate::application::workflow_lifecycle_service::{
    WorkflowProductionWorkspaceResponse, WorkflowProductionWorkspaceView,
};
use crate::domain::shot_context::{
    ContextDiagnostic, ContextDiagnosticSeverity, ContextSourceScope, PromptSegmentKind,
    ResolvedShotContext,
};
use crate::domain::shot_readiness::{
    ReadinessCheck, ReadinessCheckState, ReadinessGateKey, ReadinessGateResult, ShotReadiness,
};
use crate::domain::ShotStage;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

pub use crate::domain::shot_context::ResolvedStageInput as ReadinessStageInput;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessEnvironmentSnapshot {
    pub comfy_report: Option<ComfyPreflightReport>,
    pub workflow_workspace: WorkflowProductionWorkspaceResponse,
}

impl ReadinessEnvironmentSnapshot {
    pub fn new(
        comfy_report: Option<ComfyPreflightReport>,
        workflow_workspace: WorkflowProductionWorkspaceResponse,
    ) -> Self {
        Self {
            comfy_report,
            workflow_workspace,
        }
    }
}

pub struct ReadinessEvaluationInput<'a> {
    pub context: &'a ResolvedShotContext,
    pub environment: &'a ReadinessEnvironmentSnapshot,
    pub stage_input: Option<&'a ReadinessStageInput>,
    pub evaluated_at: DateTime<Utc>,
    pub cached: bool,
}

pub fn evaluate(input: &ReadinessEvaluationInput<'_>) -> ShotReadiness {
    let context = input.context;
    let mode = workflow_mode(context, &input.environment.workflow_workspace);
    let gates = vec![
        character_gate(context),
        scene_gate(context),
        reference_gate(context, mode.as_deref(), input.stage_input),
        prompt_gate(context),
        workflow_gate(
            context,
            &input.environment.workflow_workspace,
            mode.as_deref(),
        ),
        output_gate(context),
        comfy_gate(context, input.environment, input.cached),
    ];
    let mut gates = gates;
    append_context_diagnostics(context, &mut gates);
    let comfy_checked_at = input
        .environment
        .comfy_report
        .as_ref()
        .and_then(|report| DateTime::parse_from_rfc3339(&report.checked_at).ok())
        .map(|value| value.with_timezone(&Utc));
    ShotReadiness::from_gates(
        context.project_id.clone(),
        context.structure.shot.id.clone(),
        context.stage.as_str(),
        context.resolver_identity.context_hash.clone(),
        gates,
        input.evaluated_at,
        comfy_checked_at,
        input.cached,
        context.partial,
    )
}

pub fn evaluate_readiness(
    context: &ResolvedShotContext,
    environment: &ReadinessEnvironmentSnapshot,
    evaluated_at: DateTime<Utc>,
    cached: bool,
) -> ShotReadiness {
    evaluate(&ReadinessEvaluationInput {
        context,
        environment,
        stage_input: Some(&context.stage_input),
        evaluated_at,
        cached,
    })
}

pub fn evaluate_with_stage_input(
    context: &ResolvedShotContext,
    environment: &ReadinessEnvironmentSnapshot,
    stage_input: Option<&ReadinessStageInput>,
    evaluated_at: DateTime<Utc>,
    cached: bool,
) -> ShotReadiness {
    evaluate(&ReadinessEvaluationInput {
        context,
        environment,
        stage_input,
        evaluated_at,
        cached,
    })
}

pub fn evaluate_context(
    context: &ResolvedShotContext,
    environment: &ReadinessEnvironmentSnapshot,
    stage_input: Option<&ReadinessStageInput>,
    evaluated_at: DateTime<Utc>,
    cached: bool,
) -> ShotReadiness {
    evaluate(&ReadinessEvaluationInput {
        context,
        environment,
        stage_input,
        evaluated_at,
        cached,
    })
}

fn check(
    key: ReadinessGateKey,
    state: ReadinessCheckState,
    code: &str,
    message: impl Into<String>,
    source: &str,
) -> ReadinessCheck {
    ReadinessCheck::new(key, state, code, message, source)
}

fn character_gate(context: &ResolvedShotContext) -> ReadinessGateResult {
    let key = ReadinessGateKey::Character;
    let mut checks = Vec::new();
    for diagnostic in context
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_gate(diagnostic) == Some(key))
    {
        checks.push(diagnostic_check(key, diagnostic));
    }
    if checks.is_empty() {
        if context.reference_pack.characters.is_empty() {
            checks.push(check(
                key,
                ReadinessCheckState::Pass,
                "NO_CHARACTER_INTENT",
                "镜头按无角色镜头处理。",
                "ResolvedShotContext",
            ));
        } else {
            checks.push(check(
                key,
                ReadinessCheckState::Pass,
                "CHARACTERS_RESOLVED",
                "角色与服装上下文已解析。",
                "ResolvedShotContext",
            ));
        }
    }
    ReadinessGateResult::new(key, checks)
}

fn scene_gate(context: &ResolvedShotContext) -> ReadinessGateResult {
    let key = ReadinessGateKey::Scene;
    let mut checks = context
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_gate(diagnostic) == Some(key))
        .map(|diagnostic| diagnostic_check(key, diagnostic))
        .collect::<Vec<_>>();
    if checks.is_empty() {
        let scene = context.reference_pack.scene.as_ref();
        let has_legacy = context
            .legacy
            .prompt
            .as_deref()
            .is_some_and(|prompt| !prompt.trim().is_empty());
        let has_prompt_scene = context.prompt_context.segments.iter().any(|segment| {
            segment.kind == PromptSegmentKind::Scene && !segment.text.trim().is_empty()
        });
        let has_semantic_scene = scene.is_some_and(|value| {
            value.profile_id.is_some()
                || !value.prompt.trim().is_empty()
                || value
                    .lighting_prompt
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
        });
        checks.push(if has_semantic_scene || has_legacy || has_prompt_scene {
            check(
                key,
                ReadinessCheckState::Pass,
                "SCENE_RESOLVED",
                "场景上下文已解析。",
                "ResolvedShotContext",
            )
        } else {
            check(
                key,
                ReadinessCheckState::Incomplete,
                "SCENE_CONTEXT_MISSING",
                "当前镜头缺少可用场景上下文。",
                "ResolvedShotContext",
            )
            .with_fix_action("选择或补充当前镜头的场景上下文")
        });
    }
    ReadinessGateResult::new(key, checks)
}

fn reference_gate(
    context: &ResolvedShotContext,
    mode: Option<&str>,
    stage_input: Option<&ReadinessStageInput>,
) -> ReadinessGateResult {
    let key = ReadinessGateKey::Reference;
    let mut checks = context
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_gate(diagnostic) == Some(key))
        .map(|diagnostic| diagnostic_check(key, diagnostic))
        .collect::<Vec<_>>();
    for reference_set in &context.reference_pack.reference_sets {
        let count = context
            .reference_assets
            .iter()
            .filter(|asset| asset.source_reference_set_id == reference_set.reference_set_id)
            .count();
        if reference_set.required && count == 0 {
            checks.push(
                check(
                    key,
                    ReadinessCheckState::Incomplete,
                    "REQUIRED_REFERENCE_SET_EMPTY",
                    "必需参考集解析后没有可用图片。",
                    "ResolvedShotContext",
                )
                .with_entity(reference_set.reference_set_id.clone())
                .with_fix_action("打开镜头参考并补充必需图片"),
            );
        }
    }

    let mode = mode.unwrap_or_default();
    if context.stage == ShotStage::Video && mode == "I2V" {
        match stage_input.and_then(|input| input.selected_image_asset_id.as_deref()) {
            None => checks.push(
                check(
                    key,
                    ReadinessCheckState::Incomplete,
                    "VIDEO_KEYFRAME_REQUIRED",
                    "视频 I2V 工作流需要已确认的图片关键帧。",
                    "ResolvedStageInput",
                )
                .with_fix_action("选择已确认图片作为视频关键帧"),
            ),
            Some(asset_id) => {
                let has_sha256 = stage_input
                    .and_then(|input| input.selected_image_sha256.as_deref())
                    .is_some_and(|sha256| !sha256.trim().is_empty());
                if !has_sha256 {
                    checks.push(
                        check(
                            key,
                            ReadinessCheckState::Blocker,
                            "VIDEO_KEYFRAME_SHA256_MISSING",
                            "已选择的图片关键帧缺少内容校验值。",
                            "ResolvedStageInput",
                        )
                        .with_entity(asset_id)
                        .with_fix_action("重新选择已确认的图片关键帧"),
                    );
                }
            }
        }
    }
    if context.stage == ShotStage::Video && mode == "REF2VA" {
        let count = context.reference_assets.len();
        if count < 2 {
            checks.push(
                check(
                    key,
                    ReadinessCheckState::Incomplete,
                    "REF2VA_REFERENCES_REQUIRED",
                    "REF2VA 视频工作流至少需要两张参考图片。",
                    "ResolvedShotContext",
                )
                .with_fix_action("为当前镜头补充至少两张参考图片"),
            );
        }
        if let Some(max) = reference_max(&context.workflow.scalar_values) {
            if count > max {
                checks.push(check(
                    key,
                    ReadinessCheckState::Blocker,
                    "REF2VA_REFERENCE_LIMIT_EXCEEDED",
                    format!("当前参考图片数量超过工作流允许的上限 {max}。"),
                    "WorkflowMetadata",
                ));
            }
        }
    }
    if checks.is_empty() {
        checks.push(check(
            key,
            ReadinessCheckState::Pass,
            "REFERENCES_RESOLVED",
            "参考集与参考图片上下文可用。",
            "ResolvedShotContext",
        ));
    }
    ReadinessGateResult::new(key, checks)
}

fn prompt_gate(context: &ResolvedShotContext) -> ReadinessGateResult {
    let key = ReadinessGateKey::Prompt;
    let mut checks = context
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_gate(diagnostic) == Some(key))
        .map(|diagnostic| diagnostic_check(key, diagnostic))
        .collect::<Vec<_>>();
    if context.prompt_context.rendered_text.trim().is_empty() {
        checks.push(
            check(
                key,
                ReadinessCheckState::Incomplete,
                "PROMPT_EMPTY",
                "当前镜头没有可用的最终提示词。",
                "PromptContext",
            )
            .with_fix_action("补充镜头动作、场景或其他提示词内容"),
        );
    }
    let missing_revision = context
        .profiles
        .characters
        .iter()
        .chain(context.profiles.props.iter())
        .any(|profile| profile.revision_id.is_none())
        || context
            .profiles
            .scene
            .as_ref()
            .is_some_and(|profile| profile.revision_id.is_none())
        || context
            .profiles
            .style
            .as_ref()
            .is_some_and(|profile| profile.revision_id.is_none());
    if missing_revision {
        checks.push(check(
            key,
            ReadinessCheckState::Warning,
            "PROFILE_REVISION_MISSING",
            "部分一致性 Profile 没有活动 revision，将使用当前 live 内容。",
            "ResolvedProfiles",
        ));
    }
    if checks.is_empty() {
        checks.push(check(
            key,
            ReadinessCheckState::Pass,
            "PROMPT_RESOLVED",
            "最终提示词上下文已生成。",
            "PromptContext",
        ));
    }
    ReadinessGateResult::new(key, checks)
}

fn workflow_gate(
    context: &ResolvedShotContext,
    workspace: &WorkflowProductionWorkspaceResponse,
    mode: Option<&str>,
) -> ReadinessGateResult {
    let key = ReadinessGateKey::Workflow;
    let mut checks = Vec::new();
    let version_id = context.workflow.workflow_version_id.as_deref();
    let recipe_id = context.workflow.recipe_id.as_deref();
    let Some(version_id) = version_id else {
        checks.push(
            check(
                key,
                ReadinessCheckState::Incomplete,
                "WORKFLOW_VERSION_MISSING",
                "当前镜头尚未选择生产工作流。",
                "ResolvedWorkflowContext",
            )
            .with_fix_action("选择此阶段的 ComfyUI Workflow / Recipe"),
        );
        return ReadinessGateResult::new(key, checks);
    };
    if recipe_id.is_none() {
        checks.push(
            check(
                key,
                ReadinessCheckState::Incomplete,
                "WORKFLOW_RECIPE_MISSING",
                "当前镜头尚未选择工作流 Recipe。",
                "ResolvedWorkflowContext",
            )
            .with_fix_action("选择此阶段的 ComfyUI Workflow / Recipe"),
        );
    }
    let Some(item) = workspace
        .items
        .iter()
        .find(|item| item.workflow_version_id.as_deref() == Some(version_id))
    else {
        checks.push(check(
            key,
            ReadinessCheckState::Blocker,
            "WORKFLOW_VERSION_NOT_FOUND",
            "所选工作流版本不在当前生产工作区中。",
            "WorkflowWorkspace",
        ));
        return ReadinessGateResult::new(key, checks);
    };
    if item.archived || !item.enabled {
        checks.push(check(
            key,
            ReadinessCheckState::Blocker,
            if item.archived {
                "WORKFLOW_ARCHIVED"
            } else {
                "WORKFLOW_DISABLED"
            },
            if item.archived {
                "所选工作流版本已归档。"
            } else {
                "所选工作流版本已停用。"
            },
            "WorkflowWorkspace",
        ));
    }
    if item.package_status != "VALID" {
        checks.push(check(
            key,
            ReadinessCheckState::Blocker,
            "WORKFLOW_PACKAGE_INVALID",
            "所选工作流运行包校验未通过。",
            "WorkflowWorkspace",
        ));
    }
    if item.readiness == "BLOCKED" {
        checks.push(check(
            key,
            ReadinessCheckState::Blocker,
            "WORKFLOW_BLOCKED",
            "所选工作流尚未达到生产就绪状态。",
            "WorkflowWorkspace",
        ));
    } else if item.readiness == "DEGRADED" || item.readiness == "NOT_CHECKED" {
        checks.push(check(
            key,
            ReadinessCheckState::Warning,
            "WORKFLOW_NOT_VERIFIED",
            "所选工作流尚未完成当前环境的完整验证。",
            "WorkflowWorkspace",
        ));
    }
    if let Some(recipe_id) = recipe_id {
        if !item
            .recipes
            .iter()
            .any(|recipe| recipe.recipe_id == recipe_id)
        {
            checks.push(check(
                key,
                ReadinessCheckState::Blocker,
                "WORKFLOW_RECIPE_NOT_FOUND",
                "所选 Recipe 不存在于当前工作流版本中。",
                "WorkflowWorkspace",
            ));
        }
    }
    if let Some(mode) = mode {
        if !is_known_workflow_mode(mode) {
            checks.push(check(
                key,
                ReadinessCheckState::Warning,
                "WORKFLOW_MODE_UNKNOWN",
                "当前工作流模式无法从结构化元数据可靠判断。",
                "WorkflowMetadata",
            ));
        } else if !mode_is_compatible(context.stage, mode, item) {
            checks.push(check(
                key,
                ReadinessCheckState::Blocker,
                "WORKFLOW_STAGE_INCOMPATIBLE",
                "所选工作流模式与当前镜头阶段不兼容。",
                "WorkflowMetadata",
            ));
        }
    } else {
        checks.push(check(
            key,
            ReadinessCheckState::Warning,
            "WORKFLOW_MODE_UNKNOWN",
            "当前工作流模式无法从结构化元数据可靠判断。",
            "WorkflowMetadata",
        ));
    }
    if checks.is_empty() {
        checks.push(check(
            key,
            ReadinessCheckState::Pass,
            "WORKFLOW_READY",
            "工作流、Recipe 与镜头阶段匹配。",
            "WorkflowWorkspace",
        ));
    }
    ReadinessGateResult::new(key, checks)
}

fn output_gate(context: &ResolvedShotContext) -> ReadinessGateResult {
    let key = ReadinessGateKey::Output;
    let mut checks = Vec::new();
    for (name, value) in [
        ("width", context.output.width.map(|value| value as f64)),
        ("height", context.output.height.map(|value| value as f64)),
        ("count", context.output.count.map(|value| value as f64)),
        ("duration", context.output.duration_seconds),
    ] {
        if value.is_some_and(|value| value <= 0.0) {
            checks.push(check(
                key,
                ReadinessCheckState::Blocker,
                "OUTPUT_VALUE_INVALID",
                format!("输出参数 {name} 必须大于零。"),
                "ResolvedOutputSpec",
            ));
        }
    }
    if checks.is_empty()
        && (context.output.width.is_none()
            || context.output.height.is_none()
            || context.output.count.is_none()
            || context.output.duration_seconds.is_none())
    {
        checks.push(check(
            key,
            ReadinessCheckState::Warning,
            "OUTPUT_DEFAULT_REQUIRED",
            "当前镜头未显式设置，生产时将依赖 Recipe 默认值。",
            "ResolvedOutputSpec",
        ));
    }
    if checks.is_empty() {
        checks.push(check(
            key,
            ReadinessCheckState::Pass,
            "OUTPUT_RESOLVED",
            "输出规格有效。",
            "ResolvedOutputSpec",
        ));
    }
    ReadinessGateResult::new(key, checks)
}

fn comfy_gate(
    context: &ResolvedShotContext,
    environment: &ReadinessEnvironmentSnapshot,
    cached: bool,
) -> ReadinessGateResult {
    let key = ReadinessGateKey::ComfyCapability;
    let Some(report) = environment.comfy_report.as_ref() else {
        return ReadinessGateResult::new(
            key,
            vec![check(
                key,
                ReadinessCheckState::Incomplete,
                "COMFY_PREFLIGHT_NOT_RUN",
                "尚未执行 ComfyUI 环境预检。",
                "ReadinessEnvironmentSnapshot",
            )
            .with_fix_action("执行 ComfyUI 环境预检")],
        );
    };
    let mut checks = Vec::new();
    if !matches!(report.connection, ComfyConnectionStatus::Connected) {
        checks.push(
            check(
                key,
                ReadinessCheckState::Blocker,
                "COMFY_CONNECTION_UNAVAILABLE",
                "当前 ComfyUI 无法连接或 API 不兼容。",
                "ComfyPreflightReport",
            )
            .with_fix_action("启动或修复当前 ComfyUI 环境"),
        );
    }
    let selected = context.workflow.workflow_version_id.as_deref();
    let workspace_item = selected.and_then(|id| {
        environment
            .workflow_workspace
            .items
            .iter()
            .find(|item| item.workflow_version_id.as_deref() == Some(id))
    });
    let summary_item = selected.and_then(|id| {
        report
            .workflow_summary
            .items
            .iter()
            .find(|item| item.workflow_version_id.as_deref() == Some(id))
    });
    let mut missing_nodes = Vec::new();
    if let Some(item) = workspace_item {
        for issue in &item.capability_issues {
            if issue.code == "MISSING_NODE" {
                if let Some(class_type) = &issue.class_type {
                    missing_nodes.push(class_type.clone());
                }
            } else if issue.code == "INCOMPATIBLE_INPUT_VALUES" {
                checks.push(check(
                    key,
                    ReadinessCheckState::Blocker,
                    "COMFY_INCOMPATIBLE_INPUT",
                    issue.message.clone(),
                    "ComfyPreflightReport",
                ));
            }
        }
    }
    if let Some(item) = summary_item {
        missing_nodes.extend(item.missing_nodes.iter().cloned());
    }
    missing_nodes.sort();
    missing_nodes.dedup();
    if !missing_nodes.is_empty() {
        checks.push(
            check(
                key,
                ReadinessCheckState::Blocker,
                "COMFY_MISSING_NODES",
                format!(
                    "当前工作流缺少 ComfyUI 节点：{}。",
                    missing_nodes.join(", ")
                ),
                "ComfyPreflightReport",
            )
            .with_fix_action("安装缺失节点后重新预检"),
        );
    }
    if report.runtime_busy {
        checks.push(check(
            key,
            ReadinessCheckState::Warning,
            "COMFY_RUNTIME_BUSY",
            "ComfyUI 当前正在处理其他任务。",
            "ComfyPreflightReport",
        ));
    }
    if report.status == ComfyPreflightStatus::Blocked {
        checks.push(check(
            key,
            ReadinessCheckState::Blocker,
            "COMFY_CAPABILITY_BLOCKED",
            "当前 ComfyUI 没有可用于生产的能力。",
            "ComfyPreflightReport",
        ));
    } else if report.status == ComfyPreflightStatus::Warning && checks.is_empty() {
        checks.push(check(
            key,
            ReadinessCheckState::Warning,
            "COMFY_CAPABILITY_WARNING",
            "ComfyUI 预检存在与当前镜头无关的全局警告。",
            "ComfyPreflightReport",
        ));
    }
    if checks.is_empty() {
        checks.push(check(
            key,
            ReadinessCheckState::Pass,
            if cached {
                "COMFY_CAPABILITY_CACHED"
            } else {
                "COMFY_CAPABILITY_READY"
            },
            "当前 ComfyUI 能力满足预检要求。",
            "ComfyPreflightReport",
        ));
    }
    ReadinessGateResult::new(key, checks)
}

fn append_context_diagnostics(context: &ResolvedShotContext, gates: &mut [ReadinessGateResult]) {
    if !context.partial
        && !context.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == ContextDiagnosticSeverity::Error
                && diagnostic_gate(diagnostic).is_none()
        })
    {
        return;
    }
    let key = ReadinessGateKey::Prompt;
    let mut checks = Vec::new();
    if context.partial {
        checks.push(check(
            key,
            ReadinessCheckState::Blocker,
            "CONTEXT_PARTIAL",
            "镜头解析上下文不完整，需先修复解析错误。",
            "ResolvedShotContext",
        ));
    }
    for diagnostic in context.diagnostics.iter().filter(|diagnostic| {
        diagnostic.severity == ContextDiagnosticSeverity::Error
            && diagnostic_gate(diagnostic).is_none()
    }) {
        checks.push(diagnostic_check(key, diagnostic));
    }
    if !checks.is_empty() {
        if let Some(gate) = gates.iter_mut().find(|gate| gate.key == key) {
            gate.checks.extend(checks);
            gate.state = gate
                .checks
                .iter()
                .map(|check| check.state)
                .max_by_key(|state| state.severity())
                .unwrap_or(ReadinessCheckState::Pass);
        }
    }
}

fn diagnostic_gate(diagnostic: &ContextDiagnostic) -> Option<ReadinessGateKey> {
    let code = diagnostic.code.to_ascii_uppercase();
    if code.contains("COSTUME") || code.contains("CHARACTER") {
        return Some(ReadinessGateKey::Character);
    }
    if code.contains("SELECTED_IMAGE")
        || code.contains("IMAGE_REQUIRED")
        || code == "PROJECT_MISMATCH"
        || code == "TYPE_INVALID"
        || code.contains("NON_IMAGE")
        || code.contains("IMAGE_NOT_FOUND")
    {
        return Some(ReadinessGateKey::Reference);
    }
    if let Some(scope) = diagnostic.scope {
        return Some(match scope {
            ContextSourceScope::Scene => ReadinessGateKey::Scene,
            ContextSourceScope::Shot => ReadinessGateKey::Prompt,
            ContextSourceScope::Project
            | ContextSourceScope::Series
            | ContextSourceScope::Episode
            | ContextSourceScope::Legacy => ReadinessGateKey::Reference,
        });
    }
    if code.contains("REFERENCE") || code.contains("ASSET") {
        Some(ReadinessGateKey::Reference)
    } else if code.contains("SCENE") {
        Some(ReadinessGateKey::Scene)
    } else if code.contains("PROFILE") {
        Some(ReadinessGateKey::Prompt)
    } else if code.contains("PROMPT") || code.contains("REVISION") {
        Some(ReadinessGateKey::Prompt)
    } else {
        None
    }
}

fn diagnostic_check(key: ReadinessGateKey, diagnostic: &ContextDiagnostic) -> ReadinessCheck {
    let state = match diagnostic.severity {
        ContextDiagnosticSeverity::Warning => ReadinessCheckState::Warning,
        ContextDiagnosticSeverity::Error => ReadinessCheckState::Blocker,
    };
    let mut result = check(
        key,
        state,
        &diagnostic.code,
        diagnostic.message.clone(),
        "ResolvedShotContext",
    );
    if let Some(entity_id) = &diagnostic.entity_id {
        result.entity_ids.push(entity_id.clone());
    }
    result
}

fn workflow_mode(
    context: &ResolvedShotContext,
    workspace: &WorkflowProductionWorkspaceResponse,
) -> Option<String> {
    let workspace_mode = context
        .workflow
        .workflow_version_id
        .as_deref()
        .and_then(|version_id| {
            workspace
                .items
                .iter()
                .find(|item| item.workflow_version_id.as_deref() == Some(version_id))
        })
        .and_then(|item| item.mode.as_deref());
    workspace_mode
        .or_else(|| {
            ["mode", "workflow_mode", "workflowMode"]
                .iter()
                .find_map(|key| {
                    context
                        .workflow
                        .scalar_values
                        .get(*key)
                        .and_then(Value::as_str)
                })
        })
        .map(|value| value.to_ascii_uppercase())
}

fn reference_max(value: &Value) -> Option<usize> {
    [
        "max_reference_images",
        "max_references",
        "reference_max",
        "maxReferenceImages",
    ]
    .iter()
    .find_map(|key| value.get(*key).and_then(Value::as_u64))
    .map(|value| value as usize)
}

fn mode_is_compatible(
    stage: ShotStage,
    mode: &str,
    item: &WorkflowProductionWorkspaceView,
) -> bool {
    let mode = mode.to_ascii_uppercase();
    let category = item
        .category
        .as_deref()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let declared_video =
        category == "VIDEO" || matches!(mode.as_str(), "I2V" | "T2V" | "REF2VA" | "VIDEO");
    let declared_image = category == "IMAGE"
        || matches!(
            mode.as_str(),
            "T2I" | "I2I" | "TXT2IMG" | "IMG2IMG" | "IMAGE"
        );
    if declared_video && declared_image {
        return false;
    }
    match stage {
        ShotStage::Image => !declared_video,
        ShotStage::Video => !declared_image,
    }
}

fn is_known_workflow_mode(mode: &str) -> bool {
    matches!(
        mode,
        "I2V" | "T2V" | "REF2VA" | "VIDEO" | "T2I" | "I2I" | "TXT2IMG" | "IMG2IMG" | "IMAGE"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::shot_context::{
        LegacyContext, PromptContext, PromptSegment, ResolvedProfiles, ResolvedStageInput,
        ResolvedStructure, ResolvedWorkflowContext, ResolverIdentity, ShotReferencePack,
    };
    use crate::domain::shot_readiness::{ReadinessCheckState, ReadinessGateKey};

    fn context(stage: ShotStage) -> ResolvedShotContext {
        ResolvedShotContext {
            project_id: "project".to_owned(),
            structure: ResolvedStructure::default(),
            stage,
            stage_input: ResolvedStageInput::default(),
            reference_pack: ShotReferencePack {
                shot_id: "shot".to_owned(),
                ..ShotReferencePack::default()
            },
            profiles: ResolvedProfiles::default(),
            reference_assets: Vec::new(),
            prompt_context: PromptContext::default(),
            workflow: ResolvedWorkflowContext::default(),
            output: Default::default(),
            legacy: LegacyContext::default(),
            diagnostics: Vec::new(),
            partial: false,
            resolver_identity: ResolverIdentity::default(),
        }
    }

    #[test]
    fn state_helpers_keep_the_frozen_order() {
        assert_eq!(ReadinessGateKey::ALL.len(), 7);
        assert!(
            ReadinessCheckState::Blocker.severity() > ReadinessCheckState::Incomplete.severity()
        );
    }

    #[test]
    fn selected_i2v_image_does_not_need_to_be_a_reference_asset() {
        let mut context = context(ShotStage::Video);
        context.stage_input = ResolvedStageInput {
            selected_image_asset_id: Some("asset-selected".to_owned()),
            selected_image_sha256: Some("sha256".to_owned()),
        };
        let gate = reference_gate(&context, Some("I2V"), Some(&context.stage_input));
        assert_eq!(gate.state, ReadinessCheckState::Pass);
    }

    #[test]
    fn legacy_scene_description_in_prompt_segments_is_usable() {
        let mut context = context(ShotStage::Image);
        context.prompt_context.segments.push(PromptSegment {
            kind: PromptSegmentKind::Scene,
            text: "旧场景描述".to_owned(),
            source_scope: ContextSourceScope::Scene,
            source_entity_id: "scene".to_owned(),
            revision_id: None,
            omitted_reason: None,
        });
        assert_eq!(scene_gate(&context).state, ReadinessCheckState::Pass);
    }
}
