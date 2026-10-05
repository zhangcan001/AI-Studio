use crate::application::{
    asset_query_service::AssetSummaryView, pagination::PageCursor,
    prompt_library_service::PromptEntryView,
};
use crate::domain::consistency::{ConsistencyProfileRecord, ProfileType, ReferenceSetPurpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ResourceRef {
    Asset { id: String },
    Prompt { id: String },
    Profile { id: String },
    ReferenceSet { id: String },
}

impl ResourceRef {
    pub fn id(&self) -> &str {
        match self {
            Self::Asset { id }
            | Self::Prompt { id }
            | Self::Profile { id }
            | Self::ReferenceSet { id } => id,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LibraryCategory {
    All,
    Media,
    Images,
    Videos,
    Audio,
    Prompts,
    Profiles,
    ReferenceSets,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryCursor {
    pub project_id: String,
    pub category: LibraryCategory,
    pub keyword: Option<String>,
    pub position: PageCursor,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryQuery {
    pub category: LibraryCategory,
    pub keyword: Option<String>,
    pub cursor: Option<LibraryCursor>,
    pub limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LibraryCoverage {
    RecentSummary,
    KeysetPage,
    CompleteCategory,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub resource_ref: ResourceRef,
    pub title: String,
    pub subtype: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub thumbnail_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryList {
    pub items: Vec<LibraryItem>,
    pub next_cursor: Option<LibraryCursor>,
    pub coverage: LibraryCoverage,
    pub coverage_message: &'static str,
}

/// Typed domain profile content is kept distinct from prompt text. No JSON patch.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LibraryDetail {
    Asset {
        asset: AssetSummaryView,
    },
    Prompt {
        prompt: PromptEntryView,
    },
    Profile {
        profile: ConsistencyProfileRecord,
    },
    ReferenceSet {
        #[serde(rename = "referenceSet")]
        reference_set: LibraryReferenceSet,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryReferenceSet {
    pub id: String,
    pub name: String,
    pub description: String,
    pub purpose: ReferenceSetPurpose,
    pub owner_profile_type: Option<ProfileType>,
    pub owner_profile_id: Option<String>,
    pub active_revision_id: Option<String>,
    pub items: Vec<LibraryReferenceMember>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryReferenceMember {
    pub asset_id: String,
    pub ordinal: i64,
    pub role: Option<String>,
    pub is_primary: bool,
    pub asset_name: String,
    pub thumbnail_available: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRelation {
    pub kind: LibraryRelationKind,
    pub title: String,
    pub description: String,
    pub blocking: bool,
    pub location: Option<LibraryRelationLocation>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LibraryRelationKind {
    ShotReference,
    ShotSelectedResult,
    GenerationSnapshotInput,
    GenerationOutput,
    RunResult,
    ReferenceSetMember,
    ProfileRelation,
    ActiveTask,
    ActiveProduction,
    LegacyReviewHistory,
    ArtifactReviewPlaceholder,
    ArtifactReviewMeaningful,
    GenerationAssetVersionLineage,
    ProfileUsage,
    ReferenceSetUsage,
    PromptVersionUsage,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LibraryRelationLocation {
    Shot {
        id: String,
        stage: Option<String>,
    },
    Run {
        #[serde(rename = "runRef")]
        run_ref: super::super::run_facade::RunRef,
    },
    Resource {
        resource: ResourceRef,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDeletionInspection {
    pub allowed: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub relations: Vec<LibraryRelation>,
    pub consequences: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LibraryVersions {
    Asset {
        versions: Vec<LibraryAssetVersion>,
    },
    Prompt {
        versions: Vec<crate::application::prompt_library_service::PromptVersionView>,
    },
    Unsupported {
        reason: &'static str,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAssetVersion {
    pub id: String,
    pub version_number: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LibraryCreateIntent {
    Asset {
        project_id: String,
        asset_id: String,
        media_kind: String,
    },
    Prompt {
        project_id: String,
        prompt_id: String,
        prompt_version_id: String,
        text: String,
        model_version_id: Option<String>,
    },
    Context {
        project_id: String,
        resource: ResourceRef,
        message: &'static str,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LibraryEditRequest {
    Prompt {
        id: String,
        text: String,
        model_version_id: Option<String>,
    },
    Profile {
        id: String,
        name: String,
    },
    ReferenceSet {
        id: String,
        name: String,
        description: String,
        items: Vec<LibraryReferenceMemberEdit>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryReferenceMemberEdit {
    pub asset_id: String,
    pub ordinal: i64,
    pub role: Option<String>,
    pub is_primary: bool,
}
