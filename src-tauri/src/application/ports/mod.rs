//! Application ports are the boundary for infrastructure implementations.

pub mod comfy_adapter;
pub mod generation_snapshot_repository;
pub mod repository_error;
pub mod task_repository;

pub use comfy_adapter::{
    ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyHealth, DeviceInfo, SystemStats,
};
pub use generation_snapshot_repository::GenerationSnapshotRepository;
pub use repository_error::RepositoryError;
pub use task_repository::TaskRepository;
