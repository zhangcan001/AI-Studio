use chrono::{DateTime, Utc};
use std::{error::Error, fmt};
use uuid::Uuid;

macro_rules! structure_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!(concat!($prefix, "{}"), Uuid::new_v4().simple()))
            }

            pub fn parse(value: impl Into<String>) -> Result<Self, ProductionStructureDomainError> {
                let value = value.into();
                if value.starts_with($prefix) && value.len() > $prefix.len() {
                    Ok(Self(value))
                } else {
                    Err(ProductionStructureDomainError::InvalidId(value))
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

structure_id!(ProductionSeriesId, "ser_");
structure_id!(ProductionEpisodeId, "epi_");
structure_id!(ProductionSceneId, "scn_");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionSeries {
    pub id: ProductionSeriesId,
    pub project_id: String,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionEpisode {
    pub id: ProductionEpisodeId,
    pub series_id: ProductionSeriesId,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionScene {
    pub id: ProductionSceneId,
    pub episode_id: ProductionEpisodeId,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotSceneAssignment {
    pub shot_id: String,
    pub scene_id: ProductionSceneId,
    pub ordinal: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductionStructureDomainError {
    InvalidId(String),
}

impl fmt::Display for ProductionStructureDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid production structure id: {value}"),
        }
    }
}

impl Error for ProductionStructureDomainError {}
