use std::fs;

#[test]
fn project_backup_application_boundary_is_sqlx_free() {
    let service = fs::read_to_string("src/application/project_backup_service.rs")
        .expect("project backup service source should be readable");
    let production = service
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("project backup tests should have a stable boundary");

    for forbidden in ["sqlx", "SqlitePool", "Transaction", "FromRow"] {
        assert!(
            !production.contains(forbidden),
            "application project backup production code must not contain {forbidden}"
        );
    }

    let port = fs::read_to_string("src/application/ports/project_backup_repository.rs")
        .expect("project backup port source should be readable");
    for forbidden in ["sqlx", "SqlitePool", "Transaction", "FromRow"] {
        assert!(
            !port.contains(forbidden),
            "project backup port must not contain {forbidden}"
        );
    }
    assert!(port.contains("async fn load_export_snapshot"));
    assert!(port.contains("async fn find_missing_workflows"));
    assert!(port.contains("async fn restore_atomic"));

    let repository =
        fs::read_to_string("src/infrastructure/database/repositories/project_backup.rs")
            .expect("project backup repository source should be readable");
    assert!(repository.contains("impl ProjectBackupRepository for SqliteProjectBackupRepository"));
    assert!(repository.contains(".pool") && repository.contains(".begin()"));
    assert!(repository.contains("restore_rows_in_transaction"));
}
