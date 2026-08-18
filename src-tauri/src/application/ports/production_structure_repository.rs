use crate::domain::{
    ProductionEpisode, ProductionEpisodeId, ProductionScene, ProductionSceneId, ProductionSeries,
    ProductionSeriesId, ShotSceneAssignment,
};
use async_trait::async_trait;

use super::RepositoryError;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductionStructureTreeData {
    pub series: Vec<ProductionSeries>,
    pub episodes: Vec<ProductionEpisode>,
    pub scenes: Vec<ProductionScene>,
    pub assignments: Vec<ShotSceneAssignment>,
    pub project_shot_ids: Vec<String>,
}

#[async_trait]
pub trait ProductionStructureRepository: Send + Sync {
    async fn load_tree_data(
        &self,
        project_id: &str,
    ) -> Result<ProductionStructureTreeData, RepositoryError>;

    async fn create_series(
        &self,
        series: &ProductionSeries,
    ) -> Result<ProductionSeries, RepositoryError>;
    async fn update_series(
        &self,
        series: &ProductionSeries,
    ) -> Result<ProductionSeries, RepositoryError>;
    async fn delete_series(
        &self,
        project_id: &str,
        id: &ProductionSeriesId,
    ) -> Result<bool, RepositoryError>;
    async fn reorder_series(
        &self,
        project_id: &str,
        ordered_ids: &[ProductionSeriesId],
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError>;

    async fn create_episode(
        &self,
        project_id: &str,
        episode: &ProductionEpisode,
    ) -> Result<ProductionEpisode, RepositoryError>;
    async fn update_episode(
        &self,
        episode: &ProductionEpisode,
        project_id: &str,
    ) -> Result<ProductionEpisode, RepositoryError>;
    async fn delete_episode(
        &self,
        project_id: &str,
        id: &ProductionEpisodeId,
    ) -> Result<bool, RepositoryError>;
    async fn reorder_episodes(
        &self,
        project_id: &str,
        series_id: &ProductionSeriesId,
        ordered_ids: &[ProductionEpisodeId],
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError>;

    async fn create_scene(
        &self,
        project_id: &str,
        scene: &ProductionScene,
    ) -> Result<ProductionScene, RepositoryError>;
    async fn update_scene(
        &self,
        scene: &ProductionScene,
        project_id: &str,
    ) -> Result<ProductionScene, RepositoryError>;
    async fn delete_scene(
        &self,
        project_id: &str,
        id: &ProductionSceneId,
    ) -> Result<bool, RepositoryError>;
    async fn reorder_scenes(
        &self,
        project_id: &str,
        episode_id: &ProductionEpisodeId,
        ordered_ids: &[ProductionSceneId],
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError>;

    async fn assign_shots_atomic(
        &self,
        project_id: &str,
        scene_id: &ProductionSceneId,
        shot_ids: &[String],
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError>;
    async fn unassign_shots_atomic(
        &self,
        project_id: &str,
        shot_ids: &[String],
    ) -> Result<(), RepositoryError>;
    async fn reorder_scene_shots(
        &self,
        scene_id: &ProductionSceneId,
        ordered_shot_ids: &[String],
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError>;
}
