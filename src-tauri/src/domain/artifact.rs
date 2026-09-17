use crate::domain::{Asset, AssetId, TaskId};
use chrono::{DateTime, Utc};
use std::{error::Error, fmt};
use uuid::Uuid;

pub const MAX_ARTIFACT_REVIEW_COMMENT_BYTES: usize = 4 * 1024;

/// A generated output. File identity and media metadata remain owned by Asset;
/// this domain object gives the Task-to-output relation an explicit name.
#[derive(Clone, Debug, PartialEq)]
pub struct Artifact {
    pub asset: Asset,
    pub task_id: TaskId,
    pub output_id: String,
    pub ordinal: u32,
    pub version: u32,
}

impl Artifact {
    pub fn id(&self) -> &AssetId {
        &self.asset.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactReviewDecision {
    Pending,
    Approved,
    Rejected,
}

impl ArtifactReviewDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ArtifactReviewError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "APPROVED" => Ok(Self::Approved),
            "REJECTED" => Ok(Self::Rejected),
            other => Err(ArtifactReviewError::InvalidDecision(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactReview {
    pub id: String,
    pub project_id: String,
    pub artifact_id: AssetId,
    pub decision: ArtifactReviewDecision,
    pub comment: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ArtifactReview {
    pub fn submit(
        &mut self,
        decision: ArtifactReviewDecision,
        comment: String,
        expected_revision: i64,
        now: DateTime<Utc>,
    ) -> Result<(), ArtifactReviewError> {
        let comment = comment.trim().to_owned();
        if decision == ArtifactReviewDecision::Rejected && comment.is_empty() {
            return Err(ArtifactReviewError::MissingRejectionComment);
        }
        if comment.len() > MAX_ARTIFACT_REVIEW_COMMENT_BYTES {
            return Err(ArtifactReviewError::CommentTooLong);
        }
        if self.decision == decision && self.comment == comment {
            return Ok(());
        }
        if expected_revision != self.revision {
            return Err(ArtifactReviewError::StaleRevision {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        if self.decision != ArtifactReviewDecision::Pending
            || decision == ArtifactReviewDecision::Pending
        {
            return Err(ArtifactReviewError::InvalidTransition {
                from: self.decision,
                to: decision,
            });
        }
        self.decision = decision;
        self.comment = comment;
        self.revision = self.revision.saturating_add(1);
        self.updated_at = now;
        Ok(())
    }

    pub fn reset(
        &mut self,
        expected_revision: i64,
        now: DateTime<Utc>,
    ) -> Result<(), ArtifactReviewError> {
        if self.decision == ArtifactReviewDecision::Pending {
            return Ok(());
        }
        if expected_revision != self.revision {
            return Err(ArtifactReviewError::StaleRevision {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        self.decision = ArtifactReviewDecision::Pending;
        self.comment.clear();
        self.revision = self.revision.saturating_add(1);
        self.updated_at = now;
        Ok(())
    }

    pub fn new_pending(
        project_id: impl Into<String>,
        artifact_id: AssetId,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id: format!("arv_{}", Uuid::new_v4().simple()),
            project_id: project_id.into(),
            artifact_id,
            decision: ArtifactReviewDecision::Pending,
            comment: String::new(),
            revision: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactReviewError {
    InvalidDecision(String),
    InvalidTransition {
        from: ArtifactReviewDecision,
        to: ArtifactReviewDecision,
    },
    StaleRevision {
        expected: i64,
        actual: i64,
    },
    MissingRejectionComment,
    CommentTooLong,
}

impl fmt::Display for ArtifactReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDecision(value) => {
                write!(formatter, "invalid artifact review decision: {value}")
            }
            Self::InvalidTransition { from, to } => write!(
                formatter,
                "invalid artifact review transition: {} -> {}",
                from.as_str(),
                to.as_str()
            ),
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "ARTIFACT_REVIEW_STALE: expected revision {expected}, current revision is {actual}"
            ),
            Self::MissingRejectionComment => {
                write!(
                    formatter,
                    "a comment is required when rejecting an artifact"
                )
            }
            Self::CommentTooLong => {
                write!(formatter, "artifact review comment exceeds the 4 KiB limit")
            }
        }
    }
}

impl Error for ArtifactReviewError {}

#[cfg(test)]
mod tests {
    use super::{ArtifactReview, ArtifactReviewDecision, ArtifactReviewError};
    use crate::domain::AssetId;
    use chrono::Utc;

    fn pending() -> ArtifactReview {
        ArtifactReview::new_pending(
            "project-a",
            AssetId::parse("ast_artifact_1").unwrap(),
            Utc::now(),
        )
    }

    #[test]
    fn review_decision_is_idempotent_and_revision_guarded() {
        let now = Utc::now();
        let mut review = pending();
        review
            .submit(ArtifactReviewDecision::Approved, "ok".into(), 0, now)
            .unwrap();
        assert_eq!(review.revision, 1);
        review
            .submit(ArtifactReviewDecision::Approved, "ok".into(), 0, now)
            .unwrap();
        assert_eq!(review.revision, 1);
        assert!(matches!(
            review.submit(ArtifactReviewDecision::Rejected, "no".into(), 0, now),
            Err(ArtifactReviewError::StaleRevision { .. })
        ));
        assert!(matches!(
            review.submit(ArtifactReviewDecision::Rejected, "no".into(), 1, now),
            Err(ArtifactReviewError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn reset_is_explicit_and_advances_revision() {
        let now = Utc::now();
        let mut review = pending();
        review
            .submit(
                ArtifactReviewDecision::Rejected,
                "needs work".into(),
                0,
                now,
            )
            .unwrap();
        review.reset(1, now).unwrap();
        assert_eq!(review.decision, ArtifactReviewDecision::Pending);
        assert_eq!(review.comment, "");
        assert_eq!(review.revision, 2);
    }

    #[test]
    fn rejection_requires_a_non_empty_comment() {
        let now = Utc::now();
        let mut review = pending();
        assert_eq!(
            review.submit(ArtifactReviewDecision::Rejected, "  \n".into(), 0, now),
            Err(ArtifactReviewError::MissingRejectionComment)
        );
        review
            .submit(
                ArtifactReviewDecision::Rejected,
                "  revise framing  ".into(),
                0,
                now,
            )
            .unwrap();
        assert_eq!(review.comment, "revise framing");
    }
}
