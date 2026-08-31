#[path = "../src/domain/production_package.rs"]
pub mod domain;
#[path = "../src/application/production_package_discovery_service.rs"]
mod production_package_discovery_service;

use domain::ProductionPackageProvenance;
use production_package_discovery_service::{
    ProductionPackageDiscoveryError, ProductionPackageDiscoveryService, MAX_DIRECTORIES,
    MAX_PACKAGES,
};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tempfile::tempdir;

const MANIFEST: &str = "production-package.json";

#[test]
fn discovers_three_packages_without_inspecting_or_mutating_them() {
    let fixture = tempdir().expect("fixture root should exist");
    for name in ["EP01", "EP02", "EP03"] {
        write_manifest(&fixture.path().join(name), name.as_bytes());
    }

    let result = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect("three package roots should be discovered");

    assert_eq!(result.packages.len(), 3);
    assert_eq!(
        result
            .packages
            .iter()
            .map(|package| package.relative_path.as_str())
            .collect::<Vec<_>>(),
        vec!["EP01", "EP02", "EP03"]
    );
    assert!(result.root_path == fs::canonicalize(fixture.path()).unwrap().to_string_lossy());
    assert_eq!(
        result.packages[0].manifest_sha256,
        format!("{:x}", Sha256::digest(b"EP01"))
    );
}

#[test]
fn sorts_relative_paths_with_numeric_aware_natural_order() {
    let fixture = tempdir().expect("fixture root should exist");
    for name in ["EP10", "EP03", "EP01", "EP11", "EP02"] {
        write_manifest(&fixture.path().join(name), name.as_bytes());
    }

    let result = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect("natural sort fixture should be discovered");
    let names = result
        .packages
        .iter()
        .map(|package| package.relative_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["EP01", "EP02", "EP03", "EP10", "EP11"]);
}

#[test]
fn discovers_a_manifest_at_the_root_itself() {
    let fixture = tempdir().expect("fixture root should exist");
    let contents = br#"{"root":true}"#;
    write_manifest_bytes(fixture.path(), contents);

    let result = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect("root package should be discovered");

    assert_eq!(result.packages.len(), 1);
    assert_eq!(result.packages[0].relative_path, "");
    assert_eq!(
        result.packages[0].package_root,
        fs::canonicalize(fixture.path()).unwrap().to_string_lossy()
    );
    assert_eq!(
        result.packages[0].manifest_sha256,
        format!("{:x}", Sha256::digest(contents))
    );
}

#[test]
fn package_key_matches_formal_provenance_and_serializes_as_package_key() {
    let fixture = tempdir().expect("fixture root should exist");
    write_manifest(&fixture.path().join("EP01"), b"package-v1");

    let result = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect("package should be discovered");
    let package = &result.packages[0];
    let provenance = ProductionPackageProvenance::new(
        Path::new(&package.package_root),
        package.manifest_sha256.clone(),
        None,
        "EP01",
        0,
        1,
        Vec::new(),
    );

    assert_eq!(package.package_key, provenance.source_package_key);
    let serialized = serde_json::to_value(package).expect("package should serialize");
    assert_eq!(serialized["packageKey"], package.package_key);
    assert!(serialized.get("package_key").is_none());
}

#[test]
fn changing_manifest_bytes_changes_sha_and_package_key() {
    let fixture = tempdir().expect("fixture root should exist");
    let package_root = fixture.path().join("EP01");
    let manifest_v1 = b"package-v1";
    let manifest_v2 = b"package-v2";
    write_manifest(&package_root, manifest_v1);

    let first = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect("v1 package should be discovered")
        .packages
        .remove(0);
    write_manifest_bytes(&package_root, manifest_v2);
    let second = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect("v2 package should be discovered")
        .packages
        .remove(0);

    assert_ne!(manifest_v1, manifest_v2);
    assert_eq!(first.package_root, second.package_root);
    assert_ne!(first.manifest_sha256, second.manifest_sha256);
    assert_ne!(first.package_key, second.package_key);
}

#[test]
fn nested_manifests_are_not_scanned_below_a_package_root() {
    let fixture = tempdir().expect("fixture root should exist");
    let parent = fixture.path().join("EP01");
    write_manifest(&parent, b"parent");
    write_manifest(&parent.join("nested"), b"nested");

    let result = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect("parent package should stop recursive scanning");

    assert_eq!(result.packages.len(), 1);
    assert_eq!(result.packages[0].relative_path, "EP01");
}

#[test]
fn escaping_directory_symlink_is_skipped_without_scanning_external_content() {
    let fixture = tempdir().expect("fixture root should exist");
    let external = tempdir().expect("external fixture root should exist");
    write_manifest(&external.path().join("outside"), b"outside");
    let link = fixture.path().join("escape");
    if !create_directory_symlink(external.path(), &link) {
        return;
    }

    let result = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect("escaping symlink should be safely ignored");
    assert!(result.packages.is_empty());
}

#[test]
fn exceeding_package_limit_returns_an_explicit_error() {
    let fixture = tempdir().expect("fixture root should exist");
    for index in 0..=MAX_PACKAGES {
        write_manifest(&fixture.path().join(format!("EP{index:03}")), b"package");
    }

    let error = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect_err("the package limit must not silently truncate results");
    assert!(matches!(
        error,
        ProductionPackageDiscoveryError::MaxPackagesExceeded {
            max_packages: MAX_PACKAGES
        }
    ));
    assert!(error.to_string().contains("MAX_PACKAGES"));
}

#[test]
fn exceeding_directory_limit_returns_an_explicit_error() {
    let fixture = tempdir().expect("fixture root should exist");
    for index in 0..MAX_DIRECTORIES {
        fs::create_dir(fixture.path().join(format!("directory-{index:04}")))
            .expect("directory fixture should be created");
    }

    let error = ProductionPackageDiscoveryService::new()
        .discover(fixture.path())
        .expect_err("the directory limit must not silently truncate results");
    assert!(matches!(
        error,
        ProductionPackageDiscoveryError::MaxDirectoriesExceeded {
            max_directories: MAX_DIRECTORIES
        }
    ));
    assert!(error.to_string().contains("MAX_DIRECTORIES"));
}

fn write_manifest(directory: &Path, contents: &[u8]) {
    fs::create_dir_all(directory).expect("package directory should be created");
    write_manifest_bytes(directory, contents);
}

fn write_manifest_bytes(directory: &Path, contents: &[u8]) {
    fs::write(directory.join(MANIFEST), contents).expect("manifest should be written");
}

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, link).is_ok()
}
