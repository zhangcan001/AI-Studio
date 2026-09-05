use crate::application::ports::{
    WorkflowLibrarySource, WorkflowLibrarySourceError, WorkflowPackageFiles, WorkflowPackageLoad,
};
use async_trait::async_trait;
use std::{fs, path::PathBuf};

#[derive(Clone, Debug)]
pub struct FileSystemWorkflowLibrarySource {
    root: PathBuf,
}

impl FileSystemWorkflowLibrarySource {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn read_package(package_path: PathBuf) -> WorkflowPackageLoad {
        let package_name = package_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_owned());

        let read = |name: &str| {
            fs::read_to_string(package_path.join(name))
                .map_err(|error| format!("read {name}: {error}"))
        };

        match (
            read("manifest.yaml"),
            read("recipe.yaml"),
            read("workflow_api.json"),
        ) {
            (Ok(manifest_yaml), Ok(recipe_yaml), Ok(workflow_json)) => {
                WorkflowPackageLoad::Loaded(WorkflowPackageFiles {
                    package_name,
                    package_source_path: Some(package_path.to_string_lossy().to_string()),
                    manifest_yaml,
                    recipe_yaml,
                    workflow_json,
                })
            }
            (manifest, recipe, workflow) => {
                let message = [manifest.err(), recipe.err(), workflow.err()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("; ");
                WorkflowPackageLoad::Invalid {
                    package_name,
                    message,
                }
            }
        }
    }
}

#[async_trait]
impl WorkflowLibrarySource for FileSystemWorkflowLibrarySource {
    async fn load_packages(&self) -> Result<Vec<WorkflowPackageLoad>, WorkflowLibrarySourceError> {
        let entries = fs::read_dir(&self.root).map_err(|error| WorkflowLibrarySourceError {
            message: format!("read {}: {error}", self.root.display()),
        })?;

        let mut packages = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| WorkflowLibrarySourceError {
                message: format!("read workflow package entry: {error}"),
            })?;
            let path = entry.path();
            let is_internal_directory = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'));
            if path.is_dir() && !is_internal_directory {
                packages.push(Self::read_package(path));
            }
        }
        packages.sort_by(|left, right| package_name(left).cmp(package_name(right)));
        Ok(packages)
    }
}

fn package_name(load: &WorkflowPackageLoad) -> &str {
    match load {
        WorkflowPackageLoad::Loaded(files) => &files.package_name,
        WorkflowPackageLoad::Invalid { package_name, .. } => package_name,
    }
}

#[cfg(test)]
mod tests {
    use super::FileSystemWorkflowLibrarySource;
    use crate::application::ports::{WorkflowLibrarySource, WorkflowPackageLoad};
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn loads_packages_and_keeps_one_broken_package_isolated() {
        let root = tempdir().expect("workflow library root");
        let valid = root.path().join("valid");
        fs::create_dir_all(&valid).unwrap();
        fs::write(valid.join("manifest.yaml"), "schema_version: 1").unwrap();
        fs::write(valid.join("recipe.yaml"), "schema_version: 1").unwrap();
        fs::write(valid.join("workflow_api.json"), "{}").unwrap();
        fs::create_dir_all(root.path().join("broken")).unwrap();

        let loads = FileSystemWorkflowLibrarySource::new(root.path().to_path_buf())
            .load_packages()
            .await
            .expect("source should load");
        assert_eq!(loads.len(), 2);
        assert!(loads.iter().any(|load| matches!(
            load,
            WorkflowPackageLoad::Loaded(files) if files.package_name == "valid"
        )));
        assert!(loads.iter().any(|load| matches!(
            load,
            WorkflowPackageLoad::Invalid { package_name, .. } if package_name == "broken"
        )));
    }
}
