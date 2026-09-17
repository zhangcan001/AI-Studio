use crate::application::{
    ports::{
        ArtifactRepository, ArtifactReviewQueueFilter, ArtifactReviewQueuePage, ProjectRepository,
        RepositoryError, TaskRepository,
    },
    production_queue_service::ProductionQueueService,
};
use crate::domain::{
    Artifact, ArtifactReview, ArtifactReviewDecision, ArtifactReviewError, AssetId, AssetType,
    Task, TaskId,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactAvailability {
    Available,
    Missing,
    Unavailable,
}

impl ArtifactAvailability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Missing => "missing",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactView {
    pub id: String,
    pub task_id: String,
    pub output_id: String,
    pub ordinal: u32,
    pub media_type: String,
    pub name: String,
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub size_bytes: u64,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub thumbnail_available: bool,
    pub availability: ArtifactAvailability,
    pub review_status: Option<String>,
    pub review_revision: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionTaskContext {
    pub id: String,
    pub status: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionTaskDto {
    pub production_item_id: String,
    pub ordinal: u32,
    pub production_item_status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub task: Option<ProductionTaskContext>,
    pub artifacts: Vec<ArtifactView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchArtifactsDto {
    pub batch_id: String,
    pub batch_name: String,
    pub status: String,
    pub total: usize,
    pub pending: usize,
    pub running: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub skipped: usize,
    pub items: Vec<ProductionTaskDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReviewView {
    pub id: String,
    pub artifact_id: String,
    pub decision: String,
    pub comment: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReviewQueueItem {
    pub artifact: ArtifactView,
    pub task: Option<ProductionTaskContext>,
    pub review: ArtifactReviewView,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReviewQueueView {
    pub items: Vec<ArtifactReviewQueueItem>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactServiceError {
    NotFound(String),
    FileMissing(String),
    PathInvalid(String),
    ReviewConflict(String),
    ReviewInvalid(String),
    InvalidInput(String),
    Repository(String),
}

impl std::fmt::Display for ArtifactServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => {
                write!(formatter, "ARTIFACT_NOT_FOUND: artifact {id} was not found")
            }
            Self::FileMissing(path) => write!(
                formatter,
                "ARTIFACT_FILE_MISSING: artifact file is missing: {path}"
            ),
            Self::PathInvalid(message) => write!(formatter, "ARTIFACT_PATH_INVALID: {message}"),
            Self::ReviewConflict(message) => {
                write!(formatter, "ARTIFACT_REVIEW_CONFLICT: {message}")
            }
            Self::ReviewInvalid(message) => write!(formatter, "ARTIFACT_REVIEW_INVALID: {message}"),
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::Repository(message) => write!(formatter, "ARTIFACT_REPOSITORY_ERROR: {message}"),
        }
    }
}

impl std::error::Error for ArtifactServiceError {}

impl From<RepositoryError> for ArtifactServiceError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value.to_string())
    }
}

impl From<ArtifactReviewError> for ArtifactServiceError {
    fn from(value: ArtifactReviewError) -> Self {
        match value {
            ArtifactReviewError::StaleRevision { .. } => Self::ReviewConflict(value.to_string()),
            _ => Self::ReviewInvalid(value.to_string()),
        }
    }
}

pub struct ArtifactService {
    artifact_repository: Arc<dyn ArtifactRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    task_repository: Arc<dyn TaskRepository>,
    production_queue_service: Arc<ProductionQueueService>,
    clock: Arc<dyn crate::application::ports::Clock>,
}

impl ArtifactService {
    pub fn new(
        artifact_repository: Arc<dyn ArtifactRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        task_repository: Arc<dyn TaskRepository>,
        production_queue_service: Arc<ProductionQueueService>,
        clock: Arc<dyn crate::application::ports::Clock>,
    ) -> Self {
        Self {
            artifact_repository,
            project_repository,
            task_repository,
            production_queue_service,
            clock,
        }
    }

    pub async fn production_batch_artifacts(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<ProductionBatchArtifactsDto, ArtifactServiceError> {
        let detail = self
            .production_queue_service
            .get(project_id, batch_id)
            .await
            .map_err(|error| ArtifactServiceError::InvalidInput(error.to_string()))?;
        let task_ids = detail
            .items
            .iter()
            .filter_map(|item| item.task_id.as_deref())
            .map(|id| {
                TaskId::parse(id.to_owned())
                    .map_err(|error| ArtifactServiceError::InvalidInput(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tasks = self.task_repository.find_many_by_ids(&task_ids).await?;
        let tasks_by_id = tasks
            .into_iter()
            .map(|task| (task.id.as_str().to_owned(), task))
            .collect::<HashMap<_, _>>();
        let records = self
            .artifact_repository
            .list_for_tasks(project_id, &task_ids)
            .await?;
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await?
            .ok_or_else(|| {
                ArtifactServiceError::InvalidInput(format!(
                    "project {project_id} has no storage root"
                ))
            })?;
        let mut artifacts_by_task = HashMap::<String, Vec<ArtifactView>>::new();
        for record in records {
            if record.artifact.asset.project_id != project_id
                || record.artifact.asset.source_task_id.as_ref() != Some(&record.artifact.task_id)
            {
                continue;
            }
            let view = artifact_view(&record.artifact, record.review.as_ref(), &project_root);
            artifacts_by_task
                .entry(record.artifact.task_id.as_str().to_owned())
                .or_default()
                .push(view);
        }
        let items = detail
            .items
            .iter()
            .map(|item| {
                let task = item.task_id.as_deref().and_then(|id| tasks_by_id.get(id));
                let context = task.map(task_context);
                let artifacts = task
                    .map(|task| {
                        artifacts_by_task
                            .remove(task.id.as_str())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                ProductionTaskDto {
                    production_item_id: item.id.as_str().to_owned(),
                    ordinal: item.ordinal,
                    production_item_status: item.status.as_str().to_owned(),
                    error_code: item.error_code.clone(),
                    error_message: item.error_message.clone(),
                    task: context,
                    artifacts,
                }
            })
            .collect();
        let total = detail.items.len();
        let pending = detail
            .items
            .iter()
            .filter(|item| item.status == crate::domain::ProductionBatchItemStatus::Pending)
            .count();
        let running = detail
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.status,
                    crate::domain::ProductionBatchItemStatus::Dispatching
                        | crate::domain::ProductionBatchItemStatus::Dispatched
                )
            })
            .count();
        let succeeded = detail
            .items
            .iter()
            .filter(|item| item.status == crate::domain::ProductionBatchItemStatus::Succeeded)
            .count();
        let failed = detail
            .items
            .iter()
            .filter(|item| item.status == crate::domain::ProductionBatchItemStatus::Failed)
            .count();
        let cancelled = detail
            .items
            .iter()
            .filter(|item| item.status == crate::domain::ProductionBatchItemStatus::Cancelled)
            .count();
        let skipped = detail
            .items
            .iter()
            .filter(|item| item.status == crate::domain::ProductionBatchItemStatus::Skipped)
            .count();
        Ok(ProductionBatchArtifactsDto {
            batch_id: detail.batch.id.as_str().to_owned(),
            batch_name: detail.batch.name,
            status: detail.batch.status.as_str().to_owned(),
            total,
            pending,
            running,
            succeeded,
            failed,
            cancelled,
            skipped,
            items,
        })
    }

    pub async fn review_queue(
        &self,
        project_id: &str,
        filter: ArtifactReviewQueueFilter,
        limit: usize,
        offset: usize,
    ) -> Result<ArtifactReviewQueueView, ArtifactServiceError> {
        if project_id.trim().is_empty() {
            return Err(ArtifactServiceError::InvalidInput(
                "project ID is required".to_owned(),
            ));
        }
        let ArtifactReviewQueuePage { items, total } = self
            .artifact_repository
            .list_review_queue(project_id, filter, limit.clamp(1, 100), offset)
            .await?;
        let task_ids = items
            .iter()
            .map(|item| item.artifact.task_id.clone())
            .collect::<Vec<_>>();
        let tasks = self.task_repository.find_many_by_ids(&task_ids).await?;
        let tasks_by_id = tasks
            .into_iter()
            .map(|task| (task.id.as_str().to_owned(), task))
            .collect::<HashMap<_, _>>();
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await?
            .ok_or_else(|| {
                ArtifactServiceError::InvalidInput(format!(
                    "project {project_id} has no storage root"
                ))
            })?;
        let items = items
            .into_iter()
            .filter_map(|record| {
                let review = record.review?;
                let task = tasks_by_id
                    .get(record.artifact.task_id.as_str())
                    .map(task_context);
                Some(ArtifactReviewQueueItem {
                    artifact: artifact_view(&record.artifact, Some(&review), &project_root),
                    task,
                    review: review_view(review),
                })
            })
            .collect::<Vec<_>>();
        Ok(ArtifactReviewQueueView {
            items,
            total,
            limit: limit.clamp(1, 100),
            offset,
        })
    }

    pub async fn resolve_artifact_path(
        &self,
        artifact_id: &str,
    ) -> Result<PathBuf, ArtifactServiceError> {
        let artifact_id = AssetId::parse(artifact_id.to_owned())
            .map_err(|error| ArtifactServiceError::NotFound(error.to_string()))?;
        let artifact = self
            .artifact_repository
            .find_artifact_by_id(&artifact_id)
            .await?
            .ok_or_else(|| ArtifactServiceError::NotFound(artifact_id.as_str().to_owned()))?;
        let root = self
            .project_repository
            .get_storage_root(&artifact.asset.project_id)
            .await?
            .ok_or_else(|| {
                ArtifactServiceError::PathInvalid("project storage root is unavailable".to_owned())
            })?;
        validate_output_path(&root, Path::new(&artifact.asset.storage_path))
    }

    pub async fn submit_review(
        &self,
        project_id: &str,
        artifact_id: &str,
        decision: ArtifactReviewDecision,
        comment: String,
        expected_revision: i64,
    ) -> Result<ArtifactReviewView, ArtifactServiceError> {
        let comment = comment.trim().to_owned();
        let artifact_id = AssetId::parse(artifact_id.to_owned())
            .map_err(|error| ArtifactServiceError::NotFound(error.to_string()))?;
        self.artifact_repository
            .find_artifact(project_id, &artifact_id)
            .await?
            .ok_or_else(|| ArtifactServiceError::NotFound(artifact_id.as_str().to_owned()))?;
        let mut review = self
            .artifact_repository
            .find_review(project_id, &artifact_id)
            .await?
            .ok_or_else(|| {
                ArtifactServiceError::ReviewInvalid("artifact has no review record".to_owned())
            })?;
        let original = review.clone();
        review.submit(
            decision,
            comment.clone(),
            expected_revision,
            self.clock.now(),
        )?;
        if review.revision != original.revision {
            let updated = self
                .artifact_repository
                .update_review_if_revision(&review, expected_revision)
                .await?;
            if !updated {
                let current = self
                    .artifact_repository
                    .find_review(project_id, &artifact_id)
                    .await?
                    .ok_or_else(|| {
                        ArtifactServiceError::ReviewInvalid(
                            "artifact review disappeared during submission".to_owned(),
                        )
                    })?;
                if current.decision == decision && current.comment == comment {
                    return Ok(review_view(current));
                }
                return Err(ArtifactServiceError::ReviewConflict(format!(
                    "expected revision {expected_revision}; current revision is {}",
                    current.revision
                )));
            }
        }
        Ok(review_view(review))
    }

    pub async fn reset_review(
        &self,
        project_id: &str,
        artifact_id: &str,
        expected_revision: i64,
    ) -> Result<ArtifactReviewView, ArtifactServiceError> {
        let artifact_id = AssetId::parse(artifact_id.to_owned())
            .map_err(|error| ArtifactServiceError::NotFound(error.to_string()))?;
        let mut review = self
            .artifact_repository
            .find_review(project_id, &artifact_id)
            .await?
            .ok_or_else(|| ArtifactServiceError::NotFound(artifact_id.as_str().to_owned()))?;
        let original = review.clone();
        review.reset(expected_revision, self.clock.now())?;
        if review.revision != original.revision {
            let updated = self
                .artifact_repository
                .update_review_if_revision(&review, expected_revision)
                .await?;
            if !updated {
                let current = self
                    .artifact_repository
                    .find_review(project_id, &artifact_id)
                    .await?
                    .ok_or_else(|| {
                        ArtifactServiceError::ReviewInvalid(
                            "artifact review disappeared during reset".to_owned(),
                        )
                    })?;
                if current.decision == ArtifactReviewDecision::Pending {
                    return Ok(review_view(current));
                }
                return Err(ArtifactServiceError::ReviewConflict(format!(
                    "expected revision {expected_revision}; current revision is {}",
                    current.revision
                )));
            }
        }
        Ok(review_view(review))
    }
}

fn task_context(task: &Task) -> ProductionTaskContext {
    ProductionTaskContext {
        id: task.id.as_str().to_owned(),
        status: task.status.as_str().to_owned(),
        workflow_version_id: task.workflow_version_id.clone(),
        recipe_id: task.recipe_id.clone(),
        created_at: task.created_at,
        finished_at: task.finished_at,
    }
}

fn review_view(review: ArtifactReview) -> ArtifactReviewView {
    ArtifactReviewView {
        id: review.id,
        artifact_id: review.artifact_id.as_str().to_owned(),
        decision: review.decision.as_str().to_owned(),
        comment: review.comment,
        revision: review.revision,
        created_at: review.created_at,
        updated_at: review.updated_at,
    }
}

fn artifact_view(
    artifact: &Artifact,
    review: Option<&ArtifactReview>,
    project_root: &Path,
) -> ArtifactView {
    ArtifactView {
        id: artifact.asset.id.as_str().to_owned(),
        task_id: artifact.task_id.as_str().to_owned(),
        output_id: artifact.output_id.clone(),
        ordinal: artifact.ordinal,
        media_type: match artifact.asset.asset_type {
            AssetType::Image => "image",
            AssetType::Video => "video",
            AssetType::Audio => "audio",
        }
        .to_owned(),
        name: artifact.asset.name.clone(),
        mime_type: artifact.asset.mime_type.clone(),
        width: (artifact.asset.width > 0).then_some(artifact.asset.width),
        height: (artifact.asset.height > 0).then_some(artifact.asset.height),
        duration_ms: artifact.asset.duration_ms,
        size_bytes: artifact.asset.file_size,
        version: artifact.version,
        created_at: artifact.asset.created_at,
        thumbnail_available: artifact.asset.thumbnail_path.is_some(),
        availability: classify_output_path(project_root, Path::new(&artifact.asset.storage_path)),
        review_status: review.map(|review| review.decision.as_str().to_owned()),
        review_revision: review.map(|review| review.revision),
    }
}

fn classify_output_path(project_root: &Path, candidate: &Path) -> ArtifactAvailability {
    match validate_output_path(project_root, candidate) {
        Ok(_) => ArtifactAvailability::Available,
        Err(ArtifactServiceError::FileMissing(_)) => ArtifactAvailability::Missing,
        Err(_) => ArtifactAvailability::Unavailable,
    }
}

pub(crate) fn validate_output_path(
    project_root: &Path,
    candidate: &Path,
) -> Result<PathBuf, ArtifactServiceError> {
    if !candidate.is_absolute() {
        return Err(ArtifactServiceError::PathInvalid(
            "artifact path must be absolute".to_owned(),
        ));
    }
    let canonical_root = fs::canonicalize(project_root).map_err(|error| {
        ArtifactServiceError::PathInvalid(format!("project output root is invalid: {error}"))
    })?;
    let metadata = match fs::symlink_metadata(candidate) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ArtifactServiceError::FileMissing(
                candidate.display().to_string(),
            ));
        }
        Err(error) => {
            return Err(ArtifactServiceError::PathInvalid(format!(
                "artifact cannot be inspected: {error}"
            )));
        }
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ArtifactServiceError::PathInvalid(
            "artifact must be a regular file, not a directory or symbolic link".to_owned(),
        ));
    }
    let canonical = crate::application::ports::validate_asset_read_path(project_root, candidate)
        .map_err(|error| ArtifactServiceError::PathInvalid(error.to_string()))?;
    let output_root = canonical_root.join("assets").join("generated");
    let canonical_output_root = fs::canonicalize(&output_root).map_err(|error| {
        ArtifactServiceError::PathInvalid(format!("generated output directory is invalid: {error}"))
    })?;
    if canonical == canonical_output_root || !canonical.starts_with(&canonical_output_root) {
        return Err(ArtifactServiceError::PathInvalid(
            "artifact is outside the managed generated-output directory".to_owned(),
        ));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::{
        classify_output_path, validate_output_path, ArtifactAvailability, ArtifactServiceError,
    };
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

    #[cfg(unix)]
    fn create_dir_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_dir_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    fn skip_if_symlinks_are_unavailable(error: &std::io::Error) -> bool {
        #[cfg(windows)]
        if matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
        ) {
            eprintln!("skipping symlink regression: Windows does not permit creating symlinks here: {error}");
            return true;
        }
        let _ = error;
        false
    }

    #[test]
    fn output_path_classifies_missing_and_rejects_outside_paths() {
        let temp = tempdir().unwrap();
        let generated = temp.path().join("assets/generated/image");
        fs::create_dir_all(&generated).unwrap();
        let missing = generated.join("missing.png");
        assert_eq!(
            classify_output_path(temp.path(), &missing),
            ArtifactAvailability::Missing
        );
        let outside = temp.path().join("outside.png");
        fs::write(&outside, b"test").unwrap();
        assert!(matches!(
            validate_output_path(temp.path(), &outside),
            Err(ArtifactServiceError::PathInvalid(_))
        ));
        let valid = generated.join("output.png");
        fs::write(&valid, b"test").unwrap();
        assert!(validate_output_path(temp.path(), Path::new(&valid)).is_ok());
    }

    #[test]
    fn artifact_symlink_is_rejected_before_open_or_reveal() {
        let temp = tempdir().unwrap();
        let generated = temp.path().join("assets/generated");
        fs::create_dir_all(&generated).unwrap();
        let external_target = temp.path().join("external-target.png");
        fs::write(&external_target, b"external artifact sentinel").unwrap();
        let link = generated.join("artifact.png");
        if let Err(error) = create_file_symlink(&external_target, &link) {
            if skip_if_symlinks_are_unavailable(&error) {
                return;
            }
            panic!("test symlink should be created: {error}");
        }

        // Both artifact_open and artifact_reveal resolve through this shared guard.
        // The controlled error returns before either OS opener receives a path.
        for action in ["open", "reveal"] {
            let result = validate_output_path(temp.path(), &link);
            assert!(
                matches!(result, Err(ArtifactServiceError::PathInvalid(_))),
                "Artifact {action} must reject the symlink before invoking the OS opener"
            );
        }
        assert_eq!(
            classify_output_path(temp.path(), &link),
            ArtifactAvailability::Unavailable
        );
        assert_eq!(fs::symlink_metadata(&external_target).unwrap().len(), 26);
    }

    #[test]
    fn artifact_parent_symlink_cannot_reach_outside_generated_root() {
        let temp = tempdir().unwrap();
        let generated = temp.path().join("assets/generated");
        let external = temp.path().join("external");
        fs::create_dir_all(&generated).unwrap();
        fs::create_dir_all(&external).unwrap();
        let external_target = external.join("artifact.png");
        fs::write(&external_target, b"external artifact sentinel").unwrap();
        let link = generated.join("linked-directory");
        if let Err(error) = create_dir_symlink(&external, &link) {
            if skip_if_symlinks_are_unavailable(&error) {
                return;
            }
            panic!("test directory symlink should be created: {error}");
        }

        let artifact_path = link.join("artifact.png");
        assert!(matches!(
            validate_output_path(temp.path(), &artifact_path),
            Err(ArtifactServiceError::PathInvalid(_))
        ));
        assert_eq!(
            classify_output_path(temp.path(), &artifact_path),
            ArtifactAvailability::Unavailable
        );
        assert_eq!(fs::symlink_metadata(&external_target).unwrap().len(), 26);
    }
}
