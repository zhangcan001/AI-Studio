#[cfg(test)]
mod tests {
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::application::generation_service::{
        CreateGenerationRequest, GenerationService, GenerationServiceError, ReferenceManifest,
    };
    use crate::application::ports::{
        AssetRepository, AssetStore, Clock, ComfyAdapter, ComfyAdapterError,
        ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyImageUpload,
        ComfyNodeOutput, ComfyOutputData, ComfyOutputFile, ComfyUploadedImage,
        GenerationSnapshotRepository, PromptSubmission, RepositoryError, SystemStats,
        TaskRepository, TaskUpdateSink,
    };
    use crate::domain::{Asset, AssetId, SeedValue, Task, TaskEventType, TaskStatus};
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
    use std::collections::{BTreeMap, VecDeque};
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
        actions: Arc<Mutex<Vec<String>>>,
        upload_names: Arc<Mutex<Vec<String>>>,
        echo_upload_names: bool,
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

        async fn upload_image(
            &self,
            upload: ComfyImageUpload,
        ) -> Result<ComfyUploadedImage, ComfyAdapterError> {
            self.actions.lock().unwrap().push("upload_image".to_owned());
            let upload_name = upload.upload_name;
            let returned_name = if self.echo_upload_names {
                upload_name.clone()
            } else {
                "server_returned.png".to_owned()
            };
            self.upload_names.lock().unwrap().push(upload_name);
            Ok(ComfyUploadedImage {
                name: returned_name,
                subfolder: String::new(),
                folder_type: "input".to_owned(),
            })
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
                        ComfyNodeOutput {
                            images: files,
                            saved_results: Vec::new(),
                        },
                    )])
                }
            };
            Ok(ComfyHistory {
                prompt_id: prompt_id.to_owned(),
                status: Default::default(),
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
            self.actions
                .lock()
                .unwrap()
                .push("submit_workflow".to_owned());
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
            self.actions
                .lock()
                .unwrap()
                .push("subscribe_events".to_owned());
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
        published_statuses: Vec<String>,
        outcome: Result<(), GenerationServiceError>,
    }

    #[derive(Clone, Default)]
    struct RecordingSink {
        statuses: Arc<Mutex<Vec<String>>>,
    }

    impl TaskUpdateSink for RecordingSink {
        fn publish(&self, task: &Task) {
            self.statuses
                .lock()
                .unwrap()
                .push(task.status.as_str().to_owned());
        }
    }

    async fn run(mode: FakeMode) -> Run {
        run_mode(mode, false).await
    }

    async fn run_mode(mode: FakeMode, non_blocking: bool) -> Run {
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
            actions: Arc::new(Mutex::new(Vec::new())),
            upload_names: Arc::new(Mutex::new(Vec::new())),
            echo_upload_names: false,
        });
        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let asset_store = Arc::new(FileSystemAssetStore::new());
        let clock = Arc::new(FakeClock::new(clock_values()));
        let sink = Arc::new(RecordingSink::default());
        let service = Arc::new(
            GenerationService::new(
                task_repository.clone(),
                snapshot_repository,
                definition_repository,
                adapter,
                project_repository,
                asset_store,
                asset_repository.clone(),
                clock,
            )
            .with_task_update_sink(sink.clone()),
        );
        let request = CreateGenerationRequest {
            project_id: "project-1".to_owned(),
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values: std::collections::BTreeMap::from([
                (
                    "prompt".to_owned(),
                    GenerationInputValue::Text("hello".to_owned()),
                ),
                ("steps".to_owned(), GenerationInputValue::Integer(20)),
                (
                    "seed".to_owned(),
                    GenerationInputValue::Seed(SeedValue::Fixed(123)),
                ),
            ]),
            reference_manifest: None,
            submission_idempotency_key: None,
            submission_attempt: None,
            parent_task_id: None,
        };
        let result = if non_blocking {
            let returned_task = service
                .start_generation(request)
                .await
                .expect("start_generation should return a task");
            assert_eq!(returned_task.status, TaskStatus::Created);
            for _ in 0..100 {
                let current = task_repository
                    .list_recent("project-1", 1)
                    .await
                    .expect("task should be readable")
                    .into_iter()
                    .next();
                if current.is_some_and(|task| task.status.is_terminal()) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
                tokio::task::yield_now().await;
            }
            Ok(())
        } else {
            service.execute(request).await.map(|_| ())
        };
        let task = task_repository
            .list_recent("project-1", 1)
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
        let published_statuses = sink.statuses.lock().unwrap().clone();
        let run = Run {
            _directory: directory,
            pool,
            task,
            assets,
            events: stored_events,
            published_statuses,
            outcome: result.map(|_| ()),
        };
        run
    }

    async fn run_i2i_success() -> (Run, Arc<Mutex<Vec<String>>>) {
        let (run, actions, _upload_names) = run_image_input_success(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/simple_i2i/workflow_api.json"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/simple_i2i/recipe.yaml"
            )),
            BTreeMap::from([
                (
                    "prompt".to_owned(),
                    GenerationInputValue::Text("use the image".to_owned()),
                ),
                (
                    "reference_image".to_owned(),
                    GenerationInputValue::ImageAsset(AssetId::parse("ast_i2i_source").unwrap()),
                ),
                ("steps".to_owned(), GenerationInputValue::Integer(20)),
                (
                    "seed".to_owned(),
                    GenerationInputValue::Seed(SeedValue::Fixed(123)),
                ),
            ]),
            &[("ast_i2i_source", "reference.png")],
            None,
            false,
        )
        .await;
        (run, actions)
    }

    async fn run_multi_image_input_success() -> (Run, Arc<Mutex<Vec<String>>>) {
        let recipe = r#"
schema_version: 1
id: multi_i2i
name: Multi Image to Image
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_images:
    type: images
    label: Reference Images
    required: true
    min_items: 3
    max_items: 3
  seed:
    type: seed
    label: Seed
    default: random
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: reference_images
    target:
      node: "10"
      input: image
  - source: seed
    target:
      node: "3"
      input: seed
outputs:
  - id: generated_image
    type: image
    node: "9"
    required: true
"#;
        let workflow = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/simple_i2i/workflow_api.json"
        ));
        let (run, _actions, upload_names) = run_image_input_success(
            workflow,
            recipe,
            BTreeMap::from([
                (
                    "prompt".to_owned(),
                    GenerationInputValue::Text("use the ordered images".to_owned()),
                ),
                (
                    "reference_images".to_owned(),
                    GenerationInputValue::ImageAssets(vec![
                        AssetId::parse("ast_ref_b").unwrap(),
                        AssetId::parse("ast_ref_a").unwrap(),
                        AssetId::parse("ast_ref_c").unwrap(),
                    ]),
                ),
                (
                    "seed".to_owned(),
                    GenerationInputValue::Seed(SeedValue::Fixed(123)),
                ),
            ]),
            &[
                ("ast_ref_b", "b.png"),
                ("ast_ref_a", "a.png"),
                ("ast_ref_c", "c.png"),
            ],
            Some(ReferenceManifest {
                input_key: "reference_images".to_owned(),
                asset_ids: ["ast_ref_b", "ast_ref_a", "ast_ref_c"]
                    .into_iter()
                    .map(|id| AssetId::parse(id).unwrap())
                    .collect(),
            }),
            true,
        )
        .await;
        let names = upload_names.lock().unwrap().clone();
        assert_eq!(names.len(), 3);
        assert!(names[0].contains("ref_b_01.png"));
        assert!(names[1].contains("ref_a_02.png"));
        assert!(names[2].contains("ref_c_03.png"));
        (run, upload_names)
    }

    async fn run_image_input_success(
        workflow_json: &str,
        recipe_yaml: &str,
        values: BTreeMap<String, GenerationInputValue>,
        source_ids: &[(&str, &str)],
        reference_manifest: Option<ReferenceManifest>,
        echo_upload_names: bool,
    ) -> (Run, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<String>>>) {
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
        .bind(workflow_json)
        .execute(&pool)
        .await
        .expect("i2i workflow should update");
        sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
            .bind(recipe_yaml)
            .execute(&pool)
            .await
            .expect("i2i recipe should update");

        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let asset_store = Arc::new(FileSystemAssetStore::new());
        let source_bytes = png_bytes();
        let mut sources = Vec::with_capacity(source_ids.len());
        for (source_id, original_name) in source_ids {
            let source_id = AssetId::parse((*source_id).to_owned()).unwrap();
            let stored = asset_store
                .write_source_image(&root, &source_id, "png", &source_bytes)
                .await
                .expect("source image should store");
            sources.push(
                Asset::new_source_image(
                    source_id,
                    "project-1",
                    *original_name,
                    *original_name,
                    stored.path.to_string_lossy().to_string(),
                    format!("{:x}", Sha256::digest(&source_bytes)),
                    "image/png",
                    2,
                    3,
                    source_bytes.len() as u64,
                    json!({"source": "test"}),
                    Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap(),
                )
                .unwrap(),
            );
        }
        asset_repository.insert_many(&sources).await.unwrap();

        let actions = Arc::new(Mutex::new(Vec::new()));
        let upload_names = Arc::new(Mutex::new(Vec::new()));
        let adapter = Arc::new(FakeComfyAdapter {
            mode: FakeMode::Success { image_count: 1 },
            events: Arc::new(Mutex::new(
                vec![
                    Ok(Some(ComfyExecutionEvent::ExecutionStarted {
                        prompt_id: "CURRENT".to_owned(),
                    })),
                    Ok(Some(ComfyExecutionEvent::ExecutionSucceeded {
                        prompt_id: "CURRENT".to_owned(),
                    })),
                ]
                .into(),
            )),
            prompt_id: Arc::new(Mutex::new(None)),
            image_bytes: png_bytes(),
            actions: actions.clone(),
            upload_names: upload_names.clone(),
            echo_upload_names,
        });
        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
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
            values,
            reference_manifest,
            submission_idempotency_key: None,
            submission_attempt: None,
            parent_task_id: None,
        };
        let outcome = service.execute(request).await.map(|_| ());
        let task = task_repository
            .list_recent("project-1", 1)
            .await
            .unwrap()
            .remove(0);
        let assets = asset_repository
            .list_by_source_task(&task.id)
            .await
            .unwrap();
        let events = task_repository.list_events(&task.id).await.unwrap();
        let snapshot_repository = SqliteGenerationSnapshotRepository::new(pool.clone());
        let snapshot = snapshot_repository.find_by_task_id(&task.id).await.unwrap();
        assert!(snapshot.is_some(), "i2i must persist a snapshot");
        let run = Run {
            _directory: directory,
            pool,
            task,
            assets,
            events,
            published_statuses: Vec::new(),
            outcome,
        };
        (run, actions, upload_names)
    }

    #[tokio::test]
    async fn task_hook_failure_fails_task_before_any_comfy_submission() {
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

        let actions = Arc::new(Mutex::new(Vec::new()));
        let adapter = Arc::new(FakeComfyAdapter {
            mode: FakeMode::Success { image_count: 1 },
            events: Arc::new(Mutex::new(VecDeque::new())),
            prompt_id: Arc::new(Mutex::new(None)),
            image_bytes: png_bytes(),
            actions: actions.clone(),
            upload_names: Arc::new(Mutex::new(Vec::new())),
            echo_upload_names: false,
        });
        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let service = Arc::new(GenerationService::new(
            task_repository.clone(),
            snapshot_repository,
            definition_repository,
            adapter,
            project_repository,
            Arc::new(FileSystemAssetStore::new()),
            asset_repository,
            Arc::new(FakeClock::new(clock_values())),
        ));
        let result = service
            .start_generation_with_task_hook(
                CreateGenerationRequest {
                    project_id: "project-1".to_owned(),
                    workflow_version_id: "workflow-version-1".to_owned(),
                    recipe_id: "recipe-1".to_owned(),
                    values: BTreeMap::from([
                        (
                            "prompt".to_owned(),
                            GenerationInputValue::Text("hook test".to_owned()),
                        ),
                        ("steps".to_owned(), GenerationInputValue::Integer(20)),
                        (
                            "seed".to_owned(),
                            GenerationInputValue::Seed(SeedValue::Fixed(123)),
                        ),
                    ]),
                    reference_manifest: None,
                    submission_idempotency_key: None,
                    submission_attempt: None,
                    parent_task_id: None,
                },
                |_task| async { Err(RepositoryError::integrity("simulated Shot binding failure")) },
            )
            .await;
        assert!(matches!(
            result,
            Err(GenerationServiceError::TaskCreatedHook { .. })
        ));
        let task = task_repository
            .list_recent("project-1", 1)
            .await
            .unwrap()
            .remove(0);
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(
            task.error.as_ref().map(|error| error.code.as_str()),
            Some("TASK_HOOK_FAILED")
        );
        assert!(actions.lock().unwrap().is_empty());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM generation_snapshots")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
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
        assert!(asset
            .thumbnail_path
            .as_ref()
            .is_some_and(|path| std::path::Path::new(path).is_file()));
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
        let collecting_at = run
            .events
            .iter()
            .find(|event| event.event_type == TaskEventType::TaskCollecting)
            .unwrap()
            .created_at;
        assert!(collecting_at >= Utc.with_ymd_and_hms(2026, 1, 1, 10, 2, 2).unwrap());
        assert!(collecting_at <= run.task.finished_at.unwrap());
        let finished_at = run.task.finished_at.expect("succeeded task should finish");
        assert!(finished_at >= Utc.with_ymd_and_hms(2026, 1, 1, 10, 2, 2).unwrap());
        let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generation_snapshots")
            .fetch_one(&run.pool)
            .await
            .expect("snapshot count");
        assert_eq!(snapshot_count, 1);
    }

    #[tokio::test]
    async fn backend_i2i_mock_e2e_uploads_before_snapshot_and_post() {
        let (run, actions) = run_i2i_success().await;
        run.outcome.as_ref().expect("i2i generation should succeed");
        assert_eq!(run.task.status, TaskStatus::Succeeded);
        assert_eq!(run.assets.len(), 1);
        let actions = actions.lock().unwrap().clone();
        assert_eq!(
            actions,
            vec!["upload_image", "subscribe_events", "submit_workflow"]
        );

        let snapshot: (String, String) = sqlx::query_as(
            "SELECT user_inputs_json, resolved_inputs_json FROM generation_snapshots WHERE task_id = ?",
        )
        .bind(run.task.id.as_str())
        .fetch_one(&run.pool)
        .await
        .unwrap();
        let user_inputs: Value = serde_json::from_str(&snapshot.0).unwrap();
        let resolved_inputs: Value = serde_json::from_str(&snapshot.1).unwrap();
        assert_eq!(user_inputs["reference_image"]["type"], "image_asset");
        assert_eq!(user_inputs["reference_image"]["assetId"], "ast_i2i_source");
        assert_eq!(
            resolved_inputs["reference_image"]["assetId"],
            "ast_i2i_source"
        );
        assert_eq!(
            resolved_inputs["reference_image"]["comfy"]["name"],
            "server_returned.png"
        );
        assert_eq!(resolved_inputs["reference_image"]["comfy"]["type"], "input");
        assert!(!snapshot.0.contains("storage_path"));
        assert!(!snapshot.0.contains("reference.png"));
        assert!(!snapshot.1.contains("assets\\source"));
        assert!(!snapshot.1.contains("C:\\"));
    }

    #[tokio::test]
    async fn backend_ref2va_three_image_mock_e2e_preserves_b_a_c_compiled_order_and_snapshot() {
        let (run, upload_names) = run_multi_image_input_success().await;
        run.outcome
            .as_ref()
            .expect("multi-image generation should succeed");
        assert_eq!(run.task.status, TaskStatus::Succeeded);
        let snapshot: (String, String) = sqlx::query_as(
            "SELECT user_inputs_json, resolved_inputs_json FROM generation_snapshots WHERE task_id = ?",
        )
        .bind(run.task.id.as_str())
        .fetch_one(&run.pool)
        .await
        .unwrap();
        let user_inputs: Value = serde_json::from_str(&snapshot.0).unwrap();
        let resolved_inputs: Value = serde_json::from_str(&snapshot.1).unwrap();
        assert_eq!(
            user_inputs["reference_images"]["assetIds"],
            json!(["ast_ref_b", "ast_ref_a", "ast_ref_c"])
        );
        assert_eq!(
            resolved_inputs["reference_images"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value["assetId"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["ast_ref_b", "ast_ref_a", "ast_ref_c"]
        );
        let upload_names = upload_names.lock().unwrap();
        assert_eq!(upload_names.len(), 3);
        assert!(upload_names[0].ends_with("ref_b_01.png"));
        assert!(upload_names[1].ends_with("ref_a_02.png"));
        assert!(upload_names[2].ends_with("ref_c_03.png"));

        let workflow_json: String =
            sqlx::query_scalar("SELECT workflow_json FROM generation_snapshots WHERE task_id = ?")
                .bind(run.task.id.as_str())
                .fetch_one(&run.pool)
                .await
                .unwrap();
        let workflow: Value = serde_json::from_str(&workflow_json).unwrap();
        assert_eq!(
            workflow["10"]["inputs"]["image"],
            json!(upload_names.as_slice())
        );
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

    #[tokio::test]
    async fn start_generation_returns_created_before_background_execution_finishes() {
        let run = run_mode(FakeMode::Success { image_count: 1 }, true).await;
        run.outcome
            .as_ref()
            .expect("background generation should finish successfully");
        assert_eq!(run.task.status, TaskStatus::Succeeded);
        assert_eq!(run.assets.len(), 1);
    }

    #[tokio::test]
    async fn task_updates_include_persisted_lifecycle_states() {
        let run = run(FakeMode::Success { image_count: 1 }).await;
        run.outcome.as_ref().expect("generation should succeed");
        assert_eq!(
            run.published_statuses.first().map(String::as_str),
            Some("CREATED")
        );
        assert!(run
            .published_statuses
            .iter()
            .any(|status| status == "VALIDATING"));
        assert!(run
            .published_statuses
            .iter()
            .any(|status| status == "RUNNING"));
        assert!(run
            .published_statuses
            .iter()
            .any(|status| status == "COLLECTING"));
        assert_eq!(
            run.published_statuses.last().map(String::as_str),
            Some("SUCCEEDED")
        );
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
