use crate::application::image_inspection::inspect_bytes;
use crate::application::ports::{
    AssetRepository, AssetStore, Clock, ProjectRepository, RepositoryError,
};
use crate::domain::{Asset, AssetId};
use serde_json::json;
use std::{error::Error, fmt, sync::Arc};

pub const MAX_SOURCE_IMAGE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceAssetImportError {
    ProjectStorageMissing { project_id: String },
    SourceImageTooLarge { max_bytes: u64, actual_bytes: u64 },
    InvalidSourceImage { message: String },
    AssetPersistence { message: String },
}

impl SourceAssetImportError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProjectStorageMissing { .. } | Self::AssetPersistence { .. } => {
                "ASSET_PERSISTENCE_ERROR"
            }
            Self::SourceImageTooLarge { .. } => "SOURCE_IMAGE_TOO_LARGE",
            Self::InvalidSourceImage { .. } => "INVALID_SOURCE_IMAGE",
        }
    }
}

impl fmt::Display for SourceAssetImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectStorageMissing { project_id } => write!(
                formatter,
                "ASSET_PERSISTENCE_ERROR: project {project_id} has no storage root"
            ),
            Self::SourceImageTooLarge {
                max_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "SOURCE_IMAGE_TOO_LARGE: image is {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::InvalidSourceImage { message } => {
                write!(formatter, "INVALID_SOURCE_IMAGE: {message}")
            }
            Self::AssetPersistence { message } => {
                write!(formatter, "ASSET_PERSISTENCE_ERROR: {message}")
            }
        }
    }
}

impl Error for SourceAssetImportError {}

pub struct SourceAssetImportService {
    project_repository: Arc<dyn ProjectRepository>,
    asset_store: Arc<dyn AssetStore>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl SourceAssetImportService {
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

    pub async fn import_bytes(
        &self,
        project_id: &str,
        original_name: &str,
        bytes: &[u8],
    ) -> Result<Asset, SourceAssetImportError> {
        let original_name = safe_file_name(original_name)?;
        let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_bytes > MAX_SOURCE_IMAGE_BYTES {
            return Err(SourceAssetImportError::SourceImageTooLarge {
                max_bytes: MAX_SOURCE_IMAGE_BYTES,
                actual_bytes,
            });
        }
        let inspected =
            inspect_bytes(bytes).map_err(|error| SourceAssetImportError::InvalidSourceImage {
                message: error.to_string(),
            })?;
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| SourceAssetImportError::ProjectStorageMissing {
                project_id: project_id.to_owned(),
            })?;

        let asset_id = AssetId::new();
        let stored = self
            .asset_store
            .write_source_image(&project_root, &asset_id, inspected.extension, bytes)
            .await
            .map_err(|error| SourceAssetImportError::AssetPersistence {
                message: error.to_string(),
            })?;
        let created_at = self.clock.now();
        let asset = match Asset::new_source_image(
            asset_id,
            project_id,
            original_name.clone(),
            original_name,
            stored.path.display().to_string(),
            inspected.sha256,
            inspected.mime_type,
            inspected.width,
            inspected.height,
            bytes.len() as u64,
            json!({"source": "native_import"}),
            created_at,
        ) {
            Ok(asset) => asset,
            Err(error) => {
                let _ = self.asset_store.delete(&stored.path).await;
                return Err(SourceAssetImportError::InvalidSourceImage {
                    message: error.to_string(),
                });
            }
        };

        if let Err(error) = self
            .asset_repository
            .insert_many(std::slice::from_ref(&asset))
            .await
        {
            let _ = self.asset_store.delete(&stored.path).await;
            return Err(SourceAssetImportError::AssetPersistence {
                message: error.to_string(),
            });
        }

        Ok(asset)
    }
}

fn safe_file_name(value: &str) -> Result<String, SourceAssetImportError> {
    let candidate = std::path::Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .trim();
    if candidate.is_empty() || candidate == "." || candidate == ".." {
        return Err(SourceAssetImportError::InvalidSourceImage {
            message: "original file name is required".to_owned(),
        });
    }
    Ok(candidate.to_owned())
}

fn repository_error(error: RepositoryError) -> SourceAssetImportError {
    SourceAssetImportError::AssetPersistence {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceAssetImportError, SourceAssetImportService, MAX_SOURCE_IMAGE_BYTES};
    use crate::application::ports::{
        AssetRepository, Clock, ProjectRecord, ProjectRepository, RepositoryError,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use std::io::Cursor;
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
                return Err(RepositoryError::database(
                    "forced source asset database failure",
                ));
            }
            self.assets.lock().unwrap().extend_from_slice(assets);
            Ok(())
        }
        async fn find_by_id(&self, id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .unwrap()
                .iter()
                .find(|asset| asset.id == *id)
                .cloned())
        }
        async fn list_by_source_task(&self, id: &TaskId) -> Result<Vec<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .unwrap()
                .iter()
                .filter(|asset| asset.source_task_id.as_ref() == Some(id))
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
    struct FixedClock;
    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            Utc::now()
        }
    }

    fn png() -> Vec<u8> {
        let image = RgbImage::from_pixel(2, 3, Rgb([10, 20, 30]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn service(root: &Path, repository: FakeAssetRepository) -> SourceAssetImportService {
        SourceAssetImportService::new(
            Arc::new(FakeProjectRepository {
                root: root.to_path_buf(),
            }),
            Arc::new(FileSystemAssetStore::new()),
            Arc::new(repository),
            Arc::new(FixedClock),
        )
    }

    #[tokio::test]
    async fn imports_source_image_without_persisting_source_task_or_absolute_input_path() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let asset = service(root.path(), repository.clone())
            .import_bytes("project-1", r"C:\\Users\\me\\photo.png", &png())
            .await
            .unwrap();

        assert_eq!(asset.category, "source_image");
        assert!(asset.source_task_id.is_none());
        assert!(
            asset.storage_path.ends_with("assets/source/image")
                || asset.storage_path.contains("assets\\source\\image")
        );
        assert_eq!(asset.original_name, "photo.png");
        assert_eq!(repository.assets.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_invalid_image_and_oversized_input() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let invalid = service(root.path(), repository.clone())
            .import_bytes("project-1", "bad.png", b"not-image")
            .await
            .unwrap_err();
        assert!(matches!(
            invalid,
            SourceAssetImportError::InvalidSourceImage { .. }
        ));

        let oversized = service(root.path(), repository)
            .import_bytes(
                "project-1",
                "big.png",
                &vec![0; (MAX_SOURCE_IMAGE_BYTES + 1) as usize],
            )
            .await
            .unwrap_err();
        assert!(matches!(
            oversized,
            SourceAssetImportError::SourceImageTooLarge { .. }
        ));
    }

    #[tokio::test]
    async fn imports_jpeg_and_webp_from_actual_bytes() {
        let root = tempdir().unwrap();
        for (extension, format) in [("jpg", ImageFormat::Jpeg), ("webp", ImageFormat::WebP)] {
            let image = RgbImage::from_pixel(2, 2, Rgb([30, 40, 50]));
            let mut bytes = Cursor::new(Vec::new());
            DynamicImage::ImageRgb8(image)
                .write_to(&mut bytes, format)
                .unwrap();
            let asset = service(root.path(), FakeAssetRepository::default())
                .import_bytes(
                    "project-1",
                    &format!("reference.{extension}"),
                    &bytes.into_inner(),
                )
                .await
                .unwrap();
            assert_eq!(
                asset.mime_type,
                if extension == "jpg" {
                    "image/jpeg"
                } else {
                    "image/webp"
                }
            );
            assert!(asset.storage_path.ends_with(&format!(".{extension}")));
        }
    }

    #[tokio::test]
    async fn database_failure_removes_only_new_source_file() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository {
            assets: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        };
        let error = service(root.path(), repository)
            .import_bytes("project-1", "reference.png", &png())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SourceAssetImportError::AssetPersistence { .. }
        ));
        let directory = root.path().join("assets/source/image");
        assert!(directory.is_dir());
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 0);
    }
}
