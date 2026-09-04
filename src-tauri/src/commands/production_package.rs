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
use serde_json::json;
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
    let package_error_code = error.code().to_owned();
    let technical_message = error.to_string();
    let requires_reinspect = matches!(
        &error,
        ProductionPackageError::ProjectWorkflowChanged { .. }
    );
    let (message, mode, workflow_version_id, recipe_id, item_id) = match &error {
        ProductionPackageError::ProjectWorkflowUnavailable { mode, .. } => (
            "当前项目没有可用于该生产模式的工作流。",
            Some(mode.as_str()),
            None,
            None,
            None,
        ),
        ProductionPackageError::RecipeIncompatible {
            mode,
            workflow_version_id,
            recipe_id,
            ..
        } => (
            "工作流不兼容当前生产模式。",
            Some(mode.as_str()),
            Some(workflow_version_id.as_str()),
            Some(recipe_id.as_str()),
            None,
        ),
        ProductionPackageError::ProjectWorkflowChanged { mode } => (
            "项目工作流配置在检查后发生变化，请重新检查生产包。",
            Some(mode.as_str()),
            None,
            None,
            None,
        ),
        ProductionPackageError::ItemsAlreadyCreated { item_ids } => (
            "所选生产包项目已经创建过生产批次。",
            None,
            None,
            None,
            item_ids.first().map(String::as_str),
        ),
        ProductionPackageError::H3(_) => ("H3 导入阶段失败。", None, None, None, None),
        ProductionPackageError::Queue(_) => ("生产队列创建失败。", None, None, None, None),
        ProductionPackageError::MediaChanged(_) => (
            "检查后的媒体文件已变化，请重新检查生产包。",
            None,
            None,
            None,
            None,
        ),
        ProductionPackageError::PromptChanged(_)
        | ProductionPackageError::ModeChanged(_)
        | ProductionPackageError::ItemNotFound(_)
        | ProductionPackageError::ItemBlocked(_) => (
            "生产包内容在检查后发生变化，请重新检查。",
            None,
            None,
            None,
            None,
        ),
        ProductionPackageError::DuplicateItemId(_) => (
            "生产包包含重复项目，请修复后重新检查。",
            None,
            None,
            None,
            None,
        ),
        ProductionPackageError::SessionNotFound => (
            "生产包检查结果不存在，请重新检查文件夹。",
            None,
            None,
            None,
            None,
        ),
        ProductionPackageError::SessionExpired => (
            "生产包检查结果已过期，请重新检查文件夹。",
            None,
            None,
            None,
            None,
        ),
        ProductionPackageError::Filesystem(_) => (
            "生产包文件访问失败，请检查文件权限。",
            None,
            None,
            None,
            None,
        ),
        ProductionPackageError::Inspection(_) => {
            ("生产包检查失败，请查看错误详情。", None, None, None, None)
        }
        ProductionPackageError::InvalidInput(_) => {
            ("生产包输入无效，请检查后重试。", None, None, None, None)
        }
    };

    let item_id = match &error {
        ProductionPackageError::PromptChanged(item_id)
        | ProductionPackageError::ModeChanged(item_id)
        | ProductionPackageError::ItemNotFound(item_id)
        | ProductionPackageError::ItemBlocked(item_id)
        | ProductionPackageError::DuplicateItemId(item_id) => Some(item_id.as_str()),
        _ => item_id,
    };

    AppError::production_package(
        &package_error_code,
        message,
        json!({
            "technicalMessage": technical_message,
            "mode": mode,
            "workflowVersionId": workflow_version_id,
            "recipeId": recipe_id,
            "itemId": item_id,
            "requiresReinspect": requires_reinspect,
        }),
    )
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

    #[test]
    fn package_errors_keep_domain_code_and_safe_context() {
        let error = map_package_error(ProductionPackageError::ProjectWorkflowUnavailable {
            mode: "FL2VA_TEXT_TO_VIDEO".to_owned(),
            message: "Recipe is unavailable".to_owned(),
        });
        let value = serde_json::to_value(error).expect("app error should serialize");

        assert_eq!(value["code"], "PRODUCTION_PACKAGE_ERROR");
        assert_eq!(
            value["details"]["packageErrorCode"],
            "PROJECT_WORKFLOW_UNAVAILABLE_FOR_PACKAGE_MODE"
        );
        assert_eq!(value["details"]["mode"], "FL2VA_TEXT_TO_VIDEO");
        assert_eq!(value["details"]["technicalMessage"], "PROJECT_WORKFLOW_UNAVAILABLE_FOR_PACKAGE_MODE: mode FL2VA_TEXT_TO_VIDEO: Recipe is unavailable");
        assert!(value["details"].get("prompt").is_none());
        assert!(value["details"].get("databasePath").is_none());
    }

    #[test]
    fn package_error_mapping_does_not_collapse_h3_or_queue_to_invalid_input() {
        for error in [
            ProductionPackageError::H3("missing node".to_owned()),
            ProductionPackageError::Queue("queue unavailable".to_owned()),
        ] {
            let value =
                serde_json::to_value(map_package_error(error)).expect("error should serialize");
            assert_eq!(value["code"], "PRODUCTION_PACKAGE_ERROR");
            assert_ne!(value["code"], "INVALID_INPUT");
        }
    }

    #[test]
    fn package_recipe_and_project_change_errors_keep_structured_context() {
        let recipe = serde_json::to_value(map_package_error(
            ProductionPackageError::RecipeIncompatible {
                mode: "FL2VA_TEXT_TO_VIDEO".to_owned(),
                workflow_version_id: "wfv-123".to_owned(),
                recipe_id: "rcp-456".to_owned(),
                message: "Recipe is missing required package input width".to_owned(),
            },
        ))
        .expect("recipe error should serialize");
        assert_eq!(recipe["code"], "PRODUCTION_PACKAGE_ERROR");
        assert_eq!(
            recipe["details"]["packageErrorCode"],
            "PACKAGE_RECIPE_INCOMPATIBLE"
        );
        assert_eq!(recipe["details"]["mode"], "FL2VA_TEXT_TO_VIDEO");
        assert_eq!(recipe["details"]["workflowVersionId"], "wfv-123");
        assert_eq!(recipe["details"]["recipeId"], "rcp-456");
        assert_eq!(
            recipe["details"]["technicalMessage"],
            "PACKAGE_RECIPE_INCOMPATIBLE: mode FL2VA_TEXT_TO_VIDEO, workflowVersionId wfv-123, recipeId rcp-456: Recipe is missing required package input width"
        );

        let changed = serde_json::to_value(map_package_error(
            ProductionPackageError::ProjectWorkflowChanged {
                mode: "FL2VA_TEXT_TO_VIDEO".to_owned(),
            },
        ))
        .expect("project change error should serialize");
        assert_eq!(
            changed["details"]["packageErrorCode"],
            "PACKAGE_PROJECT_WORKFLOW_CHANGED"
        );
        assert_eq!(changed["details"]["mode"], "FL2VA_TEXT_TO_VIDEO");
        assert_eq!(changed["details"]["requiresReinspect"], true);
    }
}
