//! DEV-100 active authoring boundary and compatibility guards.

use std::{fs, path::PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn source(path: &str) -> String {
    fs::read_to_string(PathBuf::from(ROOT).join(path))
        .unwrap_or_else(|error| panic!("DEV-100 source fixture {path} should be readable: {error}"))
}

#[test]
fn retired_authoring_ipc_is_not_registered() {
    let lib = source("src/lib.rs");
    let commands = source("src/commands/mod.rs");
    assert!(!lib.contains("commands::prompt_template"));
    assert!(!lib.contains("analyze_prompt_template"));
    assert!(!lib.contains("apply_prompt_template"));
    assert!(!commands.contains("pub mod prompt_template"));
}

#[test]
fn legacy_script_schema_and_backup_v17_remain_compatibility_boundaries() {
    let migration = source("migrations/025_script_draft_foundation.sql");
    let backup = source("src/application/project_backup_service.rs");
    assert!(migration.contains("script_sources"));
    assert!(migration.contains("script_import_drafts"));
    assert!(backup.contains("BACKUP_VERSION: u32 = 18"));
    assert!(backup.contains("BackupScriptSource"));
    assert!(backup.contains("BackupScriptDraftRevision"));
}

#[test]
fn formal_shot_bulk_import_stays_input_only() {
    let service = source("src/application/shot_bulk_service.rs");
    assert!(service.contains("pub async fn preview_import"));
    assert!(service.contains("pub async fn commit_import"));
    assert!(service.contains("insert_shots_atomic"));
    assert!(!service.contains("production_queue"));
    assert!(!service.contains("Comfy"));
}
