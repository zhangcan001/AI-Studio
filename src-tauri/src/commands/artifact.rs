use crate::{
    app_state::AppState,
    application::artifact_service::{
        ArtifactReviewQueueView, ArtifactReviewView, ArtifactServiceError,
        ProductionBatchArtifactsDto,
    },
    application::ports::ArtifactReviewQueueFilter,
    domain::ArtifactReviewDecision,
    error::AppError,
};
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReviewSubmitRequest {
    pub project_id: String,
    pub artifact_id: String,
    pub decision: ArtifactReviewDecisionDto,
    #[serde(default)]
    pub comment: String,
    pub expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReviewResetRequest {
    pub project_id: String,
    pub artifact_id: String,
    pub expected_revision: i64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ArtifactReviewDecisionDto {
    Approved,
    Rejected,
}

impl From<ArtifactReviewDecisionDto> for ArtifactReviewDecision {
    fn from(value: ArtifactReviewDecisionDto) -> Self {
        match value {
            ArtifactReviewDecisionDto::Approved => Self::Approved,
            ArtifactReviewDecisionDto::Rejected => Self::Rejected,
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_batch_artifacts_get(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<ProductionBatchArtifactsDto, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .artifact_service
        .production_batch_artifacts(&project_id, &batch_id)
        .await
        .map_err(map_artifact_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn artifact_review_queue_get(
    state: State<'_, AppState>,
    project_id: String,
    filter: Option<ArtifactReviewQueueFilter>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<ArtifactReviewQueueView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .artifact_service
        .review_queue(
            &project_id,
            filter.unwrap_or_default(),
            limit.unwrap_or(50).clamp(1, 100),
            offset.unwrap_or(0),
        )
        .await
        .map_err(map_artifact_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn artifact_open(
    app: AppHandle,
    state: State<'_, AppState>,
    artifact_id: String,
) -> Result<(), AppError> {
    let path = state
        .artifact_service
        .resolve_artifact_path(&artifact_id)
        .await;
    run_artifact_opener_after_validation(path, |path| {
        app.opener()
            .open_path(path.to_string_lossy().into_owned(), None::<&str>)
            .map_err(|error| AppError::artifact_open_failed(error.to_string()))
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn artifact_reveal(
    app: AppHandle,
    state: State<'_, AppState>,
    artifact_id: String,
) -> Result<(), AppError> {
    let path = state
        .artifact_service
        .resolve_artifact_path(&artifact_id)
        .await;
    run_artifact_opener_after_validation(path, |path| {
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(|error| AppError::artifact_open_failed(error.to_string()))
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn artifact_review_submit(
    state: State<'_, AppState>,
    request: ArtifactReviewSubmitRequest,
) -> Result<ArtifactReviewView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .artifact_service
        .submit_review(
            &request.project_id,
            &request.artifact_id,
            request.decision.into(),
            request.comment,
            request.expected_revision,
        )
        .await
        .map_err(map_artifact_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn artifact_review_reset(
    state: State<'_, AppState>,
    request: ArtifactReviewResetRequest,
) -> Result<ArtifactReviewView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .artifact_service
        .reset_review(
            &request.project_id,
            &request.artifact_id,
            request.expected_revision,
        )
        .await
        .map_err(map_artifact_error)
}

fn map_artifact_error(error: ArtifactServiceError) -> AppError {
    match error {
        ArtifactServiceError::NotFound(message) => AppError::artifact_not_found(message),
        ArtifactServiceError::FileMissing(message) => AppError::artifact_file_missing(message),
        ArtifactServiceError::PathInvalid(message) => AppError::artifact_path_invalid(message),
        ArtifactServiceError::ReviewConflict(message) => {
            AppError::artifact_review_conflict(message)
        }
        ArtifactServiceError::ReviewInvalid(message) => AppError::artifact_review_invalid(message),
        ArtifactServiceError::InvalidInput(message) => AppError::invalid_input(message),
        ArtifactServiceError::Repository(message) => AppError::database(message),
    }
}

fn run_artifact_opener_after_validation(
    path: Result<PathBuf, ArtifactServiceError>,
    opener: impl FnOnce(PathBuf) -> Result<(), AppError>,
) -> Result<(), AppError> {
    let path = path.map_err(map_artifact_error)?;
    opener(path)
}

#[cfg(test)]
mod tests {
    use super::{
        run_artifact_opener_after_validation, ArtifactReviewDecisionDto, ArtifactReviewQueueFilter,
    };
    use crate::{
        application::artifact_service::{validate_output_path, ArtifactServiceError},
        error::AppError,
    };
    use serde_json::from_str;
    use std::{fs, path::Path};
    use tempfile::tempdir;

    #[cfg(unix)]
    fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    fn skip_if_symlinks_are_unavailable(error: &std::io::Error) -> bool {
        #[cfg(windows)]
        if matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
        ) {
            eprintln!("skipping command symlink regression: Windows does not permit creating symlinks here: {error}");
            return true;
        }
        let _ = error;
        false
    }

    #[test]
    fn review_decision_transport_accepts_only_terminal_decisions() {
        assert!(matches!(
            from_str::<ArtifactReviewDecisionDto>("\"APPROVED\"").unwrap(),
            ArtifactReviewDecisionDto::Approved
        ));
        assert!(matches!(
            from_str::<ArtifactReviewDecisionDto>("\"REJECTED\"").unwrap(),
            ArtifactReviewDecisionDto::Rejected
        ));
        assert!(from_str::<ArtifactReviewDecisionDto>("\"PENDING\"").is_err());
    }

    #[test]
    fn review_queue_filter_transport_defaults_to_pending_and_accepts_completed() {
        assert_eq!(
            ArtifactReviewQueueFilter::default(),
            ArtifactReviewQueueFilter::Pending
        );
        assert_eq!(
            from_str::<ArtifactReviewQueueFilter>("\"completed\"").unwrap(),
            ArtifactReviewQueueFilter::Completed
        );
        assert!(from_str::<ArtifactReviewQueueFilter>("\"all\"").is_err());
    }

    #[test]
    fn artifact_symlink_open_and_reveal_return_controlled_errors_without_invoking_opener() {
        let temp = tempdir().unwrap();
        let generated = temp.path().join("assets/generated");
        fs::create_dir_all(&generated).unwrap();
        let target = temp.path().join("outside-target.png");
        fs::write(&target, b"external artifact sentinel").unwrap();
        let link = generated.join("artifact.png");
        if let Err(error) = create_file_symlink(&target, &link) {
            if skip_if_symlinks_are_unavailable(&error) {
                return;
            }
            panic!("test symlink should be created: {error}");
        }

        for action in ["open", "reveal"] {
            let path = validate_output_path(temp.path(), &link);
            assert!(matches!(&path, Err(ArtifactServiceError::PathInvalid(_))));
            let mut opener_invoked = false;
            let result = run_artifact_opener_after_validation(path, |_| {
                opener_invoked = true;
                Ok::<(), AppError>(())
            });
            let error = result.expect_err("Artifact opener must not accept the symlink path");
            assert!(
                error.to_string().contains("ARTIFACT_PATH_INVALID"),
                "Artifact {action} should return the controlled path error: {error}"
            );
            assert!(
                !opener_invoked,
                "Artifact {action} must not open the symlink target"
            );
        }
        assert_eq!(fs::symlink_metadata(&target).unwrap().len(), 26);
    }
}
