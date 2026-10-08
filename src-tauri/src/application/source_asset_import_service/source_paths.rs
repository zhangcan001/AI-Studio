use super::{source_display_name, SourceAssetImportFailure};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

pub(super) const MAX_SOURCE_FILES: usize = 500;
const MAX_SCAN_ENTRIES: usize = 10_000;
const MAX_SCAN_DEPTH: usize = 32;

fn unsafe_path() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "unsafe source path")
}
fn is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        } // any reparse point
    }
    false
}

fn validate_components(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(unsafe_path());
    }
    for ancestor in path.ancestors() {
        if is_link(&fs::symlink_metadata(ancestor)?) {
            return Err(unsafe_path());
        }
    }
    Ok(())
}

pub(super) fn within_root(root: &Path, path: &Path) -> io::Result<()> {
    validate_components(root)?;
    validate_components(path)?;
    if !fs::canonicalize(path)?.starts_with(fs::canonicalize(root)?) {
        return Err(unsafe_path());
    }
    Ok(())
}

pub(super) fn open_source_file(path: &Path) -> io::Result<fs::File> {
    validate_components(path)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT, never dereference final link
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || is_link(&metadata) {
        return Err(unsafe_path());
    }
    validate_components(path)?;
    Ok(file)
}

#[derive(Default)]
pub(super) struct Scan {
    pub files: Vec<PathBuf>,
    pub failed: Vec<SourceAssetImportFailure>,
}
impl Scan {
    fn failure(&mut self, path: &Path, message: &str) {
        self.failed.push(SourceAssetImportFailure {
            display_name: source_display_name(path),
            error: message.into(),
        });
    }
}

/// Scan only owned selections, never follow links/junctions, never walk unbounded.
/// A limit violation imports nothing rather than an arbitrary directory prefix.
pub(super) fn scan_images(root: &Path, max_files: usize) -> Scan {
    let mut result = Scan::default();
    if validate_components(root).is_err() || !root.is_dir() {
        result.failure(root, "目录不可访问或包含不安全路径/符号链接。");
        return result;
    }
    let mut stack = vec![(root.to_path_buf(), 0)];
    let mut count = 0;
    while let Some((directory, depth)) = stack.pop() {
        if depth > MAX_SCAN_DEPTH {
            result.files.clear();
            result.failure(root, "目录层数超过安全限制。");
            return result;
        }
        if within_root(root, &directory).is_err() {
            result.failure(&directory, "跳过不安全目录或符号链接。");
            continue;
        }
        let entries = match fs::read_dir(&directory) {
            Ok(v) => v,
            Err(_) => {
                result.failure(&directory, "目录无法读取。");
                continue;
            }
        };
        let mut children = Vec::new();
        for entry in entries {
            count += 1;
            if count > MAX_SCAN_ENTRIES {
                result.files.clear();
                result.failure(root, "目录条目超过安全扫描限制。");
                return result;
            }
            match entry {
                Ok(e) => children.push(e.path()),
                Err(_) => result.failure(&directory, "跳过无法读取的目录条目。"),
            }
        }
        children.sort();
        for path in children {
            if within_root(root, &path).is_err() {
                result.failure(&path, "跳过不安全路径或符号链接。");
                continue;
            }
            let metadata = match fs::symlink_metadata(&path) {
                Ok(v) => v,
                Err(_) => {
                    result.failure(&path, "文件无法读取。");
                    continue;
                }
            };
            if metadata.is_dir() {
                stack.push((path, depth + 1));
            } else if metadata.is_file()
                && path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    matches!(
                        e.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp"
                    )
                })
            {
                result.files.push(path);
                if result.files.len() > max_files {
                    result.files.clear();
                    result.failure(root, "图片文件数量超过单次导入限制。");
                    return result;
                }
            }
        }
    }
    result.files.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recursive_scan_is_bounded_ordered_and_does_not_import_non_images() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("nested")).unwrap();
        for file in ["z.png", "a.jpg", "nested/b.webp", "nested/video.mp4"] {
            fs::write(root.join(file), b"fixture").unwrap();
        }
        let scanned = scan_images(root, 3);
        assert_eq!(scanned.files.len(), 3);
        assert!(scanned.failed.is_empty());
        assert!(scanned.files.windows(2).all(|w| w[0] < w[1]));
        let limited = scan_images(root, 2);
        assert!(limited.files.is_empty());
        assert_eq!(limited.failed.len(), 1);
        let traversed = root.join("nested/../a.jpg");
        assert!(open_source_file(&traversed).is_err());
    }
    #[test]
    fn refuses_file_and_directory_links_even_when_they_resolve_inside_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("real.png"), b"fixture").unwrap();
        fs::create_dir(root.join("real")).unwrap();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(root.join("real.png"), root.join("alias.png"));
        #[cfg(windows)]
        let linked =
            std::os::windows::fs::symlink_file(root.join("real.png"), root.join("alias.png"));
        if let Err(e) = linked {
            if e.kind() == io::ErrorKind::PermissionDenied {
                #[cfg(windows)]
                {
                    // Junctions need no Developer Mode. Still exercise a real
                    // directory reparse point instead of skipping all link safety.
                    use std::os::windows::process::CommandExt;
                    fs::write(root.join("real/inside.png"), b"fixture").unwrap();
                    let status=std::process::Command::new("powershell.exe")
                        .args(["-NoProfile","-NonInteractive","-Command","New-Item -ItemType Junction -Path $env:AI_STUDIO_TEST_JUNCTION -Target $env:AI_STUDIO_TEST_TARGET -ErrorAction Stop | Out-Null"])
                        .env("AI_STUDIO_TEST_JUNCTION",root.join("dir-alias"))
                        .env("AI_STUDIO_TEST_TARGET",root.join("real"))
                        .creation_flags(0x0800_0000).status().unwrap();
                    assert!(status.success(), "owned junction fixture must be created");
                    assert!(open_source_file(&root.join("dir-alias/inside.png")).is_err());
                    let scanned = scan_images(root, 10);
                    assert_eq!(scanned.files.len(), 2);
                    assert_eq!(scanned.failed.len(), 1);
                    eprintln!("DIRECTORY_REPARSE_FIXTURE=PASS; FILE_SYMLINK_FIXTURE=NOT_VERIFIED (Windows privilege unavailable)");
                    return;
                }
                #[cfg(not(windows))]
                {
                    eprintln!("SYMLINK_FIXTURE=NOT_VERIFIED (platform permission denied)");
                    return;
                }
            }
            panic!("owned link fixture: {e}");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("real"), root.join("dir-alias")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(root.join("real"), root.join("dir-alias")).unwrap();
        assert!(open_source_file(&root.join("alias.png")).is_err());
        let scanned = scan_images(root, 10);
        assert_eq!(scanned.files.len(), 1);
        assert_eq!(scanned.failed.len(), 2);
        eprintln!("FILE_AND_DIRECTORY_SYMLINK_FIXTURE=PASS");
    }
}
