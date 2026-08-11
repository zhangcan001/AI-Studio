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
];

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
    use super::{ensure_installed, PACKAGES};
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
        assert!(fl2va.join("manifest.yaml").is_file());
        assert!(ref2va.join("recipe.yaml").is_file());
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
}
