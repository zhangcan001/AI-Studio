//! Application ports are the boundary for infrastructure implementations.

pub mod comfy_adapter;
pub mod generation_definition_repository;
pub mod generation_snapshot_repository;
pub mod repository_error;
pub mod task_repository;

pub use comfy_adapter::{
    ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyEventSubscription,
    ComfyExecutionEvent, ComfyHealth, DeviceInfo, PromptSubmission, SystemStats,
};
pub use generation_definition_repository::{GenerationDefinition, GenerationDefinitionRepository};
pub use generation_snapshot_repository::GenerationSnapshotRepository;
pub use repository_error::RepositoryError;
pub use task_repository::TaskRepository;
