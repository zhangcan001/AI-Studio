//! New-generation policy, not a Registry or a historical-read filter.
//!
//! IDs are resolved by the existing Registry. An exact pair is authorized only
//! when its canonical PRODUCT artifact and immutable snapshots match an
//! embedded, approved package. Lifecycle and live-byte checks remain in Registry.
use crate::application::ports::{
    RuntimeRecipeRecord, RuntimeWorkflowVersionRecord, WorkflowPackageBytes,
    WorkflowRuntimeArtifactRecord,
};
use crate::application::workflow_manifest::WorkflowManifest;
use sha2::{Digest, Sha256};

pub const REQUIRED: &str = "MINIMAX_VIDEO_PRODUCT_REQUIRED";
pub const AUTHORIZED_PACKAGES: [&str; 4] = [
    "minimax_h3_fl2va_t2v_quality_2_2_0",
    "minimax_h3_fl2va_i2v_quality_2_2_0",
    "minimax_h3_fl2va_first_last_quality_2_2_0",
    "minimax_h3_reference_video_quality_2_2_0",
];

pub fn authorizes(
    version: &RuntimeWorkflowVersionRecord,
    recipe: &RuntimeRecipeRecord,
    artifact: &WorkflowRuntimeArtifactRecord,
    live: &WorkflowPackageBytes,
) -> bool {
    if !AUTHORIZED_PACKAGES.contains(&artifact.package_name.as_str())
        || artifact.source_kind != "PRODUCT"
        || artifact.workflow_version_id != version.workflow_version_id
        || artifact.recipe_id != recipe.recipe_id
    {
        return false;
    }
    let Some(expected) =
        crate::application::builtin_runtime_packages::embedded_package(&artifact.package_name)
    else {
        return false;
    };
    let Ok(manifest_text) = std::str::from_utf8(&expected.manifest_yaml) else {
        return false;
    };
    let Ok(manifest) = WorkflowManifest::parse(manifest_text) else {
        return false;
    };
    let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let workflow_hash = digest(&expected.workflow_api_json);
    let recipe_hash = digest(&expected.recipe_yaml);
    let snapshot = serde_json::from_str::<serde_json::Value>(&version.api_workflow_json);
    let graph = serde_json::from_slice::<serde_json::Value>(&expected.workflow_api_json);
    manifest.validate().is_ok()
        && manifest.workflow_version == "2.2.0"
        && manifest.recipe_version == "2.2.0"
        && version.workflow_id == manifest.id
        && version.workflow_version == manifest.workflow_version
        && recipe.version == manifest.recipe_version
        && version.workflow_sha256 == workflow_hash
        && artifact.workflow_sha256 == workflow_hash
        && recipe.recipe_sha256 == recipe_hash
        && artifact.recipe_sha256 == recipe_hash
        && recipe.recipe_yaml.as_bytes() == expected.recipe_yaml
        && matches!((snapshot, graph), (Ok(a), Ok(b)) if a == b)
        && live.manifest_yaml == expected.manifest_yaml
        && live.recipe_yaml == expected.recipe_yaml
        && live.workflow_api_json == expected.workflow_api_json
}

/// User-imported video workflows (explicitly allowed by the product owner).
/// The exact pair is authorized only when its canonical USER artifact, the
/// immutable version/recipe snapshots and the live package bytes all agree and
/// the recipe declares only video outputs. Lifecycle (removed/disabled/archived)
/// stays in Registry; execution still flows through the Production Queue.
pub fn authorizes_user_video(
    version: &RuntimeWorkflowVersionRecord,
    recipe: &RuntimeRecipeRecord,
    artifact: &WorkflowRuntimeArtifactRecord,
    live: &WorkflowPackageBytes,
) -> bool {
    if artifact.source_kind != "USER"
        || version.library_removed
        || artifact.workflow_version_id != version.workflow_version_id
        || artifact.recipe_id != recipe.recipe_id
        || crate::application::product_runtime_scope::is_retired_h3_workflow(&version.workflow_id)
    {
        return false;
    }
    let Ok(parsed) = crate::compiler::RecipeParser::parse(&recipe.recipe_yaml) else {
        return false;
    };
    if parsed.outputs.is_empty()
        || !parsed
            .outputs
            .iter()
            .all(|o| matches!(o.output_type, crate::domain::OutputType::Video))
    {
        return false;
    }
    let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let snapshot = serde_json::from_str::<serde_json::Value>(&version.api_workflow_json);
    let graph = serde_json::from_slice::<serde_json::Value>(&live.workflow_api_json);
    artifact.workflow_sha256 == version.workflow_sha256
        && artifact.recipe_sha256 == recipe.recipe_sha256
        && digest(recipe.recipe_yaml.as_bytes()) == recipe.recipe_sha256
        && live.recipe_yaml == recipe.recipe_yaml.as_bytes()
        && matches!((snapshot, graph), (Ok(a), Ok(b)) if a == b)
}

#[cfg(test)]
mod user_video_tests {
    use super::*;

    fn recipe_yaml(kind: &str) -> String {
        format!("schema_version: 1\nid: user_v\nname: User V\nworkflow:\n  file: workflow_api.json\ninputs: {{}}\nbindings: []\noutputs:\n- id: output_1\n  node: '92'\n  required: true\n  type: {kind}\n")
    }

    fn fixture(
        kind: &str,
    ) -> (
        RuntimeWorkflowVersionRecord,
        RuntimeRecipeRecord,
        WorkflowRuntimeArtifactRecord,
        WorkflowPackageBytes,
    ) {
        let yaml = recipe_yaml(kind);
        let sha = |b: &[u8]| format!("{:x}", Sha256::digest(b));
        let graph = r#"{"92":{"class_type":"SaveVideo","inputs":{}}}"#;
        let recipe = RuntimeRecipeRecord {
            recipe_id: "rcp_u".into(),
            version: "1.0.0".into(),
            schema_version: 1,
            recipe_sha256: sha(yaml.as_bytes()),
            recipe_yaml: yaml.clone(),
        };
        let version = RuntimeWorkflowVersionRecord {
            workflow_version_id: "wfv_u".into(),
            workflow_id: "wfl_user".into(),
            name: "U".into(),
            category: "video".into(),
            mode: "image_to_video".into(),
            workflow_version: "1.0.0".into(),
            workflow_sha256: "w".repeat(64),
            api_workflow_json: graph.into(),
            package_name: Some("pkg".into()),
            is_current: true,
            recipes: vec![recipe.clone()],
            active_tasks: 0,
            total_tasks: 0,
            has_successful_run: false,
            latest_success_at: None,
            latest_failure_at: None,
            library_removed: false,
        };
        let artifact = WorkflowRuntimeArtifactRecord {
            id: "a".into(),
            workflow_version_id: "wfv_u".into(),
            recipe_id: "rcp_u".into(),
            package_name: "pkg".into(),
            source_kind: "USER".into(),
            package_source_path: None,
            workflow_sha256: "w".repeat(64),
            recipe_sha256: recipe.recipe_sha256.clone(),
            created_at: chrono::Utc::now(),
        };
        let live = WorkflowPackageBytes {
            manifest_yaml: Vec::new(),
            recipe_yaml: yaml.into_bytes(),
            workflow_api_json: graph.as_bytes().to_vec(),
            source_workflow_json: None,
            recognition_metadata_json: None,
        };
        (version, recipe, artifact, live)
    }

    #[test]
    fn user_video_workflow_with_consistent_snapshots_is_authorized() {
        let (v, r, a, l) = fixture("video");
        assert!(authorizes_user_video(&v, &r, &a, &l));
        assert!(
            !authorizes(&v, &r, &a, &l),
            "PRODUCT path stays package-pinned"
        );
    }

    #[test]
    fn user_image_workflow_is_never_authorized() {
        let (v, r, a, l) = fixture("image");
        assert!(!authorizes_user_video(&v, &r, &a, &l));
    }

    #[test]
    fn user_video_rejects_drift_wrong_source_and_retired_h3() {
        let (v, r, mut a, l) = fixture("video");
        a.source_kind = "PRODUCT".into();
        assert!(!authorizes_user_video(&v, &r, &a, &l));
        let (v, r, a, mut l) = fixture("video");
        l.workflow_api_json = br#"{"92":{"class_type":"Other","inputs":{}}}"#.to_vec();
        assert!(!authorizes_user_video(&v, &r, &a, &l));
        let (v, r, mut a, l) = fixture("video");
        a.workflow_sha256 = "x".repeat(64);
        assert!(!authorizes_user_video(&v, &r, &a, &l));
        let (v, r, a, mut l) = fixture("video");
        l.recipe_yaml.push(b'\n');
        assert!(!authorizes_user_video(&v, &r, &a, &l));
        let (mut v, r, a, l) = fixture("video");
        v.library_removed = true;
        assert!(!authorizes_user_video(&v, &r, &a, &l));
        let (mut v, r, a, l) = fixture("video");
        v.workflow_id = "wfl_minimax_h3_fl2va_compatible".into();
        assert!(!authorizes_user_video(&v, &r, &a, &l));
    }
}
