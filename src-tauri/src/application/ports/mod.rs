//! Application ports are the boundary for infrastructure implementations.

pub mod comfy_adapter;

pub use comfy_adapter::{
    ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyHealth, DeviceInfo, SystemStats,
};
