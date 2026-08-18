use crate::application::organization_service::normalize_name;
use crate::application::ports::{Clock, ProductionStructureRepository, RepositoryError};
use crate::domain::{
    ProductionEpisode, ProductionEpisodeId, ProductionScene, ProductionSceneId, ProductionSeries,
    ProductionSeriesId,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};

const MAX_NAME_CHARS: usize = 100;
const MAX_DESCRIPTION_CHARS: usize = 1000;
const MAX_SHOTS: usize = 500;

#[derive(Clone, Debug)]
pub struct CreateSeriesRequest {
    pub project_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct UpdateSeriesRequest {
    pub project_id: String,
    pub series_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct CreateEpisodeRequest {
    pub project_id: String,
    pub series_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct UpdateEpisodeRequest {
    pub project_id: String,
    pub episode_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct CreateSceneRequest {
    pub project_id: String,
    pub episode_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct UpdateSceneRequest {
    pub project_id: String,
    pub scene_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSeriesView {
    pub id: String,
    pub project_id: String,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionEpisodeView {
    pub id: String,
    pub series_id: String,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSceneView {
    pub id: String,
    pub episode_id: String,
    pub ordinal: u32,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSceneTreeView {
    #[serde(flatten)]
    pub scene: ProductionSceneView,
    pub shot_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionEpisodeTreeView {
    #[serde(flatten)]
    pub episode: ProductionEpisodeView,
    pub scenes: Vec<ProductionSceneTreeView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSeriesTreeView {
    #[serde(flatten)]
    pub series: ProductionSeriesView,
    pub episodes: Vec<ProductionEpisodeTreeView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionStructureTreeView {
    pub project_id: String,
    pub series: Vec<ProductionSeriesTreeView>,
    pub unassigned_shot_ids: Vec<String>,
}

pub struct ProductionStructureService {
    repository: Arc<dyn ProductionStructureRepository>,
    clock: Arc<dyn Clock>,
}

impl ProductionStructureService {
    pub fn new(repository: Arc<dyn ProductionStructureRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn tree(
        &self,
        project_id: &str,
    ) -> Result<ProductionStructureTreeView, ProductionStructureError> {
        validate_project_id(project_id)?;
        let data = self.repository.load_tree_data(project_id).await?;
        let mut episodes_by_series: HashMap<String, Vec<ProductionEpisode>> = HashMap::new();
        for episode in data.episodes {
            episodes_by_series
                .entry(episode.series_id.as_str().to_owned())
                .or_default()
                .push(episode);
        }
        let mut scenes_by_episode: HashMap<String, Vec<ProductionScene>> = HashMap::new();
        for scene in data.scenes {
            scenes_by_episode
                .entry(scene.episode_id.as_str().to_owned())
                .or_default()
                .push(scene);
        }
        let mut shots_by_scene: HashMap<String, Vec<(u32, String)>> = HashMap::new();
        let assigned = data
            .assignments
            .into_iter()
            .map(|assignment| {
                shots_by_scene
                    .entry(assignment.scene_id.as_str().to_owned())
                    .or_default()
                    .push((assignment.ordinal, assignment.shot_id.clone()));
                assignment.shot_id
            })
            .collect::<HashSet<_>>();
        for shots in shots_by_scene.values_mut() {
            shots.sort_by_key(|(ordinal, shot_id)| (*ordinal, shot_id.clone()));
        }
        let unassigned_shot_ids = data
            .project_shot_ids
            .into_iter()
            .filter(|shot_id| !assigned.contains(shot_id))
            .collect();

        let series = data
            .series
            .into_iter()
            .map(|series| {
                let episodes = episodes_by_series
                    .remove(series.id.as_str())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|episode| {
                        let scenes = scenes_by_episode
                            .remove(episode.id.as_str())
                            .unwrap_or_default()
                            .into_iter()
                            .map(|scene| {
                                let scene_id = scene.id.as_str().to_owned();
                                ProductionSceneTreeView {
                                    scene: scene_view(scene),
                                    shot_ids: shots_by_scene
                                        .remove(&scene_id)
                                        .unwrap_or_default()
                                        .into_iter()
                                        .map(|(_, shot_id)| shot_id)
                                        .collect(),
                                }
                            })
                            .collect();
                        ProductionEpisodeTreeView {
                            episode: episode_view(episode),
                            scenes,
                        }
                    })
                    .collect();
                ProductionSeriesTreeView {
                    series: series_view(series),
                    episodes,
                }
            })
            .collect();

        Ok(ProductionStructureTreeView {
            project_id: project_id.to_owned(),
            series,
            unassigned_shot_ids,
        })
    }

    pub async fn create_series(
        &self,
        request: CreateSeriesRequest,
    ) -> Result<ProductionSeriesView, ProductionStructureError> {
        validate_project_id(&request.project_id)?;
        let (name, description) = normalize_metadata(&request.name, &request.description)?;
        let now = self.clock.now();
        let series = ProductionSeries {
            id: ProductionSeriesId::new(),
            project_id: request.project_id,
            ordinal: 0,
            name,
            description,
            created_at: now,
            updated_at: now,
        };
        Ok(series_view(self.repository.create_series(&series).await?))
    }

    pub async fn update_series(
        &self,
        request: UpdateSeriesRequest,
    ) -> Result<ProductionSeriesView, ProductionStructureError> {
        validate_project_id(&request.project_id)?;
        let id = parse_series_id(&request.series_id)?;
        let current = self
            .repository
            .load_tree_data(&request.project_id)
            .await?
            .series
            .into_iter()
            .find(|series| series.id == id)
            .ok_or_else(|| ProductionStructureError::NotFound(request.series_id.clone()))?;
        let (name, description) = normalize_metadata(&request.name, &request.description)?;
        let updated = ProductionSeries {
            name,
            description,
            updated_at: self.clock.now(),
            ..current
        };
        Ok(series_view(self.repository.update_series(&updated).await?))
    }

    pub async fn delete_series(
        &self,
        project_id: &str,
        series_id: &str,
    ) -> Result<(), ProductionStructureError> {
        validate_project_id(project_id)?;
        let id = parse_series_id(series_id)?;
        if !self.repository.delete_series(project_id, &id).await? {
            return Err(ProductionStructureError::NotFound(series_id.to_owned()));
        }
        Ok(())
    }

    pub async fn reorder_series(
        &self,
        project_id: &str,
        ordered_ids: &[String],
    ) -> Result<(), ProductionStructureError> {
        validate_project_id(project_id)?;
        let ids = ordered_ids
            .iter()
            .map(|id| parse_series_id(id))
            .collect::<Result<Vec<_>, _>>()?;
        self.repository
            .reorder_series(project_id, &ids, self.clock.now())
            .await?;
        Ok(())
    }

    pub async fn create_episode(
        &self,
        request: CreateEpisodeRequest,
    ) -> Result<ProductionEpisodeView, ProductionStructureError> {
        validate_project_id(&request.project_id)?;
        let series_id = parse_series_id(&request.series_id)?;
        let (name, description) = normalize_metadata(&request.name, &request.description)?;
        let now = self.clock.now();
        let episode = ProductionEpisode {
            id: ProductionEpisodeId::new(),
            series_id,
            ordinal: 0,
            name,
            description,
            created_at: now,
            updated_at: now,
        };
        Ok(episode_view(
            self.repository
                .create_episode(&request.project_id, &episode)
                .await?,
        ))
    }

    pub async fn update_episode(
        &self,
        request: UpdateEpisodeRequest,
    ) -> Result<ProductionEpisodeView, ProductionStructureError> {
        validate_project_id(&request.project_id)?;
        let id = parse_episode_id(&request.episode_id)?;
        let current = self
            .repository
            .load_tree_data(&request.project_id)
            .await?
            .episodes
            .into_iter()
            .find(|episode| episode.id == id)
            .ok_or_else(|| ProductionStructureError::NotFound(request.episode_id.clone()))?;
        let (name, description) = normalize_metadata(&request.name, &request.description)?;
        let updated = ProductionEpisode {
            name,
            description,
            updated_at: self.clock.now(),
            ..current
        };
        Ok(episode_view(
            self.repository
                .update_episode(&updated, &request.project_id)
                .await?,
        ))
    }

    pub async fn delete_episode(
        &self,
        project_id: &str,
        episode_id: &str,
    ) -> Result<(), ProductionStructureError> {
        validate_project_id(project_id)?;
        let id = parse_episode_id(episode_id)?;
        if !self.repository.delete_episode(project_id, &id).await? {
            return Err(ProductionStructureError::NotFound(episode_id.to_owned()));
        }
        Ok(())
    }

    pub async fn reorder_episodes(
        &self,
        project_id: &str,
        series_id: &str,
        ordered_ids: &[String],
    ) -> Result<(), ProductionStructureError> {
        validate_project_id(project_id)?;
        let series_id = parse_series_id(series_id)?;
        let ids = ordered_ids
            .iter()
            .map(|id| parse_episode_id(id))
            .collect::<Result<Vec<_>, _>>()?;
        self.repository
            .reorder_episodes(project_id, &series_id, &ids, self.clock.now())
            .await?;
        Ok(())
    }

    pub async fn create_scene(
        &self,
        request: CreateSceneRequest,
    ) -> Result<ProductionSceneView, ProductionStructureError> {
        validate_project_id(&request.project_id)?;
        let episode_id = parse_episode_id(&request.episode_id)?;
        let (name, description) = normalize_metadata(&request.name, &request.description)?;
        let now = self.clock.now();
        let scene = ProductionScene {
            id: ProductionSceneId::new(),
            episode_id,
            ordinal: 0,
            name,
            description,
            created_at: now,
            updated_at: now,
        };
        Ok(scene_view(
            self.repository
                .create_scene(&request.project_id, &scene)
                .await?,
        ))
    }

    pub async fn update_scene(
        &self,
        request: UpdateSceneRequest,
    ) -> Result<ProductionSceneView, ProductionStructureError> {
        validate_project_id(&request.project_id)?;
        let id = parse_scene_id(&request.scene_id)?;
        let current = self
            .repository
            .load_tree_data(&request.project_id)
            .await?
            .scenes
            .into_iter()
            .find(|scene| scene.id == id)
            .ok_or_else(|| ProductionStructureError::NotFound(request.scene_id.clone()))?;
        let (name, description) = normalize_metadata(&request.name, &request.description)?;
        let updated = ProductionScene {
            name,
            description,
            updated_at: self.clock.now(),
            ..current
        };
        Ok(scene_view(
            self.repository
                .update_scene(&updated, &request.project_id)
                .await?,
        ))
    }

    pub async fn delete_scene(
        &self,
        project_id: &str,
        scene_id: &str,
    ) -> Result<(), ProductionStructureError> {
        validate_project_id(project_id)?;
        let id = parse_scene_id(scene_id)?;
        if !self.repository.delete_scene(project_id, &id).await? {
            return Err(ProductionStructureError::NotFound(scene_id.to_owned()));
        }
        Ok(())
    }

    pub async fn reorder_scenes(
        &self,
        project_id: &str,
        episode_id: &str,
        ordered_ids: &[String],
    ) -> Result<(), ProductionStructureError> {
        validate_project_id(project_id)?;
        let episode_id = parse_episode_id(episode_id)?;
        let ids = ordered_ids
            .iter()
            .map(|id| parse_scene_id(id))
            .collect::<Result<Vec<_>, _>>()?;
        self.repository
            .reorder_scenes(project_id, &episode_id, &ids, self.clock.now())
            .await?;
        Ok(())
    }

    pub async fn assign_shots(
        &self,
        project_id: &str,
        scene_id: &str,
        shot_ids: &[String],
    ) -> Result<(), ProductionStructureError> {
        validate_project_id(project_id)?;
        let scene_id = parse_scene_id(scene_id)?;
        let shot_ids = normalize_shot_ids(shot_ids)?;
        self.repository
            .assign_shots_atomic(project_id, &scene_id, &shot_ids, self.clock.now())
            .await?;
        Ok(())
    }

    pub async fn unassign_shots(
        &self,
        project_id: &str,
        shot_ids: &[String],
    ) -> Result<(), ProductionStructureError> {
        validate_project_id(project_id)?;
        let shot_ids = normalize_shot_ids(shot_ids)?;
        self.repository
            .unassign_shots_atomic(project_id, &shot_ids)
            .await?;
        Ok(())
    }

    pub async fn reorder_scene_shots(
        &self,
        scene_id: &str,
        ordered_shot_ids: &[String],
    ) -> Result<(), ProductionStructureError> {
        let scene_id = parse_scene_id(scene_id)?;
        let shot_ids = normalize_ordered_shot_ids(ordered_shot_ids)?;
        self.repository
            .reorder_scene_shots(&scene_id, &shot_ids, self.clock.now())
            .await?;
        Ok(())
    }
}

fn normalize_metadata(
    name: &str,
    description: &str,
) -> Result<(String, String), ProductionStructureError> {
    let (name, _normalized_name) = normalize_name(name, MAX_NAME_CHARS, "PRODUCTION_STRUCTURE")
        .map_err(|error| ProductionStructureError::InvalidInput(error.to_string()))?;
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(ProductionStructureError::InvalidInput(format!(
            "PRODUCTION_STRUCTURE_DESCRIPTION_TOO_LONG: description must be at most {MAX_DESCRIPTION_CHARS} characters"
        )));
    }
    Ok((name, description.to_owned()))
}

fn normalize_shot_ids(shot_ids: &[String]) -> Result<Vec<String>, ProductionStructureError> {
    if shot_ids.is_empty() || shot_ids.len() > MAX_SHOTS {
        return Err(ProductionStructureError::InvalidInput(
            "PRODUCTION_STRUCTURE_SHOT_LIMIT: shot list must contain 1..500 ids".to_owned(),
        ));
    }
    let mut normalized = Vec::with_capacity(shot_ids.len());
    for shot_id in shot_ids {
        let shot_id = shot_id.trim();
        if shot_id.is_empty() {
            return Err(ProductionStructureError::InvalidInput(
                "PRODUCTION_STRUCTURE_SHOT_ID_INVALID: shot id must not be empty".to_owned(),
            ));
        }
        if normalized.iter().any(|current| current == shot_id) {
            return Err(ProductionStructureError::InvalidInput(
                "PRODUCTION_STRUCTURE_DUPLICATE_SHOT: shot ids must be unique".to_owned(),
            ));
        }
        normalized.push(shot_id.to_owned());
    }
    Ok(normalized)
}

fn normalize_ordered_shot_ids(
    shot_ids: &[String],
) -> Result<Vec<String>, ProductionStructureError> {
    if shot_ids.len() > MAX_SHOTS {
        return Err(ProductionStructureError::InvalidInput(
            "PRODUCTION_STRUCTURE_SHOT_LIMIT: shot list must contain at most 500 ids".to_owned(),
        ));
    }
    let mut normalized = Vec::with_capacity(shot_ids.len());
    for shot_id in shot_ids {
        let shot_id = shot_id.trim();
        if shot_id.is_empty() || normalized.iter().any(|current| current == shot_id) {
            return Err(ProductionStructureError::InvalidInput(
                "PRODUCTION_STRUCTURE_SHOT_ID_INVALID: ordered shot ids must be unique and non-empty"
                    .to_owned(),
            ));
        }
        normalized.push(shot_id.to_owned());
    }
    Ok(normalized)
}

fn validate_project_id(project_id: &str) -> Result<(), ProductionStructureError> {
    crate::domain::validate_project_id(project_id)
        .map_err(|error| ProductionStructureError::InvalidInput(error.to_string()))
}

fn parse_series_id(value: &str) -> Result<ProductionSeriesId, ProductionStructureError> {
    ProductionSeriesId::parse(value.trim().to_owned())
        .map_err(|error| ProductionStructureError::InvalidInput(error.to_string()))
}

fn parse_episode_id(value: &str) -> Result<ProductionEpisodeId, ProductionStructureError> {
    ProductionEpisodeId::parse(value.trim().to_owned())
        .map_err(|error| ProductionStructureError::InvalidInput(error.to_string()))
}

fn parse_scene_id(value: &str) -> Result<ProductionSceneId, ProductionStructureError> {
    ProductionSceneId::parse(value.trim().to_owned())
        .map_err(|error| ProductionStructureError::InvalidInput(error.to_string()))
}

fn series_view(series: ProductionSeries) -> ProductionSeriesView {
    ProductionSeriesView {
        id: series.id.as_str().to_owned(),
        project_id: series.project_id,
        ordinal: series.ordinal,
        name: series.name,
        description: series.description,
        created_at: series.created_at,
        updated_at: series.updated_at,
    }
}

fn episode_view(episode: ProductionEpisode) -> ProductionEpisodeView {
    ProductionEpisodeView {
        id: episode.id.as_str().to_owned(),
        series_id: episode.series_id.as_str().to_owned(),
        ordinal: episode.ordinal,
        name: episode.name,
        description: episode.description,
        created_at: episode.created_at,
        updated_at: episode.updated_at,
    }
}

fn scene_view(scene: ProductionScene) -> ProductionSceneView {
    ProductionSceneView {
        id: scene.id.as_str().to_owned(),
        episode_id: scene.episode_id.as_str().to_owned(),
        ordinal: scene.ordinal,
        name: scene.name,
        description: scene.description,
        created_at: scene.created_at,
        updated_at: scene.updated_at,
    }
}

#[derive(Debug)]
pub enum ProductionStructureError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for ProductionStructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) => formatter.write_str(message),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ProductionStructureError {}

impl From<RepositoryError> for ProductionStructureError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateEpisodeRequest, CreateSceneRequest, CreateSeriesRequest, ProductionStructureError,
        ProductionStructureService, UpdateSeriesRequest,
    };
    use crate::application::ports::Clock;
    use crate::infrastructure::database::{
        initialize,
        repositories::{test_support, SqliteProductionStructureRepository},
    };
    use chrono::{DateTime, TimeZone, Utc};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::{tempdir, TempDir};

    struct FixedClock(DateTime<Utc>);
    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    async fn setup() -> (TempDir, SqlitePool, ProductionStructureService) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET id = 'prj_default' WHERE id = 'project-1'")
            .execute(&pool)
            .await
            .unwrap();
        let service = ProductionStructureService::new(
            Arc::new(SqliteProductionStructureRepository::new(pool.clone())),
            Arc::new(FixedClock(
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
            )),
        );
        (directory, pool, service)
    }

    #[tokio::test]
    async fn service_normalizes_names_builds_tree_and_enforces_limits() {
        let (_directory, pool, service) = setup().await;
        let series = service
            .create_series(CreateSeriesRequest {
                project_id: "prj_default".to_owned(),
                name: "  Series  ".to_owned(),
                description: " desc ".to_owned(),
            })
            .await
            .unwrap();
        let episode = service
            .create_episode(CreateEpisodeRequest {
                project_id: "prj_default".to_owned(),
                series_id: series.id.clone(),
                name: "Episode".to_owned(),
                description: String::new(),
            })
            .await
            .unwrap();
        let scene = service
            .create_scene(CreateSceneRequest {
                project_id: "prj_default".to_owned(),
                episode_id: episode.id.clone(),
                name: "Scene".to_owned(),
                description: String::new(),
            })
            .await
            .unwrap();
        for (id, ordinal) in [("shot_a", 0), ("shot_b", 1)] {
            sqlx::query("INSERT INTO shots (id, project_id, ordinal, name, prompt_text, created_at, updated_at) VALUES (?, 'prj_default', ?, ?, '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
                .bind(id).bind(ordinal).bind(id).execute(&pool).await.unwrap();
        }
        service
            .assign_shots(
                "prj_default",
                &scene.id,
                &["shot_b".to_owned(), "shot_a".to_owned()],
            )
            .await
            .unwrap();
        let tree = service.tree("prj_default").await.unwrap();
        assert_eq!(tree.series[0].series.name, "Series");
        assert_eq!(
            tree.series[0].episodes[0].scenes[0].shot_ids,
            ["shot_b", "shot_a"]
        );
        assert!(tree.unassigned_shot_ids.is_empty());
        assert!(matches!(
            service
                .update_series(UpdateSeriesRequest {
                    project_id: "prj_default".to_owned(),
                    series_id: series.id,
                    name: "bad\nname".to_owned(),
                    description: String::new()
                })
                .await,
            Err(ProductionStructureError::InvalidInput(_))
        ));
    }
}
