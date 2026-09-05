use crate::application::builtin_runtime_packages::is_builtin_package_name;
use crate::application::ports::{
    Clock, RepositoryError, WorkflowLibraryRepository, WorkflowLibrarySource, WorkflowPackageFiles,
    WorkflowPackageLoad, WorkflowPackageRecord, WorkflowPackageRegistration,
};
use crate::application::workflow_manifest::WorkflowManifest;
use crate::compiler::{BindingValidator, RecipeParser, RecipeValidator, WorkflowValidator};
use crate::domain::WorkflowDocument;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, sync::Arc};

pub struct WorkflowLibraryService {
    source: Arc<dyn WorkflowLibrarySource>,
    repository: Arc<dyn WorkflowLibraryRepository>,
    clock: Arc<dyn Clock>,
}

impl WorkflowLibraryService {
    pub fn new(
        source: Arc<dyn WorkflowLibrarySource>,
        repository: Arc<dyn WorkflowLibraryRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            source,
            repository,
            clock,
        }
    }

    pub async fn sync(&self) -> Result<WorkflowSyncReport, WorkflowLibraryServiceError> {
        let packages = self
            .source
            .load_packages()
            .await
            .map_err(|error| WorkflowLibraryServiceError::Source(error.to_string()))?;
        let mut report = WorkflowSyncReport {
            packages_found: packages.len() as u32,
            valid: 0,
            invalid: 0,
            inserted: 0,
            reused: 0,
            errors: Vec::new(),
        };

        for package in packages {
            let (package_name, loaded) = match package {
                WorkflowPackageLoad::Loaded(files) => {
                    let package_name = files.package_name.clone();
                    (package_name, Ok(files))
                }
                WorkflowPackageLoad::Invalid {
                    package_name,
                    message,
                } => (package_name, Err(message)),
            };

            let files = match loaded {
                Ok(files) => files,
                Err(message) => {
                    report.invalid += 1;
                    report.errors.push(WorkflowSyncError {
                        package: package_name,
                        code: "WORKFLOW_PACKAGE_INVALID".to_owned(),
                        message,
                    });
                    continue;
                }
            };

            match self.validate_and_register(files).await {
                Ok(WorkflowPackageRegistration::Inserted) => {
                    report.valid += 1;
                    report.inserted += 1;
                }
                Ok(WorkflowPackageRegistration::Reused) => {
                    report.valid += 1;
                    report.reused += 1;
                }
                Err(error) => {
                    report.invalid += 1;
                    report.errors.push(WorkflowSyncError {
                        package: package_name,
                        code: error.code().to_owned(),
                        message: error.to_string(),
                    });
                }
            }
        }

        Ok(report)
    }

    async fn validate_and_register(
        &self,
        files: WorkflowPackageFiles,
    ) -> Result<WorkflowPackageRegistration, WorkflowPackageServiceError> {
        let manifest = WorkflowManifest::parse(&files.manifest_yaml)
            .map_err(WorkflowPackageServiceError::Invalid)?;
        manifest
            .validate()
            .map_err(WorkflowPackageServiceError::Invalid)?;

        let recipe = RecipeParser::parse(&files.recipe_yaml)
            .map_err(|error| WorkflowPackageServiceError::Invalid(error.to_string()))?;
        RecipeValidator::validate(&recipe)
            .map_err(|error| WorkflowPackageServiceError::Invalid(error.to_string()))?;
        if recipe.workflow.file != "workflow_api.json" {
            return Err(WorkflowPackageServiceError::Invalid(
                "recipe.workflow.file must be workflow_api.json in a runtime package".to_owned(),
            ));
        }

        let workflow_value: serde_json::Value = serde_json::from_str(&files.workflow_json)
            .map_err(|error| {
                WorkflowPackageServiceError::Invalid(format!("invalid workflow_api.json: {error}"))
            })?;
        let workflow = WorkflowDocument::parse(workflow_value.clone())
            .map_err(|error| WorkflowPackageServiceError::Invalid(error.to_string()))?;
        WorkflowValidator::validate(&workflow)
            .map_err(|error| WorkflowPackageServiceError::Invalid(error.to_string()))?;
        BindingValidator::validate(&recipe, &workflow)
            .map_err(|error| WorkflowPackageServiceError::Invalid(error.to_string()))?;

        if recipe.schema_version != manifest.schema_version {
            return Err(WorkflowPackageServiceError::Invalid(
                "manifest and recipe schema_version must match".to_owned(),
            ));
        }

        let package = WorkflowPackageRecord {
            workflow_id: manifest.id,
            source_kind: if is_builtin_package_name(&files.package_name) {
                "PRODUCT".to_owned()
            } else {
                "USER".to_owned()
            },
            package_name: files.package_name.clone(),
            package_source_path: files.package_source_path.clone(),
            name: manifest.name,
            category: manifest.category,
            mode: manifest.mode,
            workflow_version: manifest.workflow_version,
            workflow_json: workflow_value,
            workflow_sha256: sha256(files.workflow_json.as_bytes()),
            recipe_version: manifest.recipe_version,
            recipe_schema_version: recipe.schema_version,
            recipe_yaml: files.recipe_yaml.clone(),
            recipe_sha256: sha256(files.recipe_yaml.as_bytes()),
            created_at: self.clock.now(),
        };
        self.repository
            .register_package(&package)
            .await
            .map_err(WorkflowPackageServiceError::Repository)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSyncError {
    pub package: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSyncReport {
    pub packages_found: u32,
    pub valid: u32,
    pub invalid: u32,
    pub inserted: u32,
    pub reused: u32,
    pub errors: Vec<WorkflowSyncError>,
}

#[derive(Debug)]
enum WorkflowPackageServiceError {
    Invalid(String),
    Repository(RepositoryError),
}

impl WorkflowPackageServiceError {
    fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "WORKFLOW_PACKAGE_INVALID",
            Self::Repository(RepositoryError::WorkflowVersionConflict { .. }) => {
                "WORKFLOW_VERSION_CONFLICT"
            }
            Self::Repository(RepositoryError::RecipeVersionConflict { .. }) => {
                "RECIPE_VERSION_CONFLICT"
            }
            Self::Repository(_) => "DATABASE_ERROR",
        }
    }
}

impl fmt::Display for WorkflowPackageServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(formatter, "WORKFLOW_PACKAGE_INVALID: {message}"),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for WorkflowPackageServiceError {}

#[derive(Debug)]
pub enum WorkflowLibraryServiceError {
    Source(String),
}

impl fmt::Display for WorkflowLibraryServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(message) => write!(formatter, "WORKFLOW_LIBRARY_ERROR: {message}"),
        }
    }
}

impl Error for WorkflowLibraryServiceError {}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::WorkflowLibraryService;
    use crate::application::ports::{
        Clock, WorkflowLibrarySource, WorkflowPackageFiles, WorkflowPackageLoad,
    };
    use crate::infrastructure::database::{
        initialize, SqliteGenerationDefinitionRepository, SqliteWorkflowLibraryRepository,
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[derive(Clone, Copy)]
    struct TestClock;

    impl Clock for TestClock {
        fn now(&self) -> DateTime<Utc> {
            Utc::now()
        }
    }

    struct Source {
        package: WorkflowPackageFiles,
    }

    #[async_trait]
    impl WorkflowLibrarySource for Source {
        async fn load_packages(
            &self,
        ) -> Result<Vec<WorkflowPackageLoad>, super::super::ports::WorkflowLibrarySourceError>
        {
            Ok(vec![WorkflowPackageLoad::Loaded(self.package.clone())])
        }
    }

    fn package() -> WorkflowPackageFiles {
        WorkflowPackageFiles {
            package_name: "simple".to_owned(),
            package_source_path: None,
            manifest_yaml: "schema_version: 1\nid: wfl_simple\nname: Simple\nworkflow_version: 1.0.0\nrecipe_version: 1.0.0\ncategory: image\nmode: text_to_image\n".to_owned(),
            recipe_yaml: "schema_version: 1\nid: simple\nname: Simple\nworkflow:\n  file: workflow_api.json\ninputs: {}\nbindings: []\noutputs: []\n".to_owned(),
            workflow_json: "{\"3\":{\"inputs\":{},\"class_type\":\"KSampler\"}}".to_owned(),
        }
    }

    #[tokio::test]
    async fn sync_validates_and_registers_package() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        let repository = Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone()));
        let service = WorkflowLibraryService::new(
            Arc::new(Source { package: package() }),
            repository,
            Arc::new(TestClock),
        );
        let report = service.sync().await.unwrap();
        assert_eq!(report.packages_found, 1);
        assert_eq!(report.valid, 1);
        assert_eq!(report.inserted, 1);
        let definitions =
            crate::application::ports::GenerationDefinitionRepository::list_available(
                &SqliteGenerationDefinitionRepository::new(pool),
            )
            .await
            .unwrap();
        assert_eq!(definitions.len(), 1);
        assert!(definitions[0].recipe_yaml.contains("schema_version"));
    }
}
