//! DEV-052 Agent C contract tests.
//!
//! The core preparation service is intentionally owned by Agent B. These
//! tests keep the C boundary verifiable while that service is integrated by
//! Main: command inputs are strict and client-supplied frozen values cannot
//! cross the boundary; old scene/episode/series commands remain present; and
//! the compatibility summaries expose the readiness aggregate fields.

use std::{
    fs,
    path::{Path, PathBuf},
};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn repo_root() -> PathBuf {
    Path::new(ROOT)
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf()
}

fn read_repo(path: &str) -> String {
    fs::read_to_string(repo_root().join(path)).expect("DEV-052 source should be readable")
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing section start: {start}"));
    let rest = &source[start_index..];
    let end_index = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing section end: {end}"));
    &rest[..end_index]
}

fn assert_contains_all(source: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            source.contains(needle),
            "missing contract fragment: {needle}"
        );
    }
}

fn assert_contains_none(source: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            !source.contains(needle),
            "forbidden contract fragment: {needle}"
        );
    }
}

fn assert_occurrences(source: &str, needle: &str, expected: usize) {
    assert_eq!(
        source.matches(needle).count(),
        expected,
        "unexpected occurrence count for {needle}"
    );
}

#[test]
fn preparation_commands_use_strict_camel_case_dtos_and_keep_old_commands() {
    let preparation = read_repo("src-tauri/src/commands/production_preparation.rs");
    for command in [
        "scene_production_preflight",
        "scene_production_admit",
        "shot_production_plan_detail",
    ] {
        assert!(preparation.contains(command), "missing command {command}");
    }
    assert!(preparation.contains("rename_all = \"camelCase\", deny_unknown_fields"));

    for legacy in [
        "pub async fn scene_production_plan",
        "pub async fn scene_production_prepare",
        "pub async fn episode_production_plan",
        "pub async fn episode_production_prepare",
        "pub async fn series_production_plan",
        "pub async fn series_production_prepare",
    ] {
        let source = match legacy.split_whitespace().nth(3) {
            Some(name) => match name.split('(').next() {
                Some(name) => name,
                None => continue,
            },
            None => continue,
        };
        let path = if source.starts_with("scene_") {
            "src-tauri/src/commands/scene_production.rs"
        } else if source.starts_with("episode_") {
            "src-tauri/src/commands/episode_production.rs"
        } else {
            "src-tauri/src/commands/series_production.rs"
        };
        assert!(
            read_repo(path).contains(legacy),
            "legacy command missing: {legacy}"
        );
    }
}

#[test]
fn admission_dto_contains_only_server_authoritative_selection_inputs() {
    let preparation = read_repo("src-tauri/src/commands/production_preparation.rs");
    let start = preparation
        .find("pub struct SceneProductionAdmitRequest")
        .expect("admission request should exist");
    let request = &preparation[start..]
        .split_once("}\n")
        .map(|(body, _)| body)
        .unwrap_or_default();
    for field in [
        "project_id",
        "scene_id",
        "stage",
        "shot_ids",
        "allow_partial",
    ] {
        assert!(
            request.contains(&format!("pub {field}")),
            "missing field {field}"
        );
    }
    for forbidden in [
        "prompt",
        "context_hash",
        "values",
        "readiness",
        "snapshot",
        "workflow_version_id",
        "recipe_id",
    ] {
        assert!(
            !request.contains(forbidden),
            "client admission DTO must not accept {forbidden}"
        );
    }

    for forbidden_call in [
        "production_queue_service.start",
        "generation_service",
        "submit_prompt",
        "start_generation",
    ] {
        assert!(
            !preparation.contains(forbidden_call),
            "preparation command must not call {forbidden_call}"
        );
    }
}

#[test]
fn readiness_aware_scene_episode_series_summaries_expose_compatibility_fields() {
    for path in [
        "src-tauri/src/application/scene_production_service.rs",
        "src-tauri/src/application/episode_production_service.rs",
        "src-tauri/src/application/series_production_service.rs",
    ] {
        let source = read_repo(path);
        for field in [
            "pub total:",
            "pub ready:",
            "pub incomplete:",
            "pub blocked:",
            "pub prepared:",
            "pub done:",
            "pub existing_batch_ids:",
            "pub evaluated_at:",
        ] {
            assert!(source.contains(field), "{path} lacks {field}");
        }
        assert!(source.contains("ShotReadinessService"));
        assert!(source.contains("readiness_summary"));
    }
}

#[test]
fn preparation_commands_match_the_landed_domain_contract() {
    let domain = read_repo("src-tauri/src/domain/production_preparation.rs");
    let commands = read_repo("src-tauri/src/commands/production_preparation.rs");
    for domain_type in [
        "pub struct ShotProductionPlan",
        "pub struct ScenePreparationView",
        "pub struct ProductionPreparationAdmission",
    ] {
        assert!(
            domain.contains(domain_type),
            "missing Agent B type {domain_type}"
        );
    }
    for imported_type in [
        "ProductionPreparationAdmission",
        "ScenePreparationView",
        "ShotProductionPlan",
    ] {
        assert!(
            commands.contains(imported_type),
            "missing command type {imported_type}"
        );
    }
    assert!(commands.contains("ProductionPreparationService"));
    assert!(commands.contains(".production_structure_service"));
    assert!(commands.contains(".plan_many("));
    assert!(commands.contains(".admit("));
    assert!(commands.contains(".plan_detail("));
}

#[test]
fn scene_preflight_is_read_only_and_does_not_start_work() {
    let source = read_repo("src-tauri/src/commands/production_preparation.rs");
    let preflight = section(
        &source,
        "pub async fn scene_production_preflight",
        "/// Explicit admission",
    );
    assert_contains_all(
        preflight,
        &[
            "scene_scope(",
            ".plan_many(",
            "ProductionPreparationService::scene_view",
        ],
    );
    assert_contains_none(
        preflight,
        &[
            ".admit(",
            "insert_prepared_batch_with_bindings",
            "production_queue_service",
            ".start(",
            "start_generation",
            "create_task",
        ],
    );
}

#[test]
fn scene_preflight_uses_project_scoped_scene_membership() {
    let source = read_repo("src-tauri/src/commands/production_preparation.rs");
    let scope = section(
        &source,
        "async fn scene_scope",
        "fn validate_scene_shot_ids",
    );
    assert_contains_all(
        scope,
        &[
            "production_structure_service",
            ".tree(project_id)",
            "scene.scene.id",
            "scene.scene.name",
            "SCENE_NOT_FOUND",
        ],
    );
}

#[test]
fn scene_preflight_projects_bulk_plans_into_the_wire_view() {
    let source = read_repo("src-tauri/src/commands/production_preparation.rs");
    assert_contains_all(
        &source,
        &[
            "Result<ScenePreparationView, AppError>",
            "ShotProductionPlanSummary",
            "ShotProductionPlanSummary::from",
            "ProductionPreparationService::scene_view",
            "Utc::now()",
        ],
    );
}

#[test]
fn scene_admission_rejects_empty_duplicate_or_cross_scene_selection() {
    let source = read_repo("src-tauri/src/commands/production_preparation.rs");
    let validation = section(&source, "fn validate_scene_shot_ids", "fn parse_stage");
    assert_contains_all(
        validation,
        &[
            "shot_ids.is_empty()",
            "shot_ids.len() > 500",
            "HashSet::with_capacity",
            "镜头不能重复",
            "SHOT_NOT_IN_SCENE",
        ],
    );
}

#[test]
fn scene_admission_re_resolves_and_live_preflights_before_persistence() {
    let preparation = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let admit = section(
        &preparation,
        "pub async fn admit",
        "/// Read one frozen snapshot",
    );
    let evaluation = section(
        &preparation,
        "async fn evaluate_many",
        "fn prepare_generation_values",
    );
    assert_contains_all(
        admit,
        &[
            "evaluate_many(",
            "allow_partial",
            "insert_prepared_batch_with_bindings",
        ],
    );
    assert_contains_all(
        evaluation,
        &[
            "preflight_bundle_many(",
            "list_active_shot_bindings",
            "list_prepared_shot_records",
            "current_stage_statuses",
        ],
    );
}

#[test]
fn scene_admission_never_starts_queue_or_creates_tasks() {
    let source = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let admit = section(
        &source,
        "pub async fn admit",
        "/// Read one frozen snapshot",
    );
    assert_contains_none(
        admit,
        &[
            "production_queue_service.start",
            ".start(",
            "submit_prompt",
            "create_task",
            "TaskId::new",
        ],
    );
}

#[test]
fn scene_view_counts_ready_incomplete_blocked_without_reclassifying_them() {
    let source = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let view = section(
        &source,
        "pub fn scene_view",
        "/// Re-resolves and live-preflights",
    );
    assert_contains_all(
        view,
        &[
            "ShotReadinessStatus::Ready",
            "ShotReadinessStatus::Incomplete",
            "ShotReadinessStatus::Blocked",
            "ready_count",
            "incomplete_count",
            "blocked_count",
        ],
    );
}

#[test]
fn warning_checks_lower_score_but_keep_a_ready_status() {
    let source = read_repo("src-tauri/src/domain/shot_readiness.rs");
    let readiness = section(&source, "pub fn from_gates", "pub fn gate");
    assert_contains_all(
        readiness,
        &[
            "let mut status = ShotReadinessStatus::Ready",
            "ReadinessCheckState::Warning => score -= 5",
            "status = ShotReadinessStatus::Blocked",
            "status = ShotReadinessStatus::Incomplete",
        ],
    );
}

#[test]
fn non_partial_admission_rejects_incomplete_and_blocked_shots() {
    let source = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let admit = section(
        &source,
        "pub async fn admit",
        "/// Read one frozen snapshot",
    );
    assert_contains_all(
        admit,
        &[
            "skipped_incomplete",
            "skipped_blocked",
            "!allow_partial",
            "ProductionPreparationError::NotReady",
            "ShotReadinessStatus::Incomplete",
            "ShotReadinessStatus::Blocked",
        ],
    );
}

#[test]
fn allow_partial_admission_reports_skips_and_keeps_ready_items() {
    let source = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let admit = section(
        &source,
        "pub async fn admit",
        "/// Read one frozen snapshot",
    );
    assert_contains_all(
        admit,
        &[
            "if item.readiness.status != ShotReadinessStatus::Ready",
            "continue",
            "ready.push(item)",
            "skipped_incomplete",
            "skipped_blocked",
        ],
    );
}

#[test]
fn matching_contexts_are_idempotent_and_return_existing_batch_ids() {
    let source = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let evaluation = section(
        &source,
        "async fn evaluate_many",
        "fn prepare_generation_values",
    );
    let admit = section(
        &source,
        "pub async fn admit",
        "/// Read one frozen snapshot",
    );
    assert_contains_all(
        evaluation,
        &[
            "record.context_hash == context.resolver_identity.context_hash",
            "matching_prepared_batch_ids",
            "snapshot_identity",
        ],
    );
    assert_contains_all(
        admit,
        &[
            "already_prepared_count",
            "matching_prepared_batch_ids",
            "if ready.is_empty()",
        ],
    );
}

#[test]
fn changed_contexts_are_exposed_as_stale_preparations() {
    let source = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let evaluation = section(
        &source,
        "async fn evaluate_many",
        "fn prepare_generation_values",
    );
    let plan = section(&source, "fn to_plan", "impl From<&ShotProductionPlan>");
    assert_contains_all(
        evaluation,
        &[
            "record.context_hash != context.resolver_identity.context_hash",
            "stale_prepared_batch_ids",
        ],
    );
    assert_contains_all(plan, &["stale_prepared_batch_ids", "已有旧上下文准备版本"]);
}

#[test]
fn preparation_snapshot_freezes_context_values_and_runtime_evidence() {
    let domain = read_repo("src-tauri/src/domain/production_preparation.rs");
    let service = read_repo("src-tauri/src/application/production_preparation_service.rs");
    assert_contains_all(
        &domain,
        &[
            "pub struct PreparationSnapshotV1",
            "schema_version: u32",
            "context_hash: String",
            "reference_sets: Vec<ResolvedReferenceSet>",
            "reference_assets: Vec<ResolvedReferenceAsset>",
            "prompt: PreparationSnapshotPrompt",
            "workflow: PreparationSnapshotWorkflow",
            "output_spec: ResolvedOutputSpec",
            "stage_input: ResolvedStageInput",
            "frozen_generation_values: Value",
            "readiness: PreparationSnapshotReadiness",
            "comfy_capability_evidence: ComfyCapabilityEvidence",
        ],
    );
    assert_contains_all(
        &service,
        &["PreparationSnapshotV1::from_context", "values_json"],
    );
}

#[test]
fn admission_writes_batch_items_bindings_and_snapshots_as_one_unit() {
    let source = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let admit = section(
        &source,
        "pub async fn admit",
        "/// Read one frozen snapshot",
    );
    assert_contains_all(
        admit,
        &[
            "let mut items = Vec::with_capacity",
            "let mut bindings = Vec::with_capacity",
            "let mut snapshots = Vec::with_capacity",
            "insert_prepared_batch_with_bindings(&batch, &items, &bindings, &snapshots)",
        ],
    );
}

#[test]
fn prepared_repository_requires_one_binding_and_snapshot_per_item() {
    let source =
        read_repo("src-tauri/src/infrastructure/database/repositories/production_queue.rs");
    let validation = section(
        &source,
        "fn validate_prepared_insert",
        "async fn validate_shot_batch_bindings",
    );
    assert_contains_all(
        validation,
        &[
            "snapshots.len() != items.len() || bindings.len() != items.len()",
            "binding_item_ids",
            "snapshot_item_ids",
            "each prepared Shot batch item must be bound exactly once",
            "prepared snapshot identity must match its batch and item binding",
        ],
    );
}

#[test]
fn prepared_repository_commits_batch_bindings_and_snapshot_together() {
    let source =
        read_repo("src-tauri/src/infrastructure/database/repositories/production_queue.rs");
    let insert = section(
        &source,
        "async fn insert_prepared_batch_with_bindings",
        "async fn list_prepared_shot_records",
    );
    assert_contains_all(
        insert,
        &[
            "let mut transaction = self.pool.begin()",
            "insert_batch_records(&mut transaction, batch, items)",
            "insert_shot_batch_bindings(&mut transaction, batch, bindings)",
            "for snapshot in snapshots",
            "INSERT INTO production_preparation_snapshots",
            "transaction.commit()",
        ],
    );
    assert!(
        insert.find("for snapshot in snapshots") < insert.find("transaction.commit()"),
        "snapshot inserts must precede the transaction commit"
    );
}

#[test]
fn snapshot_reads_are_project_and_item_scoped_and_validate_identity() {
    let service = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let repository =
        read_repo("src-tauri/src/infrastructure/database/repositories/production_queue.rs");
    let read = section(
        &service,
        "pub async fn preparation_snapshot",
        "async fn evaluate_many",
    );
    let query = section(
        &repository,
        "async fn find_preparation_snapshot",
        "async fn insert_batch_with_bindings",
    );
    let row = section(&repository, "impl PreparationSnapshotRow", "#[cfg(test)]");
    assert_contains_all(read, &["validate_project", "find_preparation_snapshot"]);
    assert_contains_all(
        query,
        &[
            "WHERE sps.project_id = ? AND b.project_id = ?",
            "sps.production_batch_item_id = ?",
        ],
    );
    assert_contains_all(
        row,
        &[
            "snapshot.schema_version !=",
            "snapshot.project_id != self.project_id",
            "snapshot.shot_id != self.shot_id",
            "snapshot.stage != self.stage",
            "snapshot.context_hash != self.context_hash",
        ],
    );
}

#[test]
fn image_stage_has_no_video_selected_image_input() {
    let source = read_repo("src-tauri/src/application/shot_context_resolver.rs");
    let stage_input = section(&source, "fn resolve_stage_input", "fn stage_prompt");
    assert_contains_all(
        stage_input,
        &[
            "if loaded.stage != ShotStage::Video",
            "ResolvedStageInput::default()",
            "data.shot.selected_image_asset_id",
        ],
    );
}

#[test]
fn video_stage_freezes_selected_image_id_and_sha256_for_i2v() {
    let resolver = read_repo("src-tauri/src/application/shot_context_resolver.rs");
    let evaluator = read_repo("src-tauri/src/application/shot_readiness_evaluator.rs");
    assert_contains_all(
        &resolver,
        &[
            "selected_image_asset_id: Some(asset_id.to_owned())",
            "selected_image_sha256: Some(asset.sha256.clone())",
            "selected_image_asset_id: stage_input.selected_image_asset_id.clone()",
            "selected_image_sha256: stage_input.selected_image_sha256.clone()",
            "CONTEXT_SELECTED_IMAGE_NOT_FOUND",
            "CONTEXT_SELECTED_IMAGE_PROJECT_MISMATCH",
            "CONTEXT_SELECTED_IMAGE_TYPE_INVALID",
        ],
    );
    assert_contains_all(
        &evaluator,
        &[
            "mode == \"I2V\"",
            "VIDEO_KEYFRAME_REQUIRED",
            "selected_image_sha256",
        ],
    );
}

#[test]
fn ref2va_requires_ordered_reference_images_from_the_resolved_pack() {
    let binding = read_repo("src-tauri/src/application/ordered_reference_binding.rs");
    let batch = read_repo("src-tauri/src/application/shot_batch_service.rs");
    let domain = read_repo("src-tauri/src/domain/production_preparation.rs");
    assert_contains_all(
        &binding,
        &[
            "REF2VA",
            "reference_images",
            "let min_items = (*min_items).max(2)",
            "validate_ordered_reference_ids",
        ],
    );
    assert_contains_all(
        &batch,
        &[
            "reference_assets",
            "InputDefinition::Images",
            "GenerationInputValue::ImageAssets(references)",
        ],
    );
    assert_contains_all(
        &domain,
        &["reference_pack", "reference_sets", "reference_assets"],
    );
}

#[test]
fn legacy_shot_references_remain_visible_in_preparation_context() {
    let resolver = read_repo("src-tauri/src/application/shot_context_resolver.rs");
    let domain = read_repo("src-tauri/src/domain/production_preparation.rs");
    assert_contains_all(
        &resolver,
        &[
            "uses_legacy_shot_references",
            "source_reference_set_id: format!(\"legacy:{shot_id}\")",
        ],
    );
    let prompt_builder = read_repo("src-tauri/src/application/prompt_context_builder.rs");
    assert_contains_all(&prompt_builder, &["legacy_prompt", "unwrap_or_else"]);
    assert_contains_all(
        &domain,
        &[
            "pub legacy: crate::domain::LegacyContext",
            "pub legacy: bool",
        ],
    );
}

#[test]
fn legacy_batch_path_keeps_the_existing_100_item_limit() {
    let batch = read_repo("src-tauri/src/application/shot_batch_service.rs");
    let scene = read_repo("src-tauri/src/application/scene_production_service.rs");
    assert_contains_all(
        &batch,
        &[
            "pub const MAX_SHOT_BATCH_ITEMS: usize = 100",
            "MAX_SHOT_BATCH_ITEMS",
        ],
    );
    assert_contains_all(
        &scene,
        &[
            "MAX_SHOT_BATCH_ITEMS",
            "SceneProductionError::TooLarge",
            "max_batch_items",
        ],
    );
}

#[test]
fn preparation_and_readiness_paths_share_the_500_shot_boundary() {
    let preparation = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let readiness = read_repo("src-tauri/src/application/shot_readiness_service.rs");
    let resolver = read_repo("src-tauri/src/application/shot_context_resolver.rs");
    assert_contains_all(
        &preparation,
        &[
            "validate_shot_scope(shot_ids, 500)",
            "PREPARATION_BATCH_LIMIT",
        ],
    );
    assert_contains_all(
        &readiness,
        &[
            "READINESS_BATCH_LIMIT",
            "if shot_ids.len() > READINESS_BATCH_LIMIT",
        ],
    );
    assert_contains_all(
        &resolver,
        &[
            "pub const CONTEXT_BATCH_LIMIT: usize = 500",
            "if shot_ids.len() > CONTEXT_BATCH_LIMIT",
        ],
    );
}

#[test]
fn bulk_preflight_uses_one_resolver_batch_and_one_comfy_check() {
    let readiness = read_repo("src-tauri/src/application/shot_readiness_service.rs");
    let preparation = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let bundle = section(
        &readiness,
        "pub async fn preflight_bundle_many",
        "pub async fn preflight_many",
    );
    let evaluation = section(
        &preparation,
        "async fn evaluate_many",
        "fn prepare_generation_values",
    );
    assert_occurrences(bundle, "self.resolve_many(", 1);
    assert_occurrences(bundle, ".comfy_preflight_service", 1);
    assert_occurrences(bundle, ".current()", 1);
    assert_contains_all(
        evaluation,
        &[
            "preflight_bundle_many(project_id, shot_ids, stage)",
            "contexts.into_iter().zip(readiness)",
        ],
    );
}

#[test]
fn bulk_definition_reads_are_deduplicated_by_workflow_and_recipe() {
    let source = read_repo("src-tauri/src/application/production_preparation_service.rs");
    let evaluation = section(
        &source,
        "async fn evaluate_many",
        "fn prepare_generation_values",
    );
    assert_contains_all(
        evaluation,
        &[
            "let definition_pairs = contexts",
            "collect::<BTreeSet<_>>()",
            "definition_repository",
            "load_generation_definitions(",
            "definitions.get(key)",
        ],
    );
    assert_eq!(evaluation.matches(".find(").count(), 0);
    let loader = section(
        &source,
        "async fn load_generation_definitions",
        "fn prepare_generation_values",
    );
    assert_eq!(loader.matches(".find_many(pairs)").count(), 1);
    assert_eq!(loader.matches(".find(").count(), 0);
}

#[test]
fn episode_and_series_summaries_aggregate_child_readiness_without_mutation() {
    let episode = read_repo("src-tauri/src/application/episode_production_service.rs");
    let series = read_repo("src-tauri/src/application/series_production_service.rs");
    assert_contains_all(
        &episode,
        &[
            "readiness_tree_scope",
            "readiness_summary_from_scenes",
            "existing_batch_ids",
            "self.scene_production_service",
        ],
    );
    assert_contains_all(
        &series,
        &[
            "readiness_summary_from_episodes",
            "self.episode_production_service",
            "readiness_tree_scope",
            "existing_batch_ids",
        ],
    );
    assert_contains_none(
        &section(
            &episode,
            "pub async fn readiness_summary",
            "pub(crate) async fn plan_tree_scope",
        ),
        &["insert_batch", "create_batch", "start_generation"],
    );
}
