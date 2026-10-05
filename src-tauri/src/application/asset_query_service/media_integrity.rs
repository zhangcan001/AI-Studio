use super::*;
use crate::application::{media_probe::MediaProbeOutcome, ports::AssetReadInspection};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const MAX_IMAGE_PREVIEW_CHECK_BYTES: usize = 32 * 1024 * 1024;
pub const MEDIA_VERIFY_CHUNK_BYTES: usize = 1024 * 1024;

macro_rules! status {
    ($name:ident { $($value:ident),+ }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name { $($value),+ }
    };
}
status!(MediaBoundary {
    Safe,
    Rejected,
    NotChecked
});
status!(MediaExistence {
    Present,
    Missing,
    NotChecked
});
status!(MediaReadability {
    Readable,
    Unreadable,
    NotChecked
});
status!(MediaChecksum {
    Match,
    Mismatch,
    InvalidExpected,
    NotChecked
});
status!(MediaPreview {
    Pass,
    Fail,
    CheckUnavailable,
    NotApplicable,
    NotChecked
});

/// Transient observations only: no filesystem paths, hashes or persisted health.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaIntegrityReport {
    pub asset_id: String,
    pub asset_type: String,
    pub boundary: MediaBoundary,
    pub existence: MediaExistence,
    pub readability: MediaReadability,
    pub checksum: MediaChecksum,
    pub preview: MediaPreview,
    pub checked_at: DateTime<Utc>,
}

impl AssetQueryService {
    pub async fn verify_media(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<MediaIntegrityReport, AssetQueryError> {
        validate_project_id(project_id)?;
        let id = AssetId::parse(asset_id.to_owned())
            .map_err(|e| AssetQueryError::InvalidAssetId(e.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&id)
            .await?
            .filter(|a| a.project_id == project_id)
            .ok_or_else(|| AssetQueryError::NotFound(asset_id.into()))?;
        let mut report = MediaIntegrityReport {
            asset_id: asset_id.into(),
            asset_type: match asset.asset_type {
                AssetType::Image => "image",
                AssetType::Video => "video",
                AssetType::Audio => "audio",
            }
            .into(),
            boundary: MediaBoundary::Rejected,
            existence: MediaExistence::NotChecked,
            readability: MediaReadability::NotChecked,
            checksum: MediaChecksum::NotChecked,
            preview: MediaPreview::NotChecked,
            checked_at: Utc::now(),
        };
        let Ok(root) = self.project_root(project_id).await else {
            return Ok(report);
        };
        let canonical = match self
            .asset_store
            .inspect_read(&root, Path::new(&asset.storage_path))
            .await
        {
            AssetReadInspection::UnsafePath => return Ok(report),
            AssetReadInspection::Missing => {
                report.boundary = MediaBoundary::Safe;
                report.existence = MediaExistence::Missing;
                return Ok(report);
            }
            AssetReadInspection::Unreadable => {
                report.boundary = MediaBoundary::Safe;
                report.existence = MediaExistence::Present;
                report.readability = MediaReadability::Unreadable;
                return Ok(report);
            }
            AssetReadInspection::Present { canonical_path } => canonical_path,
        };
        report.boundary = MediaBoundary::Safe;
        report.existence = MediaExistence::Present;
        // The store enforces the same path policy again when opening the stream.
        let Ok(mut stream) = self.asset_store.open_read_stream(&root, &canonical).await else {
            return Ok(self.media_read_failure(&root, &canonical, report).await);
        };
        let mut hasher = Sha256::new();
        let mut image_bytes = Vec::new();
        let mut image_bounded = asset.asset_type == AssetType::Image;
        loop {
            match stream.next_chunk().await {
                Ok(Some(chunk)) if chunk.len() <= MEDIA_VERIFY_CHUNK_BYTES => {
                    hasher.update(&chunk);
                    if image_bounded {
                        if image_bytes.len().saturating_add(chunk.len())
                            <= MAX_IMAGE_PREVIEW_CHECK_BYTES
                        {
                            image_bytes.extend_from_slice(&chunk);
                        } else {
                            image_bounded = false;
                            image_bytes = Vec::new();
                        }
                    }
                }
                Ok(None) => break,
                _ => {
                    return Ok(self.media_read_failure(&root, &canonical, report).await);
                }
            }
        }
        report.readability = MediaReadability::Readable;
        report.checksum =
            if asset.sha256.len() != 64 || !asset.sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
                MediaChecksum::InvalidExpected
            } else if format!("{:x}", hasher.finalize()).eq_ignore_ascii_case(&asset.sha256) {
                MediaChecksum::Match
            } else {
                MediaChecksum::Mismatch
            };
        report.preview = if asset.asset_type == AssetType::Image {
            if !image_bounded {
                MediaPreview::CheckUnavailable
            } else {
                tokio::task::spawn_blocking(move || image_preview(image_bytes))
                    .await
                    .unwrap_or(MediaPreview::CheckUnavailable)
            }
        } else {
            match self.asset_store.inspect_read(&root, &canonical).await {
                AssetReadInspection::Present { canonical_path } => match self
                    .media_probe
                    .inspect_preview(&canonical_path, asset.asset_type == AssetType::Audio)
                    .await
                {
                    MediaProbeOutcome::Decodable => MediaPreview::Pass,
                    MediaProbeOutcome::DecodeFailed => MediaPreview::Fail,
                    MediaProbeOutcome::ProbeUnavailable => MediaPreview::CheckUnavailable,
                },
                _ => MediaPreview::NotChecked,
            }
        };
        report.checked_at = Utc::now();
        Ok(report)
    }
    // A file may disappear or cross the boundary after inspection. Reinspect
    // through typed facts, never classify a stream error by its message text.
    async fn media_read_failure(
        &self,
        root: &Path,
        path: &Path,
        mut report: MediaIntegrityReport,
    ) -> MediaIntegrityReport {
        report.checksum = MediaChecksum::NotChecked;
        report.preview = MediaPreview::NotChecked;
        match self.asset_store.inspect_read(root, path).await {
            AssetReadInspection::UnsafePath => {
                report.boundary = MediaBoundary::Rejected;
                report.existence = MediaExistence::NotChecked;
                report.readability = MediaReadability::NotChecked;
            }
            AssetReadInspection::Missing => {
                report.boundary = MediaBoundary::Safe;
                report.existence = MediaExistence::Missing;
                report.readability = MediaReadability::NotChecked;
            }
            _ => {
                report.boundary = MediaBoundary::Safe;
                report.existence = MediaExistence::Present;
                report.readability = MediaReadability::Unreadable;
            }
        }
        report
    }
}

fn image_preview(bytes: Vec<u8>) -> MediaPreview {
    let Ok(mut reader) = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()
    else {
        return MediaPreview::Fail;
    };
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_IMAGE_PREVIEW_CHECK_BYTES as u64);
    reader.limits(limits);
    match reader.decode() {
        Ok(_) => MediaPreview::Pass,
        Err(image::ImageError::Limits(_)) => MediaPreview::CheckUnavailable,
        Err(_) => MediaPreview::Fail,
    }
}
