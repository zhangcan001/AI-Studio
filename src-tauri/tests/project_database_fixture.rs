#[path = "support/project_database.rs"]
mod project_database;

use ai_studio_lib::infrastructure::database::initialize;
use project_database::ProjectDatabase;

#[tokio::test]
async fn phase11_project_database_isolated_resources_survive_reopen_and_clean_teardown() {
    let first = ProjectDatabase::new("project.db").await;
    let second = ProjectDatabase::new("project.db").await;
    assert_ne!(first.path(), second.path());
    let first_root = first.path().to_path_buf();
    sqlx::query("INSERT INTO projects (id,name,root_path,created_at,updated_at) VALUES ('owned','A','fixture','2026-01-01','2026-01-01')")
        .execute(&first.pool).await.unwrap();
    let foreign: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE id='owned'")
        .fetch_one(&second.pool)
        .await
        .unwrap();
    assert_eq!(foreign, 0);
    let max_version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&first.pool)
        .await
        .unwrap();
    assert_eq!(max_version, 42);
    first.pool.close().await;
    assert!(first.database_path().exists()); // TempDir remains owned through reopen.
    let reopened = initialize(&first.database_path()).await.unwrap();
    let persisted: String = sqlx::query_scalar("SELECT name FROM projects WHERE id='owned'")
        .fetch_one(&reopened)
        .await
        .unwrap();
    assert_eq!(persisted, "A");
    reopened.close().await;
    first.close().await;
    assert!(!first_root.exists());
    second.close().await;
}

#[tokio::test]
async fn phase11_project_database_preserves_real_transaction_failure_and_foreign_keys() {
    let fixture = ProjectDatabase::new("rollback.db").await;
    let mut transaction = fixture.pool.begin().await.unwrap();
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
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    let orphan = sqlx::query("INSERT INTO workflow_versions (id,workflow_id,version,api_workflow_json,workflow_sha256,created_at) VALUES ('orphan','missing','1','{}','sha','2026-01-01')")
        .execute(&fixture.pool).await;
    assert!(orphan.is_err());
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1);
    fixture.close().await;
}
