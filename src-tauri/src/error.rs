use serde::Serialize;
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
}

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
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

    pub fn code(&self) -> &'static str {
        match self.code {
            AppErrorCode::InitializationError => "INITIALIZATION_ERROR",
            AppErrorCode::DatabaseError => "DATABASE_ERROR",
            AppErrorCode::FilesystemError => "FILESYSTEM_ERROR",
            AppErrorCode::InternalError => "INTERNAL_ERROR",
            AppErrorCode::ComfyOffline => "COMFY_OFFLINE",
            AppErrorCode::ComfyTimeout => "COMFY_TIMEOUT",
            AppErrorCode::ComfyProtocolError => "COMFY_PROTOCOL_ERROR",
        }
    }

    fn new(code: AppErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
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
