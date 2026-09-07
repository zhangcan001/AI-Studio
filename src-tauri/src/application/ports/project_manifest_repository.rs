use super::RepositoryError;
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct ProjectManifestSnapshot {
    pub project: ManifestProjectRecord,
    pub series: Vec<ManifestSeriesRecord>,
    pub episodes: Vec<ManifestEpisodeRecord>,
    pub scenes: Vec<ManifestSceneRecord>,
    pub assignments: Vec<ManifestAssignmentRecord>,
    pub shots: Vec<ManifestShotRecord>,
    pub configs: Vec<ManifestStageConfigRecord>,
    pub references: Vec<ManifestReferenceRecord>,
    pub links: Vec<ManifestGenerationLinkRecord>,
    pub anchors: Vec<ManifestAnchorRecord>,
    pub anchor_assets: Vec<ManifestAnchorAssetRecord>,
    pub consistency: ManifestConsistencyRecords,
}

#[derive(Clone, Debug)]
pub struct ManifestProjectRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestSeriesRecord {
    pub id: String,
    pub ordinal: i64,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct ManifestEpisodeRecord {
    pub id: String,
    pub series_id: String,
    pub ordinal: i64,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct ManifestSceneRecord {
    pub id: String,
    pub episode_id: String,
    pub ordinal: i64,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct ManifestAssignmentRecord {
    pub shot_id: String,
    pub scene_id: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug)]
pub struct ManifestShotRecord {
    pub id: String,
    pub ordinal: i64,
    pub name: String,
    pub prompt_text: String,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
    pub selected_image_asset_id: Option<String>,
    pub selected_video_asset_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestStageConfigRecord {
    pub shot_id: String,
    pub stage: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub scalar_values_json: String,
}

#[derive(Clone, Debug)]
pub struct ManifestReferenceRecord {
    pub shot_id: String,
    pub stage: String,
    pub asset_id: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug)]
pub struct ManifestGenerationLinkRecord {
    pub shot_id: String,
    pub stage: String,
    pub task_status: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestAnchorRecord {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct ManifestAnchorAssetRecord {
    pub anchor_id: String,
    pub asset_id: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug, Default)]
pub struct ManifestConsistencyRecords {
    pub character_profiles: Vec<ManifestCharacterProfileRecord>,
    pub scene_profiles: Vec<ManifestSceneProfileRecord>,
    pub prop_profiles: Vec<ManifestPropProfileRecord>,
    pub style_profiles: Vec<ManifestStyleProfileRecord>,
    pub costume_variants: Vec<ManifestCostumeVariantRecord>,
    pub reference_sets: Vec<ManifestReferenceSetRecord>,
    pub reference_set_items: Vec<ManifestReferenceSetItemRecord>,
    pub shot_profile_bindings: Vec<ManifestShotProfileBindingRecord>,
    pub shot_reference_set_bindings: Vec<ManifestShotReferenceSetBindingRecord>,
    pub scope_profile_bindings: Vec<ManifestScopeProfileBindingRecord>,
    pub scope_reference_set_bindings: Vec<ManifestScopeReferenceSetBindingRecord>,
}

#[derive(Clone, Debug)]
pub struct ManifestCharacterProfileRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: String,
    pub negative_prompt: String,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestSceneProfileRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub environment_prompt: String,
    pub lighting_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestPropProfileRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: String,
    pub material_prompt: Option<String>,
    pub scale_prompt: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestStyleProfileRecord {
    pub id: String,
    pub name: String,
    pub style_prompt: String,
    pub color_prompt: Option<String>,
    pub line_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub output_notes: Option<String>,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestCostumeVariantRecord {
    pub id: String,
    pub character_profile_id: String,
    pub name: String,
    pub prompt_fragment: String,
    pub reference_set_id: Option<String>,
    pub is_default: i64,
    pub ordinal: i64,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestReferenceSetRecord {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub description: String,
    pub owner_profile_type: Option<String>,
    pub owner_profile_id: Option<String>,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestReferenceSetItemRecord {
    pub reference_set_id: String,
    pub asset_id: String,
    pub ordinal: i64,
    pub role: Option<String>,
    pub is_primary: i64,
}

#[derive(Clone, Debug)]
pub struct ManifestShotProfileBindingRecord {
    pub id: String,
    pub shot_id: String,
    pub role: String,
    pub profile_type: String,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: String,
}

#[derive(Clone, Debug)]
pub struct ManifestShotReferenceSetBindingRecord {
    pub id: String,
    pub shot_id: String,
    pub role: String,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: i64,
    pub inheritance_mode: String,
}

#[derive(Clone, Debug)]
pub struct ManifestScopeProfileBindingRecord {
    pub id: String,
    pub project_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub role: String,
    pub profile_type: String,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: String,
}

#[derive(Clone, Debug)]
pub struct ManifestScopeReferenceSetBindingRecord {
    pub id: String,
    pub project_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub role: String,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: i64,
    pub inheritance_mode: String,
}

#[async_trait]
pub trait ProjectManifestRepository: Send + Sync {
    async fn load_manifest_snapshot(
        &self,
        project_id: &str,
    ) -> Result<ProjectManifestSnapshot, RepositoryError>;
}
