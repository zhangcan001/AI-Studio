#[path = "support/project_database.rs"]
mod project_database;

use project_database::ProjectDatabase;
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};

#[tokio::test]
async fn phase11_project_database_isolated_resources_survive_reopen_and_clean_teardown() {
    let first = ProjectDatabase::new("project.db").await;
    let second = ProjectDatabase::new("project.db").await;
    assert_ne!(first.path(), second.path());
    let first_root = first.path().to_path_buf();
    // Own these leases explicitly: implicit pooled query drops schedule their
    // return on a background task, which is not a worker-shutdown barrier.
    let mut first_connection = first.pool.acquire().await.unwrap();
    let mut second_connection = second.pool.acquire().await.unwrap();
    sqlx::query("INSERT INTO projects (id,name,root_path,created_at,updated_at) VALUES ('owned','A','fixture','2026-01-01','2026-01-01')")
        .execute(&mut *first_connection).await.unwrap();
    let foreign: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE id='owned'")
        .fetch_one(&mut *second_connection)
        .await
        .unwrap();
    assert_eq!(foreign, 0);
    let max_version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&mut *first_connection)
        .await
        .unwrap();
    assert_eq!(max_version, 43);
    first_connection.close().await.unwrap();
    second_connection.close().await.unwrap();
    first.pool.close().await;
    // TempDir remains owned through reopen.
    assert!(first.database_path().exists());
    // Reopen the already-migrated real file with one owned SQLite worker. Its
    // Connection::close awaits worker shutdown, not a pool's queued return.
    let mut reopened = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(first.database_path())
            .foreign_keys(true),
    )
    .await
    .unwrap();
    let persisted: String = sqlx::query_scalar("SELECT name FROM projects WHERE id='owned'")
        .fetch_one(&mut reopened)
        .await
        .unwrap();
    assert_eq!(persisted, "A");
    reopened.close().await.unwrap();
    first.close().await;
    assert!(!first_root.exists());
    second.close().await;
}

#[tokio::test]
async fn phase11_project_database_preserves_real_transaction_failure_and_foreign_keys() {
    let fixture = ProjectDatabase::new("rollback.db").await;
    let mut connection = fixture.pool.acquire().await.unwrap();
    let mut transaction = connection.begin().await.unwrap();
    let insert = "INSERT INTO projects (id,name,root_path,created_at,updated_at) VALUES ('rollback','A','fixture','2026-01-01','2026-01-01')";
    sqlx::query(insert)
        .execute(&mut *transaction)
        .await
        .unwrap();
    assert!(sqlx::query(insert)
        .execute(&mut *transaction)
        .await
        .is_err());
    transaction.rollback().await.unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE id='rollback'")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    let orphan = sqlx::query("INSERT INTO workflow_versions (id,workflow_id,version,api_workflow_json,workflow_sha256,created_at) VALUES ('orphan','missing','1','{}','sha','2026-01-01')")
        .execute(&mut *connection).await;
    assert!(orphan.is_err());
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1);
    connection.close().await.unwrap();
    fixture.close().await;
}
