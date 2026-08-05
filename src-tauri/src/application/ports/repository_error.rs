use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepositoryError {
    Database { message: String },
    Serialization { context: String, message: String },
    NotFound { entity: String, id: String },
    Integrity { message: String },
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
        }
    }
}

impl Error for RepositoryError {}
