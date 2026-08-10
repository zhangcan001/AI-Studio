use crate::application::ports::{
    AssetRepository, AssetVideoPromptRecord, AssetVideoPromptRepository, Clock, RepositoryError,
};
use crate::domain::{validate_project_id, AssetId, AssetType};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{error::Error, fmt, sync::Arc};

pub const MAX_ASSET_VIDEO_PROMPT_BYTES: usize = 64 * 1024;
const MAX_BATCH_ASSET_IDS: usize = 100;

pub struct AssetVideoPromptService {
    repository: Arc<dyn AssetVideoPromptRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl AssetVideoPromptService {
    pub fn new(
        repository: Arc<dyn AssetVideoPromptRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            asset_repository,
            clock,
        }
    }

    pub async fn get(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<Option<AssetVideoPromptView>, AssetVideoPromptError> {
        validate_project_id(project_id)
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        Ok(self
            .repository
            .find(project_id, asset_id.as_str())
            .await?
            .map(AssetVideoPromptView::from))
    }

    pub async fn list(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<Vec<AssetVideoPromptView>, AssetVideoPromptError> {
        validate_project_id(project_id)
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        if asset_ids.len() > MAX_BATCH_ASSET_IDS {
            return Err(AssetVideoPromptError::InvalidInput(format!(
                "最多同时读取 {MAX_BATCH_ASSET_IDS} 个素材的视频提示词"
            )));
        }
        for asset_id in asset_ids {
            AssetId::parse(asset_id.clone())
                .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        }
        Ok(self
            .repository
            .list(project_id, asset_ids)
            .await?
            .into_iter()
            .map(AssetVideoPromptView::from)
            .collect())
    }

    pub async fn set(
        &self,
        project_id: &str,
        asset_id: &str,
        prompt_text: &str,
    ) -> Result<AssetVideoPromptView, AssetVideoPromptError> {
        validate_project_id(project_id)
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetVideoPromptError::InvalidInput(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetVideoPromptError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id {
            return Err(AssetVideoPromptError::NotFound(
                asset_id.as_str().to_owned(),
            ));
        }
        if asset.asset_type != AssetType::Image {
            return Err(AssetVideoPromptError::InvalidInput(
                "只有图片素材可以配置视频提示词".to_owned(),
            ));
        }
        let prompt_text = prompt_text.trim().to_owned();
        if prompt_text.is_empty() {
            return Err(AssetVideoPromptError::InvalidInput(
                "视频提示词不能为空".to_owned(),
            ));
        }
        if prompt_text.len() > MAX_ASSET_VIDEO_PROMPT_BYTES {
            return Err(AssetVideoPromptError::InvalidInput(
                "视频提示词不能超过 64 KiB".to_owned(),
            ));
        }
        let record = AssetVideoPromptRecord {
            asset_id: asset_id.as_str().to_owned(),
            project_id: project_id.to_owned(),
            prompt_text,
            updated_at: self.clock.now(),
        };
        self.repository.upsert(&record).await?;
        Ok(record.into())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetVideoPromptView {
    pub asset_id: String,
    pub project_id: String,
    pub prompt_text: String,
    pub updated_at: DateTime<Utc>,
}

impl From<AssetVideoPromptRecord> for AssetVideoPromptView {
    fn from(record: AssetVideoPromptRecord) -> Self {
        Self {
            asset_id: record.asset_id,
            project_id: record.project_id,
            prompt_text: record.prompt_text,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug)]
pub enum AssetVideoPromptError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for AssetVideoPromptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(formatter, "ASSET_VIDEO_PROMPT_INVALID: {message}")
            }
            Self::NotFound(asset_id) => {
                write!(formatter, "ASSET_NOT_FOUND: asset {asset_id} was not found")
            }
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for AssetVideoPromptError {}

impl From<RepositoryError> for AssetVideoPromptError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetVideoPromptError, AssetVideoPromptService, MAX_ASSET_VIDEO_PROMPT_BYTES};
    use crate::infrastructure::database::repositories::test_support;
    use crate::infrastructure::database::{
        initialize, SqliteAssetRepository, SqliteAssetVideoPromptRepository,
    };
    use crate::infrastructure::time::SystemClock;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::TempDir;

    const PROJECT_ID: &str = "prj_default";
    const OTHER_PROJECT_ID: &str = "prj_00000000-0000-0000-0000-000000000002";

    async fn fixture() -> (TempDir, SqlitePool, AssetVideoPromptService) {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES (?, 'Default', 'C:/default', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(PROJECT_ID)
        .execute(&pool)
        .await
        .expect("default project fixture should insert");
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES (?, 'Other', 'C:/other', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(OTHER_PROJECT_ID)
        .execute(&pool)
        .await
        .expect("second project fixture should insert");
        sqlx::query(
            "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at, finished_at)
             VALUES ('tsk_generated_prompt', ?, 'workflow-1', 'workflow-version-1', 'recipe-1', 'SUCCEEDED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:01Z')",
        )
        .bind(PROJECT_ID)
        .execute(&pool)
        .await
        .expect("generated asset task fixture should insert");
        for (id, project_id, asset_type, category) in [
            ("ast_source", PROJECT_ID, "image", "source_image"),
            ("ast_generated", PROJECT_ID, "image", "generated_image"),
            ("ast_video", PROJECT_ID, "video", "source_video"),
            ("ast_audio", PROJECT_ID, "audio", "source_audio"),
            ("ast_cross", OTHER_PROJECT_ID, "image", "source_image"),
        ] {
            sqlx::query(
                "INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, duration_ms, file_size, metadata_json, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 'sha', 'application/octet-stream', 1, 1, NULL, 1, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .bind(id)
            .bind(project_id)
            .bind(asset_type)
            .bind(category)
            .bind(id)
            .bind(format!("{id}.bin"))
            .bind(format!("C:/assets/{id}.bin"))
            .execute(&pool)
            .await
            .expect("asset fixture should insert");
        }
        sqlx::query(
            "UPDATE assets SET source_task_id = 'tsk_generated_prompt' WHERE id = 'ast_generated'",
        )
        .execute(&pool)
        .await
        .expect("generated asset source task should be linked");
        let service = AssetVideoPromptService::new(
            Arc::new(SqliteAssetVideoPromptRepository::new(pool.clone())),
            Arc::new(SqliteAssetRepository::new(pool.clone())),
            Arc::new(SystemClock),
        );
        (directory, pool, service)
    }

    fn assert_invalid<T>(result: Result<T, AssetVideoPromptError>) {
        assert!(matches!(
            result,
            Err(AssetVideoPromptError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn accepts_source_and_generated_images() {
        let (_directory, pool, service) = fixture().await;
        let source = service
            .set(PROJECT_ID, "ast_source", "  source camera move  ")
            .await
            .expect("source image prompt should pass");
        let generated = service
            .set(PROJECT_ID, "ast_generated", "generated image motion")
            .await
            .expect("generated image prompt should pass");
        assert_eq!(source.prompt_text, "source camera move");
        assert_eq!(generated.prompt_text, "generated image motion");
        let records = service
            .list(
                PROJECT_ID,
                &["ast_source".to_owned(), "ast_generated".to_owned()],
            )
            .await
            .expect("image prompts should list");
        assert_eq!(records.len(), 2);
        pool.close().await;
    }

    #[tokio::test]
    async fn rejects_video_audio_cross_project_and_empty_prompts() {
        let (_directory, pool, service) = fixture().await;
        assert_invalid(service.set(PROJECT_ID, "ast_video", "move").await);
        assert_invalid(service.set(PROJECT_ID, "ast_audio", "move").await);
        assert!(matches!(
            service.set(PROJECT_ID, "ast_cross", "move").await,
            Err(AssetVideoPromptError::NotFound(_))
        ));
        assert_invalid(service.set(PROJECT_ID, "ast_source", "").await);
        assert_invalid(service.set(PROJECT_ID, "ast_source", " \n\t ").await);
        pool.close().await;
    }

    #[tokio::test]
    async fn enforces_the_utf8_64_kib_prompt_limit_inclusive() {
        let (_directory, pool, service) = fixture().await;
        let accepted = "x".repeat(MAX_ASSET_VIDEO_PROMPT_BYTES);
        service
            .set(PROJECT_ID, "ast_source", &accepted)
            .await
            .expect("exactly 64 KiB should pass");
        let rejected = format!("{accepted}x");
        assert_invalid(service.set(PROJECT_ID, "ast_source", &rejected).await);
        pool.close().await;
    }
}
