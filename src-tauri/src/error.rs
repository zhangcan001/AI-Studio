use serde::Serialize;
use serde_json::Value;
use std::{error::Error, fmt};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppErrorCode {
    InitializationError,
    DatabaseError,
    FilesystemError,
    InternalError,
    ComfyOffline,
    ComfyTimeout,
    ComfyProtocolError,
    WorkflowPackageInvalid,
    WorkflowVersionConflict,
    RecipeVersionConflict,
    GenerationDefinitionNotFound,
    ProjectNotFound,
    InvalidInput,
    TaskNotFound,
    TaskNotCancellable,
    AssetNotFound,
    AssetReadFailed,
    ReusableDraftUnavailable,
    WorkflowOnboardingError,
    ProductionQueueBusy,
    ComfyEndpointInvalid,
    ComfyEndpointTestFailed,
    ComfyEndpointChangeBusy,
    SettingsSaveFailed,
    BackupInvalid,
    BackupInspectionExpired,
    BackupAssetHashMismatch,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl AppError {
    pub fn initialization(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::InitializationError, message)
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::DatabaseError, message)
    }

    pub fn filesystem(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::FilesystemError, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::InternalError, message)
    }

    pub fn comfy_offline(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ComfyOffline, message)
    }

    pub fn comfy_timeout(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ComfyTimeout, message)
    }

    pub fn comfy_protocol_error(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ComfyProtocolError, message)
    }

    pub fn workflow_package_invalid(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::WorkflowPackageInvalid, message)
    }

    pub fn workflow_version_conflict(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::WorkflowVersionConflict, message)
    }

    pub fn recipe_version_conflict(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::RecipeVersionConflict, message)
    }

    pub fn generation_definition_not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::GenerationDefinitionNotFound, message)
    }

    pub fn project_not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ProjectNotFound, message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::InvalidInput, message)
    }

    pub fn task_not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::TaskNotFound, message)
    }

    pub fn task_not_cancellable(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::TaskNotCancellable, message)
    }

    pub fn asset_not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::AssetNotFound, message)
    }

    pub fn asset_read_failed(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::AssetReadFailed, message)
    }

    pub fn reusable_draft_unavailable(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ReusableDraftUnavailable, message)
    }

    pub fn workflow_onboarding(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::WorkflowOnboardingError, message)
    }

    pub fn production_queue_busy(message: impl Into<String>, details: Value) -> Self {
        let mut error = Self::new(AppErrorCode::ProductionQueueBusy, message);
        error.details = Some(details);
        error
    }

    pub fn comfy_endpoint_invalid(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ComfyEndpointInvalid, message)
    }

    pub fn comfy_endpoint_test_failed(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ComfyEndpointTestFailed, message)
    }

    pub fn comfy_endpoint_change_busy(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ComfyEndpointChangeBusy, message)
    }

    pub fn settings_save_failed(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::SettingsSaveFailed, message)
    }

    pub fn backup_invalid(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::BackupInvalid, message)
    }

    pub fn backup_inspection_expired(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::BackupInspectionExpired, message)
    }

    pub fn backup_asset_hash_mismatch(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::BackupAssetHashMismatch, message)
    }

    pub fn code(&self) -> &'static str {
        match self.code {
            AppErrorCode::InitializationError => "INITIALIZATION_ERROR",
            AppErrorCode::DatabaseError => "DATABASE_ERROR",
            AppErrorCode::FilesystemError => "FILESYSTEM_ERROR",
            AppErrorCode::InternalError => "INTERNAL_ERROR",
            AppErrorCode::ComfyOffline => "COMFY_OFFLINE",
            AppErrorCode::ComfyTimeout => "COMFY_TIMEOUT",
            AppErrorCode::ComfyProtocolError => "COMFY_PROTOCOL_ERROR",
            AppErrorCode::WorkflowPackageInvalid => "WORKFLOW_PACKAGE_INVALID",
            AppErrorCode::WorkflowVersionConflict => "WORKFLOW_VERSION_CONFLICT",
            AppErrorCode::RecipeVersionConflict => "RECIPE_VERSION_CONFLICT",
            AppErrorCode::GenerationDefinitionNotFound => "GENERATION_DEFINITION_NOT_FOUND",
            AppErrorCode::ProjectNotFound => "PROJECT_NOT_FOUND",
            AppErrorCode::InvalidInput => "INVALID_INPUT",
            AppErrorCode::TaskNotFound => "TASK_NOT_FOUND",
            AppErrorCode::TaskNotCancellable => "TASK_NOT_CANCELLABLE",
            AppErrorCode::AssetNotFound => "ASSET_NOT_FOUND",
            AppErrorCode::AssetReadFailed => "ASSET_READ_FAILED",
            AppErrorCode::ReusableDraftUnavailable => "REUSABLE_DRAFT_UNAVAILABLE",
            AppErrorCode::WorkflowOnboardingError => "WORKFLOW_ONBOARDING_ERROR",
            AppErrorCode::ProductionQueueBusy => "PRODUCTION_QUEUE_BUSY",
            AppErrorCode::ComfyEndpointInvalid => "COMFY_ENDPOINT_INVALID",
            AppErrorCode::ComfyEndpointTestFailed => "COMFY_ENDPOINT_TEST_FAILED",
            AppErrorCode::ComfyEndpointChangeBusy => "COMFY_ENDPOINT_CHANGE_BUSY",
            AppErrorCode::SettingsSaveFailed => "SETTINGS_SAVE_FAILED",
            AppErrorCode::BackupInvalid => "BACKUP_INVALID",
            AppErrorCode::BackupInspectionExpired => "BACKUP_INSPECTION_EXPIRED",
            AppErrorCode::BackupAssetHashMismatch => "BACKUP_ASSET_HASH_MISMATCH",
        }
    }

    fn new(code: AppErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.message)
    }
}

impl Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::filesystem(error.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::database(error.to_string())
    }
}
