use std::{fs, path::PathBuf};

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

#[test]
fn legacy_batch_review_is_not_a_live_authority_but_remains_archivable() {
    for (path, forbidden) in [
        ("src/lib.rs", "commands::production_item_review"),
        ("src/application/mod.rs", "production_item_review_service"),
        (
            "src/application/ports/mod.rs",
            "production_item_review_repository",
        ),
        (
            "src/infrastructure/database/repositories/mod.rs",
            "production_item_review::SqliteProductionItemReviewRepository",
        ),
        ("src/commands/mod.rs", "pub mod production_item_review"),
    ] {
        assert!(
            !source(path).contains(forbidden),
            "legacy live review wiring should be absent from {path}"
        );
    }

    assert!(
        source("src/application/project_backup_service.rs").contains("BackupProductionItemReview"),
        "legacy review records must remain available to archive restore"
    );
    assert!(
        root_migration_exists("012_production_item_review.sql"),
        "the legacy review table migration must remain for existing databases"
    );
}

fn root_migration_exists(name: &str) -> bool {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("migrations")
        .join(name)
        .is_file()
}
