use crate::application::ports::{AssetRepository, AssetStore, ProjectRepository};
use crate::domain::{validate_project_id, AssetType};
use std::{collections::BTreeMap, sync::Arc};

pub const MAX_MEDIA_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

pub struct MediaProtocolService {
    asset_repository: Arc<dyn AssetRepository>,
    asset_store: Arc<dyn AssetStore>,
    project_repository: Arc<dyn ProjectRepository>,
}

impl MediaProtocolService {
    pub fn new(
        asset_repository: Arc<dyn AssetRepository>,
        asset_store: Arc<dyn AssetStore>,
        project_repository: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self {
            asset_repository,
            asset_store,
            project_repository,
        }
    }

    #[allow(dead_code)]
    pub async fn handle(
        &self,
        method: &str,
        project_id: &str,
        asset_id: &str,
        range: Option<&str>,
    ) -> MediaResponse {
        self.handle_path(None, method, project_id, asset_id, range)
            .await
    }

    pub async fn handle_path(
        &self,
        path: Option<&str>,
        method: &str,
        project_id: &str,
        asset_id: &str,
        range: Option<&str>,
    ) -> MediaResponse {
        if validate_project_id(project_id).is_err() {
            return MediaResponse::not_found();
        }
        let asset_id = match crate::domain::AssetId::parse(asset_id.to_owned()) {
            Ok(asset_id) => asset_id,
            Err(_) => return MediaResponse::not_found(),
        };
        let Some(asset) = (match self.asset_repository.find_by_id(&asset_id).await {
            Ok(asset) => asset,
            Err(error) => return MediaResponse::server_error(error.to_string()),
        }) else {
            return MediaResponse::not_found();
        };
        let expected_type = match path {
            Some("/video") => Some(AssetType::Video),
            Some("/audio") => Some(AssetType::Audio),
            Some(_) => return MediaResponse::not_found(),
            None => None,
        };
        if asset.project_id != project_id
            || !matches!(asset.asset_type, AssetType::Video | AssetType::Audio)
            || expected_type.is_some_and(|expected| expected != asset.asset_type)
        {
            return MediaResponse::not_found();
        }
        let Some(project_root) = (match self.project_repository.get_storage_root(project_id).await {
            Ok(root) => root,
            Err(error) => return MediaResponse::server_error(error.to_string()),
        }) else {
            return MediaResponse::not_found();
        };
        if !std::path::Path::new(&asset.storage_path).starts_with(&project_root) {
            tracing::warn!(project_id, asset_id = %asset.id, "rejected media path outside project root");
            return MediaResponse::not_found();
        }

        let total = asset.file_size;
        let requested = match range {
            Some(value) => match parse_range(value, total) {
                Ok(range) => range,
                Err(_) => return MediaResponse::range_not_satisfiable(total),
            },
            None => ByteRange {
                start: 0,
                end: total
                    .saturating_sub(1)
                    .min(MAX_MEDIA_RESPONSE_BYTES.saturating_sub(1)),
                partial: total > MAX_MEDIA_RESPONSE_BYTES,
            },
        };
        let length = requested
            .end
            .saturating_sub(requested.start)
            .saturating_add(1);
        if length > MAX_MEDIA_RESPONSE_BYTES {
            return MediaResponse::range_not_satisfiable(total);
        }

        let mut response = MediaResponse {
            status: if requested.partial { 206 } else { 200 },
            headers: BTreeMap::from([
                ("Accept-Ranges".to_owned(), "bytes".to_owned()),
                ("Content-Type".to_owned(), asset.mime_type),
                ("Content-Length".to_owned(), length.to_string()),
            ]),
            body: Vec::new(),
        };
        if requested.partial {
            response.headers.insert(
                "Content-Range".to_owned(),
                format!("bytes {}-{}/{}", requested.start, requested.end, total),
            );
        }
        if method.eq_ignore_ascii_case("HEAD") {
            response.status = 200;
            response
                .headers
                .insert("Content-Length".to_owned(), total.to_string());
            response.headers.remove("Content-Range");
            return response;
        }
        if !method.eq_ignore_ascii_case("GET") {
            response.status = 405;
            response.body.clear();
            return response;
        }
        response.body = match self
            .asset_store
            .read_range(
                std::path::Path::new(&asset.storage_path),
                requested.start,
                length,
            )
            .await
        {
            Ok(bytes) => bytes,
            Err(error) => return MediaResponse::server_error(error.to_string()),
        };
        response
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl MediaResponse {
    fn not_found() -> Self {
        Self {
            status: 404,
            headers: BTreeMap::new(),
            body: Vec::new(),
        }
    }

    fn range_not_satisfiable(total: u64) -> Self {
        Self {
            status: 416,
            headers: BTreeMap::from([
                ("Accept-Ranges".to_owned(), "bytes".to_owned()),
                ("Content-Range".to_owned(), format!("bytes */{total}")),
            ]),
            body: Vec::new(),
        }
    }

    fn server_error(_error: impl Into<String>) -> Self {
        tracing::error!("asset media protocol failed");
        Self {
            status: 500,
            headers: BTreeMap::new(),
            body: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ByteRange {
    start: u64,
    end: u64,
    partial: bool,
}

fn parse_range(value: &str, total: u64) -> Result<ByteRange, ()> {
    let value = value.strip_prefix("bytes=").ok_or(())?;
    if value.contains(',') || total == 0 {
        return Err(());
    }
    let (start, end) = value.split_once('-').ok_or(())?;
    let (start, end) = if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        (total.saturating_sub(suffix), total - 1)
    } else {
        let start = start.parse::<u64>().map_err(|_| ())?;
        if start >= total {
            return Err(());
        }
        let end = if end.is_empty() {
            total - 1
        } else {
            end.parse::<u64>().map_err(|_| ())?.min(total - 1)
        };
        if end < start {
            return Err(());
        }
        (start, end)
    };
    let end = end.min(start.saturating_add(MAX_MEDIA_RESPONSE_BYTES - 1));
    Ok(ByteRange {
        start,
        end,
        partial: true,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_range, MAX_MEDIA_RESPONSE_BYTES};
    use crate::application::ports::{
        AssetRepository, AssetStore, AssetStoreError, ProjectRecord, ProjectRepository,
        RepositoryError, StoredAssetFile,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn accepts_single_ranges_and_caps_response_window() {
        assert_eq!(
            parse_range("bytes=10-20", 100).unwrap(),
            super::ByteRange {
                start: 10,
                end: 20,
                partial: true
            }
        );
        let range = parse_range("bytes=0-", 32 * 1024 * 1024).unwrap();
        assert_eq!(range.end + 1, MAX_MEDIA_RESPONSE_BYTES);
    }

    #[test]
    fn rejects_multi_ranges_and_invalid_bounds() {
        assert!(parse_range("bytes=0-1,3-4", 10).is_err());
        assert!(parse_range("bytes=10-10", 10).is_err());
        assert!(parse_range("bytes=0-0", 0).is_err());
    }

    #[derive(Clone)]
    struct OneAssetRepository {
        asset: Asset,
    }

    #[async_trait]
    impl AssetRepository for OneAssetRepository {
        async fn insert_many(&self, _assets: &[Asset]) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn find_by_id(&self, asset_id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
            Ok((self.asset.id == *asset_id).then(|| self.asset.clone()))
        }

        async fn list_by_source_task(
            &self,
            _task_id: &TaskId,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn list_recent(
            &self,
            _project_id: &str,
            _limit: u32,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(vec![self.asset.clone()])
        }
    }

    #[derive(Clone)]
    struct OneProjectRepository {
        root: PathBuf,
    }

    #[async_trait]
    impl ProjectRepository for OneProjectRepository {
        async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn find_by_id(
            &self,
            project_id: &str,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(Some(ProjectRecord {
                id: project_id.to_owned(),
                name: "Project".to_owned(),
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
            _project_id: &str,
            _name: &str,
            _description: Option<&str>,
            _updated_at: chrono::DateTime<Utc>,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(None)
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
            created_at: chrono::DateTime<Utc>,
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

    #[derive(Clone, Copy)]
    struct FileRangeStore;

    #[async_trait]
    impl AssetStore for FileRangeStore {
        async fn write_image(
            &self,
            _project_root: &Path,
            _asset_id: &AssetId,
            _extension: &str,
            _bytes: &[u8],
        ) -> Result<StoredAssetFile, AssetStoreError> {
            Err(AssetStoreError::Write("not used".to_owned()))
        }

        async fn delete(&self, _path: &Path) -> Result<(), AssetStoreError> {
            Ok(())
        }

        async fn read(&self, path: &Path) -> Result<Vec<u8>, AssetStoreError> {
            std::fs::read(path).map_err(|error| AssetStoreError::Read(error.to_string()))
        }

        async fn read_range(
            &self,
            path: &Path,
            offset: u64,
            length: u64,
        ) -> Result<Vec<u8>, AssetStoreError> {
            let bytes =
                std::fs::read(path).map_err(|error| AssetStoreError::Read(error.to_string()))?;
            let start =
                usize::try_from(offset).map_err(|_| AssetStoreError::Read("offset".to_owned()))?;
            let end = start.saturating_add(usize::try_from(length).unwrap_or(usize::MAX));
            Ok(bytes
                .get(start..end.min(bytes.len()))
                .unwrap_or_default()
                .to_vec())
        }
    }

    #[tokio::test]
    async fn serves_bounded_ranges_and_rejects_cross_project_assets() {
        let root = tempdir().unwrap();
        let path = root.path().join("video.mp4");
        let bytes: Vec<u8> = (0..64).collect();
        std::fs::write(&path, &bytes).unwrap();
        let task_id = TaskId::parse("tsk_media").unwrap();
        let asset = Asset::new_generated_video(
            AssetId::parse("ast_media").unwrap(),
            "prj_default",
            "Video",
            "video.mp4",
            path.to_string_lossy(),
            "a".repeat(64),
            "video/mp4",
            None,
            None,
            None,
            bytes.len() as u64,
            task_id,
            serde_json::json!({}),
            Utc::now(),
        )
        .unwrap();
        let service = super::MediaProtocolService::new(
            Arc::new(OneAssetRepository { asset }),
            Arc::new(FileRangeStore),
            Arc::new(OneProjectRepository {
                root: root.path().to_path_buf(),
            }),
        );
        let response = service
            .handle("GET", "prj_default", "ast_media", Some("bytes=4-9"))
            .await;
        assert_eq!(response.status, 206);
        assert_eq!(response.body, bytes[4..10]);
        assert_eq!(
            response.headers.get("Content-Range").unwrap(),
            "bytes 4-9/64"
        );
        let head = service
            .handle("HEAD", "prj_default", "ast_media", None)
            .await;
        assert_eq!(head.status, 200);
        assert!(head.body.is_empty());
        assert_eq!(head.headers.get("Content-Length").unwrap(), "64");
        assert_eq!(
            service
                .handle(
                    "GET",
                    "prj_550e8400-e29b-41d4-a716-446655440000",
                    "ast_media",
                    None
                )
                .await
                .status,
            404
        );
        assert_eq!(
            service
                .handle("GET", "project-unsafe", "ast_media", None)
                .await
                .status,
            404
        );
    }

    #[tokio::test]
    async fn serves_source_video_and_source_audio_only_on_their_matching_routes() {
        let root = tempdir().unwrap();
        let video_path = root.path().join("source.mp4");
        let video_bytes = vec![1, 2, 3, 4, 5];
        std::fs::write(&video_path, &video_bytes).unwrap();
        let video = Asset::new_source_video(
            AssetId::parse("ast_source_video").unwrap(),
            "prj_default",
            "source.mp4",
            "source.mp4",
            video_path.to_string_lossy(),
            "a".repeat(64),
            "video/mp4",
            None,
            None,
            None,
            video_bytes.len() as u64,
            serde_json::json!({}),
            Utc::now(),
        )
        .unwrap();
        let video_service = super::MediaProtocolService::new(
            Arc::new(OneAssetRepository { asset: video }),
            Arc::new(FileRangeStore),
            Arc::new(OneProjectRepository {
                root: root.path().to_path_buf(),
            }),
        );
        assert_eq!(
            video_service
                .handle("GET", "prj_default", "ast_source_video", None)
                .await
                .status,
            200
        );
        assert_eq!(
            video_service
                .handle("GET", "prj_default", "ast_source_video", Some("bytes=0-1"))
                .await
                .status,
            206
        );

        let audio_path = root.path().join("source.wav");
        let audio_bytes = vec![7, 8, 9, 10];
        std::fs::write(&audio_path, &audio_bytes).unwrap();
        let audio = Asset::new_source_audio(
            AssetId::parse("ast_source_audio").unwrap(),
            "prj_default",
            "source.wav",
            "source.wav",
            audio_path.to_string_lossy(),
            "b".repeat(64),
            "audio/wav",
            Some(250),
            audio_bytes.len() as u64,
            serde_json::json!({}),
            Utc::now(),
        )
        .unwrap();
        let audio_service = super::MediaProtocolService::new(
            Arc::new(OneAssetRepository { asset: audio }),
            Arc::new(FileRangeStore),
            Arc::new(OneProjectRepository {
                root: root.path().to_path_buf(),
            }),
        );
        assert_eq!(
            audio_service
                .handle_path(
                    Some("/audio"),
                    "HEAD",
                    "prj_default",
                    "ast_source_audio",
                    None,
                )
                .await
                .status,
            200
        );
        assert_eq!(
            audio_service
                .handle_path(
                    Some("/audio"),
                    "GET",
                    "prj_default",
                    "ast_source_audio",
                    Some("bytes=1-2"),
                )
                .await
                .body,
            audio_bytes[1..3]
        );
        assert_eq!(
            audio_service
                .handle_path(
                    Some("/video"),
                    "GET",
                    "prj_default",
                    "ast_source_audio",
                    None,
                )
                .await
                .status,
            404
        );
        assert_eq!(
            audio_service
                .handle_path(Some("/audio"), "GET", "prj_default", "ast_missing", None,)
                .await
                .status,
            404
        );
    }
}
