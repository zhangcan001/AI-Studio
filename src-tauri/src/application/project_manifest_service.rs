use crate::domain::{derive_stage_status, ShotStage, TaskStatus};
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
const MANIFEST_VERSION: u32 = 1;

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

impl ProjectManifestService {
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
}
