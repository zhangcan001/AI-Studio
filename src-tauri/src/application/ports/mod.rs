//! Application ports are the boundary for infrastructure implementations.

pub mod asset_repository;
pub mod asset_store;
pub mod clock;
pub mod comfy_adapter;
pub mod generation_definition_repository;
pub mod generation_snapshot_repository;
pub mod project_repository;
pub mod repository_error;
pub mod task_repository;

pub use asset_repository::AssetRepository;
pub use asset_store::{AssetStore, AssetStoreError, StoredAssetFile};
pub use clock::{Clock, MonotonicEventClock};
pub use comfy_adapter::{
    ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyEventSubscription,
    ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyNodeOutput, ComfyOutputData,
    ComfyOutputFile, DeviceInfo, PromptSubmission, SystemStats,
};
pub use generation_definition_repository::{GenerationDefinition, GenerationDefinitionRepository};
pub use generation_snapshot_repository::GenerationSnapshotRepository;
pub use project_repository::ProjectRepository;
pub use repository_error::RepositoryError;
pub use task_repository::TaskRepository;
