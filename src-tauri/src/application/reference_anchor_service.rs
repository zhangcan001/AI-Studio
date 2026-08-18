use crate::application::organization_service::normalize_name;
use crate::application::ports::{
    AssetRepository, Clock, ReferenceAnchorRecord, ReferenceAnchorRepository, RepositoryError,
};
use crate::domain::{AssetId, AssetType, ReferenceAnchor, ReferenceAnchorId, ReferenceAnchorKind};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{collections::HashMap, error::Error, fmt, sync::Arc};

const MAX_ASSETS: usize = 20;
const MAX_DESCRIPTION_CHARS: usize = 500;

#[derive(Clone, Debug)]
pub struct CreateReferenceAnchorRequest {
    pub project_id: String,
    pub kind: ReferenceAnchorKind,
    pub name: String,
    pub description: String,
    pub asset_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateReferenceAnchorRequest {
    pub project_id: String,
    pub anchor_id: String,
    pub kind: ReferenceAnchorKind,
    pub name: String,
    pub description: String,
    pub asset_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceAnchorAssetView {
    pub asset_id: String,
    pub ordinal: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceAnchorView {
    pub id: String,
    pub project_id: String,
    pub kind: ReferenceAnchorKind,
    pub name: String,
    pub description: String,
    pub assets: Vec<ReferenceAnchorAssetView>,
    pub primary_asset_id: Option<String>,
    pub usable: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ReferenceAnchorService {
    repository: Arc<dyn ReferenceAnchorRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl ReferenceAnchorService {
    pub fn new(
        repository: Arc<dyn ReferenceAnchorRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            asset_repository,
            clock,
        }
    }

    pub async fn list(
        &self,
        project_id: &str,
    ) -> Result<Vec<ReferenceAnchorView>, ReferenceAnchorError> {
        validate_project_id(project_id)?;
        self.repository
            .list(project_id)
            .await?
            .into_iter()
            .map(view_from_record)
            .collect()
    }

    pub async fn get(
        &self,
        project_id: &str,
        anchor_id: &str,
    ) -> Result<ReferenceAnchorView, ReferenceAnchorError> {
        validate_project_id(project_id)?;
        let anchor_id = parse_anchor_id(anchor_id)?;
        let record = self
            .repository
            .find(project_id, &anchor_id)
            .await?
            .ok_or_else(|| ReferenceAnchorError::NotFound(anchor_id.as_str().to_owned()))?;
        view_from_record(record)
    }

    pub async fn create(
        &self,
        request: CreateReferenceAnchorRequest,
    ) -> Result<ReferenceAnchorView, ReferenceAnchorError> {
        validate_project_id(&request.project_id)?;
        let (name, normalized_name) = normalize_anchor_name(&request.name)?;
        let description = normalize_description(&request.description)?;
        let asset_ids = normalize_asset_ids(&request.asset_ids, true)?;
        self.validate_assets(&request.project_id, &asset_ids)
            .await?;

        let now = self.clock.now();
        let anchor = ReferenceAnchor {
            id: ReferenceAnchorId::new(),
            project_id: request.project_id,
            kind: request.kind,
            name,
            normalized_name,
            description,
            created_at: now,
            updated_at: now,
        };
        view_from_record(self.repository.create_atomic(&anchor, &asset_ids).await?)
    }

    pub async fn update(
        &self,
        request: UpdateReferenceAnchorRequest,
    ) -> Result<ReferenceAnchorView, ReferenceAnchorError> {
        validate_project_id(&request.project_id)?;
        let anchor_id = parse_anchor_id(&request.anchor_id)?;
        let existing = self
            .repository
            .find(&request.project_id, &anchor_id)
            .await?
            .ok_or_else(|| ReferenceAnchorError::NotFound(anchor_id.as_str().to_owned()))?;
        let (name, normalized_name) = normalize_anchor_name(&request.name)?;
        let description = normalize_description(&request.description)?;
        let asset_ids = normalize_asset_ids(&request.asset_ids, false)?;
        self.validate_assets(&request.project_id, &asset_ids)
            .await?;

        let anchor = ReferenceAnchor {
            id: anchor_id,
            project_id: request.project_id,
            kind: request.kind,
            name,
            normalized_name,
            description,
            created_at: existing.anchor.created_at,
            updated_at: self.clock.now(),
        };
        view_from_record(self.repository.update_atomic(&anchor, &asset_ids).await?)
    }

    pub async fn delete(
        &self,
        project_id: &str,
        anchor_id: &str,
    ) -> Result<(), ReferenceAnchorError> {
        validate_project_id(project_id)?;
        let anchor_id = parse_anchor_id(anchor_id)?;
        if !self.repository.delete(project_id, &anchor_id).await? {
            return Err(ReferenceAnchorError::NotFound(
                anchor_id.as_str().to_owned(),
            ));
        }
        Ok(())
    }

    async fn validate_assets(
        &self,
        project_id: &str,
        asset_ids: &[AssetId],
    ) -> Result<(), ReferenceAnchorError> {
        if asset_ids.is_empty() {
            return Ok(());
        }
        let assets = self.asset_repository.find_many_by_ids(asset_ids).await?;
        let assets_by_id: HashMap<&str, _> = assets
            .iter()
            .map(|asset| (asset.id.as_str(), asset))
            .collect();
        for asset_id in asset_ids {
            let asset = assets_by_id
                .get(asset_id.as_str())
                .ok_or_else(|| ReferenceAnchorError::AssetNotFound(asset_id.as_str().to_owned()))?;
            if asset.project_id != project_id {
                return Err(ReferenceAnchorError::AssetProjectMismatch {
                    asset_id: asset_id.as_str().to_owned(),
                    project_id: project_id.to_owned(),
                });
            }
            if asset.asset_type != AssetType::Image {
                return Err(ReferenceAnchorError::ImageRequired(
                    asset_id.as_str().to_owned(),
                ));
            }
        }
        Ok(())
    }
}

fn view_from_record(
    record: ReferenceAnchorRecord,
) -> Result<ReferenceAnchorView, ReferenceAnchorError> {
    let assets = record
        .assets
        .into_iter()
        .map(|asset| ReferenceAnchorAssetView {
            asset_id: asset.asset_id.as_str().to_owned(),
            ordinal: asset.ordinal,
            created_at: asset.created_at,
        })
        .collect::<Vec<_>>();
    Ok(ReferenceAnchorView {
        id: record.anchor.id.as_str().to_owned(),
        project_id: record.anchor.project_id,
        kind: record.anchor.kind,
        name: record.anchor.name,
        description: record.anchor.description,
        primary_asset_id: assets.first().map(|asset| asset.asset_id.clone()),
        usable: !assets.is_empty(),
        assets,
        created_at: record.anchor.created_at,
        updated_at: record.anchor.updated_at,
    })
}

fn validate_project_id(project_id: &str) -> Result<(), ReferenceAnchorError> {
    if project_id.trim().is_empty() {
        return Err(ReferenceAnchorError::InvalidInput(
            "REFERENCE_ANCHOR_PROJECT_INVALID: project id must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn parse_anchor_id(value: &str) -> Result<ReferenceAnchorId, ReferenceAnchorError> {
    ReferenceAnchorId::parse(value.trim().to_owned()).map_err(|error| {
        ReferenceAnchorError::InvalidInput(format!("REFERENCE_ANCHOR_ID_INVALID: {error}"))
    })
}

fn normalize_anchor_name(value: &str) -> Result<(String, String), ReferenceAnchorError> {
    normalize_name(value, 80, "REFERENCE_ANCHOR")
        .map_err(|error| ReferenceAnchorError::InvalidInput(error.to_string()))
}

fn normalize_description(value: &str) -> Result<String, ReferenceAnchorError> {
    let description = value.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(ReferenceAnchorError::InvalidInput(format!(
            "REFERENCE_ANCHOR_DESCRIPTION_TOO_LONG: description must be at most {MAX_DESCRIPTION_CHARS} characters"
        )));
    }
    Ok(description.to_owned())
}

fn normalize_asset_ids(
    asset_ids: &[String],
    require_one: bool,
) -> Result<Vec<AssetId>, ReferenceAnchorError> {
    let mut normalized = Vec::with_capacity(asset_ids.len());
    for asset_id in asset_ids {
        let asset_id = AssetId::parse(asset_id.trim().to_owned()).map_err(|error| {
            ReferenceAnchorError::InvalidInput(format!(
                "REFERENCE_ANCHOR_ASSET_ID_INVALID: {error}"
            ))
        })?;
        if !normalized
            .iter()
            .any(|current: &AssetId| current == &asset_id)
        {
            normalized.push(asset_id);
        }
    }
    if require_one && normalized.is_empty() {
        return Err(ReferenceAnchorError::InvalidInput(
            "REFERENCE_ANCHOR_CREATE_REQUIRES_ASSET: create requires at least one image asset"
                .to_owned(),
        ));
    }
    if normalized.len() > MAX_ASSETS {
        return Err(ReferenceAnchorError::InvalidInput(format!(
            "REFERENCE_ANCHOR_ASSET_LIMIT: an anchor supports at most {MAX_ASSETS} image assets"
        )));
    }
    Ok(normalized)
}

#[derive(Debug)]
pub enum ReferenceAnchorError {
    InvalidInput(String),
    NotFound(String),
    AssetNotFound(String),
    AssetProjectMismatch {
        asset_id: String,
        project_id: String,
    },
    ImageRequired(String),
    Repository(RepositoryError),
}

impl fmt::Display for ReferenceAnchorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) => formatter.write_str(message),
            Self::AssetNotFound(asset_id) => {
                write!(formatter, "REFERENCE_ANCHOR_ASSET_NOT_FOUND: asset {asset_id} was not found")
            }
            Self::AssetProjectMismatch { asset_id, project_id } => write!(
                formatter,
                "REFERENCE_ANCHOR_ASSET_PROJECT_MISMATCH: asset {asset_id} does not belong to project {project_id}"
            ),
            Self::ImageRequired(asset_id) => write!(
                formatter,
                "REFERENCE_ANCHOR_IMAGE_REQUIRED: asset {asset_id} must be an image"
            ),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ReferenceAnchorError {}

impl From<RepositoryError> for ReferenceAnchorError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateReferenceAnchorRequest, ReferenceAnchorError, ReferenceAnchorService,
        UpdateReferenceAnchorRequest,
    };
    use crate::application::ports::Clock;
    use crate::domain::{AssetId, AssetType};
    use crate::infrastructure::database::{
        initialize,
        repositories::{SqliteAssetRepository, SqliteReferenceAnchorRepository},
    };
    use chrono::{DateTime, TimeZone, Utc};
    use serde_json::json;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::{tempdir, TempDir};

    struct FixedClock(DateTime<Utc>);
    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    async fn setup() -> (TempDir, SqlitePool, ReferenceAnchorService) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        crate::infrastructure::database::repositories::test_support::seed_task_dependencies(&pool)
            .await;
        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        for id in ["ast_a", "ast_b"] {
            sqlx::query(
                "INSERT INTO assets
                 (id, project_id, type, category, name, original_name, storage_path,
                  sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at)
                 VALUES (?, 'project-1', 'image', 'source_image', ?, ?, ?, 'sha', 'image/png', 2, 2, 1, '{}', ?, ?)",
            )
            .bind(id)
            .bind(id)
            .bind(format!("{id}.png"))
            .bind(format!("C:/{id}.png"))
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        }
        let service = ReferenceAnchorService::new(
            Arc::new(SqliteReferenceAnchorRepository::new(pool.clone())),
            asset_repository,
            Arc::new(FixedClock(
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
            )),
        );
        (directory, pool, service)
    }

    #[tokio::test]
    async fn create_dedupes_order_and_derives_primary() {
        let (_directory, _pool, service) = setup().await;
        let view = service
            .create(CreateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                kind: crate::domain::ReferenceAnchorKind::Character,
                name: "  Character  ".to_owned(),
                description: "description".to_owned(),
                asset_ids: vec!["ast_b".to_owned(), "ast_a".to_owned(), "ast_b".to_owned()],
            })
            .await
            .unwrap();
        assert_eq!(view.name, "Character");
        assert_eq!(view.primary_asset_id.as_deref(), Some("ast_b"));
        assert!(view.usable);
        assert_eq!(
            view.assets
                .iter()
                .map(|asset| asset.ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[tokio::test]
    async fn update_allows_empty_anchor_and_rejects_non_image_or_cross_project_assets() {
        let (_directory, _pool, service) = setup().await;
        let created = service
            .create(CreateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                kind: crate::domain::ReferenceAnchorKind::Scene,
                name: "Scene".to_owned(),
                description: String::new(),
                asset_ids: vec!["ast_a".to_owned()],
            })
            .await
            .unwrap();
        let empty = service
            .update(UpdateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                anchor_id: created.id.clone(),
                kind: crate::domain::ReferenceAnchorKind::Scene,
                name: "Scene".to_owned(),
                description: String::new(),
                asset_ids: Vec::new(),
            })
            .await
            .unwrap();
        assert!(!empty.usable);
        assert_eq!(empty.primary_asset_id, None);

        let asset_repository = service.asset_repository.clone();
        let video = crate::domain::Asset {
            id: AssetId::parse("ast_video").unwrap(),
            project_id: "project-1".to_owned(),
            asset_type: AssetType::Video,
            category: "source_video".to_owned(),
            name: "video".to_owned(),
            original_name: "video.mp4".to_owned(),
            storage_path: "C:/video.mp4".to_owned(),
            thumbnail_path: None,
            sha256: "v".repeat(64),
            mime_type: "video/mp4".to_owned(),
            width: 2,
            height: 2,
            duration_ms: Some(1),
            file_size: 1,
            source_task_id: None,
            metadata_json: json!({}),
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap(),
        };
        asset_repository.insert_many(&[video]).await.unwrap();
        let error = service
            .update(UpdateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                anchor_id: created.id,
                kind: crate::domain::ReferenceAnchorKind::Scene,
                name: "Scene".to_owned(),
                description: String::new(),
                asset_ids: vec!["ast_video".to_owned()],
            })
            .await
            .unwrap_err();
        assert!(matches!(error, ReferenceAnchorError::ImageRequired(_)));
    }

    async fn insert_asset(pool: &SqlitePool, id: &str, project_id: &str, asset_type: &str) {
        let (category, mime_type) = match asset_type {
            "image" => ("source_image", "image/png"),
            "video" => ("source_video", "video/mp4"),
            other => panic!("unsupported test asset type: {other}"),
        };
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path,
              sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 2, 2, 1, '{}', ?, ?)",
        )
        .bind(id)
        .bind(project_id)
        .bind(asset_type)
        .bind(category)
        .bind(id)
        .bind(format!("{id}.asset"))
        .bind(format!("C:/{id}.asset"))
        .bind(format!("sha-{id}"))
        .bind(mime_type)
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn dev034_reference_anchor_no_gpu_e2e_covers_kinds_validation_and_cascades() {
        let (_directory, pool, service) = setup().await;
        for id in ["ast_c", "ast_s1", "ast_s2", "ast_p1"] {
            insert_asset(&pool, id, "project-1", "image").await;
        }
        insert_asset(&pool, "ast_video", "project-1", "video").await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Project 2', 'C:/project-2', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        insert_asset(&pool, "ast_x", "project-2", "image").await;

        let character = service
            .create(CreateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                kind: crate::domain::ReferenceAnchorKind::Character,
                name: "地藏菩萨".to_owned(),
                description: "角色参考".to_owned(),
                asset_ids: vec!["ast_b".to_owned(), "ast_a".to_owned(), "ast_c".to_owned()],
            })
            .await
            .unwrap();
        assert_eq!(character.primary_asset_id.as_deref(), Some("ast_b"));
        assert_eq!(
            character
                .assets
                .iter()
                .map(|asset| asset.asset_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ast_b", "ast_a", "ast_c"]
        );
        let scene = service
            .create(CreateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                kind: crate::domain::ReferenceAnchorKind::Scene,
                name: "忉利天宫".to_owned(),
                description: String::new(),
                asset_ids: vec!["ast_s1".to_owned(), "ast_s2".to_owned()],
            })
            .await
            .unwrap();
        let prop = service
            .create(CreateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                kind: crate::domain::ReferenceAnchorKind::Prop,
                name: "锡杖".to_owned(),
                description: String::new(),
                asset_ids: vec!["ast_p1".to_owned()],
            })
            .await
            .unwrap();
        let style = service
            .create(CreateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                kind: crate::domain::ReferenceAnchorKind::Style,
                name: "水墨风格".to_owned(),
                description: String::new(),
                asset_ids: vec!["ast_c".to_owned()],
            })
            .await
            .unwrap();
        assert_eq!(service.list("project-1").await.unwrap().len(), 4);

        let updated = service
            .update(UpdateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                anchor_id: character.id.clone(),
                kind: crate::domain::ReferenceAnchorKind::Character,
                name: "地藏菩萨".to_owned(),
                description: "更新后的角色参考".to_owned(),
                asset_ids: vec!["ast_a".to_owned(), "ast_b".to_owned(), "ast_c".to_owned()],
            })
            .await
            .unwrap();
        assert_eq!(updated.primary_asset_id.as_deref(), Some("ast_a"));

        let mismatch = service
            .update(UpdateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                anchor_id: updated.id.clone(),
                kind: crate::domain::ReferenceAnchorKind::Character,
                name: "地藏菩萨".to_owned(),
                description: String::new(),
                asset_ids: vec!["ast_x".to_owned()],
            })
            .await
            .unwrap_err();
        assert!(matches!(
            mismatch,
            ReferenceAnchorError::AssetProjectMismatch { .. }
        ));
        let wrong_media = service
            .update(UpdateReferenceAnchorRequest {
                project_id: "project-1".to_owned(),
                anchor_id: updated.id.clone(),
                kind: crate::domain::ReferenceAnchorKind::Character,
                name: "地藏菩萨".to_owned(),
                description: String::new(),
                asset_ids: vec!["ast_video".to_owned()],
            })
            .await
            .unwrap_err();
        assert!(matches!(
            wrong_media,
            ReferenceAnchorError::ImageRequired(_)
        ));

        sqlx::query("DELETE FROM assets WHERE id = 'ast_a'")
            .execute(&pool)
            .await
            .unwrap();
        let after_asset_delete = service.get("project-1", &updated.id).await.unwrap();
        assert_eq!(
            after_asset_delete.primary_asset_id.as_deref(),
            Some("ast_b")
        );
        assert_eq!(
            after_asset_delete
                .assets
                .iter()
                .map(|asset| asset.asset_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ast_b", "ast_c"]
        );
        service.delete("project-1", &updated.id).await.unwrap();
        assert!(service.get("project-1", &updated.id).await.is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM assets WHERE id IN ('ast_b', 'ast_c')",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            2
        );
        assert!(service.get("project-1", &scene.id).await.is_ok());
        assert!(service.get("project-1", &prop.id).await.is_ok());
        assert!(service.get("project-1", &style.id).await.is_ok());
    }
}
