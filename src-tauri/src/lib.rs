mod app_state;
mod application;
mod commands;
pub mod compiler;
pub mod domain;
mod error;
mod infrastructure;

pub use application::ports::{
    AssetRepository, AssetStore, Clock, GenerationDefinitionRepository,
    GenerationSnapshotRepository, ProductionQueueRepository, ProjectRepository, RepositoryError,
    TaskRepository, WorkflowLibraryRepository, WorkflowRunRepository, WorkflowRuntimeRepository,
    WorkflowRuntimeStateRepository,
};
pub use infrastructure::database::{
    SqliteAssetRepository, SqliteGenerationDefinitionRepository,
    SqliteGenerationSnapshotRepository, SqlitePresetRepository, SqliteProductionQueueRepository,
    SqliteProjectRepository, SqliteTaskRepository, SqliteWorkflowLibraryRepository,
    SqliteWorkflowRunRepository,
};

use app_state::AppState;
use application::{
    asset_library_service::AssetLibraryService,
    asset_query_service::AssetQueryService,
    comfy_service::ComfyService,
    diagnostics_service::DiagnosticsService,
    generation_catalog_service::GenerationCatalogService,
    generation_service::GenerationService,
    media_protocol::MediaProtocolService,
    ports::{ComfyAdapter, ComfyConnectionConfig, WorkflowLibrarySource},
    preset_service::PresetService,
    production_queue_service::ProductionQueueService,
    project_bootstrap::DefaultProjectBootstrap,
    project_service::ProjectService,
    source_asset_import_service::SourceAssetImportService,
    task_cancellation_service::TaskCancellationService,
    task_execution_registry::TaskExecutionRegistry,
    task_history_service::TaskHistoryService,
    task_query_service::TaskQueryService,
    task_recovery_service::TaskRecoveryService,
    workflow_library_service::WorkflowLibraryService,
    workflow_lifecycle_service::WorkflowLifecycleService,
    workflow_onboarding_service::WorkflowOnboardingService,
};
use error::AppError;
use infrastructure::logging::LoggingStatus;
use infrastructure::{
    comfy::ComfyHttpAdapter,
    database,
    filesystem::{
        AppDataDirs, FileSystemAssetStore, FileSystemProjectDirectoryStore,
        FileSystemWorkflowLibrarySource, FileSystemWorkflowPackageStore,
    },
    tauri::TauriTaskUpdateSink,
    time::SystemClock,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Manager, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let logging_status = initialize_logging();
    tracing::info!("application starting");

    if let Err(error) = run_application(logging_status) {
        tracing::error!(code = error.code(), "application failed to start");
        eprintln!("AI Studio failed to start ({})", error.code());
        std::process::exit(1);
    }
}

fn initialize_logging() -> LoggingStatus {
    infrastructure::logging::initialize(default_logs_dir().as_deref())
}

fn default_logs_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .map(|base| base.join("AIStudio").join("AIStudioData").join("logs"))
}

fn run_application(logging_status: LoggingStatus) -> Result<(), AppError> {
    let media_protocol_slot: Arc<Mutex<Option<Arc<MediaProtocolService>>>> =
        Arc::new(Mutex::new(None));
    let setup_media_protocol_slot = Arc::clone(&media_protocol_slot);
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }));
    builder
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol(
            "aistudio-media",
            move |_context, request, responder| {
                let slot = Arc::clone(&media_protocol_slot);
                std::thread::spawn(move || {
                    let response = if !matches!(request.uri().path(), "/video" | "/audio") {
                        application::media_protocol::MediaResponse {
                            status: 404,
                            headers: Default::default(),
                            body: Vec::new(),
                        }
                    } else {
                        let project_id = query_param(request.uri().query(), "projectId");
                        let asset_id = query_param(request.uri().query(), "assetId");
                        let range = request
                            .headers()
                            .get("range")
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned);
                        let Some(protocol) = slot.lock().ok().and_then(|value| value.clone())
                        else {
                            responder.respond(
                                tauri::http::Response::builder()
                                    .status(503)
                                    .body(Vec::new())
                                    .expect("protocol response builder should accept empty body"),
                            );
                            return;
                        };
                        match (project_id, asset_id) {
                            (Some(project_id), Some(asset_id)) => {
                                tauri::async_runtime::block_on(protocol.handle_path(
                                    Some(request.uri().path()),
                                    request.method().as_str(),
                                    &project_id,
                                    &asset_id,
                                    range.as_deref(),
                                ))
                            }
                            _ => application::media_protocol::MediaResponse {
                                status: 404,
                                headers: Default::default(),
                                body: Vec::new(),
                            },
                        }
                    };
                    let mut builder = tauri::http::Response::builder().status(response.status);
                    for (name, value) in response.headers {
                        builder = builder.header(name, value);
                    }
                    responder.respond(
                        builder
                            .body(response.body)
                            .expect("protocol response builder should accept media body"),
                    );
                });
            },
        )
        .setup(move |app| {
            let result = (|| -> Result<(), AppError> {
            let data_root = app
                .path()
                .local_data_dir()
                .map_err(|_| AppError::initialization("failed to resolve local data directory"))?
                .join("AIStudio")
                .join("AIStudioData");

            let data_dirs = AppDataDirs::initialize(data_root)?;
            tracing::info!("application data directory initialized");

            let database_pool =
                tauri::async_runtime::block_on(database::initialize(&data_dirs.database))?;
            app.manage(database_pool.clone());

            let clock: Arc<dyn Clock> = Arc::new(SystemClock);
            let task_repository: Arc<dyn TaskRepository> =
                Arc::new(SqliteTaskRepository::new(database_pool.clone()));
            let snapshot_repository: Arc<dyn GenerationSnapshotRepository> = Arc::new(
                SqliteGenerationSnapshotRepository::new(database_pool.clone()),
            );
            let definition_repository: Arc<dyn GenerationDefinitionRepository> = Arc::new(
                infrastructure::database::SqliteGenerationDefinitionRepository::new(
                    database_pool.clone(),
                ),
            );
            let project_repository: Arc<dyn ProjectRepository> =
                Arc::new(SqliteProjectRepository::new(database_pool.clone()));
            let asset_repository: Arc<dyn AssetRepository> =
                Arc::new(SqliteAssetRepository::new(database_pool.clone()));
            let task_history_repository: Arc<dyn application::ports::TaskHistoryRepository> =
                Arc::new(infrastructure::database::SqliteTaskHistoryRepository::new(
                    database_pool.clone(),
                ));
            let asset_browse_repository: Arc<dyn application::ports::AssetBrowseRepository> =
                Arc::new(infrastructure::database::SqliteAssetBrowseRepository::new(
                    database_pool.clone(),
                ));
            let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
            let preset_repository: Arc<dyn application::ports::PresetRepository> = Arc::new(
                infrastructure::database::SqlitePresetRepository::new(database_pool.clone()),
            );
            let production_queue_repository: Arc<dyn application::ports::ProductionQueueRepository> = Arc::new(
                infrastructure::database::SqliteProductionQueueRepository::new(database_pool.clone()),
            );
            let project_directory_store: Arc<dyn application::ports::ProjectDirectoryStore> =
                Arc::new(FileSystemProjectDirectoryStore::new(
                    data_dirs.projects.clone(),
                ));

            let project_bootstrap =
                DefaultProjectBootstrap::new(project_repository.clone(), clock.clone());
            tauri::async_runtime::block_on(
            project_bootstrap.ensure_default_project(&data_dirs.projects),
            )
            .map_err(|_| AppError::initialization("default project initialization failed"))?;

            let workflow_library_repository: Arc<dyn WorkflowLibraryRepository> = Arc::new(
                infrastructure::database::SqliteWorkflowLibraryRepository::new(
                    database_pool.clone(),
                ),
            );
            let workflow_library_source: Arc<dyn WorkflowLibrarySource> = Arc::new(
                FileSystemWorkflowLibrarySource::new(data_dirs.workflow_library.clone()),
            );
            let workflow_library_service = Arc::new(WorkflowLibraryService::new(
                workflow_library_source.clone(),
                workflow_library_repository,
                clock.clone(),
            ));
            match tauri::async_runtime::block_on(workflow_library_service.sync()) {
                Ok(report) => tracing::info!(
                    packages = report.packages_found,
                    valid = report.valid,
                    invalid = report.invalid,
                    "runtime workflow library synchronized"
                ),
                Err(error) => tracing::warn!(
                    error_type = std::any::type_name_of_val(&error),
                    "runtime workflow library synchronization skipped"
                ),
            }

            let comfy_config = ComfyConnectionConfig::default();
            let comfy_adapter = ComfyHttpAdapter::new(comfy_config.clone())
                .map_err(|_| AppError::initialization("ComfyUI HTTP client initialization failed"))?;
            let comfy_adapter: Arc<dyn ComfyAdapter> = Arc::new(comfy_adapter);
            let comfy_service = Arc::new(ComfyService::new(comfy_adapter.clone(), &comfy_config));
            let workflow_run_repository: Arc<dyn WorkflowRunRepository> = Arc::new(
                infrastructure::database::SqliteWorkflowRunRepository::new(database_pool.clone()),
            );
            let runtime_repository: Arc<dyn WorkflowRuntimeRepository> = Arc::new(
                infrastructure::database::SqliteWorkflowRuntimeRepository::new(
                    database_pool.clone(),
                ),
            );
            let runtime_state_repository: Arc<dyn WorkflowRuntimeStateRepository> = Arc::new(
                infrastructure::database::SqliteWorkflowRuntimeStateRepository::new(
                    database_pool.clone(),
                ),
            );
            let package_store: Arc<dyn application::ports::WorkflowPackageStore> =
                Arc::new(FileSystemWorkflowPackageStore::new(
                    data_dirs.workflow_library.clone(),
                    data_dirs.workflow_staging.clone(),
                ));
            let workflow_onboarding_service = Arc::new(WorkflowOnboardingService::new(
                workflow_library_source.clone(),
                comfy_adapter.clone(),
                workflow_library_service.clone(),
                workflow_run_repository,
                package_store.clone(),
                clock.clone(),
            ));
            let workflow_lifecycle_service = Arc::new(WorkflowLifecycleService::new(
                workflow_library_source.clone(),
                workflow_library_service.clone(),
                workflow_onboarding_service.clone(),
                runtime_repository,
                runtime_state_repository,
                package_store,
                clock.clone(),
            ));
            let task_update_sink: Arc<dyn application::ports::TaskUpdateSink> =
                Arc::new(TauriTaskUpdateSink::new(app.handle().clone()));
            let execution_registry = TaskExecutionRegistry::default();
            let generation_service = Arc::new(
                GenerationService::new(
                    task_repository.clone(),
                    snapshot_repository.clone(),
                    definition_repository.clone(),
                    comfy_adapter.clone(),
                    project_repository.clone(),
                    asset_store.clone(),
                    asset_repository.clone(),
                    clock.clone(),
                )
                .with_task_update_sink(task_update_sink.clone())
                .with_execution_registry(execution_registry.clone()),
            );
            let generation_catalog_service =
                Arc::new(GenerationCatalogService::new(definition_repository.clone()));
            let task_query_service = Arc::new(TaskQueryService::new(
                Arc::new(SqliteTaskRepository::new(database_pool.clone())),
                asset_repository.clone(),
                definition_repository.clone(),
            ));
            let asset_query_service = Arc::new(
                AssetQueryService::new(asset_repository.clone(), asset_store.clone())
                    .with_output_order_repositories(
                        Arc::new(SqliteTaskRepository::new(database_pool.clone())),
                        definition_repository.clone(),
                    ),
            );
            let asset_library_service = Arc::new(AssetLibraryService::new(asset_browse_repository));
            let task_history_service = Arc::new(TaskHistoryService::new(
                task_history_repository,
                snapshot_repository.clone(),
                definition_repository.clone(),
                asset_repository.clone(),
            ));
            let source_asset_import_service = Arc::new(SourceAssetImportService::new(
                project_repository.clone(),
                asset_store.clone(),
                asset_repository.clone(),
                clock.clone(),
            ));
            let task_cancellation_service = Arc::new(TaskCancellationService::new(
                task_repository.clone(),
                execution_registry,
                clock.clone(),
                task_update_sink.clone(),
            ));
            let task_recovery_service = Arc::new(TaskRecoveryService::new(
                task_repository.clone(),
                snapshot_repository.clone(),
                asset_repository.clone(),
                comfy_adapter.clone(),
                project_repository.clone(),
                asset_store.clone(),
                clock.clone(),
                task_update_sink,
            ));
            let production_queue_service = Arc::new(ProductionQueueService::new(
                production_queue_repository,
                task_repository.clone(),
                generation_service.clone(),
                task_recovery_service.clone(),
                clock.clone(),
            ));
            let diagnostics_service = Arc::new(DiagnosticsService::new(
                database_pool.clone(),
                task_repository,
                comfy_service.clone(),
                workflow_lifecycle_service.clone(),
                production_queue_service.clone(),
                data_dirs.logs.clone(),
                logging_status,
            ));
            let project_service = Arc::new(ProjectService::new(
                project_repository.clone(),
                project_directory_store,
                clock.clone(),
            ));
            let preset_service = Arc::new(PresetService::new(
                preset_repository,
                definition_repository,
                asset_repository.clone(),
                clock,
            ));
            if let Ok(mut slot) = setup_media_protocol_slot.lock() {
                *slot = Some(Arc::new(MediaProtocolService::new(
                    asset_repository.clone(),
                    asset_store.clone(),
                    project_repository.clone(),
                )));
            }
            let startup_recovery = task_recovery_service.clone();
            let startup_production_queue = production_queue_service.clone();
            app.manage(AppState::new(
                data_dirs,
                comfy_service,
                generation_service,
                workflow_library_service,
                workflow_onboarding_service,
                workflow_lifecycle_service,
                generation_catalog_service,
                task_query_service,
                asset_query_service,
                asset_library_service,
                task_history_service,
                source_asset_import_service,
                task_cancellation_service,
                task_recovery_service,
                project_service,
                preset_service,
                production_queue_service,
                diagnostics_service,
            ));

            tauri::async_runtime::spawn(async move {
                match startup_recovery.reconcile_active().await {
                    Ok(report) => {
                        tracing::info!(
                            examined = report.examined,
                            succeeded = report.succeeded,
                            failed = report.failed,
                            deferred = report.deferred,
                            unresolved = report.unresolved,
                            "startup task recovery completed"
                        );
                        if let Err(error) = startup_production_queue.recover_and_resume().await {
                            tracing::warn!(
                                error_type = std::any::type_name_of_val(&error),
                                "startup production queue recovery failed"
                            );
                        }
                    }
                    Err(error) => tracing::warn!(
                        error_type = std::any::type_name_of_val(&error),
                        "startup task recovery failed; production queue auto-resume skipped"
                    ),
                }
            });

            Ok(())
            })();
            if let Err(error) = &result {
                let _ = app
                    .dialog()
                    .message(format!(
                        "AI Studio 启动失败\n\n无法初始化本地创作环境。\n\n错误代码：{}\n\n请重试或查看诊断日志。",
                        error.code()
                    ))
                    .title("AI Studio")
                    .blocking_show();
            }
            Ok(result?)
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();

                let window = window.clone();
                let app_handle = window.app_handle().clone();
                let diagnostics_service: Option<Arc<DiagnosticsService>> = app_handle
                    .try_state::<AppState>()
                    .map(|state| Arc::clone(&state.diagnostics_service));

                tauri::async_runtime::spawn(async move {
                    let activity = match diagnostics_service {
                        Some(service) => tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            service.runtime_activity_status(),
                        )
                        .await
                        .ok()
                        .and_then(Result::ok),
                        None => None,
                    };

                    let Some(activity) = activity else {
                        app_handle
                            .dialog()
                            .message(
                                "暂时无法确认任务状态。为保护正在运行的任务，是否仍要退出？",
                            )
                            .title("安全退出")
                            .buttons(MessageDialogButtons::OkCancelCustom(
                                "退出应用".to_owned(),
                                "继续运行".to_owned(),
                            ))
                            .parent(&window)
                            .show(move |confirmed| {
                                if confirmed {
                                    tracing::info!("safe exit confirmed");
                                    let _ = window.destroy();
                                } else {
                                    tracing::info!("safe exit cancelled");
                                }
                            });
                        return;
                    };

                    if activity.active_task_count == 0 && !activity.production_busy {
                        tracing::info!("safe exit confirmed");
                        let _ = window.destroy();
                        return;
                    }

                    app_handle
                        .dialog()
                        .message(
                            "当前仍有生成任务或生产队列正在运行。\n\n退出不会取消任务，但关闭期间将无法显示实时进度。",
                        )
                        .title("安全退出")
                        .buttons(MessageDialogButtons::OkCancelCustom(
                            "退出应用".to_owned(),
                            "继续运行".to_owned(),
                        ))
                        .parent(&window)
                        .show(move |confirmed| {
                            if confirmed {
                                tracing::info!("safe exit confirmed");
                                let _ = window.destroy();
                            } else {
                                tracing::info!("safe exit cancelled");
                            }
                        });
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::get_app_status,
            commands::diagnostics::runtime_activity_status,
            commands::diagnostics::diagnostics_summary,
            commands::diagnostics::diagnostics_export,
            commands::comfy::comfy_get_status,
            commands::comfy::comfy_refresh_capabilities,
            commands::workflow_library::workflow_library_refresh,
            commands::workflow_onboarding::workflow_onboarding_pick_api_workflow,
            commands::workflow_onboarding::workflow_onboarding_get,
            commands::workflow_onboarding::workflow_onboarding_check_capability,
            commands::workflow_onboarding::workflow_onboarding_set_metadata,
            commands::workflow_onboarding::workflow_onboarding_set_input_mapping,
            commands::workflow_onboarding::workflow_onboarding_remove_input_mapping,
            commands::workflow_onboarding::workflow_onboarding_set_output_mapping,
            commands::workflow_onboarding::workflow_onboarding_validate,
            commands::workflow_onboarding::workflow_onboarding_publish,
            commands::workflow_onboarding::workflow_onboarding_discard,
            commands::workflow_onboarding::workflow_workspace_list,
            commands::workflow_lifecycle::workflow_runtime_workspace_list,
            commands::workflow_lifecycle::workflow_runtime_diagnostics,
            commands::workflow_lifecycle::workflow_set_enabled,
            commands::workflow_lifecycle::workflow_recheck_capability,
            commands::workflow_lifecycle::workflow_duplicate_recipe,
            commands::workflow_lifecycle::workflow_compare_versions,
            commands::workflow_lifecycle::workflow_export_package,
            commands::workflow_lifecycle::workflow_import_package_backup,
            commands::workflow_lifecycle::workflow_clean_staging,
            commands::catalog::generation_catalog_list,
            commands::generation::generation_create,
            commands::generation::generation_create_batch,
            commands::production_queue::production_queue_create,
            commands::production_queue::production_queue_list,
            commands::production_queue::production_queue_overview,
            commands::production_queue::production_queue_admission_status,
            commands::production_queue::production_queue_get,
            commands::production_queue::production_queue_start,
            commands::production_queue::production_queue_pause,
            commands::production_queue::production_queue_archive,
            commands::production_queue::production_queue_restore,
            commands::production_queue::production_queue_delete,
            commands::production_queue::production_queue_skip_item,
            commands::production_queue::production_queue_requeue_item,
            commands::project::project_list,
            commands::project::project_create,
            commands::project::project_update,
            commands::task::task_get,
            commands::task::task_list_recent,
            commands::task::task_cancel,
            commands::task::task_reconcile_active,
            commands::task::task_history_page,
            commands::task::task_get_detail,
            commands::task::task_get_reusable_draft,
            commands::preset::preset_list,
            commands::preset::preset_create,
            commands::preset::preset_update,
            commands::preset::preset_delete,
            commands::asset::asset_list_by_task,
            commands::asset::asset_list_recent,
            commands::asset::asset_pick_and_import_image,
            commands::asset::asset_pick_and_import_video,
            commands::asset::asset_pick_and_import_audio,
            commands::asset::asset_read_image,
            commands::asset::asset_read_thumbnail,
            commands::asset::asset_library_page,
            commands::asset::asset_get
        ])
        .run(tauri::generate_context!())
        .map_err(|_| AppError::initialization("Tauri runtime failed"))
}

fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    query?
        .split('&')
        .filter_map(|part| part.split_once('='))
        .find(|(candidate, _)| *candidate == key)
        .and_then(|(_, value)| percent_decode(value))
}

fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut chars = value.as_bytes().iter().copied();
    while let Some(byte) = chars.next() {
        match byte {
            b'+' => bytes.push(b' '),
            b'%' => {
                let high = chars.next().and_then(hex_value)?;
                let low = chars.next().and_then(hex_value)?;
                bytes.push((high << 4) | low);
            }
            other => bytes.push(other),
        }
    }
    String::from_utf8(bytes).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
