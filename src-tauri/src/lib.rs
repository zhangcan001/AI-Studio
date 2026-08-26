mod app_state;
pub mod application;
mod commands;
pub mod compiler;
pub mod domain;
mod error;
pub mod infrastructure;

pub use application::ports::{
    AssetDeletionRepository, AssetRepository, AssetStore, AssetUsageRepository,
    AssetVideoPromptRepository, Clock, GenerationDefinitionRepository,
    GenerationSnapshotRepository, ProductionItemReviewRepository, ProductionQueueRepository,
    ProjectRecord, ProjectRepository, RepositoryError, TaskOutputAssetMapping, TaskRepository,
    WorkflowLibraryRepository, WorkflowRunRepository, WorkflowRuntimeRepository,
    WorkflowRuntimeStateRepository,
};
pub use infrastructure::database::{
    initialize, SqliteAssetDeletionRepository, SqliteAssetRepository,
    SqliteAssetVideoPromptRepository, SqliteGenerationDefinitionRepository,
    SqliteGenerationSnapshotRepository, SqliteOrganizationRepository, SqlitePresetRepository,
    SqliteProductionItemReviewRepository, SqliteProductionQueueRepository, SqliteProjectRepository,
    SqlitePromptLibraryRepository, SqliteTaskRepository, SqliteWorkflowLibraryRepository,
    SqliteWorkflowRunRepository,
};

use app_state::AppState;
use application::{
    asset_deletion_service::AssetDeletionService,
    asset_library_service::AssetLibraryService,
    asset_query_service::AssetQueryService,
    asset_usage_service::AssetUsageService,
    asset_video_prompt_service::AssetVideoPromptService,
    batch_workflow_preset_service::BatchWorkflowPresetService,
    comfy_memory_service::ComfyMemoryService,
    comfy_preflight_service::ComfyPreflightService,
    comfy_service::{ComfyRuntime, ComfyService},
    consistency_profile_service::ConsistencyProfileService,
    diagnostics_service::DiagnosticsService,
    episode_production_service::EpisodeProductionService,
    generation_catalog_service::GenerationCatalogService,
    generation_service::GenerationService,
    h3_local_import_service::H3LocalImportService,
    media_protocol::MediaProtocolService,
    organization_service::OrganizationService,
    ports::{ComfyAdapterFactory, ComfyConnectionConfig, SettingsStore, WorkflowLibrarySource},
    preset_service::PresetService,
    production_audit_service::ProductionAuditService,
    production_batch_runbook_service::ProductionBatchRunbookService,
    production_item_review_service::ProductionItemReviewService,
    production_orchestrator_service::ProductionOrchestratorService,
    production_preparation_service::ProductionPreparationService,
    production_queue_service::ProductionQueueService,
    production_structure_service::ProductionStructureService,
    project_backup_service::ProjectBackupService,
    project_bootstrap::DefaultProjectBootstrap,
    project_command_center_service::ProjectCommandCenterService,
    project_manifest_service::ProjectManifestService,
    project_service::ProjectService,
    project_template_service::ProjectTemplateService,
    prompt_library_service::PromptLibraryService,
    prompt_template_bulk_service::PromptTemplateBulkService,
    prompt_template_service::PromptTemplateService,
    reference_anchor_service::ReferenceAnchorService,
    reference_set_service::ReferenceSetService,
    scene_production_service::SceneProductionService,
    series_production_service::SeriesProductionService,
    settings_service::SettingsService,
    shot_batch_service::ShotBatchService,
    shot_bulk_service::ShotBulkService,
    shot_context_resolver::ShotContextResolver,
    shot_readiness_service::ShotReadinessService,
    shot_service::ShotService,
    source_asset_import_service::SourceAssetImportService,
    task_cancellation_service::TaskCancellationService,
    task_execution_registry::TaskExecutionRegistry,
    task_history_service::TaskHistoryService,
    task_query_service::TaskQueryService,
    task_recovery_service::TaskRecoveryService,
    workflow_benchmark_service::WorkflowBenchmarkService,
    workflow_library_service::WorkflowLibraryService,
    workflow_lifecycle_service::WorkflowLifecycleService,
    workflow_onboarding_service::WorkflowOnboardingService,
};
use error::AppError;
use infrastructure::logging::LoggingStatus;
use infrastructure::{
    comfy::ComfyHttpAdapterFactory,
    database,
    filesystem::{
        configured_data_root, resolve_data_root, AppDataDirs, FileSystemAssetStore,
        FileSystemProjectDirectoryStore, FileSystemWorkflowLibrarySource,
        FileSystemWorkflowPackageStore,
    },
    settings::JsonSettingsStore,
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
    if let Some(root) = configured_data_root() {
        return Some(root.join("logs"));
    }
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
            let default_data_root = app
                .path()
                .local_data_dir()
                .map_err(|_| AppError::initialization("failed to resolve local data directory"))?
                .join("AIStudio")
                .join("AIStudioData");
            let data_root = resolve_data_root(default_data_root);

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
            let reference_anchor_repository: Arc<dyn application::ports::ReferenceAnchorRepository> =
                Arc::new(infrastructure::database::SqliteReferenceAnchorRepository::new(
                    database_pool.clone(),
                ));
            let production_structure_repository: Arc<dyn application::ports::ProductionStructureRepository> =
                Arc::new(infrastructure::database::SqliteProductionStructureRepository::new(
                    database_pool.clone(),
                ));
            let consistency_scope_repository: Arc<dyn application::ports::ConsistencyScopeRepository> =
                Arc::new(infrastructure::database::repositories::SqliteConsistencyScopeRepository::new(
                    database_pool.clone(),
                ));
            let consistency_profile_repository: Arc<dyn application::ports::ConsistencyProfileRepository> =
                Arc::new(infrastructure::database::SqliteConsistencyProfileRepository::new(
                    database_pool.clone(),
                ));
            let reference_set_repository: Arc<dyn application::ports::ReferenceSetRepository> =
                Arc::new(infrastructure::database::SqliteReferenceSetRepository::new(
                    database_pool.clone(),
                ));
            let asset_usage_repository: Arc<dyn application::ports::AssetUsageRepository> =
                Arc::new(infrastructure::database::repositories::SqliteAssetUsageRepository::new(
                    database_pool.clone(),
                ));
            let shot_consistency_repository: Arc<dyn application::ports::ShotConsistencyRepository> =
                Arc::new(infrastructure::database::SqliteShotConsistencyRepository::new(
                    database_pool.clone(),
                ));
            let asset_deletion_repository: Arc<dyn application::ports::AssetDeletionRepository> =
                Arc::new(SqliteAssetDeletionRepository::new(database_pool.clone()));
            let asset_video_prompt_repository: Arc<dyn application::ports::AssetVideoPromptRepository> =
                Arc::new(SqliteAssetVideoPromptRepository::new(database_pool.clone()));
            let task_history_repository: Arc<dyn application::ports::TaskHistoryRepository> =
                Arc::new(infrastructure::database::SqliteTaskHistoryRepository::new(
                    database_pool.clone(),
                ));
            let asset_browse_repository: Arc<dyn application::ports::AssetBrowseRepository> =
                Arc::new(infrastructure::database::SqliteAssetBrowseRepository::new(
                    database_pool.clone(),
                ));
            let organization_repository: Arc<dyn application::ports::OrganizationRepository> =
                Arc::new(SqliteOrganizationRepository::new(database_pool.clone()));
            let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
            let preset_repository: Arc<dyn application::ports::PresetRepository> = Arc::new(
                infrastructure::database::SqlitePresetRepository::new(database_pool.clone()),
            );
            let prompt_library_repository: Arc<dyn application::ports::PromptLibraryRepository> = Arc::new(
                infrastructure::database::SqlitePromptLibraryRepository::new(database_pool.clone()),
            );
            let shot_repository_impl = Arc::new(
                infrastructure::database::SqliteShotRepository::new(database_pool.clone()),
            );
            let shot_repository: Arc<dyn application::ports::ShotRepository> =
                shot_repository_impl.clone();
            let shot_bulk_repository: Arc<dyn application::ports::ShotBulkRepository> =
                shot_repository_impl.clone();
            let production_queue_repository_impl = Arc::new(
                infrastructure::database::SqliteProductionQueueRepository::new(database_pool.clone()),
            );
            let production_queue_repository: Arc<dyn application::ports::ProductionQueueRepository> =
                production_queue_repository_impl.clone();
            let production_item_review_repository: Arc<dyn application::ports::ProductionItemReviewRepository> =
                Arc::new(infrastructure::database::SqliteProductionItemReviewRepository::new(
                    database_pool.clone(),
                ));
            let shot_batch_repository: Arc<dyn application::ports::ShotBatchRepository> =
                production_queue_repository_impl.clone();
            let project_directory_store: Arc<dyn application::ports::ProjectDirectoryStore> =
                Arc::new(FileSystemProjectDirectoryStore::new(
                    data_dirs.projects.clone(),
                ));

            let shot_context_resolver = Arc::new(ShotContextResolver::new(
                project_repository.clone(),
                production_structure_repository.clone(),
                shot_repository.clone(),
                consistency_scope_repository,
                consistency_profile_repository.clone(),
                reference_set_repository.clone(),
                shot_consistency_repository,
                asset_repository.clone(),
                clock.clone(),
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
            if let Err(error) = application::builtin_runtime_packages::ensure_installed(
                &data_dirs.workflow_library,
            ) {
                tracing::warn!(error_type = "builtin_runtime_package_install", %error, "builtin H3 runtime package installation skipped");
            }
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

            let settings_store: Arc<dyn SettingsStore> =
                Arc::new(JsonSettingsStore::new(data_dirs.config.clone()));
            let mut loaded_settings =
                tauri::async_runtime::block_on(settings_store.load());
            let comfy_config = match ComfyConnectionConfig::from_endpoint(
                &loaded_settings.settings.comfy.endpoint,
            ) {
                Ok(config) => config,
                Err(_error) => {
                    tracing::warn!(
                        error_type = "invalid_persisted_endpoint",
                        "invalid persisted ComfyUI endpoint; using default"
                    );
                    loaded_settings.settings = application::ports::AppSettings::default();
                    loaded_settings.warning = Some(
                        "设置文件无法读取，当前已使用默认配置。".to_owned(),
                    );
                    ComfyConnectionConfig::default()
                }
            };
            let adapter_factory = Arc::new(ComfyHttpAdapterFactory);
            let initial_adapter = adapter_factory
                .create(comfy_config.clone())
                .map_err(|_| AppError::initialization("ComfyUI HTTP client initialization failed"))?;
            let comfy_runtime = Arc::new(ComfyRuntime::new(initial_adapter, comfy_config));
            let comfy_adapter = comfy_runtime.adapter();
            let comfy_service = Arc::new(ComfyService::from_runtime(comfy_runtime.clone()));
            let comfy_memory_service = Arc::new(ComfyMemoryService::new(
                comfy_adapter.clone(),
                task_repository.clone(),
                production_queue_repository.clone(),
            ));
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
            )
            .with_runtime_state(runtime_repository.clone(), runtime_state_repository.clone()));
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
                .with_workflow_compatibility_service(workflow_onboarding_service.clone())
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
                    )
                    .with_organization_repository(organization_repository.clone()),
            );
            let asset_library_service = Arc::new(AssetLibraryService::new(
                asset_browse_repository,
                organization_repository.clone(),
            ));
            let reference_anchor_service = Arc::new(ReferenceAnchorService::new(
                reference_anchor_repository.clone(),
                asset_repository.clone(),
                clock.clone(),
            ));
            let consistency_profile_service = Arc::new(ConsistencyProfileService::new(
                consistency_profile_repository.clone(),
                reference_set_repository.clone(),
                project_repository.clone(),
                clock.clone(),
            ));
            let reference_set_service = Arc::new(ReferenceSetService::new(
                reference_set_repository,
                consistency_profile_repository,
                asset_repository.clone(),
                reference_anchor_repository,
                project_repository.clone(),
                clock.clone(),
            ));
            let asset_usage_service = Arc::new(AssetUsageService::new(asset_usage_repository));
            let production_structure_service = Arc::new(ProductionStructureService::new(
                production_structure_repository.clone(),
                clock.clone(),
            ));
            let asset_deletion_service = Arc::new(AssetDeletionService::new(
                asset_repository.clone(),
                asset_deletion_repository,
                project_repository.clone(),
                asset_store.clone(),
            ));
            let asset_video_prompt_service = Arc::new(AssetVideoPromptService::new(
                asset_video_prompt_repository,
                asset_repository.clone(),
                clock.clone(),
            ));
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
                production_queue_repository.clone(),
                task_repository.clone(),
                definition_repository.clone(),
                generation_service.clone(),
                shot_batch_repository.clone(),
                task_recovery_service.clone(),
                clock.clone(),
            ));
            let production_item_review_service = Arc::new(ProductionItemReviewService::new(
                production_item_review_repository,
                production_queue_repository,
                production_queue_service.clone(),
                task_repository.clone(),
                asset_repository.clone(),
                clock.clone(),
            ));
            let production_audit_service = Arc::new(ProductionAuditService::new(database_pool.clone()));
            let workflow_benchmark_service = Arc::new(WorkflowBenchmarkService::new(
                database_pool.clone(),
                definition_repository.clone(),
                preset_repository.clone(),
                production_queue_service.clone(),
                clock.clone(),
            ));
            let production_orchestrator_service = Arc::new(ProductionOrchestratorService::new(
                database_pool.clone(),
                definition_repository.clone(),
                production_queue_service.clone(),
                task_cancellation_service.clone(),
                clock.clone(),
            ));
            let h3_local_import_service = Arc::new(H3LocalImportService::new(
                source_asset_import_service.clone(),
                asset_video_prompt_service.clone(),
                production_queue_service.clone(),
                clock.clone(),
            ));
            let diagnostics_service = Arc::new(DiagnosticsService::new(
                database_pool.clone(),
                task_repository.clone(),
                comfy_service.clone(),
                workflow_lifecycle_service.clone(),
                production_queue_service.clone(),
                data_dirs.logs.clone(),
                logging_status,
            ));
            let settings_service = Arc::new(SettingsService::new(
                settings_store,
                loaded_settings,
                comfy_runtime,
                comfy_service.clone(),
                diagnostics_service.clone(),
                production_queue_service.clone(),
                adapter_factory,
            ));
            let comfy_preflight_service = Arc::new(ComfyPreflightService::new(
                comfy_service.clone(),
                diagnostics_service.clone(),
                workflow_lifecycle_service.clone(),
            ));
            let shot_readiness_service = Arc::new(ShotReadinessService::new(
                shot_context_resolver,
                comfy_preflight_service.clone(),
                workflow_lifecycle_service.clone(),
                production_structure_repository.clone(),
            ));
            let project_command_center_service = Arc::new(
                ProjectCommandCenterService::new(database_pool.clone())
                    .with_audit_service(production_audit_service.clone())
                    .with_comfy_cache_services(
                        comfy_service.clone(),
                        comfy_preflight_service.clone(),
                    ),
            );
            let project_service = Arc::new(ProjectService::new(
                project_repository.clone(),
                project_directory_store,
                clock.clone(),
            ));
            let organization_service = Arc::new(OrganizationService::new(
                organization_repository.clone(),
                clock.clone(),
            ));
            let project_template_service = Arc::new(ProjectTemplateService::new(
                organization_repository,
                definition_repository.clone(),
                project_service.clone(),
                clock.clone(),
            ));
            let project_backup_service = Arc::new(ProjectBackupService::new(
                database_pool.clone(),
                data_dirs.projects.clone(),
                data_dirs.cache.clone(),
            ));
            let project_manifest_service = Arc::new(ProjectManifestService::new(database_pool.clone()));
            let preset_service = Arc::new(PresetService::new(
                preset_repository.clone(),
                definition_repository.clone(),
                asset_repository.clone(),
                clock.clone(),
            ));
            let prompt_library_service = Arc::new(PromptLibraryService::new(
                prompt_library_repository.clone(),
                clock.clone(),
            ));
            let prompt_template_service = Arc::new(PromptTemplateService::new());
            let prompt_template_bulk_service = Arc::new(PromptTemplateBulkService::new(
                project_repository.clone(),
                prompt_library_repository.clone(),
                shot_bulk_repository.clone(),
                production_structure_service.clone(),
                reference_anchor_service.clone(),
                prompt_template_service.clone(),
                clock.clone(),
            ));
            let shot_bulk_service = Arc::new(ShotBulkService::new(
                shot_bulk_repository.clone(),
                definition_repository.clone(),
                prompt_library_repository.clone(),
                clock.clone(),
            ));
            let shot_service = Arc::new(ShotService::new(
                shot_repository.clone(),
                task_repository.clone(),
                asset_repository.clone(),
                definition_repository.clone(),
                prompt_library_repository.clone(),
                task_query_service.clone(),
                generation_service.clone(),
                shot_batch_repository.clone(),
                clock.clone(),
            )
            .with_stage_prompt_repository(shot_bulk_repository.clone())
            .with_generation_snapshot_repository(snapshot_repository.clone()));
            let shot_batch_service = Arc::new(ShotBatchService::new(
                shot_repository,
                shot_batch_repository.clone(),
                task_repository.clone(),
                asset_repository.clone(),
                definition_repository.clone(),
                project_repository.clone(),
                clock.clone(),
            )
            .with_stage_prompt_repository(shot_bulk_repository));
            let production_preparation_service = Arc::new(ProductionPreparationService::new(
                shot_batch_service.clone(),
                shot_batch_repository.clone(),
                shot_readiness_service.clone(),
                definition_repository.clone(),
                project_repository.clone(),
                clock.clone(),
            ));
            let batch_workflow_preset_service = Arc::new(BatchWorkflowPresetService::new(
                settings_service.clone(),
                definition_repository.clone(),
            ));
            let scene_production_service = Arc::new(SceneProductionService::new(
                production_structure_service.clone(),
                shot_batch_service.clone(),
            ));
            let episode_production_service = Arc::new(EpisodeProductionService::new(
                production_structure_service.clone(),
                scene_production_service.clone(),
            ));
            let series_production_service = Arc::new(SeriesProductionService::new(
                production_structure_service.clone(),
                episode_production_service.clone(),
            ));
            let production_batch_runbook_service = Arc::new(ProductionBatchRunbookService::new(
                production_structure_service.clone(),
                production_queue_repository_impl.clone(),
                production_queue_service.clone(),
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
                comfy_memory_service,
                generation_service,
                workflow_library_service,
                workflow_onboarding_service,
                workflow_lifecycle_service,
                workflow_benchmark_service,
                production_orchestrator_service,
                generation_catalog_service,
                task_query_service,
                asset_query_service,
                asset_library_service,
                asset_usage_service,
                production_structure_service,
                project_command_center_service,
                reference_anchor_service,
                consistency_profile_service,
                reference_set_service,
                asset_deletion_service,
                asset_video_prompt_service,
                task_history_service,
                source_asset_import_service,
                h3_local_import_service,
                task_cancellation_service,
                task_recovery_service,
                project_service,
                project_backup_service,
                project_manifest_service,
                preset_service,
                prompt_library_service,
                prompt_template_service,
                prompt_template_bulk_service,
                shot_service,
                shot_batch_service,
                shot_bulk_service,
                organization_service,
                project_template_service,
                production_queue_service,
                production_item_review_service,
                production_audit_service,
                diagnostics_service,
                comfy_preflight_service,
                shot_readiness_service,
                production_preparation_service,
                settings_service,
                batch_workflow_preset_service,
                scene_production_service,
                episode_production_service,
                series_production_service,
                production_batch_runbook_service,
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
            commands::comfy::comfy_get_settings,
            commands::comfy::comfy_test_connection,
            commands::comfy::comfy_save_endpoint,
            commands::comfy::comfy_free_memory,
            commands::preflight::comfy_preflight_current,
            commands::shot_readiness::shot_readiness_cached,
            commands::shot_readiness::shot_preflight,
            commands::shot_readiness::scene_readiness_cached,
            commands::shot_readiness::scene_preflight,
            commands::settings::comfy_environment_profiles_list,
            commands::settings::comfy_environment_profile_save,
            commands::settings::comfy_environment_profile_delete,
            commands::settings::comfy_environment_profile_apply,
            commands::settings::runtime_profiles_list,
            commands::settings::runtime_profiles_save,
            commands::settings::runtime_profiles_delete,
            commands::settings::production_queue_name_presets_list,
            commands::settings::production_queue_name_preset_save,
            commands::settings::production_queue_name_preset_delete,
            commands::settings::workspace_resume_get,
            commands::settings::workspace_resume_save,
            commands::batch_workflow_preset::batch_workflow_presets_list,
            commands::batch_workflow_preset::batch_workflow_preset_create,
            commands::batch_workflow_preset::batch_workflow_preset_update,
            commands::batch_workflow_preset::batch_workflow_preset_delete,
            commands::workflow_library::workflow_library_refresh,
            commands::workflow_onboarding::workflow_onboarding_pick_api_workflow,
            commands::workflow_onboarding::workflow_onboarding_auto_import_api_workflow,
            commands::workflow_onboarding::workflow_onboarding_auto_confirm,
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
            commands::workflow_lifecycle::workflow_runtime_workspace_refresh,
            commands::workflow_lifecycle::workflow_runtime_diagnostics,
            commands::workflow_lifecycle::workflow_repair_builtin_package,
            commands::workflow_lifecycle::workflow_set_enabled,
            commands::workflow_lifecycle::workflow_recheck_capability,
            commands::workflow_lifecycle::workflow_recheck_all_capabilities,
            commands::workflow_lifecycle::workflow_duplicate_recipe,
            commands::workflow_lifecycle::workflow_compare_versions,
            commands::workflow_lifecycle::workflow_export_package,
            commands::workflow_lifecycle::workflow_import_package_backup,
            commands::workflow_lifecycle::workflow_clean_staging,
            commands::workflow_lifecycle::workflow_inspect_deletion,
            commands::workflow_lifecycle::workflow_delete_version,
            commands::workflow_lifecycle::workflow_delete_workflow,
            commands::workflow_lifecycle::workflow_restore_version,
            commands::workflow_benchmark::workflow_benchmark_preview,
            commands::workflow_benchmark::workflow_benchmark_create,
            commands::workflow_benchmark::workflow_benchmark_list,
            commands::workflow_benchmark::workflow_benchmark_get,
            commands::workflow_benchmark::workflow_benchmark_set_winner,
            commands::workflow_benchmark::workflow_benchmark_set_recommendation,
            commands::workflow_benchmark::workflow_benchmark_save_quality,
            commands::workflow_benchmark::workflow_benchmark_clone,
            commands::workflow_benchmark::workflow_benchmark_queue_existing,
            commands::workflow_benchmark::workflow_benchmark_delete,
            commands::catalog::generation_catalog_list,
            commands::generation::generation_create,
            commands::generation::generation_create_batch,
            commands::h3_local_import::h3_local_import_pick_directory,
            commands::h3_local_import::h3_local_import_rescan,
            commands::h3_local_import::h3_local_import_commit,
            commands::h3_local_import::h3_local_import_update_project_segment_draft,
            commands::production_queue::production_queue_create,
            commands::production_queue::production_queue_list,
            commands::production_queue::production_queue_overview,
            commands::production_queue::production_queue_admission_status,
            commands::production_queue::production_queue_get,
            commands::production_queue::production_queue_start,
            commands::production_queue::production_queue_pause,
            commands::production_queue::production_queue_cancel_pending,
            commands::production_queue::production_queue_archive,
            commands::production_queue::production_queue_restore,
            commands::production_queue::production_queue_delete,
            commands::production_queue::production_queue_skip_item,
            commands::production_queue::production_queue_requeue_item,
            commands::production_queue::production_queue_requeue_item_by_item,
            commands::production_queue::production_queue_partial_resume_plan,
            commands::production_queue::production_queue_partial_resume,
            commands::production_item_review::production_item_review_get,
            commands::production_item_review::production_item_review_set_status,
            commands::production_item_review::production_item_review_set_note,
            commands::production_item_review::production_item_review_regenerate,
            commands::production_item_review::production_item_review_regenerate_marked,
            commands::production_audit::production_audit_summary,
            commands::production_audit::production_audit_recent_activity,
            commands::production_audit::production_audit_lineage,
            commands::production_audit::production_audit_integrity,
            commands::production_batch_runbook::production_batch_runbook,
            commands::production_orchestrator::production_run_create,
            commands::production_orchestrator::production_run_list,
            commands::production_orchestrator::production_run_get,
            commands::production_orchestrator::production_run_run_images,
            commands::production_orchestrator::production_run_select_assets,
            commands::production_orchestrator::production_run_run_video,
            commands::production_orchestrator::production_run_retry_video,
            commands::production_orchestrator::production_run_refresh,
            commands::production_orchestrator::production_run_cancel,
            commands::production_orchestrator::production_run_template_save,
            commands::production_orchestrator::production_run_template_list,
            commands::project::project_list,
            commands::project::project_create,
            commands::project::project_update,
            commands::project::project_backup_export,
            commands::project::project_backup_inspect,
            commands::project::project_backup_restore,
            commands::project::project_manifest_export,
            commands::project_command_center::project_command_center_get,
            commands::prompt_library::prompt_library_list,
            commands::prompt_library::prompt_library_get,
            commands::prompt_library::prompt_library_create,
            commands::prompt_library::prompt_library_add_version,
            commands::prompt_library::prompt_library_update_metadata,
            commands::prompt_library::prompt_library_delete,
            commands::prompt_template::prompt_template_analyze,
            commands::prompt_template::prompt_template_preview,
            commands::prompt_template::prompt_template_bulk_preview,
            commands::prompt_template::prompt_template_apply,
            commands::reference_anchor::reference_anchors_list,
            commands::reference_anchor::reference_anchor_get,
            commands::reference_anchor::reference_anchor_create,
            commands::reference_anchor::reference_anchor_update,
            commands::reference_anchor::reference_anchor_delete,
            commands::consistency_assets::consistency_profile_list,
            commands::consistency_assets::consistency_profile_get,
            commands::consistency_assets::character_profile_create,
            commands::consistency_assets::character_profile_update,
            commands::consistency_assets::scene_profile_create,
            commands::consistency_assets::scene_profile_update,
            commands::consistency_assets::prop_profile_create,
            commands::consistency_assets::prop_profile_update,
            commands::consistency_assets::style_profile_create,
            commands::consistency_assets::style_profile_update,
            commands::consistency_assets::consistency_profile_delete,
            commands::consistency_assets::costume_variant_list,
            commands::consistency_assets::costume_variant_get,
            commands::consistency_assets::costume_variant_create,
            commands::consistency_assets::costume_variant_update,
            commands::consistency_assets::costume_variant_delete,
            commands::consistency_assets::reference_set_list,
            commands::consistency_assets::reference_set_detail_get,
            commands::consistency_assets::reference_set_create,
            commands::consistency_assets::reference_set_update,
            commands::consistency_assets::reference_set_delete,
            commands::consistency_assets::reference_set_create_from_anchor,
            commands::consistency_assets::asset_usage_get,
            commands::consistency_assets::profile_usage_get,
            commands::consistency_assets::reference_set_usage_get,
            commands::production_structure::production_structure_tree,
            commands::production_structure::production_series_create,
            commands::production_structure::production_series_update,
            commands::production_structure::production_series_delete,
            commands::production_structure::production_series_reorder,
            commands::production_structure::production_episode_create,
            commands::production_structure::production_episode_update,
            commands::production_structure::production_episode_delete,
            commands::production_structure::production_episode_reorder,
            commands::production_structure::production_scene_create,
            commands::production_structure::production_scene_update,
            commands::production_structure::production_scene_delete,
            commands::production_structure::production_scene_reorder,
            commands::production_structure::production_scene_assign_shots,
            commands::production_structure::production_scene_unassign_shots,
            commands::production_structure::production_scene_reorder_shots,
            commands::shot::shot_list,
            commands::shot::shot_get,
            commands::shot::shot_create,
            commands::shot::shot_update,
            commands::shot::shot_delete,
            commands::shot::shot_reorder,
            commands::shot::shot_stage_config_set,
            commands::shot::shot_references_replace,
            commands::shot::shot_result_select,
            commands::shot::shot_generate,
            commands::shot_batch::shot_batch_plan,
            commands::shot_batch::shot_batch_create,
            commands::scene_production::scene_production_plan,
            commands::scene_production::scene_production_prepare,
            commands::scene_production::scene_production_readiness_summary,
            commands::episode_production::episode_production_plan,
            commands::episode_production::episode_production_prepare,
            commands::episode_production::episode_production_readiness_summary,
            commands::series_production::series_production_plan,
            commands::series_production::series_production_prepare,
            commands::series_production::series_production_readiness_summary,
            commands::production_preparation::scene_production_preflight,
            commands::production_preparation::scene_production_admit,
            commands::production_preparation::shot_production_plan_detail,
            commands::shot_bulk::preview_shot_bulk_import,
            commands::shot_bulk::commit_shot_bulk_import,
            commands::shot_bulk::bulk_assign_shot_prompt,
            commands::shot_bulk::bulk_set_shot_stage_config,
            commands::organization::project_template_list,
            commands::organization::project_template_create,
            commands::organization::project_template_update,
            commands::organization::project_template_delete,
            commands::organization::project_template_create_project,
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
            commands::preset::preset_get_preferred,
            commands::preset::preset_set_preferred,
            commands::asset::asset_list_by_task,
            commands::asset::asset_list_recent,
            commands::asset::asset_pick_and_import_image,
            commands::asset::asset_pick_and_import_source_assets,
            commands::asset::asset_pick_and_import_video,
            commands::asset::asset_pick_and_import_audio,
            commands::asset::asset_read_image,
            commands::asset::asset_read_thumbnail,
            commands::asset::asset_library_page,
            commands::asset::asset_get,
            commands::asset::inspect_asset_deletion,
            commands::asset::delete_assets,
            commands::asset::asset_video_prompt_get,
            commands::asset::asset_video_prompt_list,
            commands::asset::asset_video_prompt_set,
            commands::organization::asset_tag_list,
            commands::organization::asset_tag_create,
            commands::organization::asset_tag_rename,
            commands::organization::asset_tag_delete,
            commands::organization::asset_tag_assign,
            commands::organization::asset_tag_remove,
            commands::organization::asset_set_favorite,
            commands::organization::asset_bulk_set_favorite,
            commands::organization::asset_bulk_add_tag,
            commands::organization::asset_bulk_remove_tag,
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
