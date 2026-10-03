//! Case 9: actual query work, not repository invocation counts or timing limits.
use super::*;
use ai_studio_lib::application::{
    generation_input_preparer::GenerationInputValue,
    product::run_facade::*,
    production_queue_service::{CreateProductionBatchItem, CreateProductionBatchRequest},
};
use ai_studio_lib::domain::Task;
use std::collections::BTreeMap;
use tracing_subscriber::layer::SubscriberExt;
#[path = "phase12_query_counter.rs"]
mod query_counter;

#[tokio::test]
async fn phase12_case9_parent_queries_bounded_and_projection_preserved() {
    let counter = query_counter::Counter::default();
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(counter.clone()))
        .unwrap();
    let dir = project_database::ProjectDatabase::new("query-work.db").await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(dir.database_path())
                .thread_name(|_| "phase12-query-worker".into()),
        )
        .await
        .unwrap();
    seed_database(&pool, dir.path()).await;
    let (facade, queue) = phase12_read_facade(&pool, dir.path());
    let repo = SqliteTaskRepository::new(pool.clone());
    let mut first = None;
    let mut counts = Vec::new();
    for n in 0..55 {
        let task = Task::new(
            PROJECT_ID,
            WORKFLOW_ID,
            WORKFLOW_VERSION_ID,
            RECIPE_ID,
            Utc::now() + chrono::Duration::days(if n == 0 { 100 } else { 0 }),
        );
        repo.create(&task, &task.created_event()).await.unwrap();
        if n == 0 {
            first = Some(task.id.as_str().to_owned());
        }
        if n == 4 {
            for i in 0..3 {
                let batch = queue
                    .create(CreateProductionBatchRequest {
                        project_id: PROJECT_ID.into(),
                        name: format!("Parent read fixture {i}"),
                        continue_on_failure: true,
                        items: vec![CreateProductionBatchItem {
                            workflow_version_id: WORKFLOW_VERSION_ID.into(),
                            recipe_id: RECIPE_ID.into(),
                            values: BTreeMap::from([(
                                "prompt".into(),
                                GenerationInputValue::Text("fixture".into()),
                            )]),
                        }],
                    })
                    .await
                    .unwrap();
                if i == 2 {
                    // Archived parents remain discoverable; no executor is started.
                    sqlx::query("UPDATE production_batch_items SET task_id=? WHERE id=?")
                        .bind(first.as_ref().unwrap())
                        .bind(batch.items[0].id.as_str())
                        .execute(&pool)
                        .await
                        .unwrap();
                    sqlx::query("UPDATE production_batches SET archived_at=? WHERE id=?")
                        .bind(Utc::now().to_rfc3339())
                        .bind(batch.batch.id.as_str())
                        .execute(&pool)
                        .await
                        .unwrap();
                }
            }
        }
        if n == 4 || n == 54 {
            counter.begin();
            let page = facade
                .list(PROJECT_ID, RunListFilter::All, None)
                .await
                .unwrap();
            let measured = counter.end();
            assert!(
                measured["totalSql"].as_u64().unwrap() > 0,
                "no false zero capture"
            );
            counts.push(measured["categories"]["QUEUE"].as_u64().unwrap());
            for item in &page.items {
                let live = facade.get(PROJECT_ID, item.run_ref.clone()).await.unwrap();
                assert_eq!(
                    serde_json::to_value(item).unwrap(),
                    serde_json::to_value(live).unwrap(),
                    "list read reuse preserves live projection"
                );
            }
            let parent = page
                .items
                .iter()
                .find(|run| run.run_ref.id == *first.as_ref().unwrap())
                .unwrap();
            assert!(
                parent.preferred_parent.is_some(),
                "archived parent is retained"
            );
            assert!(facade
                .get("prj_other", parent.run_ref.clone())
                .await
                .is_err());
        }
    }
    assert_eq!(
        counts[0], counts[1],
        "queue parent lookup work must not scale with standalone tasks"
    );
    pool.close().await;
    drop(facade);
    drop(queue);
    drop(repo);
    drop(pool);
    dir.close().await;
}
