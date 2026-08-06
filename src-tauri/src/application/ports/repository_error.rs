use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepositoryError {
    Database {
        message: String,
    },
    Serialization {
        context: String,
        message: String,
    },
    NotFound {
        entity: String,
        id: String,
    },
    Integrity {
        message: String,
    },
    WorkflowVersionConflict {
        workflow_id: String,
        version: String,
    },
    RecipeVersionConflict {
        workflow_version_id: String,
        version: String,
    },
}

impl RepositoryError {
    pub fn database(message: impl Into<String>) -> Self {
        Self::Database {
            message: message.into(),
        }
    }

    pub fn serialization(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Serialization {
            context: context.into(),
            message: message.into(),
        }
    }

    pub fn not_found(entity: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            entity: entity.into(),
            id: id.into(),
        }
    }

    pub fn integrity(message: impl Into<String>) -> Self {
        Self::Integrity {
            message: message.into(),
        }
    }

    pub fn workflow_version_conflict(
        workflow_id: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self::WorkflowVersionConflict {
            workflow_id: workflow_id.into(),
            version: version.into(),
        }
    }

    pub fn recipe_version_conflict(
        workflow_version_id: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self::RecipeVersionConflict {
            workflow_version_id: workflow_version_id.into(),
            version: version.into(),
        }
    }
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database { message } => write!(formatter, "database error: {message}"),
            Self::Serialization { context, message } => {
                write!(formatter, "serialization error in {context}: {message}")
            }
            Self::NotFound { entity, id } => write!(formatter, "{entity} \"{id}\" was not found"),
            Self::Integrity { message } => {
                write!(formatter, "repository integrity error: {message}")
            }
            Self::WorkflowVersionConflict { workflow_id, version } => write!(
                formatter,
                "WORKFLOW_VERSION_CONFLICT: workflow {workflow_id} version {version} has different content"
            ),
            Self::RecipeVersionConflict {
                workflow_version_id,
                version,
            } => write!(
                formatter,
                "RECIPE_VERSION_CONFLICT: workflow version {workflow_version_id} recipe version {version} has different content"
            ),
        }
    }
}

impl Error for RepositoryError {}
