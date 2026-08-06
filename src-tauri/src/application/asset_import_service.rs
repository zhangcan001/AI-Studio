use crate::application::image_inspection::{inspect_bytes, InspectedImage};
use crate::application::output_collector::CollectedImage;
use crate::application::ports::{
    AssetRepository, AssetStore, Clock, ProjectRepository, RepositoryError,
};
use crate::domain::{Asset, AssetId, TaskId};
use serde_json::json;
use std::{error::Error, fmt, path::PathBuf, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub enum AssetImportError {
    ProjectStorageMissing { project_id: String },
    OutputImportFailed { message: String },
    AssetPersistence { message: String },
}

impl AssetImportError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProjectStorageMissing { .. } | Self::AssetPersistence { .. } => {
                "ASSET_PERSISTENCE_ERROR"
            }
            Self::OutputImportFailed { .. } => "OUTPUT_IMPORT_FAILED",
        }
    }
}

impl fmt::Display for AssetImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectStorageMissing { project_id } => write!(
                formatter,
                "ASSET_PERSISTENCE_ERROR: project {project_id} has no storage root"
            ),
            Self::OutputImportFailed { message } => {
                write!(formatter, "OUTPUT_IMPORT_FAILED: {message}")
            }
            Self::AssetPersistence { message } => {
                write!(formatter, "ASSET_PERSISTENCE_ERROR: {message}")
            }
        }
    }
}

impl Error for AssetImportError {}

pub struct AssetImportService {
    project_repository: Arc<dyn ProjectRepository>,
    asset_store: Arc<dyn AssetStore>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl AssetImportService {
    pub fn new(
        project_repository: Arc<dyn ProjectRepository>,
        asset_store: Arc<dyn AssetStore>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            project_repository,
            asset_store,
            asset_repository,
            clock,
        }
    }

    pub async fn import(
        &self,
        project_id: &str,
        task_id: &TaskId,
        images: &[CollectedImage],
    ) -> Result<Vec<Asset>, AssetImportError> {
        if images.is_empty() {
            return Ok(Vec::new());
        }
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| AssetImportError::ProjectStorageMissing {
                project_id: project_id.to_owned(),
            })?;

        let inspected = images
            .iter()
            .map(inspect_image)
            .collect::<Result<Vec<_>, _>>()?;

        let mut stored_paths = Vec::<PathBuf>::with_capacity(images.len());
        let mut assets = Vec::with_capacity(images.len());
        for (image, inspected) in images.iter().zip(inspected.iter()) {
            let asset_id = AssetId::new();
            let stored = match self
                .asset_store
                .write_image(&project_root, &asset_id, inspected.extension, &image.bytes)
                .await
            {
                Ok(stored) => stored,
                Err(error) => {
                    self.compensate(&stored_paths).await;
                    return Err(AssetImportError::AssetPersistence {
                        message: error.to_string(),
                    });
                }
            };
            stored_paths.push(stored.path.clone());
            let created_at = self.clock.now();
            let metadata = json!({
                "outputId": image.output_id,
                "nodeId": image.node_id,
                "position": image.position,
                "comfyFilename": image.original_filename,
                "comfySubfolder": image.subfolder,
                "comfyType": image.folder_type,
            });
            let asset = match Asset::new_generated_image(
                asset_id,
                project_id,
                format!("Generated Image {}", image.position + 1),
                image.original_filename.clone(),
                stored.path.display().to_string(),
                inspected.sha256.clone(),
                inspected.mime_type,
                inspected.width,
                inspected.height,
                image.bytes.len() as u64,
                task_id.clone(),
                metadata,
                created_at,
            ) {
                Ok(asset) => asset,
                Err(error) => {
                    self.compensate(&stored_paths).await;
                    return Err(AssetImportError::OutputImportFailed {
                        message: error.to_string(),
                    });
                }
            };
            assets.push(asset);
        }

        if let Err(error) = self.asset_repository.insert_many(&assets).await {
            self.compensate(&stored_paths).await;
            return Err(AssetImportError::AssetPersistence {
                message: error.to_string(),
            });
        }

        Ok(assets)
    }

    async fn compensate(&self, paths: &[PathBuf]) {
        for path in paths {
            if let Err(error) = self.asset_store.delete(path).await {
                tracing::error!(
                    path = %path.display(),
                    error = %error,
                    "asset compensation delete failed"
                );
            }
        }
    }
}

fn inspect_image(image: &CollectedImage) -> Result<InspectedImage, AssetImportError> {
    inspect_bytes(&image.bytes).map_err(|error| invalid_image(error.to_string()))
}

fn invalid_image(message: impl Into<String>) -> AssetImportError {
    AssetImportError::OutputImportFailed {
        message: message.into(),
    }
}

fn repository_error(error: RepositoryError) -> AssetImportError {
    AssetImportError::AssetPersistence {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetImportError, AssetImportService};
    use crate::application::output_collector::CollectedImage;
    use crate::application::ports::{
        AssetRepository, Clock, ProjectRecord, ProjectRepository, RepositoryError,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use async_trait::async_trait;
    use chrono::{DateTime, TimeZone, Utc};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Clone)]
    struct FakeProjectRepository {
        root: PathBuf,
    }

    #[async_trait]
    impl ProjectRepository for FakeProjectRepository {
        async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn find_by_id(
            &self,
            project_id: &str,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(Some(ProjectRecord {
                id: project_id.to_owned(),
                name: "Test Project".to_owned(),
                description: None,
                root_path: self.root.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))
        }

        async fn insert(&self, _project: &ProjectRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn update_metadata(
            &self,
            project_id: &str,
            name: &str,
            description: Option<&str>,
            updated_at: DateTime<Utc>,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(Some(ProjectRecord {
                id: project_id.to_owned(),
                name: name.to_owned(),
                description: description.map(str::to_owned),
                root_path: self.root.clone(),
                created_at: updated_at,
                updated_at,
            }))
        }

        async fn get_storage_root(
            &self,
            _project_id: &str,
        ) -> Result<Option<PathBuf>, RepositoryError> {
            Ok(Some(self.root.clone()))
        }

        async fn ensure_default_project(
            &self,
            project_id: &str,
            name: &str,
            root_path: &PathBuf,
            created_at: DateTime<Utc>,
        ) -> Result<ProjectRecord, RepositoryError> {
            Ok(ProjectRecord {
                id: project_id.to_owned(),
                name: name.to_owned(),
                description: None,
                root_path: root_path.clone(),
                created_at,
                updated_at: created_at,
            })
        }
    }

    #[derive(Clone, Default)]
    struct FakeAssetRepository {
        assets: Arc<Mutex<Vec<Asset>>>,
        fail: bool,
    }

    #[async_trait]
    impl AssetRepository for FakeAssetRepository {
        async fn insert_many(&self, assets: &[Asset]) -> Result<(), RepositoryError> {
            if self.fail {
                return Err(RepositoryError::database("forced asset database failure"));
            }
            self.assets.lock().unwrap().extend_from_slice(assets);
            Ok(())
        }

        async fn find_by_id(&self, asset_id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .unwrap()
                .iter()
                .find(|asset| asset.id == *asset_id)
                .cloned())
        }

        async fn list_by_source_task(
            &self,
            task_id: &TaskId,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .unwrap()
                .iter()
                .filter(|asset| asset.source_task_id.as_ref() == Some(task_id))
                .cloned()
                .collect())
        }

        async fn list_recent(
            &self,
            _project_id: &str,
            _limit: u32,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(self.assets.lock().unwrap().clone())
        }
    }

    #[derive(Clone)]
    struct FixedClock(DateTime<Utc>);

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    fn task_id() -> TaskId {
        TaskId::parse("tsk_test-task").unwrap()
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_pixel(width, height, Rgb([10, 20, 30]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("png should encode");
        bytes.into_inner()
    }

    fn image(bytes: Vec<u8>) -> CollectedImage {
        CollectedImage {
            output_id: "generated_image".to_owned(),
            node_id: "9".to_owned(),
            original_filename: "ComfyUI_00001.jpg".to_owned(),
            bytes,
            content_type: Some("image/png".to_owned()),
            position: 0,
            subfolder: String::new(),
            folder_type: "output".to_owned(),
        }
    }

    fn service(root: &Path, repository: FakeAssetRepository) -> AssetImportService {
        AssetImportService::new(
            Arc::new(FakeProjectRepository {
                root: root.to_path_buf(),
            }),
            Arc::new(FileSystemAssetStore::new()),
            Arc::new(repository),
            Arc::new(FixedClock(
                Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap(),
            )),
        )
    }

    use std::io::Cursor;

    #[tokio::test]
    async fn validates_actual_image_format_and_records_hash_dimensions_and_extension() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let assets = service(root.path(), repository.clone())
            .import("project-1", &task_id(), &[image(png_bytes(2, 3))])
            .await
            .expect("image import should succeed");

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].mime_type, "image/png");
        assert_eq!((assets[0].width, assets[0].height), (2, 3));
        assert_eq!(
            assets[0].file_size,
            std::fs::metadata(&assets[0].storage_path).unwrap().len()
        );
        assert!(assets[0].storage_path.ends_with(".png"));
        assert_eq!(repository.assets.lock().unwrap().len(), 1);
        assert!(Path::new(&assets[0].storage_path).is_file());
    }

    #[tokio::test]
    async fn invalid_image_is_rejected_before_any_asset_file_or_row() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let error = service(root.path(), repository.clone())
            .import(
                "project-1",
                &task_id(),
                &[image(b"<html>not an image</html>".to_vec())],
            )
            .await
            .expect_err("invalid image should fail");
        assert!(matches!(error, AssetImportError::OutputImportFailed { .. }));
        assert!(repository.assets.lock().unwrap().is_empty());
        assert!(!root.path().join("assets/generated/image").exists());
    }

    #[tokio::test]
    async fn database_failure_compensates_new_files_from_this_run() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository {
            assets: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        };
        let error = service(root.path(), repository)
            .import(
                "project-1",
                &task_id(),
                &[image(png_bytes(1, 1)), image(png_bytes(1, 1))],
            )
            .await
            .expect_err("database failure should be returned");
        assert!(matches!(error, AssetImportError::AssetPersistence { .. }));
        let directory = root.path().join("assets/generated/image");
        assert!(directory.is_dir());
        assert_eq!(
            std::fs::read_dir(directory)
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            0
        );
    }
}
