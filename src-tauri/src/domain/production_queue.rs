use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProductionBatchId(String);

impl ProductionBatchId {
    pub fn new() -> Self {
        Self(format!("pbt_{}", Uuid::new_v4().simple()))
    }

    pub fn parse(value: String) -> Result<Self, ProductionQueueDomainError> {
        if !value.starts_with("pbt_") || value.len() < 8 {
            return Err(ProductionQueueDomainError::InvalidId(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProductionBatchItemId(String);

impl ProductionBatchItemId {
    pub fn new() -> Self {
        Self(format!("pbi_{}", Uuid::new_v4().simple()))
    }

    pub fn parse(value: String) -> Result<Self, ProductionQueueDomainError> {
        if !value.starts_with("pbi_") || value.len() < 8 {
            return Err(ProductionQueueDomainError::InvalidId(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductionBatchStatus {
    Ready,
    Running,
    Paused,
    Completed,
}

impl ProductionBatchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Running => "RUNNING",
            Self::Paused => "PAUSED",
            Self::Completed => "COMPLETED",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ProductionQueueDomainError> {
        match value {
            "READY" => Ok(Self::Ready),
            "RUNNING" => Ok(Self::Running),
            "PAUSED" => Ok(Self::Paused),
            "COMPLETED" => Ok(Self::Completed),
            other => Err(ProductionQueueDomainError::InvalidStatus(other.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductionBatchItemStatus {
    Pending,
    Dispatching,
    Dispatched,
    Succeeded,
    Failed,
    Cancelled,
    Skipped,
}

impl ProductionBatchItemStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Dispatching => "DISPATCHING",
            Self::Dispatched => "DISPATCHED",
            Self::Succeeded => "SUCCEEDED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Skipped => "SKIPPED",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ProductionQueueDomainError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "DISPATCHING" => Ok(Self::Dispatching),
            "DISPATCHED" => Ok(Self::Dispatched),
            "SUCCEEDED" => Ok(Self::Succeeded),
            "FAILED" => Ok(Self::Failed),
            "CANCELLED" => Ok(Self::Cancelled),
            "SKIPPED" => Ok(Self::Skipped),
            other => Err(ProductionQueueDomainError::InvalidStatus(other.to_owned())),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionBatch {
    pub id: ProductionBatchId,
    pub project_id: String,
    pub name: String,
    pub status: ProductionBatchStatus,
    pub continue_on_failure: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionBatchItem {
    pub id: ProductionBatchItemId,
    pub batch_id: ProductionBatchId,
    pub ordinal: u32,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values_json: Value,
    pub status: ProductionBatchItemStatus,
    pub task_id: Option<String>,
    pub retry_of_item_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionBatchDetail {
    pub batch: ProductionBatch,
    pub items: Vec<ProductionBatchItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionQueueDomainError {
    InvalidId(String),
    InvalidStatus(String),
}

impl fmt::Display for ProductionQueueDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(value) => write!(formatter, "invalid production queue id: {value}"),
            Self::InvalidStatus(value) => write!(formatter, "invalid production queue status: {value}"),
        }
    }
}

impl Error for ProductionQueueDomainError {}
