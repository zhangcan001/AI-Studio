//! Phase 4: an actual SQLite connection shutdown/reopen must preserve deferred
//! Start intents and global FIFO order without creating a second executor.
//! This is a database-level regression, not a Native client restart acceptance.

use ai_studio_lib::{
    domain::ProductionBatchId, initialize, ProductionQueueRepository,
    SqliteProductionQueueRepository,
};
use chrono::{TimeZone, Utc};
use tempfile::tempdir;

#[tokio::test]
async fn deferred_starts_survive_cold_sqlite_reopen_with_cross_project_fifo_and_owner_scope() {
    let directory = tempdir().expect("isolated temp root");
    let db_path = directory.path().join("phase4-deferred-restart.db");
    let pool = initialize(&db_path)
        .await
        .expect("fresh schema through migration 044");
    let projects = ["phase4-owner-a", "phase4-owner-b"];
    for project in projects {
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES (?, ?, NULL, ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(project)
        .bind(project)
        .bind(format!("C:/{project}"))
        .execute(&pool)
        .await
        .expect("create isolated project fixture");
    }

    let batch_ids = [
        ProductionBatchId::new(),
        ProductionBatchId::new(),
        ProductionBatchId::new(),
    ];
    let owners = [projects[0], projects[1], projects[0]];
    for (batch, owner) in batch_ids.iter().zip(owners) {
        sqlx::query(
            "INSERT INTO production_batches
             (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES (?, ?, 'cold-reopen-test', 'READY', 1,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(batch.as_str())
        .bind(owner)
        .execute(&pool)
        .await
        .expect("create owned READY batch");
    }

    let repository = SqliteProductionQueueRepository::new(pool.clone());
    let start = Utc.with_ymd_and_hms(2026, 10, 10, 10, 0, 0).unwrap();
    for (index, (batch, owner)) in batch_ids.iter().zip(owners).enumerate() {
        assert!(repository
            .enqueue_deferred_start(
                owner,
                batch,
                start + chrono::Duration::seconds(index as i64)
            )
            .await
            .unwrap());
    }
    // Idempotent replay must preserve the first requested_at and never let a
    // different project create or observe the same batch's deferred intent.
    assert!(repository
        .enqueue_deferred_start(
            projects[1],
            &batch_ids[1],
            start + chrono::Duration::hours(1)
        )
        .await
        .unwrap());
    assert!(!repository
        .enqueue_deferred_start(projects[0], &batch_ids[1], start)
        .await
        .unwrap());
    assert!(repository
        .deferred_start_state(projects[0], &batch_ids[1])
        .await
        .unwrap()
        .is_none());

    // Simulate a genuine app database lifecycle, not merely new repository
    // objects referencing an old open pool.
    pool.close().await;
    let restarted_pool = initialize(&db_path).await.expect("reopen durable database");
    let restarted = SqliteProductionQueueRepository::new(restarted_pool.clone());
    let after_reopen = restarted.list_deferred_starts().await.unwrap();
    assert_eq!(after_reopen.len(), 3);
    for (index, intent) in after_reopen.iter().enumerate() {
        assert_eq!(intent.batch_id, batch_ids[index].as_str());
        assert_eq!(intent.project_id, owners[index]);
        assert_eq!(
            intent.requested_at,
            start + chrono::Duration::seconds(index as i64)
        );
    }

    // A blocked head must remain auditable but cannot starve the next owner's
    // runnable intent. Wrong-project cleanup must not mutate a valid intent.
    restarted
        .finish_deferred_start(projects[0], &batch_ids[1])
        .await
        .unwrap();
    assert_eq!(restarted.list_deferred_starts().await.unwrap().len(), 3);
    restarted
        .block_deferred_start(
            projects[0],
            &batch_ids[0],
            "isolated preflight block",
            start,
        )
        .await
        .unwrap();
    let after_block = restarted.list_deferred_starts().await.unwrap();
    assert_eq!(after_block.len(), 2);
    assert_eq!(after_block[0].batch_id, batch_ids[1].as_str());
    assert_eq!(after_block[1].batch_id, batch_ids[2].as_str());
    assert_eq!(after_block[0].project_id, projects[1]);
    assert_eq!(
        restarted
            .deferred_start_state(projects[0], &batch_ids[0])
            .await
            .unwrap(),
        Some((
            "BLOCKED".to_string(),
            Some("isolated preflight block".to_string())
        ))
    );
    restarted
        .finish_deferred_start(projects[1], &batch_ids[1])
        .await
        .unwrap();
    let after_second = restarted.list_deferred_starts().await.unwrap();
    assert_eq!(after_second.len(), 1);
    assert_eq!(after_second[0].batch_id, batch_ids[2].as_str());
    assert_eq!(after_second[0].project_id, projects[0]);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&restarted_pool)
        .await
        .unwrap()
        .is_empty());
    restarted_pool.close().await;

    let final_pool = initialize(&db_path).await.expect("second cold reopen");
    let final_repository = SqliteProductionQueueRepository::new(final_pool.clone());
    assert_eq!(
        final_repository.list_deferred_starts().await.unwrap().len(),
        1
    );
    assert_eq!(
        final_repository
            .deferred_start_state(projects[0], &batch_ids[0])
            .await
            .unwrap()
            .unwrap()
            .0,
        "BLOCKED"
    );
    final_pool.close().await;
}
