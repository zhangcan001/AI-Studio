use super::*;
use ai_studio_lib::application::{
    asset_query_service::*,
    media_probe::{AudioMetadata, MediaProbe, MediaProbeOutcome, VideoMetadata},
    ports::{AssetReadInspection, AssetReadStream, AssetStore, AssetStoreError, StoredAssetFile},
};
use ai_studio_lib::domain::AssetId;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    io::Write,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Probe(MediaProbeOutcome);
#[async_trait]
impl MediaProbe for Probe {
    async fn probe_video(&self, _: &Path) -> VideoMetadata {
        VideoMetadata::default()
    }
    async fn probe_audio(&self, _: &Path) -> AudioMetadata {
        AudioMetadata::default()
    }
    async fn generate_video_poster(&self, _: &Path) -> Option<Vec<u8>> {
        panic!("no poster generation")
    }
    async fn inspect_preview(&self, _: &Path, _: bool) -> MediaProbeOutcome {
        self.0
    }
}
#[derive(Default)]
struct ObservedStore {
    unreadable: bool,
    vanish_on_open: bool,
    streams: Arc<AtomicUsize>,
    chunks: Arc<AtomicUsize>,
    max_chunk: Arc<AtomicUsize>,
}
struct ObservedStream {
    inner: Box<dyn AssetReadStream>,
    chunks: Arc<AtomicUsize>,
    max_chunk: Arc<AtomicUsize>,
}
#[async_trait]
impl AssetReadStream for ObservedStream {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, AssetStoreError> {
        let chunk = self.inner.next_chunk().await?;
        if let Some(c) = &chunk {
            self.chunks.fetch_add(1, Ordering::SeqCst);
            self.max_chunk.fetch_max(c.len(), Ordering::SeqCst);
        }
        Ok(chunk)
    }
}
#[async_trait]
impl AssetStore for ObservedStore {
    async fn write_image(
        &self,
        _: &Path,
        _: &AssetId,
        _: &str,
        _: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        panic!("verify must not write")
    }
    async fn delete(&self, _: &Path) -> Result<(), AssetStoreError> {
        panic!("verify must not delete")
    }
    async fn read(&self, _: &Path, _: &Path) -> Result<Vec<u8>, AssetStoreError> {
        panic!("verify must stream, not read entire media")
    }
    async fn inspect_read(&self, root: &Path, path: &Path) -> AssetReadInspection {
        if self.unreadable {
            AssetReadInspection::Unreadable
        } else {
            FileSystemAssetStore.inspect_read(root, path).await
        }
    }
    async fn open_read_stream(
        &self,
        root: &Path,
        path: &Path,
    ) -> Result<Box<dyn AssetReadStream>, AssetStoreError> {
        self.streams.fetch_add(1, Ordering::SeqCst);
        if self.vanish_on_open {
            std::fs::remove_file(path).unwrap();
            return Err(AssetStoreError::Read("opaque fixture failure".into()));
        }
        let inner = FileSystemAssetStore.open_read_stream(root, path).await?;
        Ok(Box::new(ObservedStream {
            inner,
            chunks: self.chunks.clone(),
            max_chunk: self.max_chunk.clone(),
        }))
    }
}
fn query(f: &Fixture, store: Arc<dyn AssetStore>, outcome: MediaProbeOutcome) -> AssetQueryService {
    AssetQueryService::new(
        Arc::new(SqliteAssetRepository::new(f.pool.clone())),
        store,
        Arc::new(SqliteProjectRepository::new(f.pool.clone())),
    )
    .with_media_probe(Arc::new(Probe(outcome)))
}
async fn bytes(f: &Fixture, id: &str, media: &str, data: &[u8]) -> std::path::PathBuf {
    f.asset(id, "prj_default", media).await;
    let path = f._dir.path().join("prj_default").join(format!("{id}.png"));
    std::fs::write(&path, data).unwrap();
    sqlx::query("UPDATE assets SET sha256=?,file_size=? WHERE id=?")
        .bind(format!("{:x}", Sha256::digest(data)))
        .bind(data.len() as i64)
        .bind(id)
        .execute(&f.pool)
        .await
        .unwrap();
    path
}
fn png() -> Vec<u8> {
    let image = image::DynamicImage::new_rgb8(2, 2);
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}
async fn db_snapshot(f: &Fixture) -> Vec<String> {
    let mut values = Vec::new();
    for table in [
        "assets",
        "asset_versions",
        "asset_relations",
        "shot_reference_assets",
        "tasks",
        "production_batches",
        "production_batch_items",
        "generation_snapshots",
    ] {
        let cols = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(&f.pool)
            .await
            .unwrap();
        let columns = cols
            .iter()
            .map(|r| format!("\"{}\"", r.get::<String, _>("name")))
            .collect::<Vec<_>>()
            .join(",");
        values.push(
            sqlx::query_scalar::<_, String>(&format!(
                "SELECT json_group_array(json_array({columns})) FROM {table}"
            ))
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        );
    }
    values
}

#[tokio::test]
async fn healthy_image_and_corrupt_preview_keep_checksums_independent_and_all_data_immutable() {
    let f = Fixture::new().await;
    let data = png();
    let path = bytes(&f, "ast_integrity_good", "image", &data).await;
    let thumb = path.with_file_name("thumb.png");
    std::fs::write(&thumb, b"thumbnail untouched").unwrap();
    sqlx::query("UPDATE assets SET thumbnail_path=? WHERE id='ast_integrity_good'")
        .bind(thumb.to_str().unwrap())
        .execute(&f.pool)
        .await
        .unwrap();
    bytes(
        &f,
        "ast_integrity_bad",
        "image",
        b"not an image but hash matches",
    )
    .await;
    let before = db_snapshot(&f).await;
    let good = f
        .facade()
        .media_verify(
            "prj_default",
            &ResourceRef::Asset {
                id: "ast_integrity_good".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (
            good.boundary,
            good.existence,
            good.readability,
            good.checksum,
            good.preview
        ),
        (
            MediaBoundary::Safe,
            MediaExistence::Present,
            MediaReadability::Readable,
            MediaChecksum::Match,
            MediaPreview::Pass
        )
    );
    let bad = f
        .asset_detail
        .verify_media("prj_default", "ast_integrity_bad")
        .await
        .unwrap();
    assert_eq!(
        (bad.checksum, bad.preview),
        (MediaChecksum::Match, MediaPreview::Fail)
    );
    assert_eq!(db_snapshot(&f).await, before);
    assert_eq!(std::fs::read(path).unwrap(), data);
    assert_eq!(std::fs::read(thumb).unwrap(), b"thumbnail untouched");
    let json = serde_json::to_string(&good).unwrap();
    assert!(!json.contains("Path"));
    assert!(!json.contains(f._dir.path().to_str().unwrap()));
    assert!(!json.contains("sha256"));
}
#[tokio::test]
async fn missing_unreadable_mismatch_and_invalid_expected_are_distinct() {
    let f = Fixture::new().await;
    let path = bytes(&f, "ast_integrity_states", "image", &png()).await;
    std::fs::remove_file(&path).unwrap();
    let missing = f
        .asset_detail
        .verify_media("prj_default", "ast_integrity_states")
        .await
        .unwrap();
    assert_eq!(
        (
            missing.boundary,
            missing.existence,
            missing.readability,
            missing.checksum,
            missing.preview
        ),
        (
            MediaBoundary::Safe,
            MediaExistence::Missing,
            MediaReadability::NotChecked,
            MediaChecksum::NotChecked,
            MediaPreview::NotChecked
        )
    );
    std::fs::write(&path, png()).unwrap();
    let store = Arc::new(ObservedStore {
        unreadable: true,
        ..Default::default()
    });
    let unreadable = query(&f, store.clone(), MediaProbeOutcome::Decodable)
        .verify_media("prj_default", "ast_integrity_states")
        .await
        .unwrap();
    assert_eq!(
        (
            unreadable.existence,
            unreadable.readability,
            unreadable.checksum
        ),
        (
            MediaExistence::Present,
            MediaReadability::Unreadable,
            MediaChecksum::NotChecked
        )
    );
    assert_eq!(store.streams.load(Ordering::SeqCst), 0);
    let expected: String =
        sqlx::query_scalar("SELECT sha256 FROM assets WHERE id='ast_integrity_states'")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let mut new_png = png();
    new_png.extend_from_slice(b"changed trailing bytes");
    std::fs::write(&path, new_png).unwrap();
    let mismatch = f
        .asset_detail
        .verify_media("prj_default", "ast_integrity_states")
        .await
        .unwrap();
    assert_eq!(
        (mismatch.checksum, mismatch.preview),
        (MediaChecksum::Mismatch, MediaPreview::Pass)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT sha256 FROM assets WHERE id='ast_integrity_states'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        expected
    );
    sqlx::query("UPDATE assets SET sha256='invalid' WHERE id='ast_integrity_states'")
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        f.asset_detail
            .verify_media("prj_default", "ast_integrity_states")
            .await
            .unwrap()
            .checksum,
        MediaChecksum::InvalidExpected
    );
}
#[tokio::test]
async fn typed_video_audio_probes_do_not_infer_failure_from_unknown_metadata() {
    let f = Fixture::new().await;
    for media in ["video", "audio"] {
        let id = format!("ast_integrity_{media}");
        bytes(&f, &id, media, b"owned fake media bytes").await;
        for (outcome, expected) in [
            (MediaProbeOutcome::Decodable, MediaPreview::Pass),
            (MediaProbeOutcome::DecodeFailed, MediaPreview::Fail),
            (
                MediaProbeOutcome::ProbeUnavailable,
                MediaPreview::CheckUnavailable,
            ),
        ] {
            let r = query(&f, Arc::new(ObservedStore::default()), outcome)
                .verify_media("prj_default", &id)
                .await
                .unwrap();
            assert_eq!(r.preview, expected);
            assert_eq!(r.checksum, MediaChecksum::Match);
        }
    }
}
#[tokio::test]
async fn integrity_rejects_foreign_project_traversal_outside_nonregular_and_nonassets_without_streams(
) {
    let f = Fixture::new().await;
    bytes(&f, "ast_integrity_scope", "image", &png()).await;
    let store = Arc::new(ObservedStore::default());
    let q = query(&f, store.clone(), MediaProbeOutcome::Decodable);
    assert!(q
        .verify_media(
            "prj_11111111-1111-4111-8111-111111111111",
            "ast_integrity_scope"
        )
        .await
        .is_err());
    for resource in [
        ResourceRef::Prompt {
            id: "prompt".into(),
        },
        ResourceRef::Profile {
            id: "profile".into(),
        },
        ResourceRef::ReferenceSet { id: "set".into() },
    ] {
        assert!(f
            .facade()
            .media_verify("prj_default", &resource)
            .await
            .is_err());
    }
    let outside = f._dir.path().join("outside.png");
    std::fs::write(&outside, png()).unwrap();
    for path in [
        "../outside.png".to_owned(),
        outside.to_string_lossy().into_owned(),
        f._dir
            .path()
            .join("prj_default")
            .to_string_lossy()
            .into_owned(),
    ] {
        sqlx::query("UPDATE assets SET storage_path=? WHERE id='ast_integrity_scope'")
            .bind(path)
            .execute(&f.pool)
            .await
            .unwrap();
        let r = q
            .verify_media("prj_default", "ast_integrity_scope")
            .await
            .unwrap();
        assert_eq!(r.boundary, MediaBoundary::Rejected);
        assert_eq!(r.existence, MediaExistence::NotChecked);
        assert_eq!(r.checksum, MediaChecksum::NotChecked);
    }
    assert_eq!(store.streams.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn large_video_hashes_128_mib_in_one_mib_chunks_without_buffered_read_or_writes() {
    let f = Fixture::new().await;
    let path = bytes(&f, "ast_integrity_large", "video", b"").await;
    let chunk = vec![7_u8; 1024 * 1024];
    let mut file = std::fs::File::create(&path).unwrap();
    let mut hasher = Sha256::new();
    for _ in 0..128 {
        file.write_all(&chunk).unwrap();
        hasher.update(&chunk);
    }
    drop(file);
    sqlx::query("UPDATE assets SET sha256=?,file_size=? WHERE id='ast_integrity_large'")
        .bind(format!("{:x}", hasher.finalize()))
        .bind(128_i64 * 1024 * 1024)
        .execute(&f.pool)
        .await
        .unwrap();
    let before = db_snapshot(&f).await;
    let store = Arc::new(ObservedStore::default());
    let r = query(&f, store.clone(), MediaProbeOutcome::ProbeUnavailable)
        .verify_media("prj_default", "ast_integrity_large")
        .await
        .unwrap();
    assert_eq!(r.checksum, MediaChecksum::Match);
    assert_eq!(store.streams.load(Ordering::SeqCst), 1);
    assert_eq!(store.chunks.load(Ordering::SeqCst), 128);
    assert_eq!(store.max_chunk.load(Ordering::SeqCst), 1024 * 1024);
    assert_eq!(db_snapshot(&f).await, before);
    assert_eq!(std::fs::metadata(path).unwrap().len(), 128 * 1024 * 1024);
}
#[tokio::test]
async fn oversized_image_preview_is_unavailable_not_corrupt() {
    let f = Fixture::new().await;
    let data = vec![1_u8; 33 * 1024 * 1024];
    bytes(&f, "ast_integrity_bigimage", "image", &data).await;
    let r = f
        .asset_detail
        .verify_media("prj_default", "ast_integrity_bigimage")
        .await
        .unwrap();
    assert_eq!(
        (r.checksum, r.preview),
        (MediaChecksum::Match, MediaPreview::CheckUnavailable)
    );
}

#[tokio::test]
async fn disappearance_between_inspection_and_open_uses_typed_reinspection_not_error_text() {
    let f = Fixture::new().await;
    bytes(&f, "ast_integrity_race", "image", &png()).await;
    let store = Arc::new(ObservedStore {
        vanish_on_open: true,
        ..Default::default()
    });
    let r = query(&f, store, MediaProbeOutcome::Decodable)
        .verify_media("prj_default", "ast_integrity_race")
        .await
        .unwrap();
    assert_eq!(
        (
            r.boundary,
            r.existence,
            r.readability,
            r.checksum,
            r.preview
        ),
        (
            MediaBoundary::Safe,
            MediaExistence::Missing,
            MediaReadability::NotChecked,
            MediaChecksum::NotChecked,
            MediaPreview::NotChecked
        )
    );
}
