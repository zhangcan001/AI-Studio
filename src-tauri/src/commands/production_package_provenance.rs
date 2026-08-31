//! Read-only transport for durable Production Package provenance bindings.
//!
//! Main must register this command after adding the module to `commands/mod.rs`.

use crate::{app_state::AppState, error::AppError};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageBatchBindingView {
    pub package_key: String,
    pub package_root: String,
    pub manifest_sha256: String,
    pub package_id: Option<String>,
    pub package_name: String,
    pub batch_id: String,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub package_item_ids: Vec<String>,
    pub created_at: String,
    pub source_kind: String,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_package_bindings_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ProductionPackageBatchBindingView>, AppError> {
    let bindings = state
        .production_queue_service
        .list_package_bindings(&project_id)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
    Ok(bindings
        .into_iter()
        .map(|binding| ProductionPackageBatchBindingView {
            package_key: binding.package_key,
            package_root: binding.package_root,
            manifest_sha256: binding.manifest_sha256,
            package_id: binding.package_id,
            package_name: binding.package_name,
            batch_id: binding.batch_id,
            chunk_index: binding.chunk_index,
            chunk_count: binding.chunk_count,
            package_item_ids: binding.package_item_ids,
            created_at: binding.created_at.to_rfc3339(),
            source_kind: binding.source_kind,
        })
        .collect())
}
