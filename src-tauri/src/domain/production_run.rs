//! Explicit state rules for the fixed three-stage production workflow.
//!
//! The first orchestrator release intentionally models a small linear flow
//! instead of a general DAG editor.  Persistence and execution remain in the
//! application layer; this module only owns names and legal transitions.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductionRunStatus {
    Draft,
    Ready,
    Running,
    WaitingForSelection,
    Succeeded,
    PartialFailed,
    Failed,
    Cancelled,
}

impl ProductionRunStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Ready => "READY",
            Self::Running => "RUNNING",
            Self::WaitingForSelection => "WAITING_FOR_SELECTION",
            Self::Succeeded => "SUCCEEDED",
            Self::PartialFailed => "PARTIAL_FAILED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "DRAFT" => Self::Draft,
            "READY" => Self::Ready,
            "RUNNING" => Self::Running,
            "WAITING_FOR_SELECTION" => Self::WaitingForSelection,
            "SUCCEEDED" => Self::Succeeded,
            "PARTIAL_FAILED" => Self::PartialFailed,
            "FAILED" => Self::Failed,
            "CANCELLED" => Self::Cancelled,
            _ => return None,
        })
    }

    pub const fn can_transition(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Draft, Self::Ready | Self::Cancelled)
                | (Self::Ready, Self::Running | Self::Cancelled)
                | (
                    Self::Running,
                    Self::WaitingForSelection
                        | Self::Succeeded
                        | Self::PartialFailed
                        | Self::Failed
                        | Self::Cancelled
                )
                | (
                    Self::WaitingForSelection,
                    Self::Ready | Self::Running | Self::Cancelled | Self::Failed
                )
                | (
                    Self::PartialFailed,
                    Self::WaitingForSelection | Self::Cancelled
                )
                | (Self::Failed, Self::Running | Self::Cancelled)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductionStageStatus {
    Pending,
    Ready,
    Running,
    Waiting,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
}

impl ProductionStageStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Ready => "READY",
            Self::Running => "RUNNING",
            Self::Waiting => "WAITING",
            Self::Succeeded => "SUCCEEDED",
            Self::Failed => "FAILED",
            Self::Skipped => "SKIPPED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "PENDING" => Self::Pending,
            "READY" => Self::Ready,
            "RUNNING" => Self::Running,
            "WAITING" => Self::Waiting,
            "SUCCEEDED" => Self::Succeeded,
            "FAILED" => Self::Failed,
            "SKIPPED" => Self::Skipped,
            "CANCELLED" => Self::Cancelled,
            _ => return None,
        })
    }

    pub const fn can_transition(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Ready | Self::Skipped | Self::Cancelled)
                | (
                    Self::Ready,
                    Self::Running | Self::Waiting | Self::Skipped | Self::Cancelled
                )
                | (
                    Self::Running,
                    Self::Waiting | Self::Succeeded | Self::Failed | Self::Cancelled
                )
                | (
                    Self::Waiting,
                    Self::Ready | Self::Succeeded | Self::Failed | Self::Cancelled
                )
                | (Self::Failed, Self::Ready | Self::Cancelled)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductionStageType {
    Krea2ImageGeneration,
    AssetSelection,
    H3VideoGeneration,
}

impl ProductionStageType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Krea2ImageGeneration => "KREA2_IMAGE_GENERATION",
            Self::AssetSelection => "ASSET_SELECTION",
            Self::H3VideoGeneration => "H3_VIDEO_GENERATION",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "KREA2_IMAGE_GENERATION" => Self::Krea2ImageGeneration,
            "ASSET_SELECTION" => Self::AssetSelection,
            "H3_VIDEO_GENERATION" => Self::H3VideoGeneration,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ProductionRunStatus, ProductionStageStatus, ProductionStageType};

    #[test]
    fn run_transitions_keep_the_linear_lifecycle_explicit() {
        assert!(ProductionRunStatus::Draft.can_transition(ProductionRunStatus::Ready));
        assert!(ProductionRunStatus::Ready.can_transition(ProductionRunStatus::Running));
        assert!(
            ProductionRunStatus::Running.can_transition(ProductionRunStatus::WaitingForSelection)
        );
        assert!(
            ProductionRunStatus::WaitingForSelection.can_transition(ProductionRunStatus::Running)
        );
        assert!(ProductionRunStatus::WaitingForSelection.can_transition(ProductionRunStatus::Ready));
        assert!(!ProductionRunStatus::Draft.can_transition(ProductionRunStatus::Succeeded));
    }

    #[test]
    fn stage_and_type_names_are_stable_for_sql_and_api() {
        assert_eq!(ProductionStageStatus::Waiting.as_str(), "WAITING");
        assert_eq!(
            ProductionStageType::H3VideoGeneration.as_str(),
            "H3_VIDEO_GENERATION"
        );
        assert_eq!(
            ProductionStageType::parse("ASSET_SELECTION"),
            Some(ProductionStageType::AssetSelection)
        );
    }
}
