//! Domain types and rules live here.
//!
//! This module intentionally has no dependency on Tauri, SQLx, HTTP clients,
//! or other infrastructure concerns.

pub mod asset;
pub mod generation_snapshot;
pub mod project_id;
pub mod recipe;
pub mod task;
pub mod workflow;

pub use asset::{Asset, AssetDomainError, AssetId, AssetType};
pub use generation_snapshot::{GenerationSnapshot, SnapshotDomainError, SnapshotId};
pub use project_id::{validate_project_id, ProjectIdValidationError};
pub use recipe::{
    Binding, BindingTarget, CompileRequest, InputDefinition, InputValue, OutputDefinition,
    OutputType, Recipe, RecipeError, ResolvedInputValue, SeedDefault, SeedValue, WorkflowRef,
};
pub use task::{
    NewTaskEvent, StoredTaskEvent, Task, TaskDomainError, TaskError, TaskEventType, TaskId,
    TaskProgress, TaskStateMachine, TaskStatus,
};
pub use workflow::{WorkflowDocument, WorkflowError};
