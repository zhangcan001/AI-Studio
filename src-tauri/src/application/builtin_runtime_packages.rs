//! Immutable product-owned MiniMax H3 runtime packages.
//!
//! The package library remains user-data-backed so older packages are never
//! overwritten. These two audited packages are copied only when their exact
//! package directory is absent, then the normal library synchronizer validates
//! and registers them like every other runtime package.

use std::{fs, path::Path};

struct BuiltinPackage {
    directory: &'static str,
    manifest: &'static str,
    recipe: &'static str,
    workflow: &'static str,
}

struct BuiltinPackageIdentity {
    /// Package directory reserved by a product-owned runtime whose files are
    /// provisioned by the local runtime integration rather than embedded here.
    directory: &'static str,
}

const PACKAGES: &[BuiltinPackage] = &[
    BuiltinPackage {
        directory: "minimax_h3_fl2va_1_0_0",
        manifest: include_str!("../../runtime_packages/minimax_h3_fl2va_1_0_0/manifest.yaml"),
        recipe: include_str!("../../runtime_packages/minimax_h3_fl2va_1_0_0/recipe.yaml"),
        workflow: include_str!("../../runtime_packages/minimax_h3_fl2va_1_0_0/workflow_api.json"),
    },
    BuiltinPackage {
        directory: "minimax_h3_reference_video_1_3_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_1_3_0/manifest.yaml"
        ),
        recipe: include_str!("../../runtime_packages/minimax_h3_reference_video_1_3_0/recipe.yaml"),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_1_3_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_t2v_quality_2_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_i2v_quality_2_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_first_last_quality_2_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_reference_video_quality_2_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        manifest: include_str!(
            "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/workflow_api.json"
        ),
    },
];

// Kera2 is provisioned by the local image runtime, so its package files are
// not embedded in this H3-only source list. Keeping its package identity here
// still makes the builtin decision come from formal package metadata rather
// than from a workflow ID conditional in the deletion service.
const PRODUCT_PACKAGE_IDENTITIES: &[BuiltinPackageIdentity] = &[
    BuiltinPackageIdentity {
        directory: "kera2_t2i_local_v2",
    },
    BuiltinPackageIdentity {
        directory: "kera2_t2i_local_v2_1_1_0_1d99a10d",
    },
    BuiltinPackageIdentity {
        directory: "krea2_t2i_local",
    },
];

/// Built-in identity is derived from formal product Runtime Package sources,
/// not from workflow IDs. User-installed packages are never matched here.
pub fn is_builtin_package_name(package_name: &str) -> bool {
    PACKAGES
        .iter()
        .any(|package| package.directory == package_name)
        || PRODUCT_PACKAGE_IDENTITIES
            .iter()
            .any(|package| package.directory == package_name)
}

pub fn ensure_installed(root: &Path) -> Result<(), String> {
    for package in PACKAGES {
        let directory = root.join(package.directory);
        if directory.exists() {
            continue;
        }
        fs::create_dir_all(&directory)
            .map_err(|error| format!("create builtin runtime package: {error}"))?;
        if let Err(error) = (|| {
            fs::write(directory.join("manifest.yaml"), package.manifest)
                .map_err(|error| format!("write manifest.yaml: {error}"))?;
            fs::write(directory.join("recipe.yaml"), package.recipe)
                .map_err(|error| format!("write recipe.yaml: {error}"))?;
            fs::write(directory.join("workflow_api.json"), package.workflow)
                .map_err(|error| format!("write workflow_api.json: {error}"))?;
            Ok::<(), String>(())
        })() {
            let _ = fs::remove_dir_all(&directory);
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_installed, is_builtin_package_name, PACKAGES};
    use crate::application::workflow_manifest::WorkflowManifest;
    use crate::compiler::{BindingValidator, RecipeParser, RecipeValidator, WorkflowValidator};
    use crate::domain::WorkflowDocument;
    use tempfile::tempdir;

    #[test]
    fn installs_missing_h3_packages_without_overwriting_existing_directory() {
        let directory = tempdir().expect("temp directory");
        ensure_installed(directory.path()).expect("builtin packages should install");
        let fl2va = directory.path().join("minimax_h3_fl2va_1_0_0");
        let ref2va = directory.path().join("minimax_h3_reference_video_1_3_0");
        let quality_t2v = directory.path().join("minimax_h3_fl2va_t2v_quality_2_0_0");
        let quality_ref = directory
            .path()
            .join("minimax_h3_reference_video_quality_2_0_0");
        assert!(fl2va.join("manifest.yaml").is_file());
        assert!(ref2va.join("recipe.yaml").is_file());
        assert!(quality_t2v.join("workflow_api.json").is_file());
        assert!(quality_ref.join("recipe.yaml").is_file());
        let sentinel = fl2va.join("sentinel.txt");
        std::fs::write(&sentinel, "keep").expect("sentinel");
        ensure_installed(directory.path()).expect("second install should be a no-op");
        assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "keep");
    }

    #[test]
    fn embedded_h3_packages_pass_the_same_contract_audit_as_user_packages() {
        for package in PACKAGES {
            let manifest = WorkflowManifest::parse(package.manifest).expect("manifest parses");
            manifest.validate().expect("manifest validates");
            let recipe = RecipeParser::parse(package.recipe).expect("recipe parses");
            RecipeValidator::validate(&recipe).expect("recipe validates");
            let workflow_value: serde_json::Value =
                serde_json::from_str(package.workflow).expect("workflow JSON parses");
            let workflow = WorkflowDocument::parse(workflow_value).expect("workflow parses");
            WorkflowValidator::validate(&workflow).expect("workflow validates");
            BindingValidator::validate(&recipe, &workflow).expect("bindings validate");
        }
    }

    #[test]
    fn product_package_identity_marks_kera2_builtin_without_workflow_id_logic() {
        assert!(is_builtin_package_name("kera2_t2i_local_v2"));
        assert!(is_builtin_package_name("kera2_t2i_local_v2_1_1_0_1d99a10d"));
        assert!(is_builtin_package_name("krea2_t2i_local"));
        assert!(!is_builtin_package_name("custom_kera2_copy"));
    }

    #[test]
    fn aitudou_fast_package_keeps_fixed_eight_step_graph_and_safe_user_inputs() {
        let package = PACKAGES
            .iter()
            .find(|package| package.directory == "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0")
            .expect("AITUDOU package");
        let manifest = WorkflowManifest::parse(package.manifest).expect("manifest parses");
        assert_eq!(manifest.id, "wfl_aitudou_minimax_h3_lightx2v_8step_fast");
        assert_eq!(manifest.category, "video");
        let recipe = RecipeParser::parse(package.recipe).expect("recipe parses");
        assert_eq!(recipe.inputs.len(), 2);
        assert!(recipe.inputs.contains_key("prompt"));
        assert!(recipe.inputs.contains_key("seed"));
        assert!(recipe
            .bindings
            .iter()
            .all(|binding| { binding.target.node == "59" || binding.target.node == "2" }));
        let workflow: serde_json::Value = serde_json::from_str(package.workflow).unwrap();
        assert_eq!(workflow["50"]["inputs"]["steps"], 8);
        assert_eq!(workflow["61"]["inputs"]["megapixels"], 0.9);
        assert_eq!(workflow["62"]["class_type"], "VHS_VideoCombine");
    }

    #[test]
    fn quality_graphs_restore_the_formal_sampling_chain_without_touching_fast_graphs() {
        let fast_fl2va: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_fl2va_1_0_0")
                .expect("fast fl2va package")
                .workflow,
        )
        .unwrap();
        let fast_ref2va: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_reference_video_1_3_0")
                .expect("fast ref2va package")
                .workflow,
        )
        .unwrap();
        assert_eq!(fast_fl2va["23"]["inputs"]["steps"], 4);
        assert_eq!(fast_ref2va["23"]["inputs"]["steps"], 4);
        assert!(fast_fl2va.get("27").is_some());
        assert!(fast_ref2va.get("27").is_some());

        for directory in [
            "minimax_h3_fl2va_t2v_quality_2_0_0",
            "minimax_h3_fl2va_i2v_quality_2_0_0",
            "minimax_h3_fl2va_first_last_quality_2_0_0",
            "minimax_h3_reference_video_quality_2_0_0",
        ] {
            let package = PACKAGES
                .iter()
                .find(|package| package.directory == directory)
                .expect("quality package");
            let workflow: serde_json::Value = serde_json::from_str(package.workflow).unwrap();
            assert_eq!(workflow["23"]["inputs"]["steps"], 20);
            assert_eq!(
                workflow["13"]["inputs"]["unet_name"]
                    .as_str()
                    .unwrap()
                    .contains("convrot"),
                true
            );
            assert!(
                workflow.get("27").is_none(),
                "Turbo LoRA must be absent in {directory}"
            );
            assert_eq!(
                workflow["16"]["class_type"],
                "MiniMaxH3MemoryEfficientSageAttentionPatch"
            );
        }

        let t2v: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_fl2va_t2v_quality_2_0_0")
                .unwrap()
                .workflow,
        )
        .unwrap();
        assert_eq!(t2v["26"]["inputs"]["mode"], "Middle-36");
        assert_eq!(t2v["26"]["inputs"]["manual_bypass_blocks"], 36);

        let i2v: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_fl2va_i2v_quality_2_0_0")
                .unwrap()
                .workflow,
        )
        .unwrap();
        assert_eq!(
            i2v["13"]["inputs"]["unet_name"],
            "minmaxh3\\minimax_h3_fl2va_int8_convrot.safetensors"
        );
        assert!(i2v.get("26").is_none());
        assert!(i2v["14"]["inputs"].get("first_frame").is_some());
        assert!(i2v["14"]["inputs"].get("last_frame").is_none());

        let first_last: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_fl2va_first_last_quality_2_0_0")
                .unwrap()
                .workflow,
        )
        .unwrap();
        assert!(first_last["14"]["inputs"].get("first_frame").is_some());
        assert!(first_last["14"]["inputs"].get("last_frame").is_some());

        let ref2va: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_reference_video_quality_2_0_0")
                .unwrap()
                .workflow,
        )
        .unwrap();
        assert_eq!(
            ref2va["13"]["inputs"]["unet_name"],
            "minmaxh3\\minimax_h3_ref2va_int8_convrot.safetensors"
        );
        assert_eq!(ref2va["26"]["inputs"]["mode"], "Middle-36");
        assert!(ref2va["14"]["inputs"]
            .get("ref_videos.ref_video_0")
            .is_some());
        assert!(ref2va["14"]["inputs"]
            .get("ref_video_audios.ref_video_audio_0")
            .is_some());
    }
}
