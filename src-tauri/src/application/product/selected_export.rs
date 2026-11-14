//! Explicit selected-video delivery. No database, Task or Queue writes.
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};
#[derive(Debug)]
pub struct ExportSource {
    pub ordinal: i64,
    pub shot_id: String,
    pub shot_name: String,
    pub asset_id: String,
    pub path: Option<PathBuf>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEntry {
    pub ordinal: i64,
    pub shot_id: String,
    pub shot_name: String,
    pub asset_id: String,
    pub file_name: Option<String>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedVideoExport {
    pub exported: usize,
    pub failed: usize,
    pub directory: String,
    pub entries: Vec<ExportEntry>,
    pub manifest_error: Option<String>,
    pub open_error: Option<String>,
}
fn safe_name(value: &str) -> String {
    let name: String = value
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .take(60)
        .collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    if name.is_empty() {
        "镜头".into()
    } else {
        name.into()
    }
}
/// Validated sources only; create a new owned directory, never overwrite user files.
pub fn copy_selected_videos(
    parent: &Path,
    sources: &[ExportSource],
) -> io::Result<SelectedVideoExport> {
    if sources.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "没有已选用的视频",
        ));
    }
    let directory = parent.join(format!("AI-Studio-selected-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory)?;
    let mut entries = Vec::new();
    for source in sources {
        let mut entry = ExportEntry {
            ordinal: source.ordinal,
            shot_id: source.shot_id.clone(),
            shot_name: source.shot_name.clone(),
            asset_id: source.asset_id.clone(),
            file_name: None,
            error: source.error.clone(),
        };
        if entry.error.is_none() {
            let result = (|| -> io::Result<String> {
                let path = source
                    .path
                    .as_ref()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "source unavailable"))?;
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !["mp4", "webm", "mov", "mkv", "avi"].contains(&ext.as_str()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "unsupported video container",
                    ));
                }
                let file_name = format!(
                    "{:04}_{}.{}",
                    source.ordinal.saturating_add(1),
                    safe_name(&source.shot_name),
                    ext
                );
                let mut input = fs::File::open(path)?;
                if !input.metadata()?.is_file() {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
                }
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(directory.join(&file_name))?;
                io::copy(&mut input, &mut output)?;
                output.sync_all()?;
                Ok(file_name)
            })();
            match result{Ok(name)=>entry.file_name=Some(name),Err(e)=>entry.error=Some(format!("视频复制失败（{:?}），请检查源文件、目录权限及剩余空间；同名未完成文件不可当作成功视频。",e.kind()))}
        }
        entries.push(entry);
    }
    let exported = entries
        .iter()
        .filter(|e| e.error.is_none() && e.file_name.is_some())
        .count();
    let manifest_result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("export-manifest.json"))?;
        serde_json::to_writer_pretty(
            &mut file,
            &serde_json::json!({"format":"AI_STUDIO_SELECTED_VIDEOS_V1","entries":entries}),
        )?;
        file.sync_all()
    })();
    Ok(SelectedVideoExport {
        exported,
        failed: entries.len() - exported,
        directory: directory.to_string_lossy().into_owned(),
        entries,
        manifest_error: manifest_result
            .err()
            .map(|_| "导出映射清单写入失败；已复制视频保留，请勿误认为全部未导出。".into()),
        open_error: None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source(ordinal: i64, name: &str, path: PathBuf) -> ExportSource {
        ExportSource {
            ordinal,
            shot_id: format!("shot-{ordinal}"),
            shot_name: name.into(),
            asset_id: format!("asset-{ordinal}"),
            path: Some(path),
            error: None,
        }
    }
    #[test]
    fn selected_video_delivery_preserves_shot_identity_bytes_and_private_path_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("source-a.mp4");
        let b = dir.path().join("source-b.webm");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let result = copy_selected_videos(
            dir.path(),
            &[source(2, "镜头三", a.clone()), source(8, "镜头九", b)],
        )
        .unwrap();
        assert_eq!(result.exported, 2);
        assert_eq!(result.failed, 0);
        let root = Path::new(&result.directory);
        assert_eq!(fs::read(root.join("0003_镜头三.mp4")).unwrap(), b"a");
        assert_eq!(fs::read(root.join("0009_镜头九.webm")).unwrap(), b"b");
        let text = fs::read_to_string(root.join("export-manifest.json")).unwrap();
        assert!(text.contains("shot-8"));
        assert!(!text.contains(&dir.path().to_string_lossy().to_string()));
        let next = copy_selected_videos(dir.path(), &[source(2, "镜头三", a.clone())]).unwrap();
        assert_ne!(result.directory, next.directory);
        assert_eq!(fs::read(a).unwrap(), b"a");
    }
    #[test]
    fn selected_video_delivery_reports_each_failure_and_sanitizes_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.mp4");
        fs::write(&a, b"data").unwrap();
        let result = copy_selected_videos(
            dir.path(),
            &[
                source(0, "../C:\"bad*", a),
                source(9, "missing", dir.path().join("missing.mp4")),
            ],
        )
        .unwrap();
        assert_eq!(result.exported, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.entries[1].shot_id, "shot-9");
        assert!(result.entries[1].error.is_some());
        let name = result.entries[0].file_name.as_ref().unwrap();
        assert!(!name.contains('/'));
        assert!(!name.contains(':'));
        assert!(!name.contains('*'));
        assert!(copy_selected_videos(dir.path(), &[]).is_err());
    }
}
