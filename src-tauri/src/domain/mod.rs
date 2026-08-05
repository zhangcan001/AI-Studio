//! Domain types and rules live here.
//!
//! This module intentionally has no dependency on Tauri, SQLx, HTTP clients,
//! or other infrastructure concerns.

pub mod recipe;
pub mod workflow;

pub use recipe::{
    Binding, BindingTarget, CompileRequest, InputDefinition, InputValue, OutputDefinition,
    OutputType, Recipe, RecipeError, ResolvedInputValue, SeedDefault, SeedValue, WorkflowRef,
};
pub use workflow::{WorkflowDocument, WorkflowError};
