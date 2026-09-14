use crate::application::ports::{
    AssetRepository, Clock, ProvenanceLineageRepository, RepositoryError, TaskRepository,
    ToolRepository,
};
use crate::domain::{
    AssetVersionId, GenerationAssetVersion, GenerationAssetVersionId,
    GenerationAssetVersionRelationType, GenerationToolUsage, GenerationToolUsageId, TaskId,
    ToolInstanceId, ToolVersionId,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::{error::Error, fmt, sync::Arc};

#[derive(Clone, Debug)]
pub struct CreateGenerationToolUsageRequest {
    pub project_id: String,
    /// Existing Task ID used as the generation identity.
    pub generation_id: String,
    pub tool_instance_id: String,
    pub tool_version_id: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct CreateGenerationAssetVersionRequest {
    pub project_id: String,
    /// Existing Task ID used as the generation identity.
    pub generation_id: String,
    pub output_id: String,
    pub ordinal: u32,
    pub asset_version_id: String,
    pub relation_type: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationToolUsageView {
    pub id: String,
    pub generation_id: String,
    pub tool_instance_id: String,
    pub tool_version_id: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationAssetVersionView {
    pub id: String,
    pub generation_id: String,
    pub output_id: String,
    pub ordinal: u32,
    pub asset_version_id: String,
    pub relation_type: String,
    pub created_at: DateTime<Utc>,
}

pub struct ProvenanceLineageService {
    repository: Arc<dyn ProvenanceLineageRepository>,
    task_repository: Arc<dyn TaskRepository>,
    tool_repository: Arc<dyn ToolRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl ProvenanceLineageService {
    pub fn new(
        repository: Arc<dyn ProvenanceLineageRepository>,
        task_repository: Arc<dyn TaskRepository>,
        tool_repository: Arc<dyn ToolRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            task_repository,
            tool_repository,
            asset_repository,
            clock,
        }
    }

    pub async fn create_tool_usage(
        &self,
        request: CreateGenerationToolUsageRequest,
    ) -> Result<GenerationToolUsageView, ProvenanceLineageError> {
        let generation_id = self
            .generation_in_project(&request.project_id, &request.generation_id)
            .await?;
        let instance_id = parse_instance_id(&request.tool_instance_id)?;
        let instance = self
            .tool_repository
            .find_instance(&instance_id)
            .await?
            .ok_or_else(|| ProvenanceLineageError::NotFound(instance_id.to_string()))?;
        let version_id = request
            .tool_version_id
            .as_deref()
            .map(parse_version_id)
            .transpose()?;
        if let Some(version_id) = &version_id {
            let version = self
                .tool_repository
                .find_version(version_id)
                .await?
                .ok_or_else(|| ProvenanceLineageError::NotFound(version_id.to_string()))?;
            if version.tool_id != instance.tool_id {
                return Err(ProvenanceLineageError::InvalidInput(
                    "tool version must belong to the selected tool instance".to_owned(),
                ));
            }
        }
        let usage = GenerationToolUsage::new(
            GenerationToolUsageId::new(),
            generation_id,
            instance_id,
            version_id,
            request.metadata,
            self.clock.now(),
        )
        .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))?;
        self.repository.insert_tool_usage(&usage).await?;
        usage_to_view(usage)
    }

    pub async fn list_tool_usages(
        &self,
        project_id: &str,
        generation_id: &str,
    ) -> Result<Vec<GenerationToolUsageView>, ProvenanceLineageError> {
        let generation_id = self
            .generation_in_project(project_id, generation_id)
            .await?;
        self.repository
            .list_tool_usages(project_id.trim(), &generation_id)
            .await?
            .into_iter()
            .map(usage_to_view)
            .collect()
    }

    pub async fn create_asset_version_link(
        &self,
        request: CreateGenerationAssetVersionRequest,
    ) -> Result<GenerationAssetVersionView, ProvenanceLineageError> {
        let project_id = validate_project_id(&request.project_id)?;
        let generation_id = self
            .generation_in_project(project_id, &request.generation_id)
            .await?;
        let asset_version_id = parse_asset_version_id(&request.asset_version_id)?;
        let version = self
            .asset_repository
            .find_asset_version_by_id(project_id, &asset_version_id)
            .await?
            .ok_or_else(|| {
                ProvenanceLineageError::NotFound(asset_version_id.as_str().to_owned())
            })?;
        let mappings = self
            .asset_repository
            .list_output_mappings(&generation_id)
            .await?;
        let mapping = mappings
            .into_iter()
            .find(|mapping| {
                mapping.output_id == request.output_id && mapping.ordinal == request.ordinal
            })
            .ok_or_else(|| {
                ProvenanceLineageError::NotFound(format!(
                    "{}:{}:{}",
                    generation_id, request.output_id, request.ordinal
                ))
            })?;
        if mapping.asset_id != version.asset_id {
            return Err(ProvenanceLineageError::InvalidInput(
                "asset version must belong to the selected generation output".to_owned(),
            ));
        }
        if version.project_id != project_id {
            return Err(ProvenanceLineageError::InvalidInput(
                "asset version must belong to the generation project".to_owned(),
            ));
        }
        let relation_type =
            GenerationAssetVersionRelationType::try_from_input(&request.relation_type)
                .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))?;
        let link = GenerationAssetVersion::new(
            GenerationAssetVersionId::new(),
            generation_id,
            request.output_id.trim(),
            request.ordinal,
            asset_version_id,
            relation_type,
            self.clock.now(),
        )
        .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))?;
        self.repository.insert_asset_version_link(&link).await?;
        link_to_view(link)
    }

    pub async fn list_asset_version_links(
        &self,
        project_id: &str,
        generation_id: &str,
    ) -> Result<Vec<GenerationAssetVersionView>, ProvenanceLineageError> {
        let generation_id = self
            .generation_in_project(project_id, generation_id)
            .await?;
        self.repository
            .list_asset_version_links(project_id.trim(), &generation_id)
            .await?
            .into_iter()
            .map(link_to_view)
            .collect()
    }

    async fn generation_in_project(
        &self,
        project_id: &str,
        generation_id: &str,
    ) -> Result<TaskId, ProvenanceLineageError> {
        let project_id = validate_project_id(project_id)?;
        let generation_id = TaskId::parse(generation_id.trim().to_owned())
            .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))?;
        let task = self
            .task_repository
            .find_by_id(&generation_id)
            .await?
            .ok_or_else(|| ProvenanceLineageError::NotFound(generation_id.to_string()))?;
        if task.project_id != project_id {
            return Err(ProvenanceLineageError::NotFound(generation_id.to_string()));
        }
        Ok(generation_id)
    }
}

fn usage_to_view(
    usage: GenerationToolUsage,
) -> Result<GenerationToolUsageView, ProvenanceLineageError> {
    usage
        .validate()
        .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))?;
    Ok(GenerationToolUsageView {
        id: usage.id.to_string(),
        generation_id: usage.generation_id.to_string(),
        tool_instance_id: usage.tool_instance_id.to_string(),
        tool_version_id: usage.tool_version_id.map(|id| id.to_string()),
        metadata: usage.metadata_json,
        created_at: usage.created_at,
    })
}

fn link_to_view(
    link: GenerationAssetVersion,
) -> Result<GenerationAssetVersionView, ProvenanceLineageError> {
    link.validate()
        .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))?;
    Ok(GenerationAssetVersionView {
        id: link.id.to_string(),
        generation_id: link.generation_id.to_string(),
        output_id: link.output_id,
        ordinal: link.ordinal,
        asset_version_id: link.asset_version_id.as_str().to_owned(),
        relation_type: link.relation_type.to_string(),
        created_at: link.created_at,
    })
}

fn validate_project_id(value: &str) -> Result<&str, ProvenanceLineageError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ProvenanceLineageError::InvalidInput(
            "project id must not be empty".to_owned(),
        ));
    }
    Ok(value)
}

fn parse_instance_id(value: &str) -> Result<ToolInstanceId, ProvenanceLineageError> {
    ToolInstanceId::parse(value.trim().to_owned())
        .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))
}

fn parse_version_id(value: &str) -> Result<ToolVersionId, ProvenanceLineageError> {
    ToolVersionId::parse(value.trim().to_owned())
        .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))
}

fn parse_asset_version_id(value: &str) -> Result<AssetVersionId, ProvenanceLineageError> {
    AssetVersionId::parse(value.trim().to_owned())
        .map_err(|error| ProvenanceLineageError::InvalidInput(error.to_string()))
}

#[derive(Debug)]
pub enum ProvenanceLineageError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for ProvenanceLineageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(formatter, "invalid provenance lineage input: {message}")
            }
            Self::NotFound(id) => {
                write!(formatter, "provenance lineage target was not found: {id}")
            }
            Self::Repository(error) => {
                write!(formatter, "provenance lineage repository failed: {error}")
            }
        }
    }
}

impl Error for ProvenanceLineageError {}

impl From<RepositoryError> for ProvenanceLineageError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateGenerationAssetVersionRequest, CreateGenerationToolUsageRequest,
        ProvenanceLineageService,
    };
    use crate::application::ports::{
        AssetRepository, Clock, TaskOutputAssetMapping, TaskRepository,
    };
    use crate::domain::{Asset, AssetId, AssetVersion, AssetVersionId, Task};
    use crate::infrastructure::database::{
        initialize,
        repositories::{
            test_support, SqliteAssetRepository, SqliteTaskRepository, SqliteToolRepository,
        },
        SqliteProvenanceLineageRepository,
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

    async fn setup() -> (
        TempDir,
        SqlitePool,
        Task,
        ProvenanceLineageService,
        SqliteAssetRepository,
    ) {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            now(),
        );
        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        task_repository
            .create(&task, &task.created_event())
            .await
            .expect("task fixture should persist");
        sqlx::query(
            "INSERT INTO tools (id, name, type, description, metadata_json, created_at)
             VALUES ('tool_service', 'Service Tool', 'image', '', '{}', ?)",
        )
        .bind(now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("tool fixture should persist");
        sqlx::query(
            "INSERT INTO tool_instances (id, tool_id, path, endpoint, status, last_checked)
             VALUES ('tins_service', 'tool_service', NULL, NULL, 'UNKNOWN', NULL)",
        )
        .execute(&pool)
        .await
        .expect("tool instance fixture should persist");
        sqlx::query(
            "INSERT INTO tool_versions
             (id, tool_id, version, observed_at, metadata_json)
             VALUES ('tver_service', 'tool_service', '1.0', ?, '{}')",
        )
        .bind(now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("tool version fixture should persist");
        let asset_repository = SqliteAssetRepository::new(pool.clone());
        let asset = Asset::new_image(
            AssetId::parse("ast_service").unwrap(),
            "project-1",
            "Service asset",
            "service.png",
            "C:/project/service.png",
            "service-sha",
            "image/png",
            2,
            2,
            64,
            task.id.clone(),
            json!({}),
            now(),
        )
        .unwrap();
        let mapping = TaskOutputAssetMapping {
            task_id: task.id.clone(),
            output_id: "output_service".to_owned(),
            ordinal: 0,
            asset_id: asset.id.clone(),
            created_at: now(),
        };
        asset_repository
            .insert_generated_outputs(std::slice::from_ref(&asset), std::slice::from_ref(&mapping))
            .await
            .expect("output fixture should persist");
        asset_repository
            .insert_asset_version(
                &AssetVersion::new(
                    AssetVersionId::parse("av_service").unwrap(),
                    "project-1",
                    asset.id,
                    1,
                    json!({}),
                    "C:/project/service.png",
                    "service-sha",
                    now(),
                )
                .unwrap(),
            )
            .await
            .expect("asset version fixture should persist");
        let service = ProvenanceLineageService::new(
            Arc::new(SqliteProvenanceLineageRepository::new(pool.clone())),
            task_repository,
            Arc::new(SqliteToolRepository::new(pool.clone())),
            Arc::new(asset_repository.clone()),
            Arc::new(FixedClock(now())),
        );
        (directory, pool, task, service, asset_repository)
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    #[tokio::test]
    async fn service_creates_explicit_tool_usage_without_touching_task_flow() {
        let (_directory, _pool, task, service, _assets) = setup().await;
        let view = service
            .create_tool_usage(CreateGenerationToolUsageRequest {
                project_id: "project-1".to_owned(),
                generation_id: task.id.to_string(),
                tool_instance_id: "tins_service".to_owned(),
                tool_version_id: Some("tver_service".to_owned()),
                metadata: json!({"source": "caller"}),
            })
            .await
            .expect("tool usage should be created");
        assert_eq!(view.generation_id, task.id.to_string());
        assert_eq!(
            service
                .list_tool_usages("project-1", task.id.as_str())
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn service_creates_asset_version_lineage_for_exact_output_key() {
        let (_directory, _pool, task, service, _assets) = setup().await;
        let view = service
            .create_asset_version_link(CreateGenerationAssetVersionRequest {
                project_id: "project-1".to_owned(),
                generation_id: task.id.to_string(),
                output_id: "output_service".to_owned(),
                ordinal: 0,
                asset_version_id: "av_service".to_owned(),
                relation_type: "output".to_owned(),
            })
            .await
            .expect("asset version lineage should be created");
        assert_eq!(view.relation_type, "OUTPUT");
        assert_eq!(
            service
                .list_asset_version_links("project-1", task.id.as_str())
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn service_rejects_cross_project_generation_access() {
        let (_directory, _pool, task, service, _assets) = setup().await;
        assert!(service
            .list_tool_usages("project-2", task.id.as_str())
            .await
            .is_err());
    }
}
