//! DEV-031 no-GPU end-to-end coverage.
//!
//! This test uses the production queue service and SQLite repositories with an
//! isolated database.  It deliberately constructs an HTTP adapter but never
//! starts ComfyUI, submits a prompt, or drives a UI.

#[cfg(test)]
mod tests {
    use crate::application::generation_service::GenerationService;
    use crate::application::ports::{
        Clock, ComfyAdapterFactory, ComfyConnectionConfig, NoopTaskUpdateSink,
        ProductionQueueRepository,
    };
    use crate::application::production_queue_service::ProductionQueueService;
    use crate::application::task_recovery_service::TaskRecoveryService;
    use crate::domain::{
        ProductionBatch, ProductionBatchId, ProductionBatchItem, ProductionBatchItemId,
        ProductionBatchItemStatus, ProductionBatchStatus,
    };
    use crate::infrastructure::comfy::ComfyHttpAdapterFactory;
    use crate::infrastructure::database::{
        initialize,
        repositories::{
            test_support, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
            SqliteGenerationSnapshotRepository, SqliteProductionQueueRepository,
            SqliteProjectRepository, SqliteTaskRepository,
        },
    };
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use crate::infrastructure::time::SystemClock;
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn dev031_no_gpu_six_item_partial_resume_is_atomic_and_idempotent() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("dev031-no-gpu.db"))
            .await
            .unwrap();
        test_support::seed_task_dependencies(&pool).await;

        let repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
        let batch_id = ProductionBatchId::new();
        let now = Utc.with_ymd_and_hms(2026, 8, 17, 9, 0, 0).unwrap();
        let items = vec![
            item(
                &batch_id,
                0,
                ProductionBatchItemStatus::Succeeded,
                None,
                None,
            ),
            item(
                &batch_id,
                1,
                ProductionBatchItemStatus::Failed,
                None,
                Some("COMFY_TIMEOUT"),
            ),
            item(
                &batch_id,
                2,
                ProductionBatchItemStatus::Failed,
                None,
                Some("EXECUTION_ERROR"),
            ),
            item(
                &batch_id,
                3,
                ProductionBatchItemStatus::Cancelled,
                None,
                None,
            ),
            item(
                &batch_id,
                4,
                ProductionBatchItemStatus::Failed,
                None,
                Some("COMFY_OFFLINE"),
            ),
            item(
                &batch_id,
                5,
                ProductionBatchItemStatus::Succeeded,
                None,
                None,
            ),
        ];
        repository
            .insert(
                &ProductionBatch {
                    id: batch_id.clone(),
                    project_id: "project-1".to_owned(),
                    name: "DEV-031 no-GPU".to_owned(),
                    status: ProductionBatchStatus::Paused,
                    continue_on_failure: true,
                    archived_at: None,
                    created_at: now,
                    updated_at: now,
                },
                &items,
            )
            .await
            .unwrap();

        let queue = build_queue(&pool, repository.clone());
        let initial = queue
            .partial_resume_plan("project-1", batch_id.as_str())
            .await
            .unwrap();
        assert_eq!((initial.logical_total, initial.attempt_total), (6, 6));
        assert_eq!((initial.resolved, initial.auto_resumable), (2, 3));
        assert_eq!(initial.review_required, 1);
        assert!(initial.can_resume);

        let selected = [
            items[1].id.as_str(),
            items[3].id.as_str(),
            items[4].id.as_str(),
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let first = queue
            .partial_resume("project-1", batch_id.as_str(), &selected)
            .await
            .unwrap();
        assert_eq!((first.created_count, first.already_prepared_count), (3, 0));
        assert_eq!(first.detail.items.len(), 9);
        assert_eq!(first.detail.batch.status, ProductionBatchStatus::Paused);

        let repeated = queue
            .partial_resume("project-1", batch_id.as_str(), &selected)
            .await
            .unwrap();
        assert_eq!(
            (repeated.created_count, repeated.already_prepared_count),
            (0, 3)
        );
        assert_eq!(repeated.detail.items.len(), 9);

        for item_id in &first.created_item_ids {
            sqlx::query(
                "UPDATE production_batch_items
                 SET status = 'SUCCEEDED', error_code = NULL, error_message = NULL
                 WHERE id = ?",
            )
            .bind(item_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        let final_plan = queue
            .partial_resume_plan("project-1", batch_id.as_str())
            .await
            .unwrap();
        assert_eq!((final_plan.resolved, final_plan.review_required), (5, 1));
        assert_eq!(final_plan.attempt_total, 9);
        pool.close().await;
    }

    fn build_queue(
        pool: &sqlx::SqlitePool,
        repository: Arc<SqliteProductionQueueRepository>,
    ) -> ProductionQueueService {
        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let asset_store = Arc::new(FileSystemAssetStore::new());
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let comfy_adapter = ComfyHttpAdapterFactory
            .create(ComfyConnectionConfig::default())
            .expect("DEV031 no-GPU adapter should construct without a runtime");
        let generation_service = Arc::new(GenerationService::new(
            task_repository.clone(),
            snapshot_repository.clone(),
            definition_repository.clone(),
            comfy_adapter.clone(),
            project_repository.clone(),
            asset_store.clone(),
            asset_repository.clone(),
            clock.clone(),
        ));
        let task_recovery_service = Arc::new(TaskRecoveryService::new(
            task_repository.clone(),
            snapshot_repository,
            asset_repository,
            comfy_adapter,
            project_repository,
            asset_store,
            clock.clone(),
            Arc::new(NoopTaskUpdateSink),
        ));
        ProductionQueueService::new(
            repository.clone(),
            task_repository,
            definition_repository,
            generation_service,
            repository,
            task_recovery_service,
            clock,
        )
    }

    fn item(
        batch_id: &ProductionBatchId,
        ordinal: u32,
        status: ProductionBatchItemStatus,
        retry_of_item_id: Option<&str>,
        error_code: Option<&str>,
    ) -> ProductionBatchItem {
        let now = Utc.with_ymd_and_hms(2026, 8, 17, 9, 0, 0).unwrap();
        ProductionBatchItem {
            id: ProductionBatchItemId::new(),
            batch_id: batch_id.clone(),
            ordinal,
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values_json: json!({
                "prompt": {"type": "string", "value": "frozen"},
                "seed": {"type": "seed_fixed", "value": "42"},
                "reference_images": {"type": "image_assets", "assetIds": ["B", "A", "C"]}
            }),
            status,
            task_id: None,
            retry_of_item_id: retry_of_item_id.map(str::to_owned),
            error_code: error_code.map(str::to_owned),
            error_message: error_code.map(|code| format!("{code} failure")),
            created_at: now,
            updated_at: now,
        }
    }
}
