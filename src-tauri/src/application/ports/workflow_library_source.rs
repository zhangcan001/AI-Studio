use async_trait::async_trait;
use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowPackageFiles {
    pub package_name: String,
    pub manifest_yaml: String,
    pub recipe_yaml: String,
    pub workflow_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowPackageLoad {
    Loaded(WorkflowPackageFiles),
    Invalid {
        package_name: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowLibrarySourceError {
    pub message: String,
}

impl fmt::Display for WorkflowLibrarySourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "workflow library source error: {}", self.message)
    }
}

impl Error for WorkflowLibrarySourceError {}

#[async_trait]
pub trait WorkflowLibrarySource: Send + Sync {
    async fn load_packages(&self) -> Result<Vec<WorkflowPackageLoad>, WorkflowLibrarySourceError>;
}
