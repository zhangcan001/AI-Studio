//! Copy explicitly selected videos to a new delivery directory. No Task/Queue writes.
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedVideoExport {
    pub exported: usize,
    pub failed: usize,
    pub directory: String,
}
/// All source identities and paths are validated by existing services before entry.
/// Never overwrite or remove any user file; a failure preserves successful copies.
pub fn copy_selected_videos(
    parent: &Path,
    sources: &[(i64, PathBuf)],
) -> io::Result<SelectedVideoExport> {
    if sources.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "没有已选用的视频",
        ));
    }
    let directory = parent.join(format!("AI-Studio-selected-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory)?;
    let mut exported = 0;
    for (index, (_, source)) in sources.iter().enumerate() {
        let ext = source
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !["mp4", "webm", "mov", "mkv", "avi"].contains(&ext.as_str()) {
            continue;
        }
        let result = (|| -> io::Result<()> {
            let mut input = fs::File::open(source)?;
            if !input.metadata()?.is_file() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "不是视频文件"));
            }
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(format!("{:04}.{}", index + 1, ext)))?;
            io::copy(&mut input, &mut output)?;
            output.sync_all()
        })();
        if result.is_ok() {
            exported += 1;
        }
    }
    Ok(SelectedVideoExport {
        exported,
        failed: sources.len() - exported,
        directory: directory.to_string_lossy().into_owned(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_video_delivery_preserves_bytes_order_and_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.mp4");
        let b = dir.path().join("b.webm");
        fs::write(&a, b"video-a").unwrap();
        fs::write(&b, b"video-b").unwrap();
        let result = copy_selected_videos(dir.path(), &[(3, a.clone()), (9, b)]).unwrap();
        assert_eq!(result.exported, 2);
        assert_eq!(result.failed, 0);
        assert_eq!(
            fs::read(Path::new(&result.directory).join("0001.mp4")).unwrap(),
            b"video-a"
        );
        assert_eq!(
            fs::read(Path::new(&result.directory).join("0002.webm")).unwrap(),
            b"video-b"
        );
        let next = copy_selected_videos(dir.path(), &[(3, a.clone())]).unwrap();
        assert_ne!(next.directory, result.directory);
        assert_eq!(fs::read(a).unwrap(), b"video-a");
    }
    #[test]
    fn selected_video_delivery_reports_partial_failure_without_overwrite_or_execution() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("missing.mp4");
        let other = dir.path().join("bad.exe");
        let result = copy_selected_videos(dir.path(), &[(0, bad), (1, other)]).unwrap();
        assert_eq!(result.exported, 0);
        assert_eq!(result.failed, 2);
        assert!(copy_selected_videos(dir.path(), &[]).is_err());
    }
}
