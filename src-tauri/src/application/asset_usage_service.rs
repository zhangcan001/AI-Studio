use crate::application::ports::{
    AssetUsageRepository, AssetUsageSummary, ProfileUsageSummary, ReferenceSetUsageSummary,
    RepositoryError,
};
use crate::domain::consistency::ProfileType;
use crate::domain::AssetId;
use std::{error::Error, fmt, sync::Arc};

/// Application-facing errors for the read-only usage inspection endpoints.
#[derive(Debug)]
pub enum AssetUsageError {
    InvalidInput(String),
    Blocked {
        entity: String,
        reasons: Vec<String>,
    },
    Repository(RepositoryError),
}

impl fmt::Display for AssetUsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(formatter, "invalid asset usage input: {message}")
            }
            Self::Blocked { entity, reasons } => write!(
                formatter,
                "{entity} is still in use: {}",
                reasons.join("; ")
            ),
            Self::Repository(error) => write!(formatter, "asset usage repository failed: {error}"),
        }
    }
}

impl Error for AssetUsageError {}

impl From<RepositoryError> for AssetUsageError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

pub type AssetUsageServiceError = AssetUsageError;

/// Coordinates project-scoped usage reads.  The repository owns the SQL
/// joins; this service only validates the boundary and exposes stable
/// application methods for commands and deletion flows.
pub struct AssetUsageService {
    repository: Arc<dyn AssetUsageRepository>,
}

impl AssetUsageService {
    pub fn new(repository: Arc<dyn AssetUsageRepository>) -> Self {
        Self { repository }
    }

    pub async fn asset_usage(
        &self,
        project_id: &str,
        asset_id_value: &str,
    ) -> Result<AssetUsageSummary, AssetUsageError> {
        validate_scope(project_id)?;
        let asset_id = AssetId::parse(asset_id_value.to_owned())
            .map_err(|error| AssetUsageError::InvalidInput(error.to_string()))?;
        self.repository
            .asset_usage(project_id, &asset_id)
            .await
            .map_err(AssetUsageError::Repository)
    }

    pub async fn profile_usage(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<ProfileUsageSummary, AssetUsageError> {
        validate_scope(project_id)?;
        validate_id("profile", profile_id)?;
        self.repository
            .profile_usage(project_id, profile_type, profile_id)
            .await
            .map_err(AssetUsageError::Repository)
    }

    pub async fn reference_set_usage(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<ReferenceSetUsageSummary, AssetUsageError> {
        validate_scope(project_id)?;
        validate_id("reference set", reference_set_id)?;
        self.repository
            .reference_set_usage(project_id, reference_set_id)
            .await
            .map_err(AssetUsageError::Repository)
    }

    /// Returns the concrete live relation details that must be shown before a
    /// Profile deletion.  The existing profile repository remains the final
    /// authority; this method supplies the semantic inspection used by UX and
    /// callers that want the same protection before attempting a delete.
    pub async fn profile_delete_blockers(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<Vec<String>, AssetUsageError> {
        let usage = self
            .profile_usage(project_id, profile_type, profile_id)
            .await?;
        Ok(blocking_details(&usage.items))
    }

    pub async fn ensure_profile_deletable(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<(), AssetUsageError> {
        let reasons = self
            .profile_delete_blockers(project_id, profile_type, profile_id)
            .await?;
        if reasons.is_empty() {
            Ok(())
        } else {
            Err(AssetUsageError::Blocked {
                entity: format!("{} profile {profile_id}", profile_type.as_str()),
                reasons,
            })
        }
    }

    /// Returns concrete live relation details for a ReferenceSet delete
    /// preflight.  Item membership is deliberately not a blocker: deleting a
    /// ReferenceSet removes its own ordered item rows, while defaults,
    /// costumes, and bindings are live semantic references.
    pub async fn reference_set_delete_blockers(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<Vec<String>, AssetUsageError> {
        let usage = self
            .reference_set_usage(project_id, reference_set_id)
            .await?;
        Ok(blocking_details(&usage.items))
    }

    pub async fn ensure_reference_set_deletable(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<(), AssetUsageError> {
        let reasons = self
            .reference_set_delete_blockers(project_id, reference_set_id)
            .await?;
        if reasons.is_empty() {
            Ok(())
        } else {
            Err(AssetUsageError::Blocked {
                entity: format!("reference set {reference_set_id}"),
                reasons,
            })
        }
    }
}

fn validate_scope(project_id: &str) -> Result<(), AssetUsageError> {
    if project_id.trim().is_empty() {
        return Err(AssetUsageError::InvalidInput(
            "project id must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn validate_id(kind: &str, value: &str) -> Result<(), AssetUsageError> {
    if value.trim().is_empty() {
        return Err(AssetUsageError::InvalidInput(format!(
            "{kind} id must not be empty"
        )));
    }
    Ok(())
}

fn blocking_details(items: &[crate::application::ports::AssetUsageItem]) -> Vec<String> {
    items
        .iter()
        .filter(|item| item.blocking)
        .map(|item| item.detail.clone())
        .filter(|detail| !detail.trim().is_empty())
        .fold(Vec::new(), |mut details, detail| {
            if !details.contains(&detail) {
                details.push(detail);
            }
            details
        })
}

#[cfg(test)]
mod tests {
    use super::{AssetUsageError, AssetUsageService};
    use crate::application::ports::{
        AssetUsageItem, AssetUsageRepository, AssetUsageSummary, ProfileUsageSummary,
        ReferenceSetUsageSummary, RepositoryError,
    };
    use crate::domain::consistency::ProfileType;
    use crate::domain::AssetId;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct FakeUsageRepository {
        seen_asset: Mutex<Option<(String, AssetId)>>,
        asset_summary: Mutex<AssetUsageSummary>,
        profile_summary: Mutex<ProfileUsageSummary>,
        reference_set_summary: Mutex<ReferenceSetUsageSummary>,
    }

    #[async_trait]
    impl AssetUsageRepository for FakeUsageRepository {
        async fn asset_usage(
            &self,
            project_id: &str,
            asset_id: &AssetId,
        ) -> Result<AssetUsageSummary, RepositoryError> {
            *self.seen_asset.lock().expect("asset call") =
                Some((project_id.to_owned(), asset_id.clone()));
            Ok(self.asset_summary.lock().expect("asset summary").clone())
        }

        async fn profile_usage(
            &self,
            _project_id: &str,
            _profile_type: ProfileType,
            _profile_id: &str,
        ) -> Result<ProfileUsageSummary, RepositoryError> {
            Ok(self
                .profile_summary
                .lock()
                .expect("profile summary")
                .clone())
        }

        async fn reference_set_usage(
            &self,
            _project_id: &str,
            _reference_set_id: &str,
        ) -> Result<ReferenceSetUsageSummary, RepositoryError> {
            Ok(self
                .reference_set_summary
                .lock()
                .expect("reference set summary")
                .clone())
        }
    }

    #[tokio::test]
    async fn asset_usage_validates_and_forwards_project_scope() {
        let repository = Arc::new(FakeUsageRepository {
            asset_summary: Mutex::new(AssetUsageSummary::new("ast_1")),
            ..Default::default()
        });
        let service = AssetUsageService::new(repository.clone());
        let summary = service
            .asset_usage("project-1", "ast_1")
            .await
            .expect("usage should load");
        assert_eq!(summary.asset_id, "ast_1");
        assert_eq!(
            repository
                .seen_asset
                .lock()
                .expect("asset call")
                .as_ref()
                .expect("forwarded call")
                .0,
            "project-1"
        );
    }

    #[tokio::test]
    async fn invalid_asset_id_does_not_reach_repository() {
        let service = AssetUsageService::new(Arc::new(FakeUsageRepository::default()));
        let error = service
            .asset_usage("project-1", "not-an-asset")
            .await
            .expect_err("invalid asset id");
        assert!(matches!(error, AssetUsageError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn profile_delete_preflight_returns_only_blocking_details() {
        let mut summary = ProfileUsageSummary::default();
        summary.related_profiles.push(AssetUsageItem::new(
            "PROFILE",
            "style-1",
            "Style",
            "DEFAULT_STYLE_PROFILE",
            None,
            None,
            None,
            Some("STYLE".to_owned()),
            None,
            false,
            "non-blocking relation",
        ));
        summary.shot_bindings.push(AssetUsageItem::new(
            "SHOT",
            "shot-1",
            "Shot",
            "SHOT_PROFILE_BINDING",
            None,
            None,
            Some("shot-1".to_owned()),
            Some("CHARACTER".to_owned()),
            None,
            true,
            "镜头 shot-1 仍绑定该档案。",
        ));
        summary.finish();
        let repository = Arc::new(FakeUsageRepository {
            profile_summary: Mutex::new(summary.clone()),
            ..Default::default()
        });
        let service = AssetUsageService::new(repository);

        let reasons = service
            .profile_delete_blockers("project-1", ProfileType::Character, "char-1")
            .await
            .expect("profile blockers should load");
        assert_eq!(reasons, vec!["镜头 shot-1 仍绑定该档案。"]);
        assert!(matches!(
            service
                .ensure_profile_deletable("project-1", ProfileType::Character, "char-1")
                .await,
            Err(AssetUsageError::Blocked { .. })
        ));
    }
}
