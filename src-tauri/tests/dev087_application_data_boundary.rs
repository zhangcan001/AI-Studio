use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

const DIRECT_SQLX_MARKERS: &[&str] = &[
    "sqlx",
    "SqlitePool",
    "Transaction<",
    "FromRow",
    "sqlx::query",
    "sqlx::query_as",
    "sqlx::query_scalar",
];

// These are the existing application SQLx exceptions outside DEV-087A. New
// application files must use a port; DEV-087B will handle the two production
// write-side services explicitly.
const EXISTING_APPLICATION_SQLX_ALLOWLIST: &[&str] = &[
    "asset_video_prompt_service.rs",
    "cancellation_e2e.rs",
    "dev036_compatibility.rs",
    "episode_production_service.rs",
    "generation_catalog_service.rs",
    "h3_local_import_service.rs",
    "preset_service.rs",
    "production_structure_service.rs",
    "project_backup_service.rs",
    "project_template_service.rs",
    "prompt_library_service.rs",
    "reference_anchor_service.rs",
    "shot_batch_service.rs",
    "shot_bulk_service.rs",
    "task_recovery_service.rs",
    "workflow_onboarding_service.rs",
];

const READ_SIDE_APPLICATION_SERVICES: &[&str] = &[
    "production_audit_service.rs",
    "project_command_center_service.rs",
    "project_manifest_service.rs",
    "diagnostics_service.rs",
    "workflow_benchmark_service.rs",
];

fn visit_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory).expect("application source directory should be readable");
    for entry in entries {
        let path = entry
            .expect("application source entry should be readable")
            .path();
        if path.is_dir() {
            visit_rust_files(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn production_source(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
}

fn contains_direct_sqlx(source: &str) -> bool {
    DIRECT_SQLX_MARKERS
        .iter()
        .any(|marker| source.contains(marker))
}

#[test]
fn application_read_side_has_no_sqlx_and_no_unlisted_sqlx_files() {
    let application_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/application");
    let mut files = Vec::new();
    visit_rust_files(&application_root, &mut files);

    let allowlist = EXISTING_APPLICATION_SQLX_ALLOWLIST
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let read_side = READ_SIDE_APPLICATION_SERVICES
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut unexpected = Vec::new();

    for path in files {
        let relative = path
            .strip_prefix(&application_root)
            .expect("application file should be below application root")
            .to_string_lossy()
            .replace('\\', "/");
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .expect("Rust source should have a file name");
        let source = fs::read_to_string(&path).expect("application source should be readable");
        let production = production_source(&source);

        if read_side.contains(file_name) && contains_direct_sqlx(production) {
            unexpected.push(format!(
                "read-side service contains direct SQLx: {relative}"
            ));
        } else if !allowlist.contains(file_name) && contains_direct_sqlx(production) {
            unexpected.push(format!("new application SQLx exception: {relative}"));
        }
    }

    assert!(unexpected.is_empty(), "{unexpected:#?}");
}

#[test]
fn application_ports_do_not_leak_sqlx_types() {
    let ports_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/application/ports");
    let mut files = Vec::new();
    visit_rust_files(&ports_root, &mut files);
    let leaking = files
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).ok()?;
            contains_direct_sqlx(&source).then(|| path.display().to_string())
        })
        .collect::<Vec<_>>();

    assert!(
        leaking.is_empty(),
        "application ports leak SQLx: {leaking:#?}"
    );
}

#[test]
fn workflow_benchmark_application_sqlx_is_zero() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/application/workflow_benchmark_service.rs");
    let source = fs::read_to_string(path).expect("workflow benchmark service should be readable");
    assert!(
        !contains_direct_sqlx(production_source(&source)),
        "WORKFLOW_BENCHMARK_APPLICATION_SQLX=0"
    );
}

#[test]
fn production_orchestrator_application_sqlx_is_zero() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/application/production_orchestrator_service.rs");
    let source =
        fs::read_to_string(path).expect("production orchestrator service should be readable");
    assert!(
        !contains_direct_sqlx(production_source(&source)),
        "PRODUCTION_ORCHESTRATOR_APPLICATION_SQLX=0"
    );
}
