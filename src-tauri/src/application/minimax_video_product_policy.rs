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
