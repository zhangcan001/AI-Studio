use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShotVideoInputScope {
    pub project_id: String,
    pub shot_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShotVideoInputToken {
    pub instance_id: String,
    pub revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShotVideoInputAsset {
    pub input_key: String,
    pub ordinal: i64,
    pub asset_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShotVideoInputSet {
    pub scope: ShotVideoInputScope,
    pub token: ShotVideoInputToken,
    pub inputs: Vec<ShotVideoInputAsset>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ShotVideoInputRepository: Send + Sync {
    async fn find(
        &self,
        scope: &ShotVideoInputScope,
    ) -> Result<Option<ShotVideoInputSet>, RepositoryError>;

    /// Atomic full replacement. None means an OCC conflict and no changes.
    /// Empty inputs keep the header, incrementing revision instead of resetting it.
    async fn replace(
        &self,
        scope: &ShotVideoInputScope,
        expected: Option<&ShotVideoInputToken>,
        inputs: &[ShotVideoInputAsset],
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShotVideoInputSet>, RepositoryError>;
}
