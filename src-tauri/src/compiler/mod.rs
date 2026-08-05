//! Pure local Recipe and API Workflow compilation.
//!
//! This module deliberately has no dependency on Tauri, ComfyUI, HTTP, SQLite,
//! or filesystem services. It only transforms in-memory domain values.

mod binding;
mod errors;
mod parser;
mod seed;
mod validator;
mod workflow_compiler;

pub use binding::BindingValidator;
pub use errors::CompileError;
pub use parser::RecipeParser;
pub use seed::SeedResolver;
pub use validator::{RecipeValidator, WorkflowValidator};
pub use workflow_compiler::{CompileResult, WorkflowCompiler};
