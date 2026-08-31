//! Read-only discovery of production-package manifests below a filesystem root.
//!
//! Discovery deliberately does not inspect manifest JSON or touch application
//! state. It only identifies package roots and hashes the manifest bytes.

use crate::domain::{normalize_package_root, production_package_source_key};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    collections::{HashSet, VecDeque},
    error::Error,
    fmt,
    fs::{self, DirEntry},
    path::{Path, PathBuf},
};

pub const PRODUCTION_PACKAGE_MANIFEST: &str = "production-package.json";
pub const MAX_DEPTH: usize = 4;
pub const MAX_PACKAGES: usize = 100;
pub const MAX_DIRECTORIES: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageDiscoveryResult {
    pub root_path: String,
    pub packages: Vec<ProductionPackageDiscoveryPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageDiscoveryPackage {
    pub package_key: String,
    pub package_root: String,
    pub relative_path: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
}

#[derive(Debug)]
pub enum ProductionPackageDiscoveryError {
    InvalidRoot(String),
    Filesystem(String),
    MaxDepthExceeded { path: PathBuf, max_depth: usize },
    MaxPackagesExceeded { max_packages: usize },
    MaxDirectoriesExceeded { max_directories: usize },
}

impl fmt::Display for ProductionPackageDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoot(message) => write!(formatter, "DISCOVERY_INVALID_ROOT: {message}"),
            Self::Filesystem(message) => write!(formatter, "DISCOVERY_FILESYSTEM_ERROR: {message}"),
            Self::MaxDepthExceeded { path, max_depth } => write!(
                formatter,
                "DISCOVERY_MAX_DEPTH_EXCEEDED: directory {} exceeds MAX_DEPTH={max_depth}",
                path.display()
            ),
            Self::MaxPackagesExceeded { max_packages } => write!(
                formatter,
                "DISCOVERY_MAX_PACKAGES_EXCEEDED: more than MAX_PACKAGES={max_packages} packages were found"
            ),
            Self::MaxDirectoriesExceeded { max_directories } => write!(
                formatter,
                "DISCOVERY_MAX_DIRECTORIES_EXCEEDED: more than MAX_DIRECTORIES={max_directories} directories were visited"
            ),
        }
    }
}

impl Error for ProductionPackageDiscoveryError {}

#[derive(Debug, Default)]
pub struct ProductionPackageDiscoveryService;

impl ProductionPackageDiscoveryService {
    pub fn new() -> Self {
        Self
    }

    pub fn discover(
        &self,
        root_path: impl AsRef<Path>,
    ) -> Result<ProductionPackageDiscoveryResult, ProductionPackageDiscoveryError> {
        discover_production_packages(root_path.as_ref())
    }
}

pub fn discover_production_packages(
    root_path: &Path,
) -> Result<ProductionPackageDiscoveryResult, ProductionPackageDiscoveryError> {
    let root = fs::canonicalize(root_path).map_err(|error| {
        ProductionPackageDiscoveryError::InvalidRoot(format!(
            "could not canonicalize {}: {error}",
            root_path.display()
        ))
    })?;
    if !root.is_dir() {
        return Err(ProductionPackageDiscoveryError::InvalidRoot(format!(
            "root is not a directory: {}",
            root.display()
        )));
    }

    let mut pending = VecDeque::from([(root.clone(), 0usize)]);
    let mut scheduled = HashSet::from([root.clone()]);
    let mut visited = HashSet::new();
    let mut packages = Vec::new();

    while let Some((directory, depth)) = pending.pop_front() {
        if !visited.insert(directory.clone()) {
            continue;
        }
        if visited.len() > MAX_DIRECTORIES {
            return Err(ProductionPackageDiscoveryError::MaxDirectoriesExceeded {
                max_directories: MAX_DIRECTORIES,
            });
        }

        if let Some(package) = discover_manifest(&root, &directory)? {
            if packages.len() >= MAX_PACKAGES {
                return Err(ProductionPackageDiscoveryError::MaxPackagesExceeded {
                    max_packages: MAX_PACKAGES,
                });
            }
            packages.push(package);
            continue;
        }

        let entries = fs::read_dir(&directory).map_err(|error| {
            ProductionPackageDiscoveryError::Filesystem(format!(
                "could not read directory {}: {error}",
                directory.display()
            ))
        })?;
        let mut child_directories = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                ProductionPackageDiscoveryError::Filesystem(format!(
                    "could not read an entry in {}: {error}",
                    directory.display()
                ))
            })?;
            let Some(child) = canonical_directory_entry(&root, &entry)? else {
                continue;
            };
            child_directories.push(child);
        }
        child_directories
            .sort_by(|left, right| natural_path_cmp(&path_string(left), &path_string(right)));

        for child in child_directories {
            if scheduled.contains(&child) {
                continue;
            }
            if depth >= MAX_DEPTH {
                return Err(ProductionPackageDiscoveryError::MaxDepthExceeded {
                    path: child,
                    max_depth: MAX_DEPTH,
                });
            }
            scheduled.insert(child.clone());
            pending.push_back((child, depth + 1));
        }
    }

    packages.sort_by(|left, right| natural_path_cmp(&left.relative_path, &right.relative_path));
    Ok(ProductionPackageDiscoveryResult {
        root_path: path_string(&root),
        packages,
    })
}

fn discover_manifest(
    root: &Path,
    directory: &Path,
) -> Result<Option<ProductionPackageDiscoveryPackage>, ProductionPackageDiscoveryError> {
    let candidate = directory.join(PRODUCTION_PACKAGE_MANIFEST);
    let metadata = match fs::symlink_metadata(&candidate) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ProductionPackageDiscoveryError::Filesystem(format!(
                "could not inspect manifest {}: {error}",
                candidate.display()
            )))
        }
    };
    if !metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
        return Ok(None);
    }

    let manifest = fs::canonicalize(&candidate).map_err(|error| {
        ProductionPackageDiscoveryError::Filesystem(format!(
            "could not canonicalize manifest {}: {error}",
            candidate.display()
        ))
    })?;
    if !manifest.starts_with(root) {
        return Ok(None);
    }
    let manifest_metadata = fs::metadata(&manifest).map_err(|error| {
        ProductionPackageDiscoveryError::Filesystem(format!(
            "could not inspect manifest {}: {error}",
            manifest.display()
        ))
    })?;
    if !manifest_metadata.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&manifest).map_err(|error| {
        ProductionPackageDiscoveryError::Filesystem(format!(
            "could not read manifest {}: {error}",
            manifest.display()
        ))
    })?;
    let relative_path = directory
        .strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let package_root = path_string(directory);
    let manifest_sha256 = format!("{:x}", Sha256::digest(bytes));
    let normalized_root = normalize_package_root(directory);
    let package_key = production_package_source_key(&normalized_root, &manifest_sha256);
    Ok(Some(ProductionPackageDiscoveryPackage {
        package_key,
        package_root,
        relative_path,
        manifest_path: path_string(&manifest),
        manifest_sha256,
    }))
}

fn canonical_directory_entry(
    root: &Path,
    entry: &DirEntry,
) -> Result<Option<PathBuf>, ProductionPackageDiscoveryError> {
    let file_type = entry.file_type().map_err(|error| {
        ProductionPackageDiscoveryError::Filesystem(format!(
            "could not inspect directory entry {}: {error}",
            entry.path().display()
        ))
    })?;
    if !file_type.is_dir() && !file_type.is_symlink() {
        return Ok(None);
    }
    let canonical = match fs::canonicalize(entry.path()) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ProductionPackageDiscoveryError::Filesystem(format!(
                "could not canonicalize directory {}: {error}",
                entry.path().display()
            )))
        }
    };
    if !canonical.is_dir() || !canonical.starts_with(root) {
        return Ok(None);
    }
    Ok(Some(canonical))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn natural_path_cmp(left: &str, right: &str) -> Ordering {
    let left = left.to_ascii_lowercase().into_bytes();
    let right = right.to_ascii_lowercase().into_bytes();
    let mut left_index = 0usize;
    let mut right_index = 0usize;
    while left_index < left.len() && right_index < right.len() {
        if left[left_index].is_ascii_digit() && right[right_index].is_ascii_digit() {
            let left_start = left_index;
            let right_start = right_index;
            while left_index < left.len() && left[left_index].is_ascii_digit() {
                left_index += 1;
            }
            while right_index < right.len() && right[right_index].is_ascii_digit() {
                right_index += 1;
            }
            let left_number = trim_leading_zeroes(&left[left_start..left_index]);
            let right_number = trim_leading_zeroes(&right[right_start..right_index]);
            match left_number
                .len()
                .cmp(&right_number.len())
                .then_with(|| left_number.cmp(right_number))
            {
                Ordering::Equal => continue,
                ordering => return ordering,
            }
        }
        match left[left_index].cmp(&right[right_index]) {
            Ordering::Equal => {
                left_index += 1;
                right_index += 1;
            }
            ordering => return ordering,
        }
    }
    left.len().cmp(&right.len())
}

fn trim_leading_zeroes(value: &[u8]) -> &[u8] {
    value.iter().position(|byte| *byte != b'0').map_or_else(
        || &value[value.len().saturating_sub(1)..],
        |index| &value[index..],
    )
}
