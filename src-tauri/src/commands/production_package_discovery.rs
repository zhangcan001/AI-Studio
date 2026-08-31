//! Tauri transport for read-only multi-package filesystem discovery.

use crate::{
    application::production_package_discovery_service::{
        ProductionPackageDiscoveryError, ProductionPackageDiscoveryPackage,
        ProductionPackageDiscoveryService,
    },
    error::AppError,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageDiscoverView {
    pub root_path: String,
    pub packages: Vec<ProductionPackageDiscoveryPackage>,
}

#[tauri::command(rename_all = "camelCase")]
pub fn production_package_discover(
    root_path: String,
) -> Result<ProductionPackageDiscoverView, AppError> {
    let result = ProductionPackageDiscoveryService::new()
        .discover(&root_path)
        .map_err(map_discovery_error)?;
    Ok(ProductionPackageDiscoverView {
        root_path: result.root_path,
        packages: result.packages,
    })
}

fn map_discovery_error(error: ProductionPackageDiscoveryError) -> AppError {
    match error {
        ProductionPackageDiscoveryError::InvalidRoot(message) => AppError::invalid_input(message),
        ProductionPackageDiscoveryError::Filesystem(message) => AppError::filesystem(message),
        other => AppError::filesystem(other.to_string()),
    }
}
