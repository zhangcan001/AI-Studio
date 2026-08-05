//! Domain types and rules live here.
//!
//! This module intentionally has no dependency on Tauri, SQLx, HTTP clients,
//! or other infrastructure concerns.

pub mod generation_snapshot;
pub mod recipe;
pub mod task;
pub mod workflow;

pub use generation_snapshot::{GenerationSnapshot, SnapshotDomainError, SnapshotId};
pub use recipe::{
    Binding, BindingTarget, CompileRequest, InputDefinition, InputValue, OutputDefinition,
    OutputType, Recipe, RecipeError, ResolvedInputValue, SeedDefault, SeedValue, WorkflowRef,
};
pub use task::{
    NewTaskEvent, StoredTaskEvent, Task, TaskDomainError, TaskError, TaskEventType, TaskId,
    TaskProgress, TaskStateMachine, TaskStatus,
};
pub use workflow::{WorkflowDocument, WorkflowError};
