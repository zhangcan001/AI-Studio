//! Pure local Recipe and API Workflow compilation.
//!
//! This module deliberately has no dependency on Tauri, ComfyUI, HTTP, SQLite,
//! or filesystem services. It only transforms in-memory domain values.

mod binding;
mod errors;
mod final_validator;
mod parser;
mod seed;
mod validator;
mod workflow_compiler;

pub use binding::BindingValidator;
pub use errors::CompileError;
pub use final_validator::{
    compiled_workflow_sha256, CompiledMediaMapping, FinalCompiledWorkflowValidator,
};
pub use parser::RecipeParser;
pub use seed::SeedResolver;
pub use validator::{number_is_aligned_to_step, RecipeValidator, WorkflowValidator};
pub use workflow_compiler::{CompileResult, WorkflowCompiler};
