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
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

const PRODUCTION_PACKAGE_PICK_ROOT_DIALOG_TITLE: &str = "选择 Production Package 文件夹";

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
    pub status: String,
    pub requested_count: usize,
    pub created_count: usize,
    pub remaining_count: usize,
    pub remaining_item_ids: Vec<String>,
    pub batch_count: usize,
    pub item_count: usize,
    pub auto_started: bool,
    pub batches: Vec<ProductionPackageBatchMappingView>,
    pub item_mappings: Vec<ProductionPackageItemMappingView>,
    pub warnings:
        Vec<crate::application::production_package_inspector::ProductionPackageDiagnostic>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_package_pick_root(
    app_handle: AppHandle,
) -> Result<Option<String>, AppError> {
    let Some(folder) = app_handle
        .dialog()
        .file()
        .set_title(PRODUCTION_PACKAGE_PICK_ROOT_DIALOG_TITLE)
        .blocking_pick_folder()
    else {
        return Ok(None);
    };

    let root_path = folder
        .into_path()
        .map_err(|_| AppError::filesystem("所选 Production Package 文件夹无法读取"))?;
    Ok(Some(root_path.to_string_lossy().into_owned()))
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
    let status = result.status.as_str().to_owned();
    ProductionPackageCreateBatchesView {
        package_name: result.package_name,
        status,
        requested_count: result.requested_count,
        created_count: result.created_count,
        remaining_count: result.remaining_count,
        remaining_item_ids: result.remaining_item_ids,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_value, json};

    #[test]
    fn pick_root_contract_keeps_the_user_facing_dialog_title() {
        assert_eq!(
            PRODUCTION_PACKAGE_PICK_ROOT_DIALOG_TITLE,
            "选择 Production Package 文件夹"
        );
    }

    #[test]
    fn package_requests_use_camel_case_and_create_only_accepts_selection() {
        let inspect: ProductionPackageInspectRequest = from_value(json!({
            "projectId": "project-1",
            "packageRoot": "C:/packages/ep01"
        }))
        .expect("inspect request should accept the stable camelCase contract");
        assert_eq!(inspect.project_id, "project-1");
        assert_eq!(inspect.package_root, "C:/packages/ep01");

        let create: ProductionPackageCreateBatchesRequest = from_value(json!({
            "inspectionId": "inspection-1",
            "selectedItemIds": ["external-item-1"]
        }))
        .expect("create request should accept the stable camelCase contract");
        assert_eq!(create.inspection_id, "inspection-1");
        assert_eq!(create.selected_item_ids, vec!["external-item-1".to_owned()]);

        let rejected = from_value::<ProductionPackageCreateBatchesRequest>(json!({
            "inspectionId": "inspection-1",
            "selectedItemIds": ["external-item-1"],
            "packageRoot": "C:/packages/ep01",
            "media": ["images/shot.png"],
            "workflowVersionId": "must-not-be-client-input"
        }));
        assert!(
            rejected.is_err(),
            "create must not accept package or execution inputs"
        );
    }
}
