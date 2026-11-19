mod media_integrity;
use crate::application::ports::{
    AssetRepository, AssetStore, AssetStoreError, GenerationDefinitionRepository,
    OrganizationRepository, ProjectRepository, RepositoryError, TaskOutputAssetMapping,
    TaskRepository,
};
use crate::compiler::RecipeParser;
use crate::domain::{AssetId, AssetType, TaskId};
use chrono::{DateTime, Utc};
pub use media_integrity::*;
use serde::Serialize;
use std::{error::Error, fmt, sync::Arc};

pub struct AssetQueryService {
    media_probe: Arc<dyn crate::application::media_probe::MediaProbe>,
    asset_repository: Arc<dyn AssetRepository>,
    asset_store: Arc<dyn AssetStore>,
    project_repository: Arc<dyn ProjectRepository>,
    task_repository: Option<Arc<dyn TaskRepository>>,
    definition_repository: Option<Arc<dyn GenerationDefinitionRepository>>,
    organization_repository: Option<Arc<dyn OrganizationRepository>>,
}

impl AssetQueryService {
    pub fn new(
        asset_repository: Arc<dyn AssetRepository>,
        asset_store: Arc<dyn AssetStore>,
        project_repository: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self {
            media_probe: Arc::new(crate::application::media_probe::CommandMediaProbe::default()),
            asset_repository,
            asset_store,
            project_repository,
            task_repository: None,
            definition_repository: None,
            organization_repository: None,
        }
    }

    pub fn with_media_probe(
        mut self,
        probe: Arc<dyn crate::application::media_probe::MediaProbe>,
    ) -> Self {
        self.media_probe = probe;
        self
    }

    pub fn with_organization_repository(
        mut self,
        repository: Arc<dyn OrganizationRepository>,
    ) -> Self {
        self.organization_repository = Some(repository);
        self
    }

    pub fn with_output_order_repositories(
        mut self,
        task_repository: Arc<dyn TaskRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
    ) -> Self {
        self.task_repository = Some(task_repository);
        self.definition_repository = Some(definition_repository);
        self
    }

    pub async fn list_by_task(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<Vec<AssetView>, AssetQueryError> {
        validate_project_id(project_id)?;
        let task_id = TaskId::parse(task_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidTaskId(error.to_string()))?;
        let mut mapped = self.asset_repository.list_mapped_assets(&task_id).await?;
        if mapped.is_empty() {
            return Ok(self
                .asset_repository
                .list_by_source_task(&task_id)
                .await?
                .into_iter()
                .filter(|asset| asset.project_id == project_id)
                .map(AssetView::from)
                .collect());
        }

        let output_order = self.output_order(&task_id).await?;
        mapped.sort_by(|(left, _), (right, _)| compare_mappings(left, right, &output_order));
        Ok(mapped
            .into_iter()
            .filter(|(_, asset)| asset.project_id == project_id)
            .map(|(_, asset)| AssetView::from(asset))
            .collect())
    }

    /// Project-scoped batch projection preserving mapping-first fallback and Recipe output order.
    pub async fn list_by_tasks(
        &self,
        project_id: &str,
        task_ids: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<AssetView>>, AssetQueryError> {
        use std::collections::{HashMap, HashSet};
        validate_project_id(project_id)?;
        let ids = task_ids
            .iter()
            .map(|id| {
                TaskId::parse(id.clone()).map_err(|e| AssetQueryError::InvalidTaskId(e.to_string()))
            })
            .collect::<Result<HashSet<_>, _>>()?
            .into_iter()
            .collect::<Vec<_>>();
        let mut out = HashMap::new();
        for chunk in ids.chunks(500) {
            let mappings = self
                .asset_repository
                .list_output_mappings_for_tasks(chunk)
                .await?;
            let asset_ids = mappings
                .iter()
                .map(|m| m.asset_id.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut by_id = HashMap::new();
            for group in asset_ids.chunks(500) {
                for asset in self.asset_repository.find_many_by_ids(group).await? {
                    by_id.insert(asset.id.clone(), asset);
                }
            }
            let mut mapped: HashMap<TaskId, Vec<_>> = HashMap::new();
            for mapping in mappings {
                if let Some(asset) = by_id.get(&mapping.asset_id) {
                    mapped
                        .entry(mapping.task_id.clone())
                        .or_default()
                        .push((mapping, asset.clone()));
                }
            }
            let fallback_ids = chunk
                .iter()
                .filter(|id| !mapped.contains_key(*id))
                .cloned()
                .collect::<Vec<_>>();
            let fallback = self
                .asset_repository
                .list_by_source_tasks(&fallback_ids)
                .await?;
            let mut orders = HashMap::new();
            if !mapped.is_empty() {
                if let (Some(tasks), Some(definitions)) =
                    (&self.task_repository, &self.definition_repository)
                {
                    let tasks = tasks
                        .find_many_by_ids(&mapped.keys().cloned().collect::<Vec<_>>())
                        .await?;
                    let pairs = tasks
                        .iter()
                        .map(|t| (t.workflow_version_id.clone(), t.recipe_id.clone()))
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    let definitions = definitions
                        .find_many(&pairs)
                        .await?
                        .into_iter()
                        .map(|d| ((d.workflow_version_id, d.recipe_id), d.recipe_yaml))
                        .collect::<HashMap<_, _>>();
                    for task in tasks {
                        if let Some(yaml) =
                            definitions.get(&(task.workflow_version_id, task.recipe_id))
                        {
                            orders.insert(
                                task.id,
                                RecipeParser::parse(yaml)
                                    .map(|r| {
                                        r.outputs.into_iter().map(|o| o.id).collect::<Vec<_>>()
                                    })
                                    .unwrap_or_default(),
                            );
                        }
                    }
                }
            }
            for id in chunk {
                let assets = if let Some(mut pairs) = mapped.remove(id) {
                    let order = orders.remove(id).unwrap_or_default();
                    pairs.sort_by(|(a, _), (b, _)| compare_mappings(a, b, &order));
                    pairs
                        .into_iter()
                        .filter(|(_, a)| a.project_id == project_id)
                        .map(|(_, a)| AssetView::from(a))
                        .collect()
                } else {
                    fallback
                        .iter()
                        .filter(|a| {
                            a.project_id == project_id && a.source_task_id.as_ref() == Some(id)
                        })
                        .cloned()
                        .map(AssetView::from)
                        .collect()
                };
                out.insert(id.as_str().to_owned(), assets);
            }
        }
        Ok(out)
    }

    /// Bulk ownership lookup plus one project-root read, bounded by the workbench page.
    /// Unavailable thumbnails remain optional exactly as in the individual projection.
    pub async fn read_thumbnails(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, AssetQueryError> {
        validate_project_id(project_id)?;
        let mut out = std::collections::HashMap::new();
        if asset_ids.is_empty() {
            return Ok(out);
        }
        let mut root = None;
        for chunk in asset_ids.chunks(500) {
            let ids = chunk
                .iter()
                .map(|id| {
                    AssetId::parse(id.clone())
                        .map_err(|e| AssetQueryError::InvalidAssetId(e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            for asset in self.asset_repository.find_many_by_ids(&ids).await? {
                if asset.project_id != project_id
                    || !matches!(asset.asset_type, AssetType::Image | AssetType::Video)
                {
                    continue;
                }
                if let Some(path) = asset.thumbnail_path {
                    if root.is_none() {
                        match self.project_root(project_id).await {
                            Ok(value) => root = Some(value),
                            Err(_) => return Ok(out),
                        }
                    }
                    let Some(root) = &root else {
                        continue;
                    };
                    if let Ok(bytes) = self
                        .asset_store
                        .read(root, std::path::Path::new(&path))
                        .await
                    {
                        if bytes.len() <= 256 * 1024 {
                            out.insert(asset.id.as_str().to_owned(), bytes);
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    async fn output_order(&self, task_id: &TaskId) -> Result<Vec<String>, AssetQueryError> {
        let (Some(task_repository), Some(definition_repository)) =
            (&self.task_repository, &self.definition_repository)
        else {
            return Ok(Vec::new());
        };
        let Some(task) = task_repository.find_by_id(task_id).await? else {
            return Ok(Vec::new());
        };
        let Some(definition) = definition_repository
            .find(&task.workflow_version_id, &task.recipe_id)
            .await?
        else {
            return Ok(Vec::new());
        };
        Ok(RecipeParser::parse(&definition.recipe_yaml)
            .map(|recipe| recipe.outputs.into_iter().map(|output| output.id).collect())
            .unwrap_or_default())
    }

    pub async fn list_recent(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<AssetView>, AssetQueryError> {
        if project_id.trim().is_empty() {
            return Err(AssetQueryError::InvalidProjectId(
                "project id must not be empty".to_owned(),
            ));
        }
        Ok(self
            .asset_repository
            .list_recent(project_id, limit.min(100))
            .await?
            .into_iter()
            .map(AssetView::from)
            .collect())
    }

    pub async fn get(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<AssetSummaryView, AssetQueryError> {
        if project_id.trim().is_empty() {
            return Err(AssetQueryError::InvalidProjectId(
                "project id must not be empty".to_owned(),
            ));
        }
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidAssetId(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetQueryError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id {
            return Err(AssetQueryError::NotFound(asset_id.as_str().to_owned()));
        }
        let receipt = self
            .asset_repository
            .find_external_import(project_id, &asset_id)
            .await?;
        let has_external_import_receipt =
            external_import_receipt_matches(project_id, &asset, receipt.as_ref());
        let organization = if let Some(repository) = &self.organization_repository {
            repository
                .organization_for_assets(project_id, &[asset_id.as_str().to_owned()])
                .await?
                .remove(asset_id.as_str())
                .unwrap_or_default()
        } else {
            Default::default()
        };
        let mut view = AssetSummaryView::from_asset_and_organization(asset, organization);
        view.has_external_import_receipt = has_external_import_receipt;
        Ok(view)
    }

    /// A database row alone is not a usable result. Recheck the managed file at
    /// the action boundary without reading an entire video into memory.
    pub async fn is_file_available(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<bool, AssetQueryError> {
        validate_project_id(project_id)?;
        let id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidAssetId(error.to_string()))?;
        let Some(asset) = self.asset_repository.find_by_id(&id).await? else {
            return Ok(false);
        };
        if asset.project_id != project_id {
            return Ok(false);
        }
        let root = self.project_root(project_id).await?;
        Ok(matches!(
            crate::application::ports::inspect_asset_read_path(
                &root,
                std::path::Path::new(&asset.storage_path)
            ),
            crate::application::ports::AssetReadInspection::Present { .. }
        ))
    }

    pub async fn read_image(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<AssetBinary, AssetQueryError> {
        validate_project_id(project_id)?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidAssetId(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetQueryError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id {
            return Err(AssetQueryError::NotFound(asset_id.as_str().to_owned()));
        }
        if asset.asset_type != AssetType::Image {
            return Err(AssetQueryError::NotImage(asset_id.as_str().to_owned()));
        }
        let project_root = self.project_root(project_id).await?;
        let bytes = self
            .asset_store
            .read(&project_root, std::path::Path::new(&asset.storage_path))
            .await
            .map_err(AssetQueryError::Read)?;
        Ok(AssetBinary { bytes })
    }

    pub async fn read_thumbnail(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<AssetBinary, AssetQueryError> {
        validate_project_id(project_id)?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidAssetId(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetQueryError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id {
            return Err(AssetQueryError::NotFound(asset_id.as_str().to_owned()));
        }
        if !matches!(asset.asset_type, AssetType::Image | AssetType::Video) {
            return Err(AssetQueryError::NotImage(asset_id.as_str().to_owned()));
        }
        let path = asset
            .thumbnail_path
            .ok_or_else(|| AssetQueryError::ThumbnailNotAvailable(asset_id.as_str().to_owned()))?;
        let project_root = self.project_root(project_id).await?;
        let bytes = self
            .asset_store
            .read(&project_root, std::path::Path::new(&path))
            .await
            .map_err(AssetQueryError::Read)?;
        Ok(AssetBinary { bytes })
    }

    async fn project_root(&self, project_id: &str) -> Result<std::path::PathBuf, AssetQueryError> {
        self.project_repository
            .get_storage_root(project_id)
            .await?
            .ok_or_else(|| {
                AssetQueryError::Read(AssetStoreError::FilesystemBoundary(format!(
                    "storage root is not configured for project {project_id}"
                )))
            })
    }
}

fn compare_mappings(
    left: &TaskOutputAssetMapping,
    right: &TaskOutputAssetMapping,
    output_order: &[String],
) -> std::cmp::Ordering {
    let left_rank = output_order
        .iter()
        .position(|output_id| output_id == &left.output_id)
        .unwrap_or(usize::MAX);
    let right_rank = output_order
        .iter()
        .position(|output_id| output_id == &right.output_id)
        .unwrap_or(usize::MAX);
    left_rank
        .cmp(&right_rank)
        .then_with(|| left.ordinal.cmp(&right.ordinal))
        .then_with(|| left.output_id.cmp(&right.output_id))
}

fn validate_project_id(project_id: &str) -> Result<(), AssetQueryError> {
    if project_id.trim().is_empty() {
        return Err(AssetQueryError::InvalidProjectId(
            "project id must not be empty".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSummaryView {
    pub id: String,
    pub asset_type: String,
    pub category: String,
    pub name: String,
    pub original_name: String,
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub file_size: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source_task_id: Option<String>,
    pub thumbnail_available: bool,
    pub is_favorite: bool,
    pub tags: Vec<TagSummaryView>,
    /// Project-scoped external import receipt identity gate for formal SOURCE binds.
    /// Populated by detail `get`; list projections leave this false.
    #[serde(default)]
    pub has_external_import_receipt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSummaryView {
    pub id: String,
    pub name: String,
}

impl From<crate::domain::Asset> for AssetSummaryView {
    fn from(asset: crate::domain::Asset) -> Self {
        let original_name = if matches!(
            asset.category.as_str(),
            crate::domain::asset::GENERATED_IMAGE_CATEGORY
                | crate::domain::asset::GENERATED_VIDEO_CATEGORY
        ) {
            asset.name.clone()
        } else {
            asset.original_name.clone()
        };
        Self {
            id: asset.id.as_str().to_owned(),
            asset_type: asset.asset_type.as_str().to_owned(),
            category: asset.category,
            name: asset.name,
            original_name,
            mime_type: asset.mime_type,
            width: (asset.width > 0).then_some(asset.width),
            height: (asset.height > 0).then_some(asset.height),
            duration_ms: asset.duration_ms,
            file_size: asset.file_size,
            created_at: asset.created_at,
            updated_at: asset.updated_at,
            source_task_id: asset
                .source_task_id
                .map(|task_id| task_id.as_str().to_owned()),
            thumbnail_available: asset.thumbnail_path.is_some(),
            is_favorite: false,
            tags: Vec::new(),
            has_external_import_receipt: false,
        }
    }
}

/// Lightweight receipt+media identity check aligned with formal image adoption.
/// Does not decode file bytes; save-time validation still rechecks content.
fn external_import_receipt_matches(
    project_id: &str,
    asset: &crate::domain::Asset,
    receipt: Option<&crate::application::ports::ExternalAssetImportRecord>,
) -> bool {
    let Some(receipt) = receipt else {
        return false;
    };
    let media_type_ok = match asset.asset_type {
        AssetType::Image => receipt.media_type == "image",
        AssetType::Video => receipt.media_type == "video",
        AssetType::Audio => receipt.media_type == "audio",
    };
    media_type_ok
        && receipt.asset_id == asset.id.as_str()
        && receipt.project_id == project_id
        && receipt.sha256 == asset.sha256
        && receipt.mime_type == asset.mime_type
        && receipt.file_size == asset.file_size
        && receipt.width == asset.width
        && receipt.height == asset.height
        && receipt.duration_ms == asset.duration_ms
}

impl AssetSummaryView {
    pub fn from_asset_and_organization(
        asset: crate::domain::Asset,
        organization: crate::application::ports::AssetOrganization,
    ) -> Self {
        let mut view = Self::from(asset);
        view.is_favorite = organization.is_favorite;
        view.tags = organization
            .tags
            .into_iter()
            .map(|tag| TagSummaryView {
                id: tag.id,
                name: tag.name,
            })
            .collect();
        view
    }
}

pub type AssetView = AssetSummaryView;

pub struct AssetBinary {
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum AssetQueryError {
    InvalidTaskId(String),
    InvalidProjectId(String),
    InvalidAssetId(String),
    NotFound(String),
    NotImage(String),
    ThumbnailNotAvailable(String),
    Repository(RepositoryError),
    Read(AssetStoreError),
}

impl fmt::Display for AssetQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTaskId(message) => write!(formatter, "INVALID_TASK_ID: {message}"),
            Self::InvalidProjectId(message) => write!(formatter, "INVALID_PROJECT_ID: {message}"),
            Self::InvalidAssetId(message) => write!(formatter, "INVALID_ASSET_ID: {message}"),
            Self::NotFound(id) => write!(formatter, "ASSET_NOT_FOUND: asset {id} was not found"),
            Self::NotImage(id) => write!(formatter, "ASSET_NOT_IMAGE: asset {id} is not an image"),
            Self::ThumbnailNotAvailable(id) => write!(
                formatter,
                "THUMBNAIL_NOT_AVAILABLE: asset {id} has no thumbnail"
            ),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Read(error) => write!(formatter, "ASSET_READ_FAILED: {error}"),
        }
    }
}

impl Error for AssetQueryError {}

impl From<RepositoryError> for AssetQueryError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetQueryError, AssetQueryService};
    use crate::application::ports::{
        AssetRepository, AssetStore, AssetStoreError, StoredAssetFile, TaskRepository,
    };
    use crate::domain::{Asset, AssetId, Task};
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteAssetRepository, SqliteProjectRepository,
        SqliteTaskRepository,
    };
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::path::Path;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tempfile::tempdir;

    struct CountingRepository {
        inner: Arc<SqliteAssetRepository>,
        singles: Arc<AtomicUsize>,
        bulk: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl AssetRepository for CountingRepository {
        async fn insert_many(
            &self,
            assets: &[Asset],
        ) -> Result<(), crate::application::ports::RepositoryError> {
            self.inner.insert_many(assets).await
        }
        async fn find_by_id(
            &self,
            id: &AssetId,
        ) -> Result<Option<Asset>, crate::application::ports::RepositoryError> {
            self.singles.fetch_add(1, Ordering::SeqCst);
            self.inner.find_by_id(id).await
        }
        async fn list_by_source_task(
            &self,
            id: &crate::domain::TaskId,
        ) -> Result<Vec<Asset>, crate::application::ports::RepositoryError> {
            self.singles.fetch_add(1, Ordering::SeqCst);
            self.inner.list_by_source_task(id).await
        }
        async fn list_by_source_tasks(
            &self,
            ids: &[crate::domain::TaskId],
        ) -> Result<Vec<Asset>, crate::application::ports::RepositoryError> {
            self.bulk.fetch_add(1, Ordering::SeqCst);
            self.inner.list_by_source_tasks(ids).await
        }
        async fn find_many_by_ids(
            &self,
            ids: &[AssetId],
        ) -> Result<Vec<Asset>, crate::application::ports::RepositoryError> {
            self.bulk.fetch_add(1, Ordering::SeqCst);
            self.inner.find_many_by_ids(ids).await
        }
        async fn list_recent(
            &self,
            p: &str,
            n: u32,
        ) -> Result<Vec<Asset>, crate::application::ports::RepositoryError> {
            self.inner.list_recent(p, n).await
        }
        async fn list_output_mappings_for_tasks(
            &self,
            ids: &[crate::domain::TaskId],
        ) -> Result<
            Vec<crate::application::ports::TaskOutputAssetMapping>,
            crate::application::ports::RepositoryError,
        > {
            self.bulk.fetch_add(1, Ordering::SeqCst);
            self.inner.list_output_mappings_for_tasks(ids).await
        }
        async fn list_output_mappings(
            &self,
            id: &crate::domain::TaskId,
        ) -> Result<
            Vec<crate::application::ports::TaskOutputAssetMapping>,
            crate::application::ports::RepositoryError,
        > {
            self.singles.fetch_add(1, Ordering::SeqCst);
            self.inner.list_output_mappings(id).await
        }
    }
    #[tokio::test]
    async fn batch_candidates_preserve_projection_without_per_task_reads() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET root_path = ? WHERE id = 'project-1'")
            .bind(directory.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .unwrap();
        std::fs::write(directory.path().join("thumb.png"), b"thumbnail").unwrap();
        let tasks = SqliteTaskRepository::new(pool.clone());
        let inner = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let mut ids = Vec::new();
        for i in 0..40 {
            let task = Task::new(
                "project-1",
                "workflow-1",
                "workflow-version-1",
                "recipe-1",
                Utc::now(),
            );
            tasks.create(&task, &task.created_event()).await.unwrap();
            ids.push(task.id.as_str().to_owned());
            let mut asset = Asset::new_image(
                AssetId::parse(format!("ast_batch_{i}")).unwrap(),
                "project-1",
                "image",
                "out.png",
                "out.png",
                "a".repeat(64),
                "image/png",
                1,
                1,
                1,
                task.id.clone(),
                json!({}),
                task.created_at,
            )
            .unwrap();
            asset.thumbnail_path = Some("thumb.png".into());
            if i % 2 == 0 {
                inner
                    .insert_generated_outputs(
                        &[asset.clone()],
                        &[crate::application::ports::TaskOutputAssetMapping {
                            task_id: task.id,
                            output_id: "image".into(),
                            ordinal: 0,
                            asset_id: asset.id,
                            created_at: task.created_at,
                        }],
                    )
                    .await
                    .unwrap();
            } else {
                inner.insert_many(&[asset]).await.unwrap();
            }
        }
        let singles = Arc::new(AtomicUsize::new(0));
        let bulk = Arc::new(AtomicUsize::new(0));
        let service = AssetQueryService::new(
            Arc::new(CountingRepository {
                inner,
                singles: singles.clone(),
                bulk: bulk.clone(),
            }),
            Arc::new(FileSystemAssetStore::new()),
            Arc::new(SqliteProjectRepository::new(pool)),
        );
        let start = std::time::Instant::now();
        let mut expected = std::collections::HashMap::new();
        for id in &ids {
            expected.insert(
                id.clone(),
                service.list_by_task("project-1", id).await.unwrap(),
            );
        }
        let before = (singles.swap(0, Ordering::SeqCst), start.elapsed());
        let start = std::time::Instant::now();
        let actual = service.list_by_tasks("project-1", &ids).await.unwrap();
        let after = singles.load(Ordering::SeqCst);
        println!("candidate_batch before_single_reads={} before_ms={:.3} after_single_reads={} after_ms={:.3}",before.0,before.1.as_secs_f64()*1000.0,after,start.elapsed().as_secs_f64()*1000.0);
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(after, 0, "workbench candidate reads must be set-based");
        assert_eq!(
            bulk.load(Ordering::SeqCst),
            3,
            "mapping, mapped assets, legacy fallback are each one batch read"
        );
        let thumbnails = (0..40)
            .map(|i| format!("ast_batch_{i}"))
            .collect::<Vec<_>>();
        let previous = bulk.load(Ordering::SeqCst);
        assert_eq!(
            service
                .read_thumbnails("project-1", &thumbnails)
                .await
                .unwrap()
                .len(),
            40
        );
        assert_eq!(
            bulk.load(Ordering::SeqCst) - previous,
            1,
            "one bulk ownership lookup for all thumbnails"
        );
        assert!(service
            .read_thumbnails("project-2", &thumbnails)
            .await
            .unwrap()
            .is_empty());
        assert!(service
            .list_by_tasks("project-2", &ids)
            .await
            .unwrap()
            .values()
            .all(Vec::is_empty));
    }

    #[derive(Clone)]
    struct CountingAssetStore {
        reads: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AssetStore for CountingAssetStore {
        async fn write_image(
            &self,
            _project_root: &Path,
            _asset_id: &AssetId,
            _extension: &str,
            _bytes: &[u8],
        ) -> Result<StoredAssetFile, AssetStoreError> {
            unreachable!()
        }

        async fn delete(&self, _path: &Path) -> Result<(), AssetStoreError> {
            unreachable!()
        }

        async fn read(
            &self,
            _project_root: &Path,
            _path: &Path,
        ) -> Result<Vec<u8>, AssetStoreError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn reads_image_by_asset_id_and_rejects_unknown_asset() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );
        let task_repository = SqliteTaskRepository::new(pool.clone());
        task_repository
            .create(&task, &task.created_event())
            .await
            .unwrap();

        let path = directory.path().join("image.png");
        let bytes = b"image-bytes".to_vec();
        std::fs::write(&path, &bytes).unwrap();
        let asset = Asset::new_image(
            AssetId::parse("ast_read_test").unwrap(),
            "project-1",
            "image",
            "ComfyUI_00001.png",
            path.to_string_lossy(),
            "a".repeat(64),
            "image/png",
            1,
            1,
            bytes.len() as u64,
            task.id.clone(),
            json!({}),
            task.created_at,
        )
        .unwrap();
        let repository = SqliteAssetRepository::new(pool.clone());
        repository.insert_many(&[asset]).await.unwrap();
        sqlx::query("UPDATE projects SET root_path = ? WHERE id = 'project-1'")
            .bind(directory.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .unwrap();
        let service = AssetQueryService::new(
            Arc::new(repository),
            Arc::new(FileSystemAssetStore::new()),
            Arc::new(SqliteProjectRepository::new(pool)),
        );

        assert_eq!(
            service
                .read_image("project-1", "ast_read_test")
                .await
                .unwrap()
                .bytes,
            bytes
        );
        let moved_path = directory.path().join("moved-image.png");
        std::fs::rename(&path, &moved_path).unwrap();
        assert!(matches!(
            service.read_image("project-1", "ast_read_test").await,
            Err(AssetQueryError::Read(_))
        ));
        assert!(matches!(
            service.read_image("project-1", "ast_missing").await,
            Err(AssetQueryError::NotFound(_))
        ));

        let summary = service.get("project-1", "ast_read_test").await.unwrap();
        let json = serde_json::to_string(&summary).unwrap();
        for forbidden in [
            "storagePath",
            "thumbnailPath",
            "sha256",
            "metadata",
            "metadataJson",
            "comfyFilename",
            "nodeId",
            "ComfyUI_",
        ] {
            assert!(
                !json.contains(forbidden),
                "secure asset DTO leaked {forbidden}"
            );
        }
        assert!(matches!(
            service.get("project-2", "ast_read_test").await,
            Err(AssetQueryError::NotFound(_))
        ));
        assert!(matches!(
            service.read_image("project-2", "ast_read_test").await,
            Err(AssetQueryError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn cross_project_binary_read_fails_before_filesystem_access() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let task_repository = SqliteTaskRepository::new(pool.clone());
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );
        task_repository
            .create(&task, &task.created_event())
            .await
            .unwrap();
        let path = directory.path().join("image.png");
        std::fs::write(&path, b"image").unwrap();
        let asset = Asset::new_image(
            AssetId::parse("ast_cross_project").unwrap(),
            "project-1",
            "image",
            "image.png",
            path.to_string_lossy(),
            "a".repeat(64),
            "image/png",
            1,
            1,
            5,
            task.id,
            json!({}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
        )
        .unwrap();
        let repository = SqliteAssetRepository::new(pool.clone());
        repository.insert_many(&[asset]).await.unwrap();
        let reads = Arc::new(AtomicUsize::new(0));
        let service = AssetQueryService::new(
            Arc::new(repository),
            Arc::new(CountingAssetStore {
                reads: reads.clone(),
            }),
            Arc::new(SqliteProjectRepository::new(pool)),
        );

        assert!(matches!(
            service.read_image("project-2", "ast_cross_project").await,
            Err(AssetQueryError::NotFound(_))
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }
}
