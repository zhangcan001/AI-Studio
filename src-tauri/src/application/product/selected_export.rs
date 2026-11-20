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
    pub path: Option<(PathBuf, String, u64)>,
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
/// Confirmation is tied to the exact current rejected set, not a blanket bypass.
pub fn check_review_confirmation(
    rejected: &[String],
    confirmed: Option<&[String]>,
) -> Result<(), super::error::ProductError> {
    use std::collections::BTreeSet;
    let actual = rejected.iter().collect::<BTreeSet<_>>();
    let expected = confirmed
        .unwrap_or_default()
        .iter()
        .collect::<BTreeSet<_>>();
    if actual == expected {
        return Ok(());
    }
    let mut error = super::error::ProductError::new(
        "EXPORT_REJECTED_CONFIRMATION_REQUIRED",
        "已选用视频包含已退回候选，请明确确认后导出。",
        None,
    );
    error.details.rejected_asset_ids = Some(rejected.to_vec());
    Err(error)
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
                let (path, sha256, size) = source
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
                let metadata = input.metadata()?;
                if !metadata.is_file() || metadata.len() == 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
                }
                if metadata.len() != *size {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "video size changed",
                    ));
                }
                publish_new_file(&directory, &file_name, |output| {
                    use sha2::{Digest, Sha256};
                    use std::io::{Read, Write};
                    let mut hash = Sha256::new();
                    let mut buffer = [0; 64 * 1024];
                    let mut copied = 0u64;
                    loop {
                        let count = input.read(&mut buffer)?;
                        if count == 0 {
                            break;
                        }
                        copied += count as u64;
                        if copied > *size {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "video size changed",
                            ));
                        }
                        hash.update(&buffer[..count]);
                        output.write_all(&buffer[..count])?;
                    }
                    if copied != *size
                        || !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(sha256)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "video content changed",
                        ));
                    }
                    Ok(())
                })?;
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
    let manifest_result = publish_new_file(&directory, "export-manifest.json", |file| {
        serde_json::to_writer_pretty(
            file,
            &serde_json::json!({"format":"AI_STUDIO_SELECTED_VIDEOS_V1","entries":entries}),
        )
        .map_err(io::Error::from)
    });
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
fn publish_new_file(
    directory: &Path,
    name: &str,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let temporary = directory.join(format!(".{}.partial", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = write(&mut file).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| {
        let destination = directory.join(name);
        if destination.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "export file already exists",
            ));
        }
        fs::rename(&temporary, destination)
    });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
/// Identity of the videos actually selected for export. Unselected shots never
/// participate; a name or ordinal change of a selected shot is a new identity.
pub fn export_selection_identity<'a, I>(
    selected: I,
) -> std::collections::BTreeMap<String, (String, String, i64)>
where
    I: IntoIterator<Item = (&'a str, &'a str, &'a str, i64)>,
{
    selected
        .into_iter()
        .map(|(shot_id, asset_id, name, ordinal)| {
            (
                shot_id.to_owned(),
                (asset_id.to_owned(), name.to_owned(), ordinal),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_export_requires_exact_fresh_confirmation() {
        let rejected = vec!["a".to_owned()];
        assert!(check_review_confirmation(&rejected, None).is_err());
        assert!(check_review_confirmation(&rejected, Some(&rejected)).is_ok());
        assert!(check_review_confirmation(&["a".into(), "b".into()], Some(&rejected)).is_err());
        assert!(check_review_confirmation(&[], Some(&rejected)).is_err());
        assert!(check_review_confirmation(&[], None).is_ok());
    }
    #[test]
    fn export_command_uses_nonblocking_native_picker_and_revalidates_before_copy() {
        let source = include_str!("../../commands/product.rs");
        let command = source
            .split("pub async fn product_selected_videos_export(")
            .nth(1)
            .unwrap()
            .split("#[tauri::command")
            .next()
            .unwrap();
        assert!(!command.contains("blocking_pick_folder"));
        assert!(command.contains(".pick_folder("));
        assert!(command.contains("rx.await"));
        assert_eq!(command.matches("check_review_confirmation(").count(), 2);
        assert_eq!(command.matches("selected_video_export_sources(").count(), 2);
        assert!(
            command.find("current_selection != selection").unwrap()
                < command.find("copy_selected_videos(").unwrap()
        );
        assert!(
            command
                .find("selected_video_export_sources(&state, &project_id, &current_shots)")
                .unwrap()
                > command.find("current_selection != selection").unwrap()
        );
        assert!(command.contains("(asset.clone(), s.name.clone(), s.ordinal)"));
    }
    #[test]
    fn r57_r58_selection_identity_includes_name_and_ordinal_but_ignores_unselected_shots() {
        let selected = export_selection_identity([
            ("shot-a", "asset-a", "镜头一", 0),
            ("shot-c", "asset-c", "镜头三", 2),
        ]);
        let renamed = export_selection_identity([
            ("shot-a", "asset-a", "新名称", 0),
            ("shot-c", "asset-c", "镜头三", 2),
        ]);
        let reordered = export_selection_identity([
            ("shot-a", "asset-a", "镜头一", 1),
            ("shot-c", "asset-c", "镜头三", 2),
        ]);
        let unselected_tail_changed = export_selection_identity([
            ("shot-a", "asset-a", "镜头一", 0),
            ("shot-c", "asset-c", "镜头三", 2),
        ]);
        assert_ne!(selected, renamed);
        assert_ne!(selected, reordered);
        assert_eq!(selected, unselected_tail_changed);
        assert!(!selected.contains_key("shot-b"));
    }

    fn source(ordinal: i64, name: &str, path: PathBuf) -> ExportSource {
        ExportSource {
            ordinal,
            shot_id: format!("shot-{ordinal}"),
            shot_name: name.into(),
            asset_id: format!("asset-{ordinal}"),
            path: {
                use sha2::{Digest, Sha256};
                let bytes = fs::read(&path).unwrap_or_default();
                Some((
                    path,
                    format!("{:x}", Sha256::digest(&bytes)),
                    bytes.len() as u64,
                ))
            },
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
    #[test]
    fn rejects_replaced_video_and_removes_partial_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("video.mp4");
        fs::write(&path, b"original").unwrap();
        let item = source(0, "镜头", path.clone());
        fs::write(&path, b"replaced").unwrap();
        let receipt = copy_selected_videos(dir.path(), &[item]).unwrap();
        assert_eq!((receipt.exported, receipt.failed), (0, 1));
        let names: Vec<_> = fs::read_dir(receipt.directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["export-manifest.json"]);
    }
    #[test]
    fn r23_empty_video_is_a_failed_manifest_entry_not_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.mp4");
        fs::write(&path, []).unwrap();
        let receipt = copy_selected_videos(dir.path(), &[source(0, "空视频", path)]).unwrap();
        assert_eq!(receipt.exported, 0);
        assert_eq!(receipt.failed, 1);
        assert!(receipt.entries[0].error.is_some());
        assert!(receipt.entries[0].file_name.is_none());
    }
}
