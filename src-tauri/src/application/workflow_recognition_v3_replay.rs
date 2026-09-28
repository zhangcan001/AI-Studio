#![cfg(test)]

//! Evidence-minimal offline replay for the frozen Workflow Recognition V3 baseline.
//! The runner invokes production parsing and recognition code; synthetic cases
//! dispatch into their attested existing unit tests.

use crate::application::{
    workflow_analysis_service,
    workflow_analysis_service::{OutputRoot, WorkflowAnalysisReport, WorkflowAnalysisService},
    workflow_graph_analysis::WorkflowGraph,
    workflow_onboarding_service,
    workflow_recognition_schema::RecognitionSchemaContext,
    workflow_semantic_graph,
    workflow_semantic_graph::{
        build_capability_profile_for_roots, resolve_semantic_graph, ActiveDependencyGraph,
        CapabilityProfile, RootDependencyClosure,
    },
};
use crate::domain::WorkflowDocument;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/workflow_recognition_v3"
);
const REAL_MANIFEST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/workflow_recognition_v3/real/manifest.json"
));
const SYNTHETIC_REGISTRY_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/workflow_recognition_v3/synthetic_registry.json"
));
const HISTORICAL_REAL_MANIFEST_SHA256: &str =
    "5566941f90b18c7838d97c79ed417fdb86293921faf9578e695c0b78a7f4232b";
const HISTORICAL_REVIEW_SHA256: &str =
    "1d5a691b40dcf54f959994589bc92e1cb12263e2338af30ccaf9749a3a6f1e6b";
const R10_UI_SHA256_FROM_SOURCE_MANIFEST: &str =
    "9646b9b9b72f6ccb5db717a2ceee7b32aae7f7a0ce2149c8354ebd4fc173fe0d";
const EXPECTED_REAL_COUNT: usize = 15;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RealManifest {
    baseline_id: String,
    source_manifest_id: String,
    source_manifest_sha256: String,
    review_artifact_sha256: String,
    base_head: String,
    expected_real_count: usize,
    schema_snapshot_sha256: String,
    object_info_path: String,
    historical_synthetic_expected_total: usize,
    verified_synthetic_membership_count: usize,
    synthetic_membership_verified: bool,
    network_required: bool,
    gpu_required: bool,
    live_comfyui_required: bool,
    live_object_info_required: bool,
    observed_only_fields: Vec<String>,
    samples: Vec<RealSample>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RealSample {
    sample_id: String,
    source_origin: String,
    baseline_commit: String,
    ui_sha256: String,
    api_sha256: String,
    object_info_sha256: String,
    provenance_sha256: String,
    ui_path: String,
    api_path: String,
    object_info_path: String,
    provenance_path: String,
    expected_category: String,
    expected_mode: String,
    expected_root_count: usize,
    expected_evidence_status: String,
    expected_semantic_counting_status: String,
    paired_api_profile_mode: Option<String>,
    paired_api_profile_evidence: Option<String>,
    field_evidence: BTreeMap<String, FieldEvidence>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct FieldEvidence {
    level: String,
    source: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct SyntheticRegistry {
    baseline_id: String,
    historical_synthetic_expected_total: usize,
    verified_synthetic_case_count: usize,
    historical_synthetic_membership_verified: bool,
    synthetic_baseline_complete_mode_enabled: bool,
    phase2abc_readiness_proven_count: usize,
    phase1_expected_count: usize,
    phase1_explicitly_proven_count: usize,
    verified_cases: Vec<SyntheticCase>,
    terminal_media_delta_candidates: Vec<TerminalMediaCandidate>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct SyntheticCase {
    wrapper_test: String,
    phase: String,
    source_test: String,
    source_suite: String,
    case_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct TerminalMediaCandidate {
    test: String,
    introduced_commit: String,
    present_at_final_review_baseline: bool,
    referenced_by_phase1_review: bool,
    can_prove_phase1_membership: bool,
    semantic_family: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvidenceGrade {
    Attested,
    IndependentlyDerived,
    ObservedOnly,
    Unverified,
}

fn real_manifest() -> RealManifest {
    serde_json::from_str(REAL_MANIFEST_JSON).expect("repository real manifest parses")
}

fn synthetic_registry() -> SyntheticRegistry {
    serde_json::from_str(SYNTHETIC_REGISTRY_JSON).expect("repository synthetic registry parses")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn parse_grade(value: &str) -> EvidenceGrade {
    match value {
        "attested" => EvidenceGrade::Attested,
        "independently_derived" => EvidenceGrade::IndependentlyDerived,
        "observed_only" => EvidenceGrade::ObservedOnly,
        _ => EvidenceGrade::Unverified,
    }
}

fn require_hard_golden_grade(grade: EvidenceGrade, field: &str) -> Result<(), String> {
    match grade {
        EvidenceGrade::Attested | EvidenceGrade::IndependentlyDerived => Ok(()),
        EvidenceGrade::ObservedOnly | EvidenceGrade::Unverified => Err(format!(
            "field {field} is not independently attested for a hard golden"
        )),
    }
}

fn require_manifest_evidence(sample: &RealSample, field: &str) -> Result<(), String> {
    let evidence = sample
        .field_evidence
        .get(field)
        .ok_or_else(|| format!("{} has no provenance for {field}", sample.sample_id))?;
    if evidence.source.trim().is_empty() {
        return Err(format!(
            "{} has empty provenance source for {field}",
            sample.sample_id
        ));
    }
    require_hard_golden_grade(parse_grade(&evidence.level), field)
}

fn portable_fixture_path(path: &str) -> Result<PathBuf, String> {
    if path.is_empty() || path.contains('\\') || path.contains(':') {
        return Err(format!("fixture path is not portable: {path}"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("fixture path escapes repository corpus: {path}"));
    }
    Ok(parsed.to_owned())
}

fn validate_real_manifest_metadata(manifest: &RealManifest) -> Result<(), String> {
    if manifest.baseline_id != "V3_REPLAY_BASELINE_V1" {
        return Err(format!(
            "unexpected real baseline id {}",
            manifest.baseline_id
        ));
    }
    if manifest.source_manifest_id != "PRIMARY_BLACK_BOX_MANIFEST_V2"
        || manifest.source_manifest_sha256 != HISTORICAL_REAL_MANIFEST_SHA256
    {
        return Err("real source-manifest identity/hash mismatch".to_owned());
    }
    if manifest.review_artifact_sha256 != HISTORICAL_REVIEW_SHA256 {
        return Err("historical review artifact hash mismatch".to_owned());
    }
    if manifest.expected_real_count != EXPECTED_REAL_COUNT
        || manifest.samples.len() != manifest.expected_real_count
    {
        return Err(format!(
            "wrong real count: expected metadata {}, discovered {} (V1 baseline is {})",
            manifest.expected_real_count,
            manifest.samples.len(),
            EXPECTED_REAL_COUNT
        ));
    }
    if manifest.historical_synthetic_expected_total != 72
        || manifest.verified_synthetic_membership_count != 69
        || manifest.synthetic_membership_verified
    {
        return Err("historical synthetic partial-membership metadata changed".to_owned());
    }
    if manifest.network_required
        || manifest.gpu_required
        || manifest.live_comfyui_required
        || manifest.live_object_info_required
    {
        return Err("replay must remain offline and GPU/live-service independent".to_owned());
    }
    for field in ["actual_root_ids", "actual_capabilities", "actual_readiness"] {
        if !manifest
            .observed_only_fields
            .iter()
            .any(|entry| entry == field)
        {
            return Err(format!("{field} must remain OBSERVED_ONLY"));
        }
    }
    portable_fixture_path(&manifest.object_info_path)?;

    let mut ids = HashSet::new();
    let mut conflict_count = 0;
    let mut counted_pass_count = 0;
    for sample in &manifest.samples {
        if !ids.insert(sample.sample_id.as_str()) {
            return Err(format!("duplicate real sample id {}", sample.sample_id));
        }
        if sample.baseline_commit != manifest.base_head {
            return Err(format!(
                "{} baseline commit does not match corpus baseline",
                sample.sample_id
            ));
        }
        for path in [
            &sample.ui_path,
            &sample.api_path,
            &sample.object_info_path,
            &sample.provenance_path,
        ] {
            portable_fixture_path(path)?;
        }
        for field in [
            "sample_id",
            "ui_sha256",
            "api_sha256",
            "object_info_sha256",
            "provenance_sha256",
            "expected_category",
            "expected_mode",
            "expected_root_count",
            "expected_evidence_status",
            "expected_semantic_counting_status",
        ] {
            require_manifest_evidence(sample, field)?;
        }
        if sample.object_info_sha256 != manifest.schema_snapshot_sha256 {
            return Err(format!(
                "{} object_info hash disagrees with locked schema hash",
                sample.sample_id
            ));
        }
        match sample.expected_evidence_status.as_str() {
            "CONSISTENT" => {
                if sample.expected_semantic_counting_status != "COUNTED_SEMANTIC_PASS" {
                    return Err(format!(
                        "{} has inconsistent semantic counting status",
                        sample.sample_id
                    ));
                }
                if let Some(paired_mode) = &sample.paired_api_profile_mode {
                    if paired_mode != &sample.expected_mode {
                        return Err(format!(
                            "{} has an unclassified API/profile mode conflict",
                            sample.sample_id
                        ));
                    }
                }
                counted_pass_count += 1;
            }
            "EVIDENCE_CONTRACT_CONFLICT" => {
                if sample.expected_semantic_counting_status
                    != "NOT_COUNTED_DUE_TO_EVIDENCE_CONFLICT"
                    || sample.paired_api_profile_mode.as_deref() != Some("text_to_image")
                    || sample.expected_mode != "image_to_image"
                    || sample
                        .paired_api_profile_evidence
                        .as_deref()
                        .unwrap_or("")
                        .is_empty()
                {
                    return Err(format!(
                        "{} does not carry the attested R11 evidence conflict",
                        sample.sample_id
                    ));
                }
                conflict_count += 1;
            }
            status => {
                return Err(format!(
                    "{} has unsupported expected evidence status {status}",
                    sample.sample_id
                ))
            }
        }
    }
    if conflict_count != 1 || counted_pass_count != 14 {
        return Err(format!(
            "expected historical 14-pass/1-conflict evidence, found {counted_pass_count}/{conflict_count}"
        ));
    }
    let r10 = manifest
        .samples
        .iter()
        .find(|sample| sample.sample_id == "R10_LEAPFUSION_CUSTOM_I2V")
        .ok_or_else(|| "R10 sample missing".to_owned())?;
    if r10.ui_sha256 != R10_UI_SHA256_FROM_SOURCE_MANIFEST {
        return Err("R10 UI hash differs from the exact source-manifest value".to_owned());
    }
    let r10_evidence = r10
        .field_evidence
        .get("ui_sha256")
        .ok_or_else(|| "R10 source-manifest UI hash provenance missing".to_owned())?;
    if !r10_evidence
        .source
        .contains("PRIMARY_BLACK_BOX_MANIFEST_V2.samples.uiSha256")
    {
        return Err("R10 UI hash must be sourced directly from the source manifest".to_owned());
    }
    Ok(())
}

fn read_hashed_fixture(root: &Path, relative: &str, expected: &str) -> Result<Vec<u8>, String> {
    let safe = portable_fixture_path(relative)?;
    let path = root.join(safe);
    let bytes =
        fs::read(&path).map_err(|error| format!("missing fixture {}: {error}", path.display()))?;
    let actual = sha256(&bytes);
    if actual != expected {
        return Err(format!(
            "fixture hash mismatch for {relative}: expected {expected}, got {actual}"
        ));
    }
    Ok(bytes)
}

fn validate_real_fixtures(manifest: &RealManifest, root: &Path) -> Result<(), String> {
    validate_real_manifest_metadata(manifest)?;
    let object_info = read_hashed_fixture(
        root,
        &manifest.object_info_path,
        &manifest.schema_snapshot_sha256,
    )?;
    let _: Value = serde_json::from_slice(&object_info)
        .map_err(|error| format!("object_info fixture JSON invalid: {error}"))?;
    for sample in &manifest.samples {
        read_hashed_fixture(root, &sample.ui_path, &sample.ui_sha256)?;
        read_hashed_fixture(root, &sample.api_path, &sample.api_sha256)?;
        read_hashed_fixture(root, &sample.object_info_path, &sample.object_info_sha256)?;
        read_hashed_fixture(root, &sample.provenance_path, &sample.provenance_sha256)?;
    }
    Ok(())
}

fn discovered_case_names() -> Result<BTreeSet<String>, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let output = Command::new(&executable)
        .arg("--list")
        .output()
        .map_err(|error| format!("could not enumerate replay tests: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "replay test discovery failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let names = listing
        .lines()
        .filter_map(|line| line.split_once(": test").map(|(name, _)| name))
        .filter_map(|name| name.rsplit("::").next())
        .filter(|name| name.starts_with("case_"))
        .map(str::to_owned)
        .collect();
    Ok(names)
}

fn validate_synthetic_registry(
    registry: &SyntheticRegistry,
    discovered: &BTreeSet<String>,
) -> Result<(), String> {
    if registry.baseline_id != "V3_REPLAY_BASELINE_V1"
        || registry.historical_synthetic_expected_total != 72
        || registry.phase1_expected_count != 15
    {
        return Err("synthetic registry historical baseline metadata changed".to_owned());
    }
    if registry.verified_synthetic_case_count != registry.verified_cases.len() {
        return Err(format!(
            "synthetic registry count mismatch: declared {}, actual {}",
            registry.verified_synthetic_case_count,
            registry.verified_cases.len()
        ));
    }
    let mut case_ids = HashSet::new();
    let mut wrapper_names = HashSet::new();
    let mut phase_counts = BTreeMap::<String, usize>::new();
    for case in &registry.verified_cases {
        if case.case_id.trim().is_empty() || !case_ids.insert(case.case_id.as_str()) {
            return Err(format!(
                "duplicate/empty synthetic case id {}",
                case.case_id
            ));
        }
        if !wrapper_names.insert(case.wrapper_test.as_str()) {
            return Err(format!(
                "duplicate synthetic wrapper test {}",
                case.wrapper_test
            ));
        }
        if !case.wrapper_test.starts_with("case_") || case.source_test.trim().is_empty() {
            return Err(format!(
                "invalid synthetic case registration: {}",
                case.wrapper_test
            ));
        }
        *phase_counts.entry(case.phase.clone()).or_default() += 1;
    }
    let phase2abc_readiness = ["PHASE2A", "PHASE2B", "PHASE2C", "READINESS"]
        .iter()
        .map(|phase| phase_counts.get(*phase).copied().unwrap_or_default())
        .sum::<usize>();
    let phase1_explicit = phase_counts
        .get("PHASE1_EXPLICIT")
        .copied()
        .unwrap_or_default();
    if phase2abc_readiness != registry.phase2abc_readiness_proven_count
        || phase1_explicit != registry.phase1_explicitly_proven_count
    {
        return Err(
            "synthetic phase membership counts disagree with registered identities".to_owned(),
        );
    }
    if registry.historical_synthetic_membership_verified {
        if registry.verified_cases.len() != registry.historical_synthetic_expected_total {
            return Err("complete historical membership count does not reconcile".to_owned());
        }
    } else if registry.synthetic_baseline_complete_mode_enabled {
        return Err("complete-baseline mode cannot be enabled for partial membership".to_owned());
    }
    for candidate in &registry.terminal_media_delta_candidates {
        if candidate.can_prove_phase1_membership
            || candidate.present_at_final_review_baseline
            || candidate.referenced_by_phase1_review
        {
            return Err(format!(
                "unproven terminal-media candidate {} was promoted into Phase1",
                candidate.test
            ));
        }
        if candidate.introduced_commit != "dacafca3ef1f7ba0900d4507555641c0bf10bfd7"
            || candidate.semantic_family.trim().is_empty()
        {
            return Err(format!(
                "terminal-media provenance changed for {}",
                candidate.test
            ));
        }
    }
    if candidate_count(&registry.terminal_media_delta_candidates) != 3 {
        return Err("expected exactly three unproven terminal-media delta candidates".to_owned());
    }

    let missing = wrapper_names
        .difference(&discovered.iter().map(String::as_str).collect())
        .cloned()
        .collect::<Vec<_>>();
    let unregistered = discovered
        .iter()
        .filter(|name| !wrapper_names.contains(name.as_str()))
        .collect::<Vec<_>>();
    if !missing.is_empty() || !unregistered.is_empty() {
        return Err(format!(
            "synthetic wrapper discovery mismatch: missing={missing:?}, unregistered={unregistered:?}"
        ));
    }
    Ok(())
}

fn candidate_count(candidates: &[TerminalMediaCandidate]) -> usize {
    candidates.len()
}

fn run_synthetic_case(case: &SyntheticCase) {
    match case.source_suite.as_str() {
        "workflow_analysis_service::tests" => {
            workflow_analysis_service::tests::run_replay_test(&case.source_test)
        }
        "workflow_semantic_graph::tests" => {
            workflow_semantic_graph::tests::run_replay_test(&case.source_test)
        }
        "workflow_onboarding_service::tests" => {
            workflow_onboarding_service::tests::run_replay_test(&case.source_test)
        }
        suite => panic!("unregistered synthetic source suite: {suite}"),
    }
}

fn observed_capability_profile(
    workflow: &WorkflowDocument,
    schema: &RecognitionSchemaContext,
    analysis: &WorkflowAnalysisReport,
) -> Option<CapabilityProfile> {
    let roots: &[OutputRoot] = analysis.output_root_resolution.roots();
    if roots.is_empty() {
        return None;
    }
    let graph = WorkflowGraph::from_document(workflow).ok()?;
    let root_nodes = roots
        .iter()
        .map(|root| root.node_id.clone())
        .collect::<Vec<_>>();
    let active = ActiveDependencyGraph::from_graph(&graph, &root_nodes).ok()?;
    let semantic = resolve_semantic_graph(workflow, schema, active);
    let closures = roots
        .iter()
        .map(|root| {
            RootDependencyClosure::from_graph(&graph, root.output_id.clone(), &root.node_id)
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    Some(build_capability_profile_for_roots(
        analysis,
        &semantic,
        &closures,
        roots,
        analysis.selected_root_id.as_deref(),
    ))
}

#[derive(Debug, Default, PartialEq, Eq)]
struct RealReplayCounts {
    total: usize,
    semantic_pass: usize,
    semantic_failure: usize,
    expected_evidence_conflict: usize,
    hard_golden_mismatches: Vec<String>,
}

fn replay_real(root: &Path) -> RealReplayCounts {
    let manifest = real_manifest();
    validate_real_fixtures(&manifest, root).expect("locked real corpus must validate");
    let object_info_bytes =
        fs::read(root.join(portable_fixture_path(&manifest.object_info_path).unwrap()))
            .expect("locked object_info exists");
    let object_info: Value =
        serde_json::from_slice(&object_info_bytes).expect("locked object_info parses");
    let schema = RecognitionSchemaContext::parse(&object_info);
    let mut counts = RealReplayCounts::default();

    for sample in &manifest.samples {
        let api_bytes = read_hashed_fixture(root, &sample.api_path, &sample.api_sha256)
            .expect("API hash was validated");
        let api_value: Value =
            serde_json::from_slice(&api_bytes).expect("locked API workflow parses as JSON");
        let workflow =
            WorkflowDocument::parse(api_value).expect("locked API workflow is an object");
        let analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &workflow,
            &api_bytes,
            Some(&schema),
        );
        let roots = analysis.output_root_resolution.roots();
        let mut mismatches = Vec::new();
        if roots.len() != sample.expected_root_count {
            mismatches.push(format!(
                "root_count expected={} actual={}",
                sample.expected_root_count,
                roots.len()
            ));
        }
        if analysis.category != sample.expected_category {
            mismatches.push(format!(
                "category expected={} actual={}",
                sample.expected_category, analysis.category
            ));
        }

        let capability_profile = observed_capability_profile(&workflow, &schema, &analysis);
        let actual_root_ids = roots
            .iter()
            .map(|root| root.node_id.as_str())
            .collect::<Vec<_>>();
        let observed = json!({
            "evidence": "OBSERVED_ONLY",
            "readiness_scope": "SEMANTIC_CAPABILITY",
            "actual_root_ids": actual_root_ids,
            "actual_capabilities": capability_profile.as_ref().map(|p| &p.aggregate_capabilities),
            "semantic_capability_status": capability_profile.as_ref().map(|p| p.readiness),
            // Retained as an observed-only compatibility alias; it never means runtime import readiness.
            "actual_readiness": capability_profile.as_ref().map(|p| p.readiness),
            "analysis_mode": analysis.mode,
            "capability_mode": capability_profile.as_ref().and_then(|p| p.primary_capability.as_deref()),
        });
        if sample.expected_evidence_status == "EVIDENCE_CONTRACT_CONFLICT" {
            counts.expected_evidence_conflict += 1;
            assert_eq!(
                sample.expected_semantic_counting_status,
                "NOT_COUNTED_DUE_TO_EVIDENCE_CONFLICT"
            );
            println!(
                "V3_REAL_CASE id={} source={} result=EXPECTED_EVIDENCE_CONFLICT actual_category={} actual_mode={} diagnostics={}",
                sample.sample_id,
                sample.source_origin,
                analysis.category,
                analysis.mode,
                observed
            );
            if !mismatches.is_empty() {
                counts.hard_golden_mismatches.push(format!(
                    "{}: {}",
                    sample.sample_id,
                    mismatches.join(", ")
                ));
            }
        } else {
            if sample.expected_evidence_status != "CONSISTENT" {
                mismatches.push(format!(
                    "unexpected evidence status {}",
                    sample.expected_evidence_status
                ));
            }
            if analysis.mode != sample.expected_mode {
                mismatches.push(format!(
                    "mode expected={} actual={}",
                    sample.expected_mode, analysis.mode
                ));
            }
            if mismatches.is_empty() {
                counts.semantic_pass += 1;
                println!(
                    "V3_REAL_CASE id={} source={} result=SEMANTIC_PASS category={} mode={} roots={} diagnostics={}",
                    sample.sample_id,
                    sample.source_origin,
                    analysis.category,
                    analysis.mode,
                    roots.len(),
                    observed
                );
            } else {
                counts.semantic_failure += 1;
                let mismatch = format!("{}: {}", sample.sample_id, mismatches.join(", "));
                counts.hard_golden_mismatches.push(mismatch.clone());
                println!(
                    "V3_REAL_CASE id={} source={} result=SEMANTIC_FAILURE mismatch={} actual_category={} actual_mode={} roots={} diagnostics={}",
                    sample.sample_id,
                    sample.source_origin,
                    mismatch,
                    analysis.category,
                    analysis.mode,
                    roots.len(),
                    observed
                );
            }
        }
        counts.total += 1;
    }
    println!(
        "V3_REAL_SUMMARY total={} semantic_pass={} semantic_failure={} expected_evidence_conflict={} hard_golden_mismatches={:?}",
        counts.total,
        counts.semantic_pass,
        counts.semantic_failure,
        counts.expected_evidence_conflict,
        counts.hard_golden_mismatches
    );
    counts
}

macro_rules! synthetic_case {
    ($wrapper:ident, $suite:ident, $source_test:literal) => {
        #[test]
        fn $wrapper() {
            crate::application::$suite::tests::run_replay_test($source_test);
        }
    };
}

macro_rules! replay_cases {
    ($($name:ident!($($args:tt)*);)*) => {
        $($name!($($args)*);)*
    };
}

fn replay_synthetic() -> usize {
    let registry = synthetic_registry();
    let discovered = discovered_case_names().expect("synthetic test discovery succeeds");
    validate_synthetic_registry(&registry, &discovered)
        .expect("verified synthetic registry matches test discovery");
    for case in &registry.verified_cases {
        run_synthetic_case(case);
        println!(
            "V3_SYNTHETIC_CASE id={} phase={} source={} result=PASS",
            case.case_id, case.phase, case.source_test
        );
    }
    registry.verified_cases.len()
}

#[test]
fn replay_real_corpus() {
    let counts = replay_real(Path::new(FIXTURE_ROOT));
    assert_eq!(counts.total, EXPECTED_REAL_COUNT);
    assert_eq!(counts.semantic_pass, 14);
    assert_eq!(counts.semantic_failure, 0);
    assert_eq!(counts.expected_evidence_conflict, 1);
    assert!(counts.hard_golden_mismatches.is_empty());
}

#[test]
fn replay_synthetic_verified() {
    let registry = synthetic_registry();
    let count = replay_synthetic();
    assert_eq!(count, registry.verified_synthetic_case_count);
    println!(
        "VERIFIED_SYNTHETIC_CASE_COUNT={} HISTORICAL_EXPECTED_TOTAL={} MEMBERSHIP_VERIFIED={}",
        count,
        registry.historical_synthetic_expected_total,
        registry.historical_synthetic_membership_verified
    );
}

#[test]
fn replay_all_verified() {
    let real = replay_real(Path::new(FIXTURE_ROOT));
    let synthetic = replay_synthetic();
    assert_eq!(real.total, EXPECTED_REAL_COUNT);
    assert_eq!(real.semantic_pass, 14);
    assert_eq!(real.semantic_failure, 0);
    assert_eq!(real.expected_evidence_conflict, 1);
    assert!(real.hard_golden_mismatches.is_empty());
    assert_eq!(
        synthetic,
        synthetic_registry().verified_synthetic_case_count
    );
}

#[test]
fn replay_rejects_missing_real_fixture() {
    let manifest = real_manifest();
    let empty = tempfile::tempdir().expect("temporary test directory");
    assert!(validate_real_fixtures(&manifest, empty.path()).is_err());
}

#[test]
fn replay_rejects_real_hash_mismatch() {
    let mut manifest = real_manifest();
    manifest.samples[0].ui_sha256 = "0".repeat(64);
    assert!(validate_real_fixtures(&manifest, Path::new(FIXTURE_ROOT)).is_err());
}

#[test]
fn replay_rejects_duplicate_real_id() {
    let mut manifest = real_manifest();
    manifest.samples[1].sample_id = manifest.samples[0].sample_id.clone();
    assert!(validate_real_manifest_metadata(&manifest).is_err());
}

#[test]
fn replay_rejects_wrong_real_count() {
    let mut manifest = real_manifest();
    manifest.expected_real_count = 14;
    assert!(validate_real_manifest_metadata(&manifest).is_err());
}

#[test]
fn replay_rejects_unexpected_evidence_conflict() {
    let mut manifest = real_manifest();
    let sample = manifest
        .samples
        .iter_mut()
        .find(|sample| sample.sample_id == "R01_LTX23_T2V")
        .expect("R01 exists in the locked corpus");
    sample.expected_mode = "image_to_image".to_owned();
    sample.expected_evidence_status = "EVIDENCE_CONTRACT_CONFLICT".to_owned();
    sample.expected_semantic_counting_status = "NOT_COUNTED_DUE_TO_EVIDENCE_CONFLICT".to_owned();
    sample.paired_api_profile_mode = Some("text_to_image".to_owned());
    sample.paired_api_profile_evidence = Some("test-only unexpected conflict".to_owned());

    let error = validate_real_manifest_metadata(&manifest)
        .expect_err("an additional evidence conflict must fail the 14-pass/1-conflict gate");
    assert!(error.contains("expected historical 14-pass/1-conflict evidence"));
}

#[test]
fn replay_rejects_duplicate_synthetic_id() {
    let mut registry = synthetic_registry();
    registry.verified_cases[1].case_id = registry.verified_cases[0].case_id.clone();
    let discovered = discovered_case_names().expect("test discovery succeeds");
    assert!(validate_synthetic_registry(&registry, &discovered).is_err());
}

#[test]
fn replay_rejects_unregistered_synthetic() {
    let registry = synthetic_registry();
    let mut discovered = discovered_case_names().expect("test discovery succeeds");
    discovered.insert("case_not_in_evidence_registry".to_owned());
    assert!(validate_synthetic_registry(&registry, &discovered).is_err());
}

#[test]
fn replay_discovery_is_deterministic() {
    let first = discovered_case_names().expect("first discovery succeeds");
    let second = discovered_case_names().expect("second discovery succeeds");
    assert_eq!(first, second);
    assert_eq!(first.len(), 69);
}

#[test]
fn replay_golden_rejects_unattested_expectation() {
    assert!(
        require_hard_golden_grade(EvidenceGrade::ObservedOnly, "expected_root_node_id").is_err()
    );
}

replay_cases! {
    synthetic_case!(case_no_root_returns_unknown_not_full_graph, workflow_analysis_service, "no_root_returns_unknown_not_full_graph");
    synthetic_case!(case_ambiguous_root_returns_ambiguous, workflow_analysis_service, "ambiguous_root_returns_ambiguous");
    synthetic_case!(case_strong_root_beats_weak_preview, workflow_analysis_service, "strong_root_beats_weak_preview");
    synthetic_case!(case_explicit_output_mapping_resolves_unknown, workflow_analysis_service, "explicit_output_mapping_resolves_unknown");
    synthetic_case!(case_invalid_explicit_output_mapping_fails_safe, workflow_analysis_service, "invalid_explicit_output_mapping_fails_safe");
    synthetic_case!(case_terminal_input_node_is_not_output_root, workflow_analysis_service, "terminal_input_node_is_not_output_root");
    synthetic_case!(case_weak_class_title_hint_alone_does_not_resolve_root, workflow_analysis_service, "weak_class_title_hint_alone_does_not_resolve_root");
    synthetic_case!(case_two_compatible_strong_roots_remain_resolved, workflow_analysis_service, "two_compatible_strong_roots_remain_resolved");
    synthetic_case!(case_root_resolution_is_order_independent, workflow_analysis_service, "root_resolution_is_order_independent");
    synthetic_case!(case_per_root_closure_does_not_union_inputs, workflow_semantic_graph, "PER_ROOT_CLOSURE_DOES_NOT_UNION_INPUTS");
    synthetic_case!(case_two_compatible_video_roots, workflow_semantic_graph, "TWO_COMPATIBLE_VIDEO_ROOTS");
    synthetic_case!(case_two_different_capability_roots, workflow_semantic_graph, "TWO_DIFFERENT_CAPABILITY_ROOTS");
    synthetic_case!(case_root_a_ready_root_b_unsupported, workflow_semantic_graph, "ROOT_A_READY_ROOT_B_UNSUPPORTED");
    synthetic_case!(case_image_input_does_not_leak_across_roots, workflow_semantic_graph, "IMAGE_INPUT_DOES_NOT_LEAK_ACROSS_ROOTS");
    synthetic_case!(case_reference_semantic_does_not_leak_across_roots, workflow_semantic_graph, "REFERENCE_SEMANTIC_DOES_NOT_LEAK_ACROSS_ROOTS");
    synthetic_case!(case_shared_upstream_can_belong_to_multiple_roots, workflow_semantic_graph, "SHARED_UPSTREAM_CAN_BELONG_TO_MULTIPLE_ROOTS");
    synthetic_case!(case_internal_generated_media_is_not_external_input, workflow_semantic_graph, "INTERNAL_GENERATED_MEDIA_IS_NOT_EXTERNAL_INPUT");
    synthetic_case!(case_root_unknown_dependency_is_scoped, workflow_semantic_graph, "ROOT_UNKNOWN_DEPENDENCY_IS_SCOPED");
    synthetic_case!(case_explicit_selected_root_controls_primary_without_dropping_other_roots, workflow_semantic_graph, "EXPLICIT_SELECTED_ROOT_CONTROLS_PRIMARY_WITHOUT_DROPPING_OTHER_ROOTS");
    synthetic_case!(case_per_root_output_types_are_preserved, workflow_semantic_graph, "PER_ROOT_OUTPUT_TYPES_ARE_PRESERVED");
    synthetic_case!(case_per_root_capability_is_order_independent, workflow_semantic_graph, "PER_ROOT_CAPABILITY_IS_ORDER_INDEPENDENT");
    synthetic_case!(case_schema_type_overrides_alias_name, workflow_semantic_graph, "SCHEMA_TYPE_OVERRIDES_ALIAS_NAME");
    synthetic_case!(case_class_title_cannot_override_schema_type, workflow_semantic_graph, "CLASS_TITLE_CANNOT_OVERRIDE_SCHEMA_TYPE");
    synthetic_case!(case_sparse_schema_t2v, workflow_semantic_graph, "SPARSE_SCHEMA_T2V");
    synthetic_case!(case_sparse_schema_i2v, workflow_semantic_graph, "SPARSE_SCHEMA_I2V");
    synthetic_case!(case_unknown_role_known_types_nonblocking, workflow_semantic_graph, "UNKNOWN_ROLE_KNOWN_TYPES_NONBLOCKING");
    synthetic_case!(case_unknown_transform_with_known_media_types, workflow_semantic_graph, "UNKNOWN_TRANSFORM_WITH_KNOWN_MEDIA_TYPES");
    synthetic_case!(case_unknown_source_with_known_image_output, workflow_semantic_graph, "UNKNOWN_SOURCE_WITH_KNOWN_IMAGE_OUTPUT");
    synthetic_case!(case_unknown_sink_with_known_video_input, workflow_semantic_graph, "UNKNOWN_SINK_WITH_KNOWN_VIDEO_INPUT");
    synthetic_case!(case_unknown_required_custom_type_fails_closed, workflow_semantic_graph, "UNKNOWN_REQUIRED_CUSTOM_TYPE_FAILS_CLOSED");
    synthetic_case!(case_reference_list_schema_is_reference, workflow_semantic_graph, "REFERENCE_LIST_SCHEMA_IS_REFERENCE");
    synthetic_case!(case_reference_indexed_slots_are_reference, workflow_semantic_graph, "REFERENCE_INDEXED_SLOTS_ARE_REFERENCE");
    synthetic_case!(case_multiple_image_inputs_not_reference, workflow_semantic_graph, "MULTIPLE_IMAGE_INPUTS_NOT_REFERENCE");
    synthetic_case!(case_reference_alias_cannot_override_non_image_schema, workflow_semantic_graph, "REFERENCE_ALIAS_CANNOT_OVERRIDE_NON_IMAGE_SCHEMA");
    synthetic_case!(case_canonical_prompt_hint_is_shared, workflow_semantic_graph, "CANONICAL_PROMPT_HINT_IS_SHARED");
    synthetic_case!(case_canonical_media_hint_is_shared, workflow_semantic_graph, "CANONICAL_MEDIA_HINT_IS_SHARED");
    synthetic_case!(case_internal_generated_media_remains_internal, workflow_semantic_graph, "INTERNAL_GENERATED_MEDIA_REMAINS_INTERNAL");
    synthetic_case!(case_reference_semantic_remains_root_scoped, workflow_semantic_graph, "REFERENCE_SEMANTIC_REMAINS_ROOT_SCOPED");
    synthetic_case!(case_canonical_resolution_is_order_independent, workflow_semantic_graph, "CANONICAL_RESOLUTION_IS_ORDER_INDEPENDENT");
    synthetic_case!(case_legacy_suggestion_consumes_canonical_hints, workflow_onboarding_service, "LEGACY_SUGGESTION_CONSUMES_CANONICAL_HINTS");
    synthetic_case!(case_schema_type_layering_preserves_raw_custom_types, workflow_semantic_graph, "SCHEMA_TYPE_LAYERING_PRESERVES_RAW_CUSTOM_TYPES");
    synthetic_case!(case_opaque_custom_control_nonblocking, workflow_semantic_graph, "OPAQUE_CUSTOM_CONTROL_NONBLOCKING");
    synthetic_case!(case_arbitrary_opaque_types_do_not_require_adapter, workflow_semantic_graph, "ARBITRARY_OPAQUE_TYPES_DO_NOT_REQUIRE_ADAPTER");
    synthetic_case!(case_opaque_external_media_boundary_fails_closed, workflow_semantic_graph, "OPAQUE_EXTERNAL_MEDIA_BOUNDARY_FAILS_CLOSED");
    synthetic_case!(case_opaque_output_boundary_fails_closed, workflow_semantic_graph, "OPAQUE_OUTPUT_BOUNDARY_FAILS_CLOSED");
    synthetic_case!(case_opaque_media_sink_output_is_nonblocking, workflow_semantic_graph, "OPAQUE_MEDIA_SINK_OUTPUT_IS_NONBLOCKING");
    synthetic_case!(case_wildcard_resolves_from_known_source, workflow_semantic_graph, "WILDCARD_RESOLVES_FROM_KNOWN_SOURCE");
    synthetic_case!(case_wildcard_resolves_from_known_target, workflow_semantic_graph, "WILDCARD_RESOLVES_FROM_KNOWN_TARGET");
    synthetic_case!(case_unconstrained_wildcard_capability_boundary_fails_closed, workflow_semantic_graph, "UNCONSTRAINED_WILDCARD_CAPABILITY_BOUNDARY_FAILS_CLOSED");
    synthetic_case!(case_unconstrained_wildcard_auxiliary_nonblocking, workflow_semantic_graph, "UNCONSTRAINED_WILDCARD_AUXILIARY_NONBLOCKING");
    synthetic_case!(case_matchtype_propagates_known_media, workflow_semantic_graph, "MATCHTYPE_PROPAGATES_KNOWN_MEDIA");
    synthetic_case!(case_matchtype_unresolved_critical_fails_closed, workflow_semantic_graph, "MATCHTYPE_UNRESOLVED_CRITICAL_FAILS_CLOSED");
    synthetic_case!(case_autogrow_input_resolves_from_known_source, workflow_semantic_graph, "AUTOGROW_INPUT_RESOLVES_FROM_KNOWN_SOURCE");
    synthetic_case!(case_conditional_input_resolves_selected_option, workflow_semantic_graph, "CONDITIONAL_INPUT_RESOLVES_SELECTED_OPTION");
    synthetic_case!(case_autogrow_prefix_input_resolves_from_known_source, workflow_semantic_graph, "AUTOGROW_PREFIX_INPUT_RESOLVES_FROM_KNOWN_SOURCE");
    synthetic_case!(case_custom_type_name_does_not_change_capability, workflow_semantic_graph, "CUSTOM_TYPE_NAME_DOES_NOT_CHANGE_CAPABILITY");
    synthetic_case!(case_opaque_type_order_independent, workflow_semantic_graph, "OPAQUE_TYPE_ORDER_INDEPENDENT");
    synthetic_case!(case_unknown_t2v, workflow_semantic_graph, "capability_uses_active_image_connection_not_optional_schema_presence");
    synthetic_case!(case_unknown_i2v, workflow_semantic_graph, "capability_uses_active_image_connection_not_optional_schema_presence");
    synthetic_case!(case_optional_image_unconnected, workflow_semantic_graph, "capability_uses_active_image_connection_not_optional_schema_presence");
    synthetic_case!(case_optional_image_connected, workflow_semantic_graph, "capability_uses_active_image_connection_not_optional_schema_presence");
    synthetic_case!(case_disconnected_image_source, workflow_semantic_graph, "active_dependency_graph_excludes_disconnected_nodes_and_keeps_multiple_roots");
    synthetic_case!(case_disconnected_missing_node, workflow_semantic_graph, "disconnected_missing_schema_is_noncritical_but_active_missing_schema_blocks");
    synthetic_case!(case_active_missing_node, workflow_semantic_graph, "disconnected_missing_schema_is_noncritical_but_active_missing_schema_blocks");
    synthetic_case!(case_unknown_reference_video, workflow_semantic_graph, "generic_reference_media_is_distinct_from_generic_image_to_video");
    synthetic_case!(case_reference_node_disconnected, workflow_semantic_graph, "generic_reference_media_is_distinct_from_generic_image_to_video");
    synthetic_case!(case_unknown_identity_known_capability, workflow_semantic_graph, "capability_uses_active_image_connection_not_optional_schema_presence");
    synthetic_case!(case_unknown_critical_semantic_type, workflow_semantic_graph, "active_unknown_critical_socket_fails_closed_without_provider_rules");
    synthetic_case!(case_multi_output_workflow, workflow_semantic_graph, "active_dependency_graph_excludes_disconnected_nodes_and_keeps_multiple_roots");
}
