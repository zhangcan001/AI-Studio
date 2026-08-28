//! Thin Tauri transport for the external Production Package V1 boundary.
//!
//! Inspection accepts only a package root and returns a short-lived
//! `inspectionId`. Commit accepts that id plus explicitly selected external
//! item labels; the package JSON is never submitted a second time and the
//! service performs the authoritative revalidation.

use crate::{
    app_state::AppState,
    application::{
        production_package_inspector::ProductionPackageInspection,
        production_package_service::{
            ProductionPackageBatchMapping, ProductionPackageCreateBatchesResult,
            ProductionPackageError, ProductionPackageItemMapping,
        },
    },
    error::AppError,
};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductionPackageInspectRequest {
    pub project_id: String,
    pub package_root: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageInspectionView {
    pub inspection_id: String,
    #[serde(flatten)]
    pub inspection: ProductionPackageInspection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductionPackageCreateBatchesRequest {
    pub inspection_id: String,
    pub selected_item_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageItemMappingView {
    pub package_item_id: String,
    pub batch_id: String,
    pub batch_item_id: String,
    pub imported_asset_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageBatchMappingView {
    pub batch_id: String,
    pub batch_name: String,
    pub item_count: usize,
    pub item_mappings: Vec<ProductionPackageItemMappingView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageCreateBatchesView {
    pub package_name: String,
    pub batch_count: usize,
    pub item_count: usize,
    pub auto_started: bool,
    pub batches: Vec<ProductionPackageBatchMappingView>,
    pub item_mappings: Vec<ProductionPackageItemMappingView>,
    pub warnings:
        Vec<crate::application::production_package_inspector::ProductionPackageDiagnostic>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_package_inspect(
    state: State<'_, AppState>,
    request: ProductionPackageInspectRequest,
) -> Result<ProductionPackageInspectionView, AppError> {
    let (inspection_id, inspection) = state
        .production_package_service
        .inspect_session(&request.project_id, request.package_root.into())
        .await
        .map_err(map_package_error)?;
    Ok(ProductionPackageInspectionView {
        inspection_id,
        inspection,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_package_create_batches(
    state: State<'_, AppState>,
    request: ProductionPackageCreateBatchesRequest,
) -> Result<ProductionPackageCreateBatchesView, AppError> {
    let result = state
        .production_package_service
        .create_batches(&request.inspection_id, &request.selected_item_ids)
        .await
        .map_err(map_package_error)?;
    Ok(create_batches_view(result))
}

fn create_batches_view(
    result: ProductionPackageCreateBatchesResult,
) -> ProductionPackageCreateBatchesView {
    ProductionPackageCreateBatchesView {
        package_name: result.package_name,
        batch_count: result.batch_count,
        item_count: result.item_count,
        auto_started: result.auto_started,
        batches: result.batches.into_iter().map(batch_mapping_view).collect(),
        item_mappings: result
            .item_mappings
            .into_iter()
            .map(item_mapping_view)
            .collect(),
        warnings: result.warnings,
    }
}

fn batch_mapping_view(mapping: ProductionPackageBatchMapping) -> ProductionPackageBatchMappingView {
    ProductionPackageBatchMappingView {
        batch_id: mapping.batch_id,
        batch_name: mapping.batch_name,
        item_count: mapping.item_count,
        item_mappings: mapping
            .item_mappings
            .into_iter()
            .map(item_mapping_view)
            .collect(),
    }
}

fn item_mapping_view(mapping: ProductionPackageItemMapping) -> ProductionPackageItemMappingView {
    ProductionPackageItemMappingView {
        package_item_id: mapping.package_item_id,
        batch_id: mapping.batch_id,
        batch_item_id: mapping.batch_item_id,
        imported_asset_ids: mapping.imported_asset_ids,
    }
}

fn map_package_error(error: ProductionPackageError) -> AppError {
    match error {
        ProductionPackageError::Filesystem(message) => AppError::filesystem(message),
        ProductionPackageError::H3(message) => AppError::invalid_input(message),
        ProductionPackageError::Queue(message) => AppError::database(message),
        ProductionPackageError::Inspection(error) => AppError::invalid_input(error.to_string()),
        ProductionPackageError::InvalidInput(message)
        | ProductionPackageError::MediaChanged(message)
        | ProductionPackageError::PromptChanged(message)
        | ProductionPackageError::ModeChanged(message)
        | ProductionPackageError::ItemNotFound(message)
        | ProductionPackageError::ItemBlocked(message)
        | ProductionPackageError::DuplicateItemId(message) => AppError::invalid_input(message),
        ProductionPackageError::SessionNotFound => {
            AppError::invalid_input("production package inspection session was not found")
        }
        ProductionPackageError::SessionExpired => {
            AppError::invalid_input("production package inspection session expired")
        }
    }
}
