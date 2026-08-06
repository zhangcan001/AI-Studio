#[cfg(test)]
mod tests {
    use crate::application::generation_service::{
        CreateGenerationRequest, GenerationService, GenerationServiceError,
    };
    use crate::application::ports::{
        AssetRepository, Clock, ComfyAdapter, ComfyAdapterError, ComfyEventSubscription,
        ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyNodeOutput, ComfyOutputData,
        ComfyOutputFile, PromptSubmission, SystemStats, TaskRepository,
    };
    use crate::domain::{InputValue, SeedValue, Task, TaskEventType, TaskStatus};
    use crate::infrastructure::database::{
        initialize,
        repositories::{
            test_support, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
            SqliteGenerationSnapshotRepository, SqliteTaskRepository,
        },
        SqliteProjectRepository,
    };
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use async_trait::async_trait;
    use chrono::{DateTime, Duration, TimeZone, Utc};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use sqlx::SqlitePool;
    use std::collections::VecDeque;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tempfile::{tempdir, TempDir};

    const RECIPE_YAML: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_t2i/recipe.yaml"
    ));
    const WORKFLOW_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_t2i/workflow_api.json"
    ));

    #[derive(Clone, Copy)]
    enum FakeMode {
        Success { image_count: usize },
        Missing,
        InvalidImage,
        DownloadFailure,
    }

    #[derive(Clone)]
    struct FakeComfyAdapter {
        mode: FakeMode,
        events: Arc<Mutex<VecDeque<Result<Option<ComfyExecutionEvent>, ComfyAdapterError>>>>,
        prompt_id: Arc<Mutex<Option<String>>>,
        image_bytes: Vec<u8>,
    }

    struct FakeSubscription {
        events: VecDeque<Result<Option<ComfyExecutionEvent>, ComfyAdapterError>>,
        prompt_id: Arc<Mutex<Option<String>>>,
    }

    #[async_trait]
    impl ComfyEventSubscription for FakeSubscription {
        async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
            let event = self.events.pop_front().unwrap_or(Ok(None))?;
            let prompt_id = self.prompt_id.lock().unwrap().clone();
            Ok(event.map(|event| replace_prompt(event, prompt_id.as_deref())))
        }
    }

    #[async_trait]
    impl ComfyAdapter for FakeComfyAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            let outputs = if matches!(self.mode, FakeMode::Missing) {
                std::collections::BTreeMap::new()
            } else {
                let image_count = match self.mode {
                    FakeMode::Success { image_count } => image_count,
                    FakeMode::InvalidImage | FakeMode::DownloadFailure => 1,
                    FakeMode::Missing => 0,
                };
                {
                    let files = (0..image_count)
                        .map(|index| ComfyOutputFile {
                            filename: format!("ComfyUI_{:05}.png", index + 1),
                            subfolder: String::new(),
                            folder_type: "output".to_owned(),
                        })
                        .collect();
                    std::collections::BTreeMap::from([(
                        "9".to_owned(),
                        ComfyNodeOutput { images: files },
                    )])
                }
            };
            Ok(ComfyHistory {
                prompt_id: prompt_id.to_owned(),
                outputs,
            })
        }

        async fn download_output(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<ComfyOutputData, ComfyAdapterError> {
            if matches!(self.mode, FakeMode::DownloadFailure) {
                return Err(ComfyAdapterError::OutputDownload(
                    "HTTP 500 from /view".to_owned(),
                ));
            }
            let bytes = if matches!(self.mode, FakeMode::InvalidImage) {
                b"<html>not image</html>".to_vec()
            } else {
                self.image_bytes.clone()
            };
            Ok(ComfyOutputData {
                bytes,
                content_type: Some("image/png".to_owned()),
            })
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            prompt_id: &str,
            _workflow: Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            *self.prompt_id.lock().unwrap() = Some(prompt_id.to_owned());
            Ok(PromptSubmission {
                prompt_id: prompt_id.to_owned(),
                number: Some(1),
                node_errors: json!({}),
            })
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            Ok(Box::new(FakeSubscription {
                events: self.events.lock().unwrap().clone(),
                prompt_id: self.prompt_id.clone(),
            }))
        }
    }

    fn replace_prompt(event: ComfyExecutionEvent, prompt_id: Option<&str>) -> ComfyExecutionEvent {
        let prompt_id = prompt_id.unwrap_or_default().to_owned();
        match event {
            ComfyExecutionEvent::ExecutionStarted { .. } => {
                ComfyExecutionEvent::ExecutionStarted { prompt_id }
            }
            ComfyExecutionEvent::NodeStarted { node_id, .. } => {
                ComfyExecutionEvent::NodeStarted { prompt_id, node_id }
            }
            ComfyExecutionEvent::Progress {
                node_id,
                current,
                total,
                ..
            } => ComfyExecutionEvent::Progress {
                prompt_id,
                node_id,
                current,
                total,
            },
            ComfyExecutionEvent::ExecutionSucceeded { .. } => {
                ComfyExecutionEvent::ExecutionSucceeded { prompt_id }
            }
            other => other,
        }
    }

    #[derive(Clone)]
    struct FakeClock {
        values: Arc<Mutex<VecDeque<DateTime<Utc>>>>,
        last: DateTime<Utc>,
    }

    impl FakeClock {
        fn new(values: Vec<DateTime<Utc>>) -> Self {
            let last = values
                .last()
                .copied()
                .unwrap_or_else(|| Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
            Self {
                values: Arc::new(Mutex::new(values.into())),
                last,
            }
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> DateTime<Utc> {
            self.values.lock().unwrap().pop_front().unwrap_or(self.last)
        }
    }

    fn png_bytes() -> Vec<u8> {
        let image = RgbImage::from_pixel(2, 3, Rgb([1, 2, 3]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("png should encode");
        bytes.into_inner()
    }

    fn clock_values() -> Vec<DateTime<Utc>> {
        [
            (10, 0, 0),
            (10, 0, 1),
            (10, 0, 2),
            (10, 0, 5),
            (10, 0, 10),
            (10, 0, 11),
            (10, 0, 12),
            (10, 0, 13),
            (10, 0, 14),
            (10, 2, 0),
            (10, 2, 1),
            (10, 2, 2),
        ]
        .into_iter()
        .map(|(hour, minute, second)| {
            Utc.with_ymd_and_hms(2026, 1, 1, hour, minute, second)
                .unwrap()
        })
        .collect()
    }

    #[derive(Debug)]
    struct Run {
        _directory: TempDir,
        pool: SqlitePool,
        task: Task,
        assets: Vec<crate::domain::Asset>,
        events: Vec<crate::domain::StoredTaskEvent>,
        outcome: Result<(), GenerationServiceError>,
    }

    async fn run(mode: FakeMode) -> Run {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path().join("project");
        std::fs::create_dir_all(&root).expect("project root");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET root_path = ? WHERE id = 'project-1'")
            .bind(root.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("project root should update");
        sqlx::query(
            "UPDATE workflow_versions SET api_workflow_json = ? WHERE id = 'workflow-version-1'",
        )
        .bind(WORKFLOW_JSON)
        .execute(&pool)
        .await
        .expect("workflow should update");
        sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
            .bind(RECIPE_YAML)
            .execute(&pool)
            .await
            .expect("recipe should update");

        let events = vec![
            Ok(Some(ComfyExecutionEvent::ExecutionStarted {
                prompt_id: "CURRENT".to_owned(),
            })),
            Ok(Some(ComfyExecutionEvent::NodeStarted {
                prompt_id: "CURRENT".to_owned(),
                node_id: "9".to_owned(),
            })),
            Ok(Some(ComfyExecutionEvent::Progress {
                prompt_id: "CURRENT".to_owned(),
                node_id: Some("9".to_owned()),
                current: 1,
                total: 1,
            })),
            Ok(Some(ComfyExecutionEvent::ExecutionSucceeded {
                prompt_id: "CURRENT".to_owned(),
            })),
        ];
        let adapter = Arc::new(FakeComfyAdapter {
            mode,
            events: Arc::new(Mutex::new(events.into())),
            prompt_id: Arc::new(Mutex::new(None)),
            image_bytes: png_bytes(),
        });
        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let asset_store = Arc::new(FileSystemAssetStore::new());
        let clock = Arc::new(FakeClock::new(clock_values()));
        let service = GenerationService::new(
            task_repository.clone(),
            snapshot_repository,
            definition_repository,
            adapter,
            project_repository,
            asset_store,
            asset_repository.clone(),
            clock,
        );
        let request = CreateGenerationRequest {
            project_id: "project-1".to_owned(),
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values: std::collections::BTreeMap::from([
                ("prompt".to_owned(), InputValue::String("hello".to_owned())),
                ("steps".to_owned(), InputValue::Integer(20)),
                ("seed".to_owned(), InputValue::Seed(SeedValue::Fixed(123))),
            ]),
        };
        let result = service.execute(request).await;
        let task = task_repository
            .list_recent(1)
            .await
            .expect("task should be readable")
            .into_iter()
            .next()
            .expect("task should exist");
        let assets = asset_repository
            .list_by_source_task(&task.id)
            .await
            .expect("assets should be readable");
        let stored_events = task_repository
            .list_events(&task.id)
            .await
            .expect("events should be readable");
        Run {
            _directory: directory,
            pool,
            task,
            assets,
            events: stored_events,
            outcome: result.map(|_| ()),
        }
    }

    #[tokio::test]
    async fn backend_e2e_imports_output_and_reaches_succeeded() {
        let run = run(FakeMode::Success { image_count: 1 }).await;
        run.outcome.as_ref().expect("generation should succeed");
        assert_eq!(run.task.status, TaskStatus::Succeeded);
        assert_eq!(run.assets.len(), 1);
        let asset = &run.assets[0];
        assert_eq!(asset.category, "generated_image");
        assert_eq!(asset.mime_type, "image/png");
        assert_eq!((asset.width, asset.height), (2, 3));
        assert_eq!(
            asset.file_size,
            std::fs::metadata(&asset.storage_path).unwrap().len()
        );
        assert_eq!(
            asset.sha256,
            format!(
                "{:x}",
                Sha256::digest(std::fs::read(&asset.storage_path).unwrap())
            )
        );
        assert!(std::path::Path::new(&asset.storage_path).is_file());
        assert!(run
            .events
            .iter()
            .any(|event| event.event_type == TaskEventType::TaskCollecting));
        assert!(run
            .events
            .iter()
            .any(|event| event.event_type == TaskEventType::TaskSucceeded));
        assert_eq!(
            run.events
                .iter()
                .find(|event| event.event_type == TaskEventType::TaskCreated)
                .unwrap()
                .created_at,
            Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap()
        );
        assert_eq!(
            run.events
                .iter()
                .find(|event| event.event_type == TaskEventType::TaskCollecting)
                .unwrap()
                .created_at,
            Utc.with_ymd_and_hms(2026, 1, 1, 10, 2, 0).unwrap()
        );
        assert_eq!(
            run.task.finished_at,
            Some(Utc.with_ymd_and_hms(2026, 1, 1, 10, 2, 2).unwrap())
        );
        let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generation_snapshots")
            .fetch_one(&run.pool)
            .await
            .expect("snapshot count");
        assert_eq!(snapshot_count, 1);
    }

    #[tokio::test]
    async fn backend_e2e_multiple_images_create_ordered_assets() {
        let run = run(FakeMode::Success { image_count: 3 }).await;
        run.outcome.as_ref().expect("generation should succeed");
        assert_eq!(run.assets.len(), 3);
        assert_eq!(
            run.assets
                .iter()
                .map(|asset| asset.metadata_json["position"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[tokio::test]
    async fn backend_e2e_missing_required_output_fails_without_assets() {
        let run = run(FakeMode::Missing).await;
        assert!(matches!(
            run.outcome,
            Err(GenerationServiceError::OutputCollection(_))
        ));
        assert_eq!(run.task.status, TaskStatus::Failed);
        assert_eq!(run.task.error.as_ref().unwrap().code, "OUTPUT_MISSING");
        assert!(run.assets.is_empty());
    }

    #[tokio::test]
    async fn backend_e2e_invalid_image_fails_without_assets() {
        let run = run(FakeMode::InvalidImage).await;
        assert!(matches!(
            run.outcome,
            Err(GenerationServiceError::AssetImport(_))
        ));
        assert_eq!(run.task.status, TaskStatus::Failed);
        assert_eq!(
            run.task.error.as_ref().unwrap().code,
            "OUTPUT_IMPORT_FAILED"
        );
        assert!(run.assets.is_empty());
    }

    #[tokio::test]
    async fn backend_e2e_download_failure_fails_without_assets() {
        let run = run(FakeMode::DownloadFailure).await;
        assert!(matches!(
            run.outcome,
            Err(GenerationServiceError::OutputCollection(_))
        ));
        assert_eq!(run.task.status, TaskStatus::Failed);
        assert_eq!(
            run.task.error.as_ref().unwrap().code,
            "OUTPUT_DOWNLOAD_FAILED"
        );
        assert!(run.assets.is_empty());
    }

    #[allow(dead_code)]
    fn _path_type(path: PathBuf) -> PathBuf {
        path
    }

    #[allow(dead_code)]
    fn _duration() -> Duration {
        Duration::seconds(1)
    }
}
