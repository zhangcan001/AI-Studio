//! Application ports are the boundary for infrastructure implementations.

pub mod asset_browse_repository;
pub mod asset_repository;
pub mod asset_store;
pub mod clock;
pub mod comfy_adapter;
pub mod generation_definition_repository;
pub mod generation_snapshot_repository;
pub mod preset_repository;
pub mod project_directory_store;
pub mod project_repository;
pub mod repository_error;
pub mod task_history_repository;
pub mod task_repository;
pub mod task_update_sink;
pub mod workflow_library_repository;
pub mod workflow_library_source;

pub use asset_browse_repository::{AssetBrowseRepository, AssetCategoryFilter};
pub use asset_repository::{AssetRepository, TaskOutputAssetMapping};
pub use asset_store::{AssetStore, AssetStoreError, AssetWriteSession, StoredAssetFile};
pub use clock::{Clock, MonotonicEventClock};
pub use comfy_adapter::{
    CancelPromptResult, ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig,
    ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyHistoryStatus,
    ComfyImageUpload, ComfyNodeOutput, ComfyOutputData, ComfyOutputFile, ComfyOutputStream,
    ComfyQueueState, ComfySavedResult, ComfyUploadedImage, DeviceInfo, PromptSubmission,
    SystemStats,
};
pub use generation_definition_repository::{
    AvailableGenerationDefinition, GenerationDefinition, GenerationDefinitionRepository,
};
pub use generation_snapshot_repository::GenerationSnapshotRepository;
pub use preset_repository::PresetRepository;
pub use project_directory_store::{ProjectDirectoryStore, ProjectDirectoryStoreError};
pub use project_repository::{ProjectRecord, ProjectRepository};
pub use repository_error::RepositoryError;
pub use task_history_repository::{TaskHistoryFilter, TaskHistoryRecord, TaskHistoryRepository};
pub use task_repository::TaskRepository;
pub use task_update_sink::{
    NoopTaskUpdateSink, TaskUpdatePayload, TaskUpdateSink, TASK_UPDATED_EVENT,
};
pub use workflow_library_repository::{
    WorkflowLibraryRepository, WorkflowPackageRecord, WorkflowPackageRegistration,
};
pub use workflow_library_source::{
    WorkflowLibrarySource, WorkflowLibrarySourceError, WorkflowPackageFiles, WorkflowPackageLoad,
};
