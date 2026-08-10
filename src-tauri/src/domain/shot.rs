use crate::domain::TaskStatus;
use serde_json::Value;
use std::{error::Error, fmt};

const MAX_SHOT_NAME_CHARS: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShotStage {
    Image,
    Video,
}

impl ShotStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, ShotDomainError> {
        match value {
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            other => Err(ShotDomainError::InvalidStage(other.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotViewStatus {
    Draft,
    Ready,
    GeneratingImage,
    ImageReview,
    ImageSelected,
    GeneratingVideo,
    VideoReview,
    Completed,
    Failed,
}

impl ShotViewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Ready => "READY",
            Self::GeneratingImage => "GENERATING_IMAGE",
            Self::ImageReview => "IMAGE_REVIEW",
            Self::ImageSelected => "IMAGE_SELECTED",
            Self::GeneratingVideo => "GENERATING_VIDEO",
            Self::VideoReview => "VIDEO_REVIEW",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
        }
    }
}

pub fn canonical_shot_name(value: &str) -> Result<String, ShotDomainError> {
    let name = value.trim();
    if name.is_empty() || name.contains(['\r', '\n']) {
        return Err(ShotDomainError::InvalidName(
            "镜头名称必须是 1–120 个字符的单行文本".to_owned(),
        ));
    }
    if name.chars().count() > MAX_SHOT_NAME_CHARS {
        return Err(ShotDomainError::InvalidName(
            "镜头名称最多 120 个字符".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

pub fn validate_scalar_values(value: &Value) -> Result<(), ShotDomainError> {
    let Some(values) = value.as_object() else {
        return Err(ShotDomainError::InvalidScalarValues(
            "参数必须是 JSON object".to_owned(),
        ));
    };
    for (key, value) in values {
        if key.trim().is_empty() {
            return Err(ShotDomainError::InvalidScalarValues(
                "参数名不能为空".to_owned(),
            ));
        }
        let Some(kind) = value.get("type").and_then(Value::as_str) else {
            return Err(ShotDomainError::InvalidScalarValues(format!(
                "参数 {key} 缺少类型"
            )));
        };
        match kind {
            "integer"
                if value.as_object().is_some_and(|object| object.len() == 2)
                    && value.get("value").and_then(Value::as_i64).is_some() => {}
            "seed_random" if value.as_object().is_some_and(|object| object.len() == 1) => {}
            "seed_fixed"
                if value.as_object().is_some_and(|object| object.len() == 2)
                    && value
                        .get("value")
                        .and_then(Value::as_str)
                        .is_some_and(|seed| seed.parse::<u64>().is_ok()) => {}
            _ => {
                return Err(ShotDomainError::InvalidScalarValues(format!(
                    "参数 {key} 只能是 integer 或 seed"
                )))
            }
        }
    }
    Ok(())
}

pub fn derive_stage_status(
    stage: ShotStage,
    configured: bool,
    selected: bool,
    latest_task_status: Option<TaskStatus>,
) -> ShotViewStatus {
    if let Some(status) = latest_task_status {
        if matches!(
            status,
            TaskStatus::Created
                | TaskStatus::Validating
                | TaskStatus::Preparing
                | TaskStatus::Queued
                | TaskStatus::Running
                | TaskStatus::CancelRequested
                | TaskStatus::Collecting
        ) {
            return match stage {
                ShotStage::Image => ShotViewStatus::GeneratingImage,
                ShotStage::Video => ShotViewStatus::GeneratingVideo,
            };
        }
        if status == TaskStatus::Failed {
            return ShotViewStatus::Failed;
        }
        if status == TaskStatus::Succeeded && !selected {
            return match stage {
                ShotStage::Image => ShotViewStatus::ImageReview,
                ShotStage::Video => ShotViewStatus::VideoReview,
            };
        }
    }
    if selected {
        return match stage {
            ShotStage::Image => ShotViewStatus::ImageSelected,
            ShotStage::Video => ShotViewStatus::Completed,
        };
    }
    if configured {
        ShotViewStatus::Ready
    } else {
        ShotViewStatus::Draft
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShotDomainError {
    InvalidName(String),
    InvalidStage(String),
    InvalidScalarValues(String),
}

impl fmt::Display for ShotDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(message)
            | Self::InvalidStage(message)
            | Self::InvalidScalarValues(message) => formatter.write_str(message),
        }
    }
}

impl Error for ShotDomainError {}

#[cfg(test)]
mod tests {
    use super::{
        canonical_shot_name, derive_stage_status, validate_scalar_values, ShotStage, ShotViewStatus,
    };
    use crate::domain::TaskStatus;
    use serde_json::json;

    #[test]
    fn shot_name_and_scalar_rules_are_explicit() {
        assert_eq!(canonical_shot_name("  镜头 01 ").unwrap(), "镜头 01");
        assert!(canonical_shot_name("多行\n名称").is_err());
        validate_scalar_values(&json!({
            "steps": {"type": "integer", "value": 4},
            "seed": {"type": "seed_fixed", "value": "42"}
        }))
        .unwrap();
        assert!(validate_scalar_values(&json!({
            "prompt": {"type": "string", "value": "不应进入 scalar"}
        }))
        .is_err());
    }

    #[test]
    fn stage_status_is_derived_from_task_truth_and_selection() {
        assert_eq!(
            derive_stage_status(ShotStage::Image, true, false, Some(TaskStatus::Running)),
            ShotViewStatus::GeneratingImage
        );
        assert_eq!(
            derive_stage_status(ShotStage::Image, true, false, Some(TaskStatus::Succeeded)),
            ShotViewStatus::ImageReview
        );
        assert_eq!(
            derive_stage_status(ShotStage::Video, true, true, Some(TaskStatus::Succeeded)),
            ShotViewStatus::Completed
        );
    }
}
