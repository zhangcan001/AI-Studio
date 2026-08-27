use crate::domain::script_draft::diagnostic::{Diagnostic, DiagnosticSeverity};
use crate::domain::script_draft::ids::{DraftId, DraftNodeId, DraftRevisionId, SourceId};
use crate::domain::script_draft::source::{ProviderMetadata, SourceSpan};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DRAFT_SCHEMA_VERSION: u32 = 1;
pub const DRAFT_CONTRACT_VERSION: u32 = 1;
pub const MAX_EPISODES: usize = 100;
pub const MAX_SCENES: usize = 1_000;
pub const MAX_SHOTS: usize = 5_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DraftStatus {
    Draft,
    Reviewed,
    Promoted,
    Archived,
}

impl DraftStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Reviewed => "REVIEWED",
            Self::Promoted => "PROMOTED",
            Self::Archived => "ARCHIVED",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DraftRevisionKind {
    Parsed,
    Reparsed,
    UserEdit,
    Review,
    Merge,
    Split,
    Reorder,
}

impl DraftRevisionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parsed => "PARSED",
            Self::Reparsed => "REPARSED",
            Self::UserEdit => "USER_EDIT",
            Self::Review => "REVIEW",
            Self::Merge => "MERGE",
            Self::Split => "SPLIT",
            Self::Reorder => "REORDER",
        }
    }

    pub fn try_from_storage(value: &str) -> Result<Self, &'static str> {
        match value {
            "PARSED" => Ok(Self::Parsed),
            "REPARSED" => Ok(Self::Reparsed),
            "USER_EDIT" => Ok(Self::UserEdit),
            "REVIEW" => Ok(Self::Review),
            "MERGE" => Ok(Self::Merge),
            "SPLIT" => Ok(Self::Split),
            "REORDER" => Ok(Self::Reorder),
            _ => Err("DRAFT_REVISION_KIND_INVALID"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DraftNodeOrigin {
    Imported,
    Ai,
    Human,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DraftReviewState {
    AiSuggested,
    PendingReview,
    Accepted,
    Rejected,
    Edited,
    Conflict,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityType {
    Character,
    Scene,
    Prop,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityMention {
    /// Stable within the revision payload; this is not a formal Profile ID.
    pub id: String,
    pub entity_type: EntityType,
    pub text: String,
    pub normalized_text: String,
    #[serde(default)]
    pub source_spans: Vec<SourceSpan>,
    pub confidence: Option<f32>,
    #[serde(default)]
    pub candidate_profile_ids: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub selected_profile_id: Option<String>,
    pub confirmed: bool,
}

impl Eq for EntityMention {}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftEpisode {
    pub draft_node_id: DraftNodeId,
    pub parent_draft_node_id: Option<DraftNodeId>,
    pub ordinal: u32,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub source_spans: Vec<SourceSpan>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub review_state: DraftReviewState,
    pub origin: DraftNodeOrigin,
    pub original_suggestion: Option<String>,
    pub current_value: Option<String>,
    #[serde(default)]
    pub scenes: Vec<DraftScene>,
}

impl Eq for DraftEpisode {}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftScene {
    pub draft_node_id: DraftNodeId,
    pub parent_draft_node_id: Option<DraftNodeId>,
    pub ordinal: u32,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub source_spans: Vec<SourceSpan>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub review_state: DraftReviewState,
    pub origin: DraftNodeOrigin,
    pub original_suggestion: Option<String>,
    pub current_value: Option<String>,
    pub scene_mention: Option<EntityMention>,
    #[serde(default)]
    pub shots: Vec<DraftShot>,
}

impl Eq for DraftScene {}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftShot {
    pub draft_node_id: DraftNodeId,
    pub parent_draft_node_id: Option<DraftNodeId>,
    pub parent_scene_draft_id: DraftNodeId,
    pub ordinal: u32,
    pub name: String,
    pub purpose: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub character_mentions: Vec<EntityMention>,
    pub scene_mention: Option<EntityMention>,
    #[serde(default)]
    pub prop_mentions: Vec<EntityMention>,
    pub action: Option<String>,
    pub dialogue: Option<String>,
    pub camera_suggestion: Option<String>,
    pub lighting_suggestion: Option<String>,
    pub duration_suggestion: Option<f32>,
    pub image_prompt_draft: Option<String>,
    pub video_prompt_draft: Option<String>,
    #[serde(default)]
    pub source_spans: Vec<SourceSpan>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub review_state: DraftReviewState,
    pub origin: DraftNodeOrigin,
    pub original_suggestion: Option<String>,
    pub current_value: Option<String>,
}

impl Eq for DraftShot {}

pub type Episode = DraftEpisode;
pub type Scene = DraftScene;
pub type Shot = DraftShot;
pub type DraftEpisodeV1 = DraftEpisode;
pub type DraftSceneV1 = DraftScene;
pub type DraftShotV1 = DraftShot;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftStructureV1 {
    pub schema_version: u32,
    pub contract_version: u32,
    pub draft_id: DraftId,
    pub source_id: SourceId,
    pub revision_id: DraftRevisionId,
    pub status: DraftStatus,
    #[serde(default)]
    pub episodes: Vec<DraftEpisode>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl Eq for DraftStructureV1 {}

impl DraftStructureV1 {
    pub fn new(draft_id: DraftId, source_id: SourceId, revision_id: DraftRevisionId) -> Self {
        Self {
            schema_version: DRAFT_SCHEMA_VERSION,
            contract_version: DRAFT_CONTRACT_VERSION,
            draft_id,
            source_id,
            revision_id,
            status: DraftStatus::Draft,
            episodes: Vec::new(),
            diagnostics: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn counts(&self) -> DraftCounts {
        let scenes = self
            .episodes
            .iter()
            .map(|episode| episode.scenes.len())
            .sum();
        let shots = self
            .episodes
            .iter()
            .flat_map(|episode| &episode.scenes)
            .map(|scene| scene.shots.len())
            .sum();
        DraftCounts {
            episodes: self.episodes.len(),
            scenes,
            shots,
        }
    }

    pub fn is_draft(&self) -> bool {
        self.status == DraftStatus::Draft
    }

    pub fn validate(
        &self,
        raw_source: &[u8],
        db_schema_version: u32,
    ) -> Result<(), crate::domain::script_draft::DraftValidationError> {
        crate::domain::script_draft::validate_structure(self, raw_source, db_schema_version)
    }

    pub fn checksum(&self) -> Result<String, crate::domain::script_draft::DraftValidationError> {
        crate::domain::script_draft::draft_checksum(self)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftCounts {
    pub episodes: usize,
    pub scenes: usize,
    pub shots: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftRevisionMetadata {
    pub draft_id: DraftId,
    pub source_id: SourceId,
    pub revision_id: DraftRevisionId,
    pub revision: u32,
    pub kind: DraftRevisionKind,
    pub schema_version: u32,
    pub contract_version: u32,
    pub source_checksum: String,
    pub draft_checksum: String,
    pub parser_version: String,
    pub provider_metadata: Option<ProviderMetadata>,
    pub previous_revision_id: Option<DraftRevisionId>,
    pub created_at: String,
    pub editor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftRevision {
    pub metadata: DraftRevisionMetadata,
    pub structure: DraftStructureV1,
}

impl Eq for DraftRevision {}

impl DraftRevision {
    pub fn is_immutable_kind(&self) -> bool {
        matches!(
            self.metadata.kind,
            DraftRevisionKind::Parsed
                | DraftRevisionKind::Reparsed
                | DraftRevisionKind::UserEdit
                | DraftRevisionKind::Review
                | DraftRevisionKind::Merge
                | DraftRevisionKind::Split
                | DraftRevisionKind::Reorder
        )
    }
}

pub type DraftRevisionV1 = DraftRevision;

pub fn has_blocking_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.blocks_confirm())
}

pub fn has_unresolved_nodes(structure: &DraftStructureV1) -> bool {
    structure.episodes.iter().any(|episode| {
        episode.review_state == DraftReviewState::Unresolved
            || episode.scenes.iter().any(|scene| {
                scene.review_state == DraftReviewState::Unresolved
                    || scene.shots.iter().any(|shot| {
                        shot.review_state == DraftReviewState::Unresolved
                            || shot.diagnostics.iter().any(|diagnostic| {
                                diagnostic.severity == DiagnosticSeverity::Blocker
                            })
                    })
            })
    })
}
