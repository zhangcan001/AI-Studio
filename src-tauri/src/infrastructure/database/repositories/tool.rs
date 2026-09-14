use super::{
    format_datetime, map_domain_error, map_sqlx_error, parse_datetime, parse_json, serialize_json,
};
use crate::application::ports::{RepositoryError, ToolRepository};
use crate::domain::{
    Capability, Tool, ToolHealthStatus, ToolId, ToolInstance, ToolInstanceId, ToolVersion,
    ToolVersionId,
};
use async_trait::async_trait;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone)]
pub struct SqliteToolRepository {
    pool: SqlitePool,
}

impl SqliteToolRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ToolRepository for SqliteToolRepository {
    async fn list_tools(&self) -> Result<Vec<Tool>, RepositoryError> {
        let rows = sqlx::query_as::<_, ToolRow>(
            "SELECT id, name, type AS tool_type, description, metadata_json, created_at
             FROM tools
             ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(ToolRow::try_into_domain).collect()
    }

    async fn find_tool(&self, tool_id: &ToolId) -> Result<Option<Tool>, RepositoryError> {
        let row = sqlx::query_as::<_, ToolRow>(
            "SELECT id, name, type AS tool_type, description, metadata_json, created_at
             FROM tools WHERE id = ?",
        )
        .bind(tool_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(ToolRow::try_into_domain).transpose()
    }

    async fn create_tool(&self, tool: &Tool) -> Result<(), RepositoryError> {
        tool.validate()
            .map_err(|error| map_domain_error("tool validation", error))?;
        let metadata_json = serialize_json("tool metadata", Some(&tool.metadata_json))?
            .ok_or_else(|| RepositoryError::serialization("tool metadata", "missing value"))?;
        sqlx::query(
            "INSERT INTO tools (id, name, type, description, metadata_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(tool.id.as_str())
        .bind(&tool.name)
        .bind(&tool.tool_type)
        .bind(&tool.description)
        .bind(metadata_json)
        .bind(format_datetime(tool.created_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn update_tool(&self, tool: &Tool) -> Result<Option<Tool>, RepositoryError> {
        tool.validate()
            .map_err(|error| map_domain_error("tool validation", error))?;
        let metadata_json = serialize_json("tool metadata", Some(&tool.metadata_json))?
            .ok_or_else(|| RepositoryError::serialization("tool metadata", "missing value"))?;
        let result = sqlx::query(
            "UPDATE tools
             SET name = ?, type = ?, description = ?, metadata_json = ?
             WHERE id = ?",
        )
        .bind(&tool.name)
        .bind(&tool.tool_type)
        .bind(&tool.description)
        .bind(metadata_json)
        .bind(tool.id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_tool(&tool.id).await
    }

    async fn delete_tool(&self, tool_id: &ToolId) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM tools WHERE id = ?")
            .bind(tool_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn list_instances(&self, tool_id: &ToolId) -> Result<Vec<ToolInstance>, RepositoryError> {
        let rows = sqlx::query_as::<_, ToolInstanceRow>(
            "SELECT id, tool_id, path, endpoint, status, last_checked
             FROM tool_instances
             WHERE tool_id = ?
             ORDER BY id ASC",
        )
        .bind(tool_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ToolInstanceRow::try_into_domain)
            .collect()
    }

    async fn find_instance(
        &self,
        instance_id: &ToolInstanceId,
    ) -> Result<Option<ToolInstance>, RepositoryError> {
        let row = sqlx::query_as::<_, ToolInstanceRow>(
            "SELECT id, tool_id, path, endpoint, status, last_checked
             FROM tool_instances WHERE id = ?",
        )
        .bind(instance_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(ToolInstanceRow::try_into_domain).transpose()
    }

    async fn create_instance(&self, instance: &ToolInstance) -> Result<(), RepositoryError> {
        instance
            .validate()
            .map_err(|error| map_domain_error("tool instance validation", error))?;
        sqlx::query(
            "INSERT INTO tool_instances
             (id, tool_id, path, endpoint, status, last_checked)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(instance.id.as_str())
        .bind(instance.tool_id.as_str())
        .bind(&instance.path)
        .bind(&instance.endpoint)
        .bind(instance.status.as_str())
        .bind(instance.last_checked.map(format_datetime))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn update_instance(
        &self,
        instance: &ToolInstance,
    ) -> Result<Option<ToolInstance>, RepositoryError> {
        instance
            .validate()
            .map_err(|error| map_domain_error("tool instance validation", error))?;
        let result = sqlx::query(
            "UPDATE tool_instances
             SET tool_id = ?, path = ?, endpoint = ?, status = ?, last_checked = ?
             WHERE id = ?",
        )
        .bind(instance.tool_id.as_str())
        .bind(&instance.path)
        .bind(&instance.endpoint)
        .bind(instance.status.as_str())
        .bind(instance.last_checked.map(format_datetime))
        .bind(instance.id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_instance(&instance.id).await
    }

    async fn list_versions(&self, tool_id: &ToolId) -> Result<Vec<ToolVersion>, RepositoryError> {
        let rows = sqlx::query_as::<_, ToolVersionRow>(
            "SELECT id, tool_id, version, observed_at, metadata_json
             FROM tool_versions
             WHERE tool_id = ?
             ORDER BY observed_at ASC, id ASC",
        )
        .bind(tool_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ToolVersionRow::try_into_domain)
            .collect()
    }

    async fn find_version(
        &self,
        version_id: &ToolVersionId,
    ) -> Result<Option<ToolVersion>, RepositoryError> {
        let row = sqlx::query_as::<_, ToolVersionRow>(
            "SELECT id, tool_id, version, observed_at, metadata_json
             FROM tool_versions WHERE id = ?",
        )
        .bind(version_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(ToolVersionRow::try_into_domain).transpose()
    }

    async fn create_version(&self, version: &ToolVersion) -> Result<(), RepositoryError> {
        version
            .validate()
            .map_err(|error| map_domain_error("tool version validation", error))?;
        let metadata_json = serialize_json("tool version metadata", Some(&version.metadata_json))?
            .ok_or_else(|| {
                RepositoryError::serialization("tool version metadata", "missing value")
            })?;
        sqlx::query(
            "INSERT INTO tool_versions
             (id, tool_id, version, observed_at, metadata_json)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(version.id.as_str())
        .bind(version.tool_id.as_str())
        .bind(&version.version)
        .bind(format_datetime(version.observed_at))
        .bind(metadata_json)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn list_capabilities(
        &self,
        tool_id: &ToolId,
    ) -> Result<Vec<Capability>, RepositoryError> {
        let rows = sqlx::query_as::<_, ToolCapabilityRow>(
            "SELECT tool_id, capability_name, metadata_json
             FROM tool_capabilities
             WHERE tool_id = ?
             ORDER BY capability_name COLLATE NOCASE ASC",
        )
        .bind(tool_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ToolCapabilityRow::try_into_domain)
            .collect()
    }

    async fn create_capability(&self, capability: &Capability) -> Result<(), RepositoryError> {
        capability
            .validate()
            .map_err(|error| map_domain_error("tool capability validation", error))?;
        let metadata_json =
            serialize_json("tool capability metadata", Some(&capability.metadata_json))?
                .ok_or_else(|| {
                    RepositoryError::serialization("tool capability metadata", "missing value")
                })?;
        sqlx::query(
            "INSERT INTO tool_capabilities (tool_id, capability_name, metadata_json)
             VALUES (?, ?, ?)",
        )
        .bind(capability.tool_id.as_str())
        .bind(&capability.capability_name)
        .bind(metadata_json)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }
}

#[derive(Debug, FromRow)]
struct ToolRow {
    id: String,
    name: String,
    tool_type: String,
    description: String,
    metadata_json: String,
    created_at: String,
}

impl ToolRow {
    fn try_into_domain(self) -> Result<Tool, RepositoryError> {
        let tool = Tool::new(
            ToolId::parse(self.id).map_err(|error| map_domain_error("tool id", error))?,
            self.name,
            self.tool_type,
            self.description,
            parse_json("tool metadata", Some(&self.metadata_json))?
                .ok_or_else(|| RepositoryError::serialization("tool metadata", "missing value"))?,
            parse_datetime("tool created_at", &self.created_at)?,
        )
        .map_err(|error| map_domain_error("tool integrity", error))?;
        Ok(tool)
    }
}

#[derive(Debug, FromRow)]
struct ToolInstanceRow {
    id: String,
    tool_id: String,
    path: Option<String>,
    endpoint: Option<String>,
    status: String,
    last_checked: Option<String>,
}

impl ToolInstanceRow {
    fn try_into_domain(self) -> Result<ToolInstance, RepositoryError> {
        ToolInstance::new(
            ToolInstanceId::parse(self.id)
                .map_err(|error| map_domain_error("tool instance id", error))?,
            ToolId::parse(self.tool_id)
                .map_err(|error| map_domain_error("tool instance tool_id", error))?,
            self.path,
            self.endpoint,
            ToolHealthStatus::try_from_db(&self.status)
                .map_err(|error| map_domain_error("tool instance status", error))?,
            self.last_checked
                .as_deref()
                .map(|value| parse_datetime("tool instance last_checked", value))
                .transpose()?,
        )
        .map_err(|error| map_domain_error("tool instance integrity", error))
    }
}

#[derive(Debug, FromRow)]
struct ToolVersionRow {
    id: String,
    tool_id: String,
    version: String,
    observed_at: String,
    metadata_json: String,
}

impl ToolVersionRow {
    fn try_into_domain(self) -> Result<ToolVersion, RepositoryError> {
        ToolVersion::new(
            ToolVersionId::parse(self.id)
                .map_err(|error| map_domain_error("tool version id", error))?,
            ToolId::parse(self.tool_id)
                .map_err(|error| map_domain_error("tool version tool_id", error))?,
            self.version,
            parse_datetime("tool version observed_at", &self.observed_at)?,
            parse_json("tool version metadata", Some(&self.metadata_json))?.ok_or_else(|| {
                RepositoryError::serialization("tool version metadata", "missing value")
            })?,
        )
        .map_err(|error| map_domain_error("tool version integrity", error))
    }
}

#[derive(Debug, FromRow)]
struct ToolCapabilityRow {
    tool_id: String,
    capability_name: String,
    metadata_json: String,
}

impl ToolCapabilityRow {
    fn try_into_domain(self) -> Result<Capability, RepositoryError> {
        Capability::new(
            ToolId::parse(self.tool_id)
                .map_err(|error| map_domain_error("tool capability tool_id", error))?,
            self.capability_name,
            parse_json("tool capability metadata", Some(&self.metadata_json))?.ok_or_else(
                || RepositoryError::serialization("tool capability metadata", "missing value"),
            )?,
        )
        .map_err(|error| map_domain_error("tool capability integrity", error))
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteToolRepository;
    use crate::application::ports::ToolRepository;
    use crate::domain::{
        Capability, Tool, ToolHealthStatus, ToolId, ToolInstance, ToolInstanceId, ToolVersion,
        ToolVersionId,
    };
    use crate::infrastructure::database::initialize;
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use tempfile::tempdir;

    fn timestamp(second: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, second).unwrap()
    }

    fn tool(id: &str) -> Tool {
        Tool::new(
            ToolId::parse(id).unwrap(),
            "ComfyUI",
            "image",
            "external runtime",
            json!({"managed": false}),
            timestamp(0),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn repository_persists_tool_children_and_health_observations() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("tools.db"))
            .await
            .unwrap();
        let repository = SqliteToolRepository::new(pool);
        let first = tool("tool_comfy");
        repository.create_tool(&first).await.unwrap();

        assert_eq!(repository.list_tools().await.unwrap(), vec![first.clone()]);
        assert_eq!(
            repository.find_tool(&first.id).await.unwrap(),
            Some(first.clone())
        );

        let instance = ToolInstance::new(
            ToolInstanceId::parse("tins_comfy").unwrap(),
            first.id.clone(),
            Some("C:/ComfyUI".to_owned()),
            Some("http://127.0.0.1:8188".to_owned()),
            ToolHealthStatus::Unknown,
            None,
        )
        .unwrap();
        repository.create_instance(&instance).await.unwrap();
        assert_eq!(
            repository.list_instances(&first.id).await.unwrap(),
            vec![instance.clone()]
        );

        let available = ToolInstance::new(
            instance.id.clone(),
            instance.tool_id.clone(),
            instance.path.clone(),
            instance.endpoint.clone(),
            ToolHealthStatus::Available,
            Some(timestamp(1)),
        )
        .unwrap();
        assert_eq!(
            repository.update_instance(&available).await.unwrap(),
            Some(available.clone())
        );
        assert_eq!(
            repository.find_instance(&instance.id).await.unwrap(),
            Some(available)
        );

        let version = ToolVersion::new(
            ToolVersionId::parse("tver_comfy_1").unwrap(),
            first.id.clone(),
            "0.3.0",
            timestamp(2),
            json!({"source": "manual"}),
        )
        .unwrap();
        repository.create_version(&version).await.unwrap();
        assert_eq!(
            repository.list_versions(&first.id).await.unwrap(),
            vec![version.clone()]
        );
        assert_eq!(
            repository.find_version(&version.id).await.unwrap(),
            Some(version)
        );

        let capability = Capability::new(
            first.id.clone(),
            "image_generation",
            json!({"available": true}),
        )
        .unwrap();
        repository.create_capability(&capability).await.unwrap();
        assert_eq!(
            repository.list_capabilities(&first.id).await.unwrap(),
            vec![capability]
        );

        assert!(repository.delete_tool(&first.id).await.is_err());
        let second = tool("tool_delete");
        repository.create_tool(&second).await.unwrap();
        assert!(repository.delete_tool(&second.id).await.unwrap());
        assert!(!repository.delete_tool(&second.id).await.unwrap());
    }
}
