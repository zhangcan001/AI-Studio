//! DEV-061B Agent C queue/recovery integration tests.
//!
//! These tests deliberately assemble the same public application seams used by
//! the app: package inspection/import, the production queue, generation, and
//! task recovery.  The database and project asset store are real; ComfyUI is a
//! small controlled boundary adapter.

use ai_studio_lib::application::{
    asset_video_prompt_service::AssetVideoPromptService,
    generation_service::GenerationService,
    h3_local_import_service::H3LocalImportService,
    ports::{
        AssetRepository, AssetStore, AssetStoreError, AssetVideoPromptRepository, Clock,
        ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth,
        ComfyHistory, ComfyInputUpload, ComfyNodeOutput, ComfyOutputData, ComfyOutputFile,
        ComfyOutputStream, ComfyQueueState, ComfySavedResult, GenerationDefinitionRepository,
        GenerationSnapshotRepository, NoopTaskUpdateSink, ProductionQueueRepository,
        ProjectRepository, PromptSubmission, ShotBatchRepository, StoredAssetFile, SystemStats,
        TaskRepository,
    },
    production_package_inspector::{ProductionPackageInspector, PRODUCTION_PACKAGE_TYPE},
    production_package_service::{ProductionPackageH3Config, ProductionPackageService},
    production_queue_service::{
        ProductionPartialResumePlan, ProductionQueueError, ProductionQueueService,
    },
    source_asset_import_service::SourceAssetImportService,
    task_recovery_service::TaskRecoveryService,
};
use ai_studio_lib::domain::{ProductionBatchItemStatus, ProductionBatchStatus};
use ai_studio_lib::infrastructure::{
    database::{
        initialize, SqliteAssetRepository, SqliteAssetVideoPromptRepository,
        SqliteGenerationDefinitionRepository, SqliteGenerationSnapshotRepository,
        SqliteProductionQueueRepository, SqliteProjectRepository, SqliteTaskRepository,
    },
    filesystem::FileSystemAssetStore,
    time::SystemClock,
};
use async_trait::async_trait;
use chrono::Utc;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tempfile::tempdir;

const PROJECT_ID: &str = "prj_default";
const WORKFLOW_ID: &str = "dev061b-queue-workflow";
const WORKFLOW_VERSION_ID: &str = "dev061b-queue-workflow-version";
const RECIPE_ID: &str = "dev061b-queue-recipe";
const CREATED_AT: &str = "2026-08-28T00:00:00Z";

const WORKFLOW_JSON: &str = r#"{"6":{"inputs":{"text":""},"class_type":"CLIPTextEncode"},"10":{"inputs":{"image":""},"class_type":"LoadImage"},"11":{"inputs":{},"class_type":"SaveVideo"}}"#;

const RECIPE_YAML: &str = r#"
schema_version: 1
id: dev061b-queue-recipe
name: DEV-061B Queue Fixture Recipe
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  first_frame:
    type: image
    label: First frame
    required: false
  width:
    type: integer
    label: Width
    required: false
  height:
    type: integer
    label: Height
    required: false
  duration_seconds:
    type: integer
    label: Duration
    required: false
  seed:
    type: seed
    label: Seed
    default: random
    min: 0
    max: 4294967295
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: first_frame
    target:
      node: "10"
      input: image
outputs:
  - id: generated_video
    type: video
    node: "11"
    required: true
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComfyBehavior {
    Offline,
    Success,
    DisconnectThenHistorySuccess,
}

#[derive(Clone)]
struct ControlledComfy {
    behavior: Arc<Mutex<ComfyBehavior>>,
    submit_calls: Arc<AtomicUsize>,
    uploaded_images: Arc<Mutex<Vec<Vec<u8>>>>,
    last_prompt_id: Arc<Mutex<Option<String>>>,
}

impl ControlledComfy {
    fn new(behavior: ComfyBehavior) -> Self {
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            submit_calls: Arc::new(AtomicUsize::new(0)),
            uploaded_images: Arc::new(Mutex::new(Vec::new())),
            last_prompt_id: Arc::new(Mutex::new(None)),
        }
    }

    fn set_behavior(&self, behavior: ComfyBehavior) {
        *self
            .behavior
            .lock()
            .expect("Comfy behavior mutex should work") = behavior;
    }

    fn uploaded_images(&self) -> Vec<Vec<u8>> {
        self.uploaded_images
            .lock()
            .expect("uploaded image mutex should work")
            .clone()
    }
}

struct ControlledEvents {
    behavior: Arc<Mutex<ComfyBehavior>>,
    last_prompt_id: Arc<Mutex<Option<String>>>,
    started: bool,
}

struct ControlledVideoStream {
    sent: bool,
}

const CONTROLLED_VIDEO_HEX: &str = concat!(
    "000000246674797069736f6d0000020069736f6d69736f3669736f32617663316d703431000002ef6d6f6f760000006c",
    "6d766864000000000000000000000000000003e800000000000100000100000000000000000000000001000000000000",
    "000000000000000000010000000000000000000000000000400000000000000000000000000000000000000000000000",
    "0000000000000002000001f17472616b0000005c746b6864000000030000000000000000000000010000000000000000",
    "000000000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000",
    "4000000000020000000200000000018d6d646961000000206d6468640000000000000000000000000000320000000000",
    "55c400000000002d68646c72000000000000000076696465000000000000000000000000566964656f48616e646c6572",
    "00000001386d696e6600000014766d68640000000100000000000000000000002464696e660000001c64726566000000",
    "00000000010000000c75726c2000000001000000f87374626c000000ac7374736400000000000000010000009c617663",
    "31000000000000000100000000000000000000000000000000000200020048000000480000000000000001154c617663",
    "36322e32392e313031206c696278323634000000000000000000000018ffff00000036617663430164000affe1001967",
    "64000aacd95f8888c044000003000400000300c83c48965801000668ebe3cb22c0fdf8f8000000001070617370000000",
    "01000000010000001073747473000000000000000000000010737473630000000000000000000000147374737a000000",
    "000000000000000000000000107374636f0000000000000000000000286d766578000000207472657800000000000000",
    "010000000100000000000000000000000000000062756474610000005a6d657461000000000000002168646c72000000",
    "00000000006d6469726170706c0000000000000000000000002d696c737400000025a9746f6f0000001d646174610000",
    "0001000000004c61766636322e31332e313031000000886d6f6f66000000106d66686400000000000000010000007074",
    "72616600000024746668640000003900000001000000000000031300000200000002ca01010000000000147466647401",
    "0000000000000000000000000000307472756e00000a05000000030000009002000000000002ca000004000000000c00",
    "0006000000000c00000200000002ea6d646174000002ae0605ffffaadc45e9bde6d948b7962cd820d923eeef78323634",
    "202d20636f7265203136352072333232332030343830636230202d20482e3236342f4d5045472d342041564320636f64",
    "6563202d20436f70796c65667420323030332d32303235202d20687474703a2f2f7777772e766964656f6c616e2e6f72",
    "672f783236342e68746d6c202d206f7074696f6e733a2063616261633d31207265663d33206465626c6f636b3d313a30",
    "3a3020616e616c7973653d3078333a3078313133206d653d686578207375626d653d37207073793d31207073795f7264",
    "3d312e30303a302e3030206d697865645f7265663d31206d655f72616e67653d3136206368726f6d615f6d653d312074",
    "72656c6c69733d31203878386463743d312063716d3d3020646561647a6f6e653d32312c313120666173745f70736b69",
    "703d31206368726f6d615f71705f6f66667365743d2d3220746872656164733d31206c6f6f6b61686561645f74687265",
    "6164733d3120736c696365645f746872656164733d30206e723d3020646563696d6174653d3120696e7465726c616365",
    "643d3020626c757261795f636f6d7061743d3020636f6e73747261696e65645f696e7472613d3020626672616d65733d",
    "3320625f707972616d69643d3220625f61646170743d3120625f626961733d30206469726563743d3120776569676874",
    "623d31206f70656e5f676f703d3020776569676874703d32206b6579696e743d323530206b6579696e745f6d696e3d32",
    "35207363656e656375743d343020696e7472615f726566726573683d302072635f6c6f6f6b61686561643d3430207263",
    "3d637266206d62747265653d31206372663d32332e302071636f6d703d302e36302071706d696e3d302071706d61783d",
    "3639207170737465703d342069705f726174696f3d312e34302061713d313a312e30300080000000146588840033fffe",
    "df32f814cdc589cecc805c9e9700000008419a226c42bffec000000008019e41790affc481000000436d667261000000",
    "2b746672610100000000000001000000000000000100000000000004000000000000000313010101000000106d66726f",
    "0000000000000043",
);

#[async_trait]
impl ComfyOutputStream for ControlledVideoStream {
    fn content_type(&self) -> Option<&str> {
        Some("video/mp4")
    }

    fn content_length(&self) -> Option<u64> {
        Some((CONTROLLED_VIDEO_HEX.len() / 2) as u64)
    }

    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ComfyAdapterError> {
        if self.sent {
            return Ok(None);
        }
        self.sent = true;
        Ok(Some(decode_hex(CONTROLLED_VIDEO_HEX)))
    }
}

fn decode_hex(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16).expect("valid hex") as u8;
            let low = (pair[1] as char).to_digit(16).expect("valid hex") as u8;
            (high << 4) | low
        })
        .collect()
}

#[async_trait]
impl ComfyEventSubscription for ControlledEvents {
    async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
        let behavior = *self
            .behavior
            .lock()
            .expect("Comfy behavior mutex should work");
        match behavior {
            ComfyBehavior::DisconnectThenHistorySuccess => {
                *self
                    .behavior
                    .lock()
                    .expect("Comfy behavior mutex should work") = ComfyBehavior::Success;
                Err(ComfyAdapterError::StreamDisconnected(
                    "DEV-061B controlled stream disconnect".to_owned(),
                ))
            }
            ComfyBehavior::Success => {
                let prompt_id = self
                    .last_prompt_id
                    .lock()
                    .expect("prompt id mutex should work")
                    .clone()
                    .expect("submit should precede execution events");
                if self.started {
                    Ok(Some(ComfyExecutionEvent::ExecutionSucceeded { prompt_id }))
                } else {
                    self.started = true;
                    Ok(Some(ComfyExecutionEvent::ExecutionStarted { prompt_id }))
                }
            }
            ComfyBehavior::Offline => Err(ComfyAdapterError::Offline(
                "DEV-061B controlled Comfy is offline".to_owned(),
            )),
        }
    }
}

#[async_trait]
impl ComfyAdapter for ControlledComfy {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        if *self
            .behavior
            .lock()
            .expect("Comfy behavior mutex should work")
            == ComfyBehavior::Offline
        {
            return Err(ComfyAdapterError::Offline(
                "DEV-061B controlled Comfy is offline".to_owned(),
            ));
        }
        Ok(ComfyHealth {
            system: SystemStats {
                comfyui_version: Some("dev061b-controlled".to_owned()),
                python_version: None,
                os: None,
                ram_total: None,
                ram_free: None,
                devices: Vec::new(),
            },
        })
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        self.health_check().await.map(|health| health.system)
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        Ok(json!({}))
    }

    async fn upload_input_file(
        &self,
        mut upload: ComfyInputUpload,
    ) -> Result<ai_studio_lib::application::ports::ComfyUploadedInput, ComfyAdapterError> {
        let mut bytes = Vec::new();
        while let Some(chunk) = upload
            .stream
            .next_chunk()
            .await
            .map_err(ComfyAdapterError::InputUpload)?
        {
            bytes.extend(chunk);
        }
        self.uploaded_images
            .lock()
            .expect("uploaded image mutex should work")
            .push(bytes);
        Ok(ai_studio_lib::application::ports::ComfyUploadedInput {
            name: upload.filename,
            subfolder: String::new(),
            folder_type: "input".to_owned(),
        })
    }

    async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        let behavior = *self
            .behavior
            .lock()
            .expect("Comfy behavior mutex should work");
        if matches!(behavior, ComfyBehavior::Success) {
            return Ok(ComfyHistory {
                prompt_id: prompt_id.to_owned(),
                status: ai_studio_lib::application::ports::ComfyHistoryStatus {
                    status_str: Some("success".to_owned()),
                    completed: Some(true),
                    messages: None,
                },
                outputs: std::collections::BTreeMap::from([(
                    "11".to_owned(),
                    ComfyNodeOutput {
                        images: vec![ComfyOutputFile {
                            filename: "ComfyUI_00001.mp4".to_owned(),
                            subfolder: String::new(),
                            folder_type: "output".to_owned(),
                        }],
                        saved_results: vec![ComfySavedResult {
                            file: ComfyOutputFile {
                                filename: "ComfyUI_00001.mp4".to_owned(),
                                subfolder: String::new(),
                                folder_type: "output".to_owned(),
                            },
                            animated: Some(true),
                        }],
                    },
                )]),
            });
        }
        Err(ComfyAdapterError::HistoryNotFound(prompt_id.to_owned()))
    }

    async fn download_output(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        Err(ComfyAdapterError::OutputDownload(file.filename.clone()))
    }

    async fn open_output_stream(
        &self,
        _file: &ComfyOutputFile,
    ) -> Result<Box<dyn ComfyOutputStream>, ComfyAdapterError> {
        Ok(Box::new(ControlledVideoStream { sent: false }))
    }

    async fn submit_workflow(
        &self,
        _client_id: &str,
        prompt_id: &str,
        _workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        self.submit_calls.fetch_add(1, Ordering::SeqCst);
        let behavior = *self
            .behavior
            .lock()
            .expect("Comfy behavior mutex should work");
        if behavior == ComfyBehavior::Offline {
            return Err(ComfyAdapterError::Offline(
                "DEV-061B controlled Comfy is offline".to_owned(),
            ));
        }
        *self
            .last_prompt_id
            .lock()
            .expect("prompt id mutex should work") = Some(prompt_id.to_owned());
        Ok(PromptSubmission {
            prompt_id: prompt_id.to_owned(),
            number: Some(1),
            node_errors: json!({}),
        })
    }

    async fn get_queue_state(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
        Ok(ComfyQueueState {
            running_prompt_ids: Vec::new(),
            pending_prompt_ids: Vec::new(),
        })
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Ok(Box::new(ControlledEvents {
            behavior: Arc::clone(&self.behavior),
            last_prompt_id: Arc::clone(&self.last_prompt_id),
            started: false,
        }))
    }
}

#[derive(Clone)]
struct GuardedAssetStore {
    inner: FileSystemAssetStore,
    package_root: PathBuf,
    reads: Arc<Mutex<Vec<PathBuf>>>,
}

impl GuardedAssetStore {
    fn new(package_root: &Path) -> (Self, Arc<Mutex<Vec<PathBuf>>>) {
        let reads = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                inner: FileSystemAssetStore::new(),
                package_root: package_root.to_path_buf(),
                reads: Arc::clone(&reads),
            },
            reads,
        )
    }
}

#[async_trait]
impl AssetStore for GuardedAssetStore {
    async fn write_image(
        &self,
        project_root: &Path,
        asset_id: &ai_studio_lib::domain::AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        self.inner
            .write_image(project_root, asset_id, extension, bytes)
            .await
    }

    async fn write_source_image(
        &self,
        project_root: &Path,
        asset_id: &ai_studio_lib::domain::AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        self.inner
            .write_source_image(project_root, asset_id, extension, bytes)
            .await
    }

    async fn write_thumbnail(
        &self,
        project_root: &Path,
        asset_id: &ai_studio_lib::domain::AssetId,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        self.inner
            .write_thumbnail(project_root, asset_id, bytes)
            .await
    }

    async fn begin_video_write(
        &self,
        project_root: &Path,
        asset_id: &ai_studio_lib::domain::AssetId,
        extension: &str,
    ) -> Result<Box<dyn ai_studio_lib::application::ports::AssetWriteSession>, AssetStoreError>
    {
        self.inner
            .begin_video_write(project_root, asset_id, extension)
            .await
    }

    async fn delete(&self, path: &Path) -> Result<(), AssetStoreError> {
        self.inner.delete(path).await
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>, AssetStoreError> {
        self.reads
            .lock()
            .expect("asset read mutex should work")
            .push(path.to_path_buf());
        if path.starts_with(&self.package_root) {
            return Err(AssetStoreError::Read(format!(
                "DEV-061B retry attempted to read package disk: {}",
                path.display()
            )));
        }
        self.inner.read(path).await
    }
}

struct Services {
    package: ProductionPackageService,
    queue: Arc<ProductionQueueService>,
    asset_reads: Arc<Mutex<Vec<PathBuf>>>,
}

fn build_services(pool: &SqlitePool, comfy: Arc<ControlledComfy>, package_root: &Path) -> Services {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let project_repository: Arc<dyn ProjectRepository> =
        Arc::new(SqliteProjectRepository::new(pool.clone()));
    let asset_repository: Arc<dyn AssetRepository> =
        Arc::new(SqliteAssetRepository::new(pool.clone()));
    let (asset_store_impl, asset_reads) = GuardedAssetStore::new(package_root);
    let asset_store: Arc<dyn AssetStore> = Arc::new(asset_store_impl);
    let task_repository: Arc<dyn TaskRepository> =
        Arc::new(SqliteTaskRepository::new(pool.clone()));
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> =
        Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
    let definition_repository: Arc<dyn GenerationDefinitionRepository> =
        Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let comfy_adapter: Arc<dyn ComfyAdapter> = comfy;

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
        asset_repository.clone(),
        comfy_adapter,
        project_repository.clone(),
        asset_store.clone(),
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));

    let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let production_queue_repository: Arc<dyn ProductionQueueRepository> = queue_repository.clone();
    let shot_batch_repository: Arc<dyn ShotBatchRepository> = queue_repository.clone();
    let queue = Arc::new(ProductionQueueService::new(
        production_queue_repository,
        task_repository,
        definition_repository,
        generation_service,
        shot_batch_repository,
        task_recovery_service,
        clock.clone(),
    ));

    let asset_video_prompt_repository: Arc<dyn AssetVideoPromptRepository> =
        Arc::new(SqliteAssetVideoPromptRepository::new(pool.clone()));
    let asset_video_prompt_service = Arc::new(AssetVideoPromptService::new(
        asset_video_prompt_repository,
        asset_repository.clone(),
        clock.clone(),
    ));
    let source_asset_import_service = Arc::new(SourceAssetImportService::new(
        project_repository,
        asset_store,
        asset_repository,
        clock.clone(),
    ));
    let h3_local_import_service = Arc::new(H3LocalImportService::new(
        source_asset_import_service.clone(),
        asset_video_prompt_service,
        Arc::clone(&queue),
        clock.clone(),
    ));

    let package = ProductionPackageService::new(
        ProductionPackageInspector::new(),
        h3_local_import_service,
        source_asset_import_service,
        queue.clone(),
        ProductionPackageH3Config {
            workflow_version_id: WORKFLOW_VERSION_ID.to_owned(),
            recipe_id: RECIPE_ID.to_owned(),
            fl2va_workflow_version_id: None,
            fl2va_recipe_id: None,
            ref2va_workflow_version_id: None,
            ref2va_recipe_id: None,
            quality_profile: None,
            quality_recipes: Vec::new(),
        },
        clock,
    );
    Services {
        package,
        queue,
        asset_reads,
    }
}

async fn seed_database(pool: &SqlitePool, project_root: &Path) {
    SqliteProjectRepository::new(pool.clone())
        .ensure_default_project(
            PROJECT_ID,
            "DEV-061B queue project",
            &project_root.to_path_buf(),
            Utc::now(),
        )
        .await
        .expect("project fixture should be created");
    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES (?, ?, 'video', 'fl2va', NULL, ?, ?)",
    )
    .bind(WORKFLOW_ID)
    .bind("DEV-061B queue workflow")
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("workflow fixture should be inserted");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES (?, ?, '1', ?, 'dev061b-queue-workflow-sha', ?)",
    )
    .bind(WORKFLOW_VERSION_ID)
    .bind(WORKFLOW_ID)
    .bind(WORKFLOW_JSON)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("workflow version fixture should be inserted");
    sqlx::query(
        "INSERT INTO recipes
         (id, workflow_version_id, version, schema_version, recipe_yaml,
          recipe_sha256, created_at)
         VALUES (?, ?, '1', 1, ?, 'dev061b-queue-recipe-sha', ?)",
    )
    .bind(RECIPE_ID)
    .bind(WORKFLOW_VERSION_ID)
    .bind(RECIPE_YAML)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .expect("recipe fixture should be inserted");
    sqlx::query("UPDATE workflows SET current_version_id = ? WHERE id = ?")
        .bind(WORKFLOW_VERSION_ID)
        .bind(WORKFLOW_ID)
        .execute(pool)
        .await
        .expect("current workflow version should be set");
}

fn png_bytes(color: [u8; 4]) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba(color)));
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .expect("PNG fixture should encode");
    output.into_inner()
}

async fn write_package(root: &Path, original_png: &[u8]) {
    fs::create_dir_all(root.join("frames")).expect("package frames should exist");
    fs::write(root.join("frames/first.png"), original_png).expect("first frame should write");
    let manifest = json!({
        "schemaVersion": 1,
        "packageType": PRODUCTION_PACKAGE_TYPE,
        "name": "DEV-061B queue recovery package",
        "defaults": {"durationSeconds": 7, "width": 864, "height": 480},
        "items": [{
            "id": "DEV061B-QUEUE-001",
            "name": "Frozen first frame",
            "videoPrompt": "DEV-061B original frozen prompt",
            "mode": "I2V",
            "firstFrame": "frames/first.png"
        }]
    });
    tokio::fs::write(
        root.join("production-package.json"),
        serde_json::to_vec_pretty(&manifest).expect("package manifest should serialize"),
    )
    .await
    .expect("package manifest should write");
}

async fn create_package_batch(services: &Services, package_root: &Path) -> (String, String, Value) {
    let (session_id, inspection) = services
        .package
        .inspect_session(PROJECT_ID, package_root.to_path_buf())
        .await
        .expect("package should inspect");
    assert_eq!(inspection.items[0].mode, "FL2VA_IMAGE_TO_VIDEO");
    assert_eq!(inspection.items[0].duration_seconds, 7);
    assert_eq!(inspection.items[0].width, 864);
    assert_eq!(inspection.items[0].height, 480);
    let item_id = inspection.items[0].id.clone();
    let result = services
        .package
        .create_batches(&session_id, &[item_id])
        .await
        .expect("package batch should be created");
    assert_eq!(result.batch_count, 1);
    assert_eq!(result.item_count, 1);
    assert!(!result.auto_started);
    let batch_id = result.batches[0].batch_id.clone();
    let values = services
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("created batch should be readable")
        .items[0]
        .values_json
        .clone();
    (
        batch_id,
        result.item_mappings[0].batch_item_id.clone(),
        values,
    )
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    let query = match table {
        "tasks" => "SELECT COUNT(*) FROM tasks WHERE project_id = ?",
        "batches" => "SELECT COUNT(*) FROM production_batches",
        "items" => "SELECT COUNT(*) FROM production_batch_items",
        other => panic!("unsupported count table {other}"),
    };
    let mut query = sqlx::query_scalar::<_, i64>(query);
    if table == "tasks" {
        query = query.bind(PROJECT_ID);
    }
    query
        .fetch_one(pool)
        .await
        .expect("count query should work")
}

async fn wait_for_item_status(
    pool: &SqlitePool,
    item_id: &str,
    expected: ProductionBatchItemStatus,
) {
    for _ in 0..160 {
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_batch_items WHERE id = ?",
        )
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("item status should be readable");
        if status == expected.as_str() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let actual = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT status, error_code, error_message
         FROM production_batch_items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("item status should remain readable for diagnostics");
    panic!(
        "item {item_id} did not reach {}; actual={actual:?}",
        expected.as_str()
    );
}

async fn wait_for_batch_status(pool: &SqlitePool, batch_id: &str, expected: ProductionBatchStatus) {
    for _ in 0..160 {
        let status =
            sqlx::query_scalar::<_, String>("SELECT status FROM production_batches WHERE id = ?")
                .bind(batch_id)
                .fetch_one(pool)
                .await
                .expect("batch status should be readable");
        if status == expected.as_str() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let actual =
        sqlx::query_scalar::<_, String>("SELECT status FROM production_batches WHERE id = ?")
            .bind(batch_id)
            .fetch_one(pool)
            .await
            .expect("batch status should remain readable for diagnostics");
    let items = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
        "SELECT id, status, error_code, error_message
         FROM production_batch_items WHERE batch_id = ? ORDER BY ordinal",
    )
    .bind(batch_id)
    .fetch_all(pool)
    .await
    .expect("batch items should remain readable for diagnostics");
    panic!(
        "batch {batch_id} did not reach {}; actual={actual}, items={items:?}",
        expected.as_str()
    );
}

async fn wait_for_event(pool: &SqlitePool, event_type: &str) {
    for _ in 0..160 {
        let events =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM task_events WHERE event_type = ?")
                .bind(event_type)
                .fetch_one(pool)
                .await
                .expect("task event count should be readable");
        if events > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("task event {event_type} was not recorded");
}

fn assert_plan_is_resolved(plan: &ProductionPartialResumePlan) {
    assert_eq!(plan.logical_total, 1);
    assert_eq!(plan.resolved, 1);
    assert_eq!(plan.auto_resumable, 0);
    assert_eq!(plan.review_required, 0);
    assert_eq!(plan.entries[0].status, "RESOLVED");
    assert_eq!(plan.entries[0].eligibility, "NONE");
}

#[tokio::test]
async fn admitted_start_seams_do_not_reacquire_the_existing_admission_gate() {
    let directory = tempdir().expect("temporary workspace should exist");
    let pool = initialize(&directory.path().join("dev078-admitted-start.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;
    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    write_package(&package_root, &png_bytes([10, 20, 30, 255])).await;

    let comfy = Arc::new(ControlledComfy::new(ComfyBehavior::Offline));
    let services = build_services(&pool, Arc::clone(&comfy), &package_root);
    let (batch_id, _, _) = create_package_batch(&services, &package_root).await;

    let gate = services
        .queue
        .acquire_runtime_configuration_admission()
        .await;
    let detail = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        services.queue.inspect_start_admitted(PROJECT_ID, &batch_id),
    )
    .await
    .expect("inspect_start_admitted must not wait for the already-held gate")
    .expect("the ready batch should be admitted");
    assert_eq!(detail.batch.id.as_str(), batch_id);
    assert_eq!(detail.batch.status, ProductionBatchStatus::Ready);

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        services.queue.commit_start_admitted(&detail),
    )
    .await
    .expect("commit_start_admitted must not wait for the already-held gate")
    .expect("an admitted ready batch should commit");

    drop(gate);
}

#[tokio::test]
async fn admitted_start_commit_does_not_spawn_after_a_zero_row_batch_update() {
    let directory = tempdir().expect("temporary workspace should exist");
    let pool = initialize(&directory.path().join("dev078-stale-commit.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;
    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    write_package(&package_root, &png_bytes([40, 50, 60, 255])).await;

    let comfy = Arc::new(ControlledComfy::new(ComfyBehavior::Offline));
    let services = build_services(&pool, Arc::clone(&comfy), &package_root);
    let (batch_id, _, _) = create_package_batch(&services, &package_root).await;
    let detail = services
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("the ready batch should be readable");

    let mut stale_detail = detail.clone();
    stale_detail.batch.project_id = "prj_missing".to_owned();
    let error = services
        .queue
        .commit_start_admitted(&stale_detail)
        .await
        .expect_err("a zero-row batch update must fail closed");
    assert!(matches!(
        error,
        ProductionQueueError::InvalidState(message)
            if message == "production batch start commit did not update a batch"
    ));

    let unchanged = services
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("the original batch should remain readable");
    assert_eq!(unchanged.batch.status, ProductionBatchStatus::Ready);
    assert_eq!(count(&pool, "tasks").await, 0);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn restart_keeps_package_batch_idle_then_explicit_start_recovers_offline_and_retries_frozen_asset(
) {
    let directory = tempdir().expect("temporary workspace should exist");
    let pool = initialize(&directory.path().join("dev061b-restart.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;
    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    let original_png = png_bytes([10, 20, 30, 255]);
    write_package(&package_root, &original_png).await;

    let comfy = Arc::new(ControlledComfy::new(ComfyBehavior::Offline));
    let first_services = build_services(&pool, Arc::clone(&comfy), &package_root);
    let (batch_id, source_item_id, frozen_values) =
        create_package_batch(&first_services, &package_root).await;
    assert_eq!(count(&pool, "batches").await, 1);
    assert_eq!(count(&pool, "items").await, 1);
    assert_eq!(count(&pool, "tasks").await, 0);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);
    drop(first_services);

    // The external package file is changed after import.  Its imported Asset
    // and the queue's frozen values must remain the source of truth.
    let replacement_png = png_bytes([240, 30, 40, 255]);
    fs::write(package_root.join("frames/first.png"), &replacement_png)
        .expect("replacement package PNG should write");
    assert_ne!(
        fs::read(package_root.join("frames/first.png")).expect("replacement should read"),
        original_png
    );

    let services = build_services(&pool, Arc::clone(&comfy), &package_root);
    let idle_detail = services
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("batch should survive service recreation");
    assert_eq!(idle_detail.batch.status, ProductionBatchStatus::Ready);
    assert_eq!(
        idle_detail.items[0].status,
        ProductionBatchItemStatus::Pending
    );
    assert_eq!(idle_detail.items[0].values_json, frozen_values);
    assert_eq!(count(&pool, "tasks").await, 0);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 0);

    services
        .queue
        .start_for_test(PROJECT_ID, &batch_id)
        .await
        .expect("explicit queue start should succeed");
    wait_for_item_status(&pool, &source_item_id, ProductionBatchItemStatus::Failed).await;
    wait_for_batch_status(&pool, &batch_id, ProductionBatchStatus::Paused).await;
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 1);
    let failed = services
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("failed batch should be readable");
    assert_eq!(failed.items[0].error_code.as_deref(), Some("COMFY_OFFLINE"));
    assert_eq!(failed.items[0].values_json, frozen_values);

    let plan = services
        .queue
        .partial_resume_plan(PROJECT_ID, &batch_id)
        .await
        .expect("offline failure should have a partial resume plan");
    assert_eq!(plan.auto_resumable, 1);
    assert_eq!(plan.resolved, 0);
    assert_eq!(plan.entries[0].status, "AUTO_RESUMABLE");
    assert_eq!(plan.entries[0].eligibility, "AUTO_RESUMABLE");

    let retry_result = services
        .queue
        .partial_resume(PROJECT_ID, &batch_id, &[source_item_id.clone()])
        .await
        .expect("failed leaf should create a retry child");
    assert_eq!(retry_result.created_count, 1);
    assert_eq!(retry_result.already_prepared_count, 0);
    let retry_item_id = retry_result.created_item_ids[0].clone();
    let after_retry_prepare = services
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("retry child should be readable");
    let retry_item = after_retry_prepare
        .items
        .iter()
        .find(|item| item.id.as_str() == retry_item_id)
        .expect("retry item should exist");
    assert_eq!(
        retry_item.retry_of_item_id.as_deref(),
        Some(source_item_id.as_str())
    );
    assert_eq!(retry_item.values_json, frozen_values);
    assert_eq!(retry_item.workflow_version_id.as_str(), WORKFLOW_VERSION_ID);
    assert_eq!(retry_item.recipe_id.as_str(), RECIPE_ID);
    assert_eq!(count(&pool, "items").await, 2);

    let duplicate = services
        .queue
        .partial_resume(PROJECT_ID, &batch_id, &[source_item_id.clone()])
        .await
        .expect("repeating partial resume should be idempotent");
    assert_eq!(duplicate.created_count, 0);
    assert_eq!(duplicate.already_prepared_count, 1);
    assert_eq!(
        duplicate.existing_retry_item_ids,
        vec![retry_item_id.clone()]
    );
    assert_eq!(count(&pool, "items").await, 2);

    let imported_asset_id = frozen_values["first_frame"]["assetId"]
        .as_str()
        .expect("frozen first frame should contain an asset id")
        .to_owned();
    let (asset_sha, asset_path) = sqlx::query_as::<_, (String, String)>(
        "SELECT sha256, storage_path FROM assets WHERE id = ?",
    )
    .bind(&imported_asset_id)
    .fetch_one(&pool)
    .await
    .expect("imported asset should be readable");
    assert_eq!(asset_sha, sha256(&original_png));
    assert_eq!(
        fs::read(&asset_path).expect("imported asset should exist"),
        original_png
    );

    let reads = Arc::clone(&services.asset_reads);
    reads.lock().expect("asset read mutex should work").clear();
    comfy.set_behavior(ComfyBehavior::Success);
    services
        .queue
        .start_for_test(PROJECT_ID, &batch_id)
        .await
        .expect("retry requires an explicit start");
    wait_for_item_status(&pool, &retry_item_id, ProductionBatchItemStatus::Succeeded).await;
    wait_for_batch_status(&pool, &batch_id, ProductionBatchStatus::Completed).await;
    assert_eq!(count(&pool, "tasks").await, 2);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 2);
    let retry_reads = reads.lock().expect("asset read mutex should work").clone();
    assert!(
        !retry_reads.is_empty(),
        "retry should upload the imported asset"
    );
    assert!(retry_reads
        .iter()
        .all(|path| !path.starts_with(&package_root)));
    assert!(retry_reads
        .iter()
        .any(|path| path == Path::new(&asset_path)));
    assert!(comfy
        .uploaded_images()
        .last()
        .is_some_and(|bytes| bytes == &original_png));

    let final_detail = services
        .queue
        .get(PROJECT_ID, &batch_id)
        .await
        .expect("completed retry batch should be readable");
    let final_retry = final_detail
        .items
        .iter()
        .find(|item| item.id.as_str() == retry_item_id)
        .expect("completed retry item should exist");
    assert_eq!(final_retry.values_json, frozen_values);
    let final_plan = services
        .queue
        .partial_resume_plan(PROJECT_ID, &batch_id)
        .await
        .expect("completed batch should still expose its plan");
    assert_plan_is_resolved(&final_plan);
}

#[tokio::test]
async fn stream_disconnect_recovery_uses_existing_prompt_without_second_task_or_post() {
    let directory = tempdir().expect("temporary workspace should exist");
    let pool = initialize(&directory.path().join("dev061b-stream.db"))
        .await
        .expect("SQLite should initialize");
    let project_root = directory.path().join("project-storage");
    fs::create_dir_all(&project_root).expect("project storage should exist");
    seed_database(&pool, &project_root).await;
    let package_root = directory.path().join("package");
    fs::create_dir_all(&package_root).expect("package root should exist");
    write_package(&package_root, &png_bytes([70, 80, 90, 255])).await;

    let comfy = Arc::new(ControlledComfy::new(
        ComfyBehavior::DisconnectThenHistorySuccess,
    ));
    let services = build_services(&pool, Arc::clone(&comfy), &package_root);
    let (batch_id, item_id, _) = create_package_batch(&services, &package_root).await;
    services
        .queue
        .start_for_test(PROJECT_ID, &batch_id)
        .await
        .expect("explicit queue start should succeed");
    wait_for_event(&pool, "TASK_STREAM_DISCONNECTED").await;

    let submitted_task = sqlx::query_as::<_, (String, String)>(
        "SELECT id, prompt_id FROM tasks WHERE project_id = ?",
    )
    .bind(PROJECT_ID)
    .fetch_one(&pool)
    .await
    .expect("submitted task should exist");
    assert!(!submitted_task.1.is_empty());
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&pool, "tasks").await, 1);

    services
        .queue
        .recover_and_resume()
        .await
        .expect("explicit recovery should reconcile the existing prompt");
    wait_for_item_status(&pool, &item_id, ProductionBatchItemStatus::Succeeded).await;
    wait_for_batch_status(&pool, &batch_id, ProductionBatchStatus::Completed).await;
    assert_eq!(count(&pool, "tasks").await, 1);
    assert_eq!(comfy.submit_calls.load(Ordering::SeqCst), 1);
    let recovery_succeeded = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM task_events WHERE task_id = ? AND event_type = 'TASK_RECOVERY_SUCCEEDED'",
    )
    .bind(&submitted_task.0)
    .fetch_one(&pool)
    .await
    .expect("recovery event count should be readable");
    assert_eq!(recovery_succeeded, 1);

    let plan = services
        .queue
        .partial_resume_plan(PROJECT_ID, &batch_id)
        .await
        .expect("succeeded leaf should have a resolved plan");
    assert_plan_is_resolved(&plan);
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
