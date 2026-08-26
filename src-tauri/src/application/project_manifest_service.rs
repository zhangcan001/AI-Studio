use crate::domain::{
    derive_stage_status, BindingRole, InheritanceMode, ProfileType, ReferenceSetPurpose, ShotStage,
    TaskStatus,
};
use crate::error::AppError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MANIFEST_FORMAT: &str = "ai-studio-project-manifest";
const MANIFEST_VERSION: u32 = 2;

#[derive(Clone)]
pub struct ProjectManifestService {
    pool: SqlitePool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifestExportView {
    pub file_name: String,
    pub bytes: u64,
    pub series_count: usize,
    pub episode_count: usize,
    pub scene_count: usize,
    pub shot_count: usize,
    pub unassigned_shot_count: usize,
    pub anchor_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub format: String,
    pub version: u32,
    pub generated_at: String,
    pub project: ManifestProject,
    pub structure: ManifestStructure,
    pub shots: Vec<ManifestShot>,
    pub reference_anchors: Vec<ManifestReferenceAnchor>,
    #[serde(default)]
    pub profiles: Vec<ManifestProfile>,
    #[serde(default)]
    pub costume_variants: Vec<ManifestCostumeVariant>,
    #[serde(default)]
    pub reference_sets: Vec<ManifestReferenceSet>,
    #[serde(default)]
    pub reference_set_items: Vec<ManifestReferenceSetItem>,
    #[serde(default)]
    pub shot_profile_bindings: Vec<ManifestShotProfileBinding>,
    #[serde(default)]
    pub shot_reference_set_bindings: Vec<ManifestShotReferenceSetBinding>,
    #[serde(default)]
    pub scope_profile_bindings: Vec<ManifestScopeProfileBinding>,
    #[serde(default)]
    pub scope_reference_set_bindings: Vec<ManifestScopeReferenceSetBinding>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestProject {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestStructure {
    pub series: Vec<ManifestSeries>,
    pub unassigned_shot_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSeries {
    pub id: String,
    pub ordinal: i64,
    pub name: String,
    pub description: String,
    pub episodes: Vec<ManifestEpisode>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEpisode {
    pub id: String,
    pub ordinal: i64,
    pub name: String,
    pub description: String,
    pub scenes: Vec<ManifestScene>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestScene {
    pub id: String,
    pub ordinal: i64,
    pub name: String,
    pub description: String,
    pub shot_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestShot {
    pub id: String,
    pub global_ordinal: i64,
    pub name: String,
    pub scene_id: Option<String>,
    pub scene_ordinal: Option<i64>,
    pub prompt_text: String,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
    pub image_status: String,
    pub video_status: String,
    pub selected_image_asset_id: Option<String>,
    pub selected_video_asset_id: Option<String>,
    pub stage_configs: Vec<ManifestStageConfig>,
    pub references: ManifestReferences,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestReferences {
    pub image: Vec<ManifestReference>,
    pub video: Vec<ManifestReference>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestReference {
    pub asset_id: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestStageConfig {
    pub stage: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestReferenceAnchor {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub ordered_asset_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestProfile {
    pub id: String,
    pub profile_type: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub environment_prompt: Option<String>,
    pub lighting_prompt: Option<String>,
    pub material_prompt: Option<String>,
    pub scale_prompt: Option<String>,
    pub style_prompt: Option<String>,
    pub color_prompt: Option<String>,
    pub line_prompt: Option<String>,
    pub output_notes: Option<String>,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCostumeVariant {
    pub id: String,
    pub character_profile_id: String,
    pub name: String,
    pub prompt_fragment: String,
    pub reference_set_id: Option<String>,
    pub is_default: bool,
    pub ordinal: i64,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestReferenceSet {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub description: String,
    pub owner_profile_type: Option<String>,
    pub owner_profile_id: Option<String>,
    pub active_revision_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestReferenceSetItem {
    pub reference_set_id: String,
    pub asset_id: String,
    pub ordinal: i64,
    pub role: Option<String>,
    pub is_primary: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestShotProfileBinding {
    pub id: String,
    pub shot_id: String,
    pub role: String,
    pub profile_type: String,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestShotReferenceSetBinding {
    pub id: String,
    pub shot_id: String,
    pub role: String,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestScopeProfileBinding {
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestScopeReferenceSetBinding {
    pub id: String,
    pub project_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub role: String,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: String,
}

#[derive(FromRow)]
struct ProjectRow {
    id: String,
    name: String,
    description: Option<String>,
}

#[derive(FromRow)]
struct SeriesRow {
    id: String,
    ordinal: i64,
    name: String,
    description: String,
}

#[derive(FromRow)]
struct EpisodeRow {
    id: String,
    series_id: String,
    ordinal: i64,
    name: String,
    description: String,
}

#[derive(FromRow)]
struct SceneRow {
    id: String,
    episode_id: String,
    ordinal: i64,
    name: String,
    description: String,
}

#[derive(FromRow)]
struct AssignmentRow {
    shot_id: String,
    scene_id: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct ShotRow {
    id: String,
    ordinal: i64,
    name: String,
    prompt_text: String,
    prompt_entry_id: Option<String>,
    prompt_version_id: Option<String>,
    selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>,
}

#[derive(FromRow)]
struct StageConfigRow {
    shot_id: String,
    stage: String,
    workflow_version_id: String,
    recipe_id: String,
    scalar_values_json: String,
}

#[derive(FromRow)]
struct ReferenceRow {
    shot_id: String,
    stage: String,
    asset_id: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct GenerationLinkRow {
    shot_id: String,
    stage: String,
    task_status: Option<String>,
}

#[derive(FromRow)]
struct AnchorRow {
    id: String,
    kind: String,
    name: String,
    description: String,
}

#[derive(FromRow)]
struct AnchorAssetRow {
    anchor_id: String,
    asset_id: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct CharacterProfileRow {
    id: String,
    name: String,
    description: String,
    canonical_prompt: String,
    negative_prompt: String,
    default_style_profile_id: Option<String>,
    default_reference_set_id: Option<String>,
    active_revision_id: Option<String>,
}

#[derive(FromRow)]
struct SceneProfileRow {
    id: String,
    name: String,
    description: String,
    environment_prompt: String,
    lighting_prompt: Option<String>,
    negative_prompt: Option<String>,
    default_style_profile_id: Option<String>,
    default_reference_set_id: Option<String>,
    active_revision_id: Option<String>,
}

#[derive(FromRow)]
struct PropProfileRow {
    id: String,
    name: String,
    description: String,
    canonical_prompt: String,
    material_prompt: Option<String>,
    scale_prompt: Option<String>,
    default_reference_set_id: Option<String>,
    active_revision_id: Option<String>,
}

#[derive(FromRow)]
struct StyleProfileRow {
    id: String,
    name: String,
    style_prompt: String,
    color_prompt: Option<String>,
    line_prompt: Option<String>,
    negative_prompt: Option<String>,
    output_notes: Option<String>,
    active_revision_id: Option<String>,
}

#[derive(FromRow)]
struct CostumeVariantRow {
    id: String,
    character_profile_id: String,
    name: String,
    prompt_fragment: String,
    reference_set_id: Option<String>,
    is_default: i64,
    ordinal: i64,
    active_revision_id: Option<String>,
}

#[derive(FromRow)]
struct ReferenceSetRow {
    id: String,
    name: String,
    purpose: String,
    description: String,
    owner_profile_type: Option<String>,
    owner_profile_id: Option<String>,
    active_revision_id: Option<String>,
}

#[derive(FromRow)]
struct ReferenceSetItemRow {
    reference_set_id: String,
    asset_id: String,
    ordinal: i64,
    role: Option<String>,
    is_primary: i64,
}

#[derive(FromRow)]
struct ShotProfileBindingRow {
    id: String,
    shot_id: String,
    role: String,
    profile_type: String,
    profile_id: String,
    costume_variant_id: Option<String>,
    ordinal: i64,
    inheritance_mode: String,
}

#[derive(FromRow)]
struct ShotReferenceSetBindingRow {
    id: String,
    shot_id: String,
    role: String,
    reference_set_id: String,
    ordinal: i64,
    required: i64,
    inheritance_mode: String,
}

#[derive(FromRow)]
struct ScopeProfileBindingRow {
    id: String,
    project_id: String,
    scope_type: String,
    scope_id: String,
    role: String,
    profile_type: String,
    profile_id: String,
    costume_variant_id: Option<String>,
    ordinal: i64,
    inheritance_mode: String,
}

#[derive(FromRow)]
struct ScopeReferenceSetBindingRow {
    id: String,
    project_id: String,
    scope_type: String,
    scope_id: String,
    role: String,
    reference_set_id: String,
    ordinal: i64,
    required: i64,
    inheritance_mode: String,
}

struct ConsistencyManifestRows {
    character_profiles: Vec<CharacterProfileRow>,
    scene_profiles: Vec<SceneProfileRow>,
    prop_profiles: Vec<PropProfileRow>,
    style_profiles: Vec<StyleProfileRow>,
    costume_variants: Vec<CostumeVariantRow>,
    reference_sets: Vec<ReferenceSetRow>,
    reference_set_items: Vec<ReferenceSetItemRow>,
    shot_profile_bindings: Vec<ShotProfileBindingRow>,
    shot_reference_set_bindings: Vec<ShotReferenceSetBindingRow>,
    scope_profile_bindings: Vec<ScopeProfileBindingRow>,
    scope_reference_set_bindings: Vec<ScopeReferenceSetBindingRow>,
}

impl ProjectManifestService {
    pub fn parse(bytes: &[u8]) -> Result<ProjectManifest, AppError> {
        let manifest = serde_json::from_slice::<ProjectManifest>(bytes)
            .map_err(|error| AppError::backup_invalid(format!("项目清单 JSON 无效：{error}")))?;
        if manifest.format != MANIFEST_FORMAT || !matches!(manifest.version, 1 | 2) {
            return Err(AppError::backup_invalid("项目清单格式或版本不受支持"));
        }
        Ok(manifest)
    }

    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn export(
        &self,
        project_id: &str,
        destination: PathBuf,
    ) -> Result<ProjectManifestExportView, AppError> {
        let manifest = self.build(project_id).await?;
        let bytes = manifest_bytes(&manifest)?;
        let destination = if destination.is_dir() {
            destination.join(suggested_manifest_filename(&manifest.project.name))
        } else {
            destination
        };
        publish_manifest(&destination, &bytes)?;
        Ok(ProjectManifestExportView {
            file_name: destination
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("AI-Studio-Project-Manifest.json")
                .to_owned(),
            bytes: bytes.len() as u64,
            series_count: manifest.structure.series.len(),
            episode_count: manifest
                .structure
                .series
                .iter()
                .map(|series| series.episodes.len())
                .sum(),
            scene_count: manifest
                .structure
                .series
                .iter()
                .flat_map(|series| &series.episodes)
                .map(|episode| episode.scenes.len())
                .sum(),
            shot_count: manifest.shots.len(),
            unassigned_shot_count: manifest.structure.unassigned_shot_ids.len(),
            anchor_count: manifest.reference_anchors.len(),
        })
    }

    async fn build(&self, project_id: &str) -> Result<ProjectManifest, AppError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| AppError::database(error.to_string()))?;
        let project = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, description FROM projects WHERE id = ?",
        )
        .bind(project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?
        .ok_or_else(|| AppError::project_not_found(project_id.to_owned()))?;

        let (series, episodes, scenes, assignments) =
            query_structure(&mut transaction, project_id).await?;
        let (structure, scene_by_shot) = assemble_structure(series, episodes, scenes, assignments);
        let shots = sqlx::query_as::<_, ShotRow>(
            "SELECT id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id,
                    selected_image_asset_id, selected_video_asset_id
             FROM shots WHERE project_id = ? ORDER BY ordinal, id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
        let configs = sqlx::query_as::<_, StageConfigRow>(
            "SELECT shot_id, stage, workflow_version_id, recipe_id, scalar_values_json
             FROM shot_stage_configs
             WHERE shot_id IN (SELECT id FROM shots WHERE project_id = ?)
             ORDER BY shot_id, stage",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
        let references = sqlx::query_as::<_, ReferenceRow>(
            "SELECT r.shot_id, r.stage, r.asset_id, r.ordinal
             FROM shot_reference_assets r
             JOIN shots s ON s.id = r.shot_id
             WHERE s.project_id = ?
             ORDER BY r.shot_id, r.stage, r.ordinal, r.asset_id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
        let links = sqlx::query_as::<_, GenerationLinkRow>(
            "SELECT l.shot_id, l.stage, t.status AS task_status
             FROM shot_generation_links l
             JOIN shots s ON s.id = l.shot_id
             LEFT JOIN tasks t ON t.id = l.task_id
             WHERE s.project_id = ?
             ORDER BY l.shot_id, l.stage, l.created_at DESC, l.id DESC",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
        let (anchors, anchor_assets) = query_anchors(&mut transaction, project_id).await?;
        let consistency = query_consistency(&mut transaction, project_id).await?;
        transaction
            .commit()
            .await
            .map_err(|error| AppError::database(error.to_string()))?;

        let mut configs_by_shot = HashMap::<String, Vec<ManifestStageConfig>>::new();
        for config in configs {
            let values = serde_json::from_str(&config.scalar_values_json)
                .map_err(|error| AppError::database(format!("镜头阶段参数无效：{error}")))?;
            configs_by_shot
                .entry(config.shot_id)
                .or_default()
                .push(ManifestStageConfig {
                    stage: config.stage,
                    workflow_version_id: config.workflow_version_id,
                    recipe_id: config.recipe_id,
                    values,
                });
        }
        let mut references_by_shot = HashMap::<String, ManifestReferences>::new();
        for reference in references {
            let target = match reference.stage.as_str() {
                "image" => {
                    &mut references_by_shot
                        .entry(reference.shot_id.clone())
                        .or_default()
                        .image
                }
                "video" => {
                    &mut references_by_shot
                        .entry(reference.shot_id.clone())
                        .or_default()
                        .video
                }
                _ => continue,
            };
            target.push(ManifestReference {
                asset_id: reference.asset_id,
                ordinal: reference.ordinal,
            });
        }
        let mut latest_status = HashMap::<(String, String), Option<TaskStatus>>::new();
        for link in links {
            let key = (link.shot_id, link.stage);
            latest_status.entry(key).or_insert_with(|| {
                link.task_status
                    .as_deref()
                    .and_then(|status| TaskStatus::try_from_db(status).ok())
            });
        }
        let manifest_shots = shots
            .into_iter()
            .map(|shot| {
                let image_configured = configs_by_shot
                    .get(&shot.id)
                    .is_some_and(|configs| configs.iter().any(|config| config.stage == "image"));
                let video_configured = configs_by_shot
                    .get(&shot.id)
                    .is_some_and(|configs| configs.iter().any(|config| config.stage == "video"));
                let image_status = derive_stage_status(
                    ShotStage::Image,
                    image_configured,
                    shot.selected_image_asset_id.is_some(),
                    latest_status
                        .get(&(shot.id.clone(), "image".to_owned()))
                        .copied()
                        .flatten(),
                );
                let video_status = derive_stage_status(
                    ShotStage::Video,
                    video_configured,
                    shot.selected_video_asset_id.is_some(),
                    latest_status
                        .get(&(shot.id.clone(), "video".to_owned()))
                        .copied()
                        .flatten(),
                );
                let (scene_id, scene_ordinal) =
                    scene_by_shot.get(&shot.id).cloned().unwrap_or((None, None));
                ManifestShot {
                    id: shot.id.clone(),
                    global_ordinal: shot.ordinal,
                    name: shot.name,
                    scene_id,
                    scene_ordinal,
                    prompt_text: shot.prompt_text,
                    prompt_entry_id: shot.prompt_entry_id,
                    prompt_version_id: shot.prompt_version_id,
                    image_status: image_status.as_str().to_owned(),
                    video_status: video_status.as_str().to_owned(),
                    selected_image_asset_id: shot.selected_image_asset_id,
                    selected_video_asset_id: shot.selected_video_asset_id,
                    stage_configs: configs_by_shot.remove(&shot.id).unwrap_or_default(),
                    references: references_by_shot.remove(&shot.id).unwrap_or_default(),
                }
            })
            .collect::<Vec<_>>();

        let mut assets_by_anchor = HashMap::<String, Vec<ManifestReference>>::new();
        for asset in anchor_assets {
            assets_by_anchor
                .entry(asset.anchor_id)
                .or_default()
                .push(ManifestReference {
                    asset_id: asset.asset_id,
                    ordinal: asset.ordinal,
                });
        }
        let reference_anchors = anchors
            .into_iter()
            .map(|anchor| {
                let mut ordered = assets_by_anchor.remove(&anchor.id).unwrap_or_default();
                ordered.sort_by_key(|asset| asset.ordinal);
                ManifestReferenceAnchor {
                    id: anchor.id,
                    kind: anchor.kind,
                    name: anchor.name,
                    description: anchor.description,
                    ordered_asset_ids: ordered.into_iter().map(|asset| asset.asset_id).collect(),
                }
            })
            .collect();
        let consistency = assemble_consistency(consistency)?;
        Ok(ProjectManifest {
            format: MANIFEST_FORMAT.to_owned(),
            version: MANIFEST_VERSION,
            generated_at: Utc::now().to_rfc3339(),
            project: ManifestProject {
                id: project.id,
                name: project.name,
                description: project.description,
            },
            structure,
            shots: manifest_shots,
            reference_anchors,
            profiles: consistency.profiles,
            costume_variants: consistency.costume_variants,
            reference_sets: consistency.reference_sets,
            reference_set_items: consistency.reference_set_items,
            shot_profile_bindings: consistency.shot_profile_bindings,
            shot_reference_set_bindings: consistency.shot_reference_set_bindings,
            scope_profile_bindings: consistency.scope_profile_bindings,
            scope_reference_set_bindings: consistency.scope_reference_set_bindings,
        })
    }
}

async fn query_structure(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
) -> Result<
    (
        Vec<SeriesRow>,
        Vec<EpisodeRow>,
        Vec<SceneRow>,
        Vec<AssignmentRow>,
    ),
    AppError,
> {
    let table_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table' AND name IN
           ('production_series', 'production_episodes', 'production_scenes',
            'shot_scene_assignments')",
    )
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    if table_count == 0 {
        return Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new()));
    }
    if table_count != 4 {
        return Err(AppError::database(
            "生产结构表不完整，请先应用 migration 021",
        ));
    }
    let series = sqlx::query_as::<_, SeriesRow>(
        "SELECT id, ordinal, name, description FROM production_series
         WHERE project_id = ? ORDER BY ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let episodes = sqlx::query_as::<_, EpisodeRow>(
        "SELECT e.id, e.series_id, e.ordinal, e.name, e.description
         FROM production_episodes e JOIN production_series s ON s.id = e.series_id
         WHERE s.project_id = ? ORDER BY e.series_id, e.ordinal, e.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let scenes = sqlx::query_as::<_, SceneRow>(
        "SELECT c.id, c.episode_id, c.ordinal, c.name, c.description
         FROM production_scenes c
         JOIN production_episodes e ON e.id = c.episode_id
         JOIN production_series s ON s.id = e.series_id
         WHERE s.project_id = ? ORDER BY c.episode_id, c.ordinal, c.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let assignments = sqlx::query_as::<_, AssignmentRow>(
        "SELECT a.shot_id, a.scene_id, a.ordinal
         FROM shot_scene_assignments a
         JOIN production_scenes c ON c.id = a.scene_id
         JOIN production_episodes e ON e.id = c.episode_id
         JOIN production_series s ON s.id = e.series_id
         JOIN shots h ON h.id = a.shot_id
         WHERE s.project_id = ? AND h.project_id = ?
         ORDER BY a.scene_id, a.ordinal, a.shot_id",
    )
    .bind(project_id)
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    Ok((series, episodes, scenes, assignments))
}

fn assemble_structure(
    series: Vec<SeriesRow>,
    episodes: Vec<EpisodeRow>,
    scenes: Vec<SceneRow>,
    assignments: Vec<AssignmentRow>,
) -> (
    ManifestStructure,
    HashMap<String, (Option<String>, Option<i64>)>,
) {
    let mut structure = ManifestStructure {
        series: series
            .into_iter()
            .map(|row| ManifestSeries {
                id: row.id,
                ordinal: row.ordinal,
                name: row.name,
                description: row.description,
                episodes: Vec::new(),
            })
            .collect(),
        unassigned_shot_ids: Vec::new(),
    };
    let mut series_index = HashMap::new();
    for (index, series) in structure.series.iter().enumerate() {
        series_index.insert(series.id.clone(), index);
    }
    let mut episode_index = HashMap::new();
    for row in episodes {
        let Some(&series_index_value) = series_index.get(&row.series_id) else {
            continue;
        };
        let episode_index_value = structure.series[series_index_value].episodes.len();
        episode_index.insert(row.id.clone(), (series_index_value, episode_index_value));
        structure.series[series_index_value]
            .episodes
            .push(ManifestEpisode {
                id: row.id,
                ordinal: row.ordinal,
                name: row.name,
                description: row.description,
                scenes: Vec::new(),
            });
    }
    let mut scene_index = HashMap::new();
    for row in scenes {
        let Some(&(series_index_value, episode_index_value)) = episode_index.get(&row.episode_id)
        else {
            continue;
        };
        let scene_index_value = structure.series[series_index_value].episodes[episode_index_value]
            .scenes
            .len();
        scene_index.insert(
            row.id.clone(),
            (series_index_value, episode_index_value, scene_index_value),
        );
        structure.series[series_index_value].episodes[episode_index_value]
            .scenes
            .push(ManifestScene {
                id: row.id,
                ordinal: row.ordinal,
                name: row.name,
                description: row.description,
                shot_ids: Vec::new(),
            });
    }
    let mut scene_by_shot = HashMap::new();
    for assignment in assignments {
        let Some(&(series_index_value, episode_index_value, scene_index_value)) =
            scene_index.get(&assignment.scene_id)
        else {
            continue;
        };
        structure.series[series_index_value].episodes[episode_index_value].scenes
            [scene_index_value]
            .shot_ids
            .push(assignment.shot_id.clone());
        scene_by_shot.insert(
            assignment.shot_id,
            (Some(assignment.scene_id), Some(assignment.ordinal)),
        );
    }
    (structure, scene_by_shot)
}

async fn query_anchors(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
) -> Result<(Vec<AnchorRow>, Vec<AnchorAssetRow>), AppError> {
    let anchors = sqlx::query_as::<_, AnchorRow>(
        "SELECT id, kind, name, description FROM reference_anchors
         WHERE project_id = ? ORDER BY kind, name, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let assets = sqlx::query_as::<_, AnchorAssetRow>(
        "SELECT m.anchor_id, m.asset_id, m.ordinal
         FROM reference_anchor_assets m
         JOIN reference_anchors a ON a.id = m.anchor_id
         WHERE a.project_id = ? ORDER BY m.anchor_id, m.ordinal, m.asset_id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    Ok((anchors, assets))
}

async fn query_consistency(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
) -> Result<ConsistencyManifestRows, AppError> {
    let character_profiles = sqlx::query_as::<_, CharacterProfileRow>(
        "SELECT id, name, description, canonical_prompt, negative_prompt,
                default_style_profile_id, default_reference_set_id, active_revision_id
         FROM character_profiles
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let scene_profiles = sqlx::query_as::<_, SceneProfileRow>(
        "SELECT id, name, description, environment_prompt, lighting_prompt,
                negative_prompt, default_style_profile_id, default_reference_set_id,
                active_revision_id
         FROM scene_profiles
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let prop_profiles = sqlx::query_as::<_, PropProfileRow>(
        "SELECT id, name, description, canonical_prompt, material_prompt, scale_prompt,
                default_reference_set_id, active_revision_id
         FROM prop_profiles
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let style_profiles = sqlx::query_as::<_, StyleProfileRow>(
        "SELECT id, name, style_prompt, color_prompt, line_prompt, negative_prompt,
                output_notes, active_revision_id
         FROM style_profiles
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let costume_variants = sqlx::query_as::<_, CostumeVariantRow>(
        "SELECT v.id, v.character_profile_id, v.name, v.prompt_fragment,
                v.reference_set_id, v.is_default, v.ordinal, v.active_revision_id
         FROM costume_variants v
         JOIN character_profiles p ON p.id = v.character_profile_id
         WHERE p.project_id = ?
         ORDER BY v.character_profile_id, v.ordinal, v.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let reference_sets = sqlx::query_as::<_, ReferenceSetRow>(
        "SELECT id, name, purpose, description, owner_profile_type,
                owner_profile_id, active_revision_id
         FROM reference_sets
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let reference_set_items = sqlx::query_as::<_, ReferenceSetItemRow>(
        "SELECT i.reference_set_id, i.asset_id, i.ordinal, i.role, i.is_primary
         FROM reference_set_items i
         JOIN reference_sets r ON r.id = i.reference_set_id
         WHERE r.project_id = ?
         ORDER BY i.reference_set_id, i.ordinal, i.asset_id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let shot_profile_bindings = sqlx::query_as::<_, ShotProfileBindingRow>(
        "SELECT b.id, b.shot_id, b.role, b.profile_type, b.profile_id,
                b.costume_variant_id, b.ordinal, b.inheritance_mode
         FROM shot_profile_bindings b
         JOIN shots s ON s.id = b.shot_id
         WHERE s.project_id = ?
         ORDER BY b.shot_id, b.role, b.ordinal, b.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let shot_reference_set_bindings = sqlx::query_as::<_, ShotReferenceSetBindingRow>(
        "SELECT b.id, b.shot_id, b.role, b.reference_set_id, b.ordinal,
                b.required, b.inheritance_mode
         FROM shot_reference_set_bindings b
         JOIN shots s ON s.id = b.shot_id
         WHERE s.project_id = ?
         ORDER BY b.shot_id, b.role, b.ordinal, b.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let scope_profile_bindings = sqlx::query_as::<_, ScopeProfileBindingRow>(
        "SELECT id, project_id, scope_type, scope_id, role, profile_type,
                profile_id, costume_variant_id, ordinal, inheritance_mode
         FROM consistency_scope_profile_bindings
         WHERE project_id = ?
         ORDER BY scope_type, scope_id, role, ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    let scope_reference_set_bindings = sqlx::query_as::<_, ScopeReferenceSetBindingRow>(
        "SELECT id, project_id, scope_type, scope_id, role, reference_set_id,
                ordinal, required, inheritance_mode
         FROM consistency_scope_reference_set_bindings
         WHERE project_id = ?
         ORDER BY scope_type, scope_id, role, ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    Ok(ConsistencyManifestRows {
        character_profiles,
        scene_profiles,
        prop_profiles,
        style_profiles,
        costume_variants,
        reference_sets,
        reference_set_items,
        shot_profile_bindings,
        shot_reference_set_bindings,
        scope_profile_bindings,
        scope_reference_set_bindings,
    })
}

struct AssembledConsistency {
    profiles: Vec<ManifestProfile>,
    costume_variants: Vec<ManifestCostumeVariant>,
    reference_sets: Vec<ManifestReferenceSet>,
    reference_set_items: Vec<ManifestReferenceSetItem>,
    shot_profile_bindings: Vec<ManifestShotProfileBinding>,
    shot_reference_set_bindings: Vec<ManifestShotReferenceSetBinding>,
    scope_profile_bindings: Vec<ManifestScopeProfileBinding>,
    scope_reference_set_bindings: Vec<ManifestScopeReferenceSetBinding>,
}

fn assemble_consistency(rows: ConsistencyManifestRows) -> Result<AssembledConsistency, AppError> {
    let mut profiles = Vec::with_capacity(
        rows.character_profiles.len()
            + rows.scene_profiles.len()
            + rows.prop_profiles.len()
            + rows.style_profiles.len(),
    );
    profiles.extend(
        rows.character_profiles
            .into_iter()
            .map(|row| ManifestProfile {
                id: row.id,
                profile_type: ProfileType::Character.as_str().to_owned(),
                name: row.name,
                description: row.description,
                canonical_prompt: Some(row.canonical_prompt),
                negative_prompt: Some(row.negative_prompt),
                environment_prompt: None,
                lighting_prompt: None,
                material_prompt: None,
                scale_prompt: None,
                style_prompt: None,
                color_prompt: None,
                line_prompt: None,
                output_notes: None,
                default_style_profile_id: row.default_style_profile_id,
                default_reference_set_id: row.default_reference_set_id,
                active_revision_id: row.active_revision_id,
            }),
    );
    profiles.extend(rows.scene_profiles.into_iter().map(|row| ManifestProfile {
        id: row.id,
        profile_type: ProfileType::Scene.as_str().to_owned(),
        name: row.name,
        description: row.description,
        canonical_prompt: None,
        negative_prompt: row.negative_prompt,
        environment_prompt: Some(row.environment_prompt),
        lighting_prompt: row.lighting_prompt,
        material_prompt: None,
        scale_prompt: None,
        style_prompt: None,
        color_prompt: None,
        line_prompt: None,
        output_notes: None,
        default_style_profile_id: row.default_style_profile_id,
        default_reference_set_id: row.default_reference_set_id,
        active_revision_id: row.active_revision_id,
    }));
    profiles.extend(rows.prop_profiles.into_iter().map(|row| ManifestProfile {
        id: row.id,
        profile_type: ProfileType::Prop.as_str().to_owned(),
        name: row.name,
        description: row.description,
        canonical_prompt: Some(row.canonical_prompt),
        negative_prompt: None,
        environment_prompt: None,
        lighting_prompt: None,
        material_prompt: row.material_prompt,
        scale_prompt: row.scale_prompt,
        style_prompt: None,
        color_prompt: None,
        line_prompt: None,
        output_notes: None,
        default_style_profile_id: None,
        default_reference_set_id: row.default_reference_set_id,
        active_revision_id: row.active_revision_id,
    }));
    profiles.extend(rows.style_profiles.into_iter().map(|row| ManifestProfile {
        id: row.id,
        profile_type: ProfileType::Style.as_str().to_owned(),
        name: row.name,
        description: String::new(),
        canonical_prompt: None,
        negative_prompt: row.negative_prompt,
        environment_prompt: None,
        lighting_prompt: None,
        material_prompt: None,
        scale_prompt: None,
        style_prompt: Some(row.style_prompt),
        color_prompt: row.color_prompt,
        line_prompt: row.line_prompt,
        output_notes: row.output_notes,
        default_style_profile_id: None,
        default_reference_set_id: None,
        active_revision_id: row.active_revision_id,
    }));

    let costume_variants = rows
        .costume_variants
        .into_iter()
        .map(|row| {
            Ok(ManifestCostumeVariant {
                id: row.id,
                character_profile_id: row.character_profile_id,
                name: row.name,
                prompt_fragment: row.prompt_fragment,
                reference_set_id: row.reference_set_id,
                is_default: sqlite_bool("costume variant is_default", row.is_default)?,
                ordinal: row.ordinal,
                active_revision_id: row.active_revision_id,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let reference_sets = rows
        .reference_sets
        .into_iter()
        .map(|row| {
            ReferenceSetPurpose::try_from_db(&row.purpose).map_err(|error| {
                AppError::database(format!("Reference Set purpose 无效：{error}"))
            })?;
            Ok(ManifestReferenceSet {
                id: row.id,
                name: row.name,
                purpose: row.purpose,
                description: row.description,
                owner_profile_type: row.owner_profile_type,
                owner_profile_id: row.owner_profile_id,
                active_revision_id: row.active_revision_id,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let reference_set_items = rows
        .reference_set_items
        .into_iter()
        .map(|row| {
            Ok(ManifestReferenceSetItem {
                reference_set_id: row.reference_set_id,
                asset_id: row.asset_id,
                ordinal: row.ordinal,
                role: row.role,
                is_primary: sqlite_bool("reference set item is_primary", row.is_primary)?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let shot_profile_bindings = rows
        .shot_profile_bindings
        .into_iter()
        .map(|row| {
            ProfileType::try_from_db(&row.profile_type).map_err(|error| {
                AppError::database(format!("Shot Profile profile_type 无效：{error}"))
            })?;
            BindingRole::try_from_db(&row.role)
                .map_err(|error| AppError::database(format!("Shot Profile role 无效：{error}")))?;
            InheritanceMode::try_from_db(&row.inheritance_mode).map_err(|error| {
                AppError::database(format!("Shot Profile inheritance_mode 无效：{error}"))
            })?;
            Ok(ManifestShotProfileBinding {
                id: row.id,
                shot_id: row.shot_id,
                role: row.role,
                profile_type: row.profile_type,
                profile_id: row.profile_id,
                costume_variant_id: row.costume_variant_id,
                ordinal: row.ordinal,
                inheritance_mode: row.inheritance_mode,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let shot_reference_set_bindings = rows
        .shot_reference_set_bindings
        .into_iter()
        .map(|row| {
            BindingRole::try_from_db(&row.role).map_err(|error| {
                AppError::database(format!("Shot Reference Set role 无效：{error}"))
            })?;
            InheritanceMode::try_from_db(&row.inheritance_mode).map_err(|error| {
                AppError::database(format!("Shot Reference Set inheritance_mode 无效：{error}"))
            })?;
            Ok(ManifestShotReferenceSetBinding {
                id: row.id,
                shot_id: row.shot_id,
                role: row.role,
                reference_set_id: row.reference_set_id,
                ordinal: row.ordinal,
                required: sqlite_bool("shot reference set required", row.required)?,
                inheritance_mode: row.inheritance_mode,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let scope_profile_bindings = rows
        .scope_profile_bindings
        .into_iter()
        .map(|row| {
            BindingRole::try_from_db(&row.role)
                .map_err(|error| AppError::database(format!("Scope Profile role 无效：{error}")))?;
            ProfileType::try_from_db(&row.profile_type).map_err(|error| {
                AppError::database(format!("Scope Profile profile_type 无效：{error}"))
            })?;
            InheritanceMode::try_from_db(&row.inheritance_mode).map_err(|error| {
                AppError::database(format!("Scope Profile inheritance_mode 无效：{error}"))
            })?;
            Ok(ManifestScopeProfileBinding {
                id: row.id,
                project_id: row.project_id,
                scope_type: row.scope_type,
                scope_id: row.scope_id,
                role: row.role,
                profile_type: row.profile_type,
                profile_id: row.profile_id,
                costume_variant_id: row.costume_variant_id,
                ordinal: row.ordinal,
                inheritance_mode: row.inheritance_mode,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let scope_reference_set_bindings = rows
        .scope_reference_set_bindings
        .into_iter()
        .map(|row| {
            BindingRole::try_from_db(&row.role).map_err(|error| {
                AppError::database(format!("Scope Reference Set role 无效：{error}"))
            })?;
            InheritanceMode::try_from_db(&row.inheritance_mode).map_err(|error| {
                AppError::database(format!(
                    "Scope Reference Set inheritance_mode 无效：{error}"
                ))
            })?;
            Ok(ManifestScopeReferenceSetBinding {
                id: row.id,
                project_id: row.project_id,
                scope_type: row.scope_type,
                scope_id: row.scope_id,
                role: row.role,
                reference_set_id: row.reference_set_id,
                ordinal: row.ordinal,
                required: sqlite_bool("Scope Reference Set required", row.required)?,
                inheritance_mode: row.inheritance_mode,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(AssembledConsistency {
        profiles,
        costume_variants,
        reference_sets,
        reference_set_items,
        shot_profile_bindings,
        shot_reference_set_bindings,
        scope_profile_bindings,
        scope_reference_set_bindings,
    })
}

fn sqlite_bool(field: &str, value: i64) -> Result<bool, AppError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(AppError::database(format!(
            "{field} 必须为 SQLite 布尔值 0/1，实际为 {other}"
        ))),
    }
}

pub fn suggested_manifest_filename(project_name: &str) -> String {
    let sanitized = sanitize_filename_component(project_name);
    format!("AI-Studio-{sanitized}-Manifest.json")
}

fn sanitize_filename_component(value: &str) -> String {
    let mut result = value
        .chars()
        .map(|character| {
            if character.is_control() || r#"<>:/\\|?*"#.contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .trim_end_matches(['.', ' '])
        .to_owned();
    if result.is_empty() {
        result = "Project".to_owned();
    }
    result
}

fn manifest_bytes(manifest: &ProjectManifest) -> Result<Vec<u8>, AppError> {
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|error| AppError::internal(format!("项目清单序列化失败：{error}")))?;
    Ok(format!("{}\n", json.replace("\r\n", "\n")).into_bytes())
}

fn publish_manifest(destination: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::filesystem("项目清单保存目录不可用"))?;
    fs::create_dir_all(parent).map_err(|error| AppError::filesystem(error.to_string()))?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project-manifest");
    let temporary = parent.join(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file =
            File::create(&temporary).map_err(|error| AppError::filesystem(error.to_string()))?;
        file.write_all(bytes)
            .map_err(|error| AppError::filesystem(error.to_string()))?;
        file.sync_all()
            .map_err(|error| AppError::filesystem(error.to_string()))?;
        drop(file);
        fs::rename(&temporary, destination)
            .map_err(|error| AppError::filesystem(format!("项目清单发布失败：{error}")))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> ProjectManifest {
        ProjectManifest {
            format: MANIFEST_FORMAT.to_owned(),
            version: MANIFEST_VERSION,
            generated_at: "2026-08-18T00:00:00Z".to_owned(),
            project: ManifestProject {
                id: "prj_a".to_owned(),
                name: "AI Drama".to_owned(),
                description: Some("仅项目元数据".to_owned()),
            },
            structure: ManifestStructure {
                series: vec![ManifestSeries {
                    id: "ser_a".to_owned(),
                    ordinal: 0,
                    name: "第一季".to_owned(),
                    description: String::new(),
                    episodes: vec![ManifestEpisode {
                        id: "ep_a".to_owned(),
                        ordinal: 0,
                        name: "第 1 集".to_owned(),
                        description: String::new(),
                        scenes: vec![ManifestScene {
                            id: "scn_a".to_owned(),
                            ordinal: 0,
                            name: "开场".to_owned(),
                            description: String::new(),
                            shot_ids: vec!["sht_a".to_owned()],
                        }],
                    }],
                }],
                unassigned_shot_ids: vec![],
            },
            shots: vec![ManifestShot {
                id: "sht_a".to_owned(),
                global_ordinal: 0,
                name: "镜头 1".to_owned(),
                scene_id: Some("scn_a".to_owned()),
                scene_ordinal: Some(0),
                prompt_text: "prompt".to_owned(),
                prompt_entry_id: None,
                prompt_version_id: None,
                image_status: "DRAFT".to_owned(),
                video_status: "DRAFT".to_owned(),
                selected_image_asset_id: None,
                selected_video_asset_id: None,
                stage_configs: vec![ManifestStageConfig {
                    stage: "image".to_owned(),
                    workflow_version_id: "wf_v1".to_owned(),
                    recipe_id: "recipe_v1".to_owned(),
                    values: json!({"seed": {"type": "seed_random"}}),
                }],
                references: ManifestReferences::default(),
            }],
            reference_anchors: vec![ManifestReferenceAnchor {
                id: "anc_a".to_owned(),
                kind: "CHARACTER".to_owned(),
                name: "主角".to_owned(),
                description: String::new(),
                ordered_asset_ids: vec!["ast_a".to_owned()],
            }],
            profiles: Vec::new(),
            costume_variants: Vec::new(),
            reference_sets: Vec::new(),
            reference_set_items: Vec::new(),
            shot_profile_bindings: Vec::new(),
            shot_reference_set_bindings: Vec::new(),
            scope_profile_bindings: Vec::new(),
            scope_reference_set_bindings: Vec::new(),
        }
    }

    #[test]
    fn manifest_json_is_pretty_utf8_lf_and_deterministic_except_generated_at() {
        let first = manifest_bytes(&fixture()).unwrap();
        let second = manifest_bytes(&fixture()).unwrap();
        assert_eq!(first, second);
        assert!(first.ends_with(b"\n"));
        assert!(!first.windows(2).any(|pair| pair == b"\r\n"));
        assert!(String::from_utf8(first)
            .unwrap()
            .contains("\n  \"project\""));
    }

    #[test]
    fn manifest_whitelist_excludes_local_runtime_data() {
        let json = serde_json::to_string(&fixture()).unwrap();
        for forbidden in [
            "rootPath",
            "storagePath",
            "comfyEndpoint",
            "settings",
            "apiKey",
            "C:\\\\",
            "D:\\\\",
        ] {
            assert!(!json.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn manifest_filename_is_safe_for_windows_publish() {
        let name = suggested_manifest_filename(" Drama: S1/Final? ");
        assert_eq!(name, "AI-Studio-Drama_ S1_Final_-Manifest.json");
        assert!(!name.contains([':', '/', '?', '\\']));
    }

    #[test]
    fn manifest_v1_defaults_consistency_sections_to_empty() {
        let mut value = serde_json::to_value(fixture()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("version".to_owned(), json!(1));
        for field in [
            "profiles",
            "costumeVariants",
            "referenceSets",
            "referenceSetItems",
            "shotProfileBindings",
            "shotReferenceSetBindings",
            "scopeProfileBindings",
            "scopeReferenceSetBindings",
        ] {
            object.remove(field);
        }
        let bytes = serde_json::to_vec(&value).unwrap();
        let parsed = ProjectManifestService::parse(&bytes).unwrap();
        assert_eq!(parsed.version, 1);
        assert!(parsed.profiles.is_empty());
        assert!(parsed.reference_sets.is_empty());
        assert!(parsed.shot_profile_bindings.is_empty());
        assert!(parsed.scope_reference_set_bindings.is_empty());
    }

    #[test]
    fn manifest_v2_roundtrip_keeps_logical_consistency_relations() {
        let mut manifest = fixture();
        manifest.profiles.push(ManifestProfile {
            id: "cp_a".to_owned(),
            profile_type: "CHARACTER".to_owned(),
            name: "主角".to_owned(),
            description: String::new(),
            canonical_prompt: Some("hero".to_owned()),
            negative_prompt: Some("blur".to_owned()),
            environment_prompt: None,
            lighting_prompt: None,
            material_prompt: None,
            scale_prompt: None,
            style_prompt: None,
            color_prompt: None,
            line_prompt: None,
            output_notes: None,
            default_style_profile_id: Some("sp_a".to_owned()),
            default_reference_set_id: Some("rs_a".to_owned()),
            active_revision_id: Some("rev_a".to_owned()),
        });
        manifest.costume_variants.push(ManifestCostumeVariant {
            id: "cv_a".to_owned(),
            character_profile_id: "cp_a".to_owned(),
            name: "外套".to_owned(),
            prompt_fragment: "dark coat".to_owned(),
            reference_set_id: Some("rs_a".to_owned()),
            is_default: true,
            ordinal: 0,
            active_revision_id: None,
        });
        manifest.reference_sets.push(ManifestReferenceSet {
            id: "rs_a".to_owned(),
            name: "主角参考".to_owned(),
            purpose: "CHARACTER".to_owned(),
            description: String::new(),
            owner_profile_type: Some("CHARACTER".to_owned()),
            owner_profile_id: Some("cp_a".to_owned()),
            active_revision_id: None,
        });
        manifest.reference_set_items.push(ManifestReferenceSetItem {
            reference_set_id: "rs_a".to_owned(),
            asset_id: "ast_a".to_owned(),
            ordinal: 0,
            role: Some("front".to_owned()),
            is_primary: true,
        });
        manifest
            .shot_profile_bindings
            .push(ManifestShotProfileBinding {
                id: "spb_a".to_owned(),
                shot_id: "sht_a".to_owned(),
                role: "CHARACTER".to_owned(),
                profile_type: "CHARACTER".to_owned(),
                profile_id: "cp_a".to_owned(),
                costume_variant_id: Some("cv_a".to_owned()),
                ordinal: 0,
                inheritance_mode: "EXPLICIT".to_owned(),
            });
        manifest
            .shot_reference_set_bindings
            .push(ManifestShotReferenceSetBinding {
                id: "srb_a".to_owned(),
                shot_id: "sht_a".to_owned(),
                role: "CHARACTER".to_owned(),
                reference_set_id: "rs_a".to_owned(),
                ordinal: 0,
                required: true,
                inheritance_mode: "REPLACE".to_owned(),
            });
        manifest
            .scope_profile_bindings
            .push(ManifestScopeProfileBinding {
                id: "hpb_a".to_owned(),
                project_id: "prj_a".to_owned(),
                scope_type: "PROJECT".to_owned(),
                scope_id: "prj_a".to_owned(),
                role: "CHARACTER".to_owned(),
                profile_type: "CHARACTER".to_owned(),
                profile_id: "cp_a".to_owned(),
                costume_variant_id: None,
                ordinal: 0,
                inheritance_mode: "INHERITED".to_owned(),
            });
        manifest
            .scope_reference_set_bindings
            .push(ManifestScopeReferenceSetBinding {
                id: "hrb_a".to_owned(),
                project_id: "prj_a".to_owned(),
                scope_type: "PROJECT".to_owned(),
                scope_id: "prj_a".to_owned(),
                role: "CHARACTER".to_owned(),
                reference_set_id: "rs_a".to_owned(),
                ordinal: 0,
                required: true,
                inheritance_mode: "EXPLICIT".to_owned(),
            });
        let parsed = ProjectManifestService::parse(&manifest_bytes(&manifest).unwrap()).unwrap();
        assert_eq!(parsed, manifest);
        let json = serde_json::to_string(&parsed).unwrap();
        for forbidden in [
            "storagePath",
            "thumbnail",
            "comfyEndpoint",
            "runtime",
            "gpu",
            "taskHistory",
        ] {
            assert!(!json.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn manifest_parse_rejects_corrupt_format_and_version() {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value["format"] = json!("other-format");
        assert!(ProjectManifestService::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        value["format"] = json!(MANIFEST_FORMAT);
        value["version"] = json!(3);
        assert!(ProjectManifestService::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(ProjectManifestService::parse(b"{not-json}").is_err());
    }
}
