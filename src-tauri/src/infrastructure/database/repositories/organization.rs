use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    AssetOrganization, AssetTag, NewProjectTemplate, OrganizationRepository, ProjectTemplate,
    RepositoryError,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::collections::HashMap;

#[derive(Clone)]
pub struct SqliteOrganizationRepository {
    pool: SqlitePool,
}

impl SqliteOrganizationRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct TagRow {
    id: String,
    project_id: String,
    name: String,
    normalized_name: String,
    created_at: String,
    updated_at: String,
}

impl TagRow {
    fn into_model(self) -> Result<AssetTag, RepositoryError> {
        Ok(AssetTag {
            id: self.id,
            project_id: self.project_id,
            name: self.name,
            normalized_name: self.normalized_name,
            created_at: parse_datetime("asset tag created_at", &self.created_at)?,
            updated_at: parse_datetime("asset tag updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct TemplateRow {
    id: String,
    name: String,
    normalized_name: String,
    description: Option<String>,
    workflow_version_id: String,
    recipe_id: String,
    values_json: String,
    available: i64,
    created_at: String,
    updated_at: String,
}

impl TemplateRow {
    fn into_model(self) -> Result<ProjectTemplate, RepositoryError> {
        Ok(ProjectTemplate {
            id: self.id,
            name: self.name,
            normalized_name: self.normalized_name,
            description: self.description,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            values: serde_json::from_str(&self.values_json).map_err(|error| {
                RepositoryError::serialization("project template values_json", error.to_string())
            })?,
            available: self.available != 0,
            created_at: parse_datetime("project template created_at", &self.created_at)?,
            updated_at: parse_datetime("project template updated_at", &self.updated_at)?,
        })
    }
}

const TEMPLATE_SELECT: &str = "SELECT pt.id, pt.name, pt.normalized_name, pt.description,
    pt.workflow_version_id, pt.recipe_id, pt.values_json, pt.created_at, pt.updated_at,
    CASE WHEN w.current_version_id = pt.workflow_version_id
      AND r.id IS NOT NULL AND r.workflow_version_id = pt.workflow_version_id
      AND COALESCE(wrs.enabled, 1) = 1
      AND r.version = (SELECT MAX(latest.version) FROM recipes latest WHERE latest.workflow_version_id = pt.workflow_version_id)
      THEN 1 ELSE 0 END AS available
    FROM project_templates pt
    LEFT JOIN workflow_versions wv ON wv.id = pt.workflow_version_id
    LEFT JOIN workflows w ON w.id = wv.workflow_id
    LEFT JOIN recipes r ON r.id = pt.recipe_id
    LEFT JOIN workflow_runtime_states wrs ON wrs.workflow_version_id = pt.workflow_version_id";

#[async_trait]
impl OrganizationRepository for SqliteOrganizationRepository {
    async fn list_tags(&self, project_id: &str) -> Result<Vec<AssetTag>, RepositoryError> {
        let rows = sqlx::query_as::<_, TagRow>(
            "SELECT id, project_id, name, normalized_name, created_at, updated_at
             FROM asset_tags WHERE project_id = ? ORDER BY name ASC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(TagRow::into_model).collect()
    }

    async fn create_tag(&self, tag: AssetTag) -> Result<AssetTag, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset_tags WHERE project_id = ?")
            .bind(&tag.project_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        if count >= 100 {
            return Err(RepositoryError::integrity(
                "ASSET_TAG_PROJECT_LIMIT: a project supports at most 100 tags",
            ));
        }
        sqlx::query("INSERT INTO asset_tags (id, project_id, name, normalized_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&tag.id).bind(&tag.project_id).bind(&tag.name).bind(&tag.normalized_name)
            .bind(format_datetime(tag.created_at)).bind(format_datetime(tag.updated_at))
            .execute(&mut *tx).await.map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(tag)
    }

    async fn update_tag(
        &self,
        project_id: &str,
        tag_id: &str,
        name: &str,
        normalized_name: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<AssetTag>, RepositoryError> {
        let result = sqlx::query("UPDATE asset_tags SET name = ?, normalized_name = ?, updated_at = ? WHERE id = ? AND project_id = ?")
            .bind(name).bind(normalized_name).bind(format_datetime(updated_at)).bind(tag_id).bind(project_id)
            .execute(&self.pool).await.map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, TagRow>("SELECT id, project_id, name, normalized_name, created_at, updated_at FROM asset_tags WHERE id = ?")
            .bind(tag_id).fetch_one(&self.pool).await.map_err(map_sqlx_error)?;
        Ok(Some(row.into_model()?))
    }

    async fn delete_tag(&self, project_id: &str, tag_id: &str) -> Result<bool, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query("DELETE FROM asset_tag_links WHERE project_id = ? AND tag_id = ?")
            .bind(project_id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        let result = sqlx::query("DELETE FROM asset_tags WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn assign_tag(
        &self,
        project_id: &str,
        asset_id: &str,
        tag_id: &str,
        created_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        let asset_project: Option<String> =
            sqlx::query_scalar("SELECT project_id FROM assets WHERE id = ?")
                .bind(asset_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_sqlx_error)?;
        let tag_project: Option<String> =
            sqlx::query_scalar("SELECT project_id FROM asset_tags WHERE id = ?")
                .bind(tag_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_sqlx_error)?;
        if asset_project.as_deref() != Some(project_id)
            || tag_project.as_deref() != Some(project_id)
        {
            return Err(RepositoryError::integrity("ASSET_TAG_PROJECT_MISMATCH: asset, tag and request must belong to the same project"));
        }
        let linked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM asset_tag_links WHERE asset_id = ? AND tag_id = ?",
        )
        .bind(asset_id)
        .bind(tag_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;
        if linked == 0 {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM asset_tag_links WHERE asset_id = ?")
                    .bind(asset_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(map_sqlx_error)?;
            if count >= 20 {
                return Err(RepositoryError::integrity(
                    "ASSET_TAG_ASSET_LIMIT: an asset supports at most 20 tags",
                ));
            }
            sqlx::query("INSERT INTO asset_tag_links (asset_id, tag_id, project_id, created_at) VALUES (?, ?, ?, ?)")
                .bind(asset_id).bind(tag_id).bind(project_id).bind(format_datetime(created_at))
                .execute(&mut *tx).await.map_err(map_sqlx_error)?;
        }
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn remove_tag(
        &self,
        project_id: &str,
        asset_id: &str,
        tag_id: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "DELETE FROM asset_tag_links WHERE project_id = ? AND asset_id = ? AND tag_id = ?",
        )
        .bind(project_id)
        .bind(asset_id)
        .bind(tag_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn set_favorite(
        &self,
        project_id: &str,
        asset_id: &str,
        favorite: bool,
        created_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let asset_project: Option<String> =
            sqlx::query_scalar("SELECT project_id FROM assets WHERE id = ?")
                .bind(asset_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_sqlx_error)?;
        if asset_project.as_deref() != Some(project_id) {
            return Err(RepositoryError::integrity("ASSET_FAVORITE_PROJECT_MISMATCH: asset and request must belong to the same project"));
        }
        if favorite {
            sqlx::query("INSERT INTO asset_favorites (asset_id, project_id, created_at) VALUES (?, ?, ?) ON CONFLICT(asset_id) DO NOTHING")
                .bind(asset_id).bind(project_id).bind(format_datetime(created_at)).execute(&self.pool).await.map_err(map_sqlx_error)?;
        } else {
            sqlx::query("DELETE FROM asset_favorites WHERE project_id = ? AND asset_id = ?")
                .bind(project_id)
                .bind(asset_id)
                .execute(&self.pool)
                .await
                .map_err(map_sqlx_error)?;
        }
        Ok(())
    }

    async fn organization_for_assets(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<HashMap<String, AssetOrganization>, RepositoryError> {
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut result = asset_ids
            .iter()
            .cloned()
            .map(|id| (id, AssetOrganization::default()))
            .collect::<HashMap<_, _>>();
        let mut favorites =
            QueryBuilder::<Sqlite>::new("SELECT asset_id FROM asset_favorites WHERE project_id = ");
        favorites.push_bind(project_id).push(" AND asset_id IN (");
        let mut separated = favorites.separated(", ");
        for id in asset_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        for id in favorites
            .build_query_scalar::<String>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
        {
            if let Some(item) = result.get_mut(&id) {
                item.is_favorite = true;
            }
        }
        let mut tags = QueryBuilder::<Sqlite>::new("SELECT l.asset_id, t.id, t.project_id, t.name, t.normalized_name, t.created_at, t.updated_at FROM asset_tag_links l JOIN asset_tags t ON t.id = l.tag_id WHERE l.project_id = ");
        tags.push_bind(project_id).push(" AND l.asset_id IN (");
        let mut separated = tags.separated(", ");
        for id in asset_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(") ORDER BY t.name ASC, t.id ASC");
        #[derive(sqlx::FromRow)]
        struct LinkedTagRow {
            asset_id: String,
            id: String,
            project_id: String,
            name: String,
            normalized_name: String,
            created_at: String,
            updated_at: String,
        }
        for row in tags
            .build_query_as::<LinkedTagRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
        {
            let asset_id = row.asset_id;
            let tag = TagRow {
                id: row.id,
                project_id: row.project_id,
                name: row.name,
                normalized_name: row.normalized_name,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
            .into_model()?;
            if let Some(item) = result.get_mut(&asset_id) {
                item.tags.push(tag);
            }
        }
        Ok(result)
    }

    async fn create_template(
        &self,
        template: NewProjectTemplate,
    ) -> Result<ProjectTemplate, RepositoryError> {
        let values_json = serde_json::to_string(&template.values).map_err(|error| {
            RepositoryError::serialization("project template values", error.to_string())
        })?;
        sqlx::query("INSERT INTO project_templates (id, name, normalized_name, description, workflow_version_id, recipe_id, values_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&template.id).bind(&template.name).bind(&template.normalized_name).bind(&template.description)
            .bind(&template.workflow_version_id).bind(&template.recipe_id).bind(values_json)
            .bind(format_datetime(template.now)).bind(format_datetime(template.now))
            .execute(&self.pool).await.map_err(map_sqlx_error)?;
        self.find_template(&template.id)
            .await?
            .ok_or_else(|| RepositoryError::not_found("project template", template.id))
    }

    async fn list_templates(&self) -> Result<Vec<ProjectTemplate>, RepositoryError> {
        let rows = sqlx::query_as::<_, TemplateRow>(&format!(
            "{TEMPLATE_SELECT} ORDER BY pt.updated_at DESC, pt.id ASC"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(TemplateRow::into_model).collect()
    }

    async fn find_template(
        &self,
        template_id: &str,
    ) -> Result<Option<ProjectTemplate>, RepositoryError> {
        let row = sqlx::query_as::<_, TemplateRow>(&format!("{TEMPLATE_SELECT} WHERE pt.id = ?"))
            .bind(template_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        row.map(TemplateRow::into_model).transpose()
    }

    async fn update_template(
        &self,
        template_id: &str,
        name: &str,
        normalized_name: &str,
        description: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ProjectTemplate>, RepositoryError> {
        let result = sqlx::query("UPDATE project_templates SET name = ?, normalized_name = ?, description = ?, updated_at = ? WHERE id = ?")
            .bind(name).bind(normalized_name).bind(description).bind(format_datetime(updated_at)).bind(template_id)
            .execute(&self.pool).await.map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_template(template_id).await
    }

    async fn delete_template(&self, template_id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM project_templates WHERE id = ?")
            .bind(template_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteOrganizationRepository;
    use crate::application::ports::{AssetTag, NewProjectTemplate, OrganizationRepository};
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use serde_json::json;
    use tempfile::tempdir;

    async fn setup() -> (sqlx::SqlitePool, SqliteOrganizationRepository) {
        let directory = tempdir().unwrap();
        let path = directory.keep().join("app.db");
        let pool = initialize(&path).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("INSERT INTO projects (id, name, root_path, created_at, updated_at) VALUES ('project-2', 'Other', 'C:/other', ?, ?)")
            .bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        for (id, project) in [
            ("ast_a", "project-1"),
            ("ast_b", "project-1"),
            ("ast_other", "project-2"),
        ] {
            sqlx::query("INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at) VALUES (?, ?, 'image', 'source_image', ?, ?, ?, ?, 'image/png', 1, 1, 1, '{}', ?, ?)")
                .bind(id).bind(project).bind(id).bind(format!("{id}.png")).bind(format!("C:/{id}.png")).bind("a".repeat(64)).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        }
        (pool.clone(), SqliteOrganizationRepository::new(pool))
    }

    fn tag(id: &str, project: &str, name: &str) -> AssetTag {
        let now = Utc::now();
        AssetTag {
            id: id.to_owned(),
            project_id: project.to_owned(),
            name: name.to_owned(),
            normalized_name: name.to_lowercase(),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn tags_assign_remove_delete_transaction_and_cross_project_guard() {
        let (pool, repository) = setup().await;
        repository
            .create_tag(tag("tag_people", "project-1", "人物"))
            .await
            .unwrap();
        repository
            .create_tag(tag("tag_character", "project-1", "Character"))
            .await
            .unwrap();
        assert!(repository
            .create_tag(tag("tag_duplicate", "project-1", "character"))
            .await
            .is_err());
        repository
            .assign_tag("project-1", "ast_a", "tag_people", Utc::now())
            .await
            .unwrap();
        repository
            .assign_tag("project-1", "ast_a", "tag_people", Utc::now())
            .await
            .unwrap();
        assert!(repository
            .assign_tag("project-2", "ast_other", "tag_people", Utc::now())
            .await
            .unwrap_err()
            .to_string()
            .contains("PROJECT_MISMATCH"));
        let organization = repository
            .organization_for_assets("project-1", &["ast_a".to_owned(), "ast_b".to_owned()])
            .await
            .unwrap();
        assert_eq!(organization["ast_a"].tags.len(), 1);
        repository
            .remove_tag("project-1", "ast_a", "tag_people")
            .await
            .unwrap();
        repository
            .assign_tag("project-1", "ast_a", "tag_people", Utc::now())
            .await
            .unwrap();
        repository
            .delete_tag("project-1", "tag_people")
            .await
            .unwrap();
        let links: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_tag_links WHERE tag_id = 'tag_people'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(links, 0);
    }

    #[tokio::test]
    async fn favorites_are_idempotent_and_project_scoped() {
        let (_pool, repository) = setup().await;
        repository
            .set_favorite("project-1", "ast_a", true, Utc::now())
            .await
            .unwrap();
        repository
            .set_favorite("project-1", "ast_a", true, Utc::now())
            .await
            .unwrap();
        assert!(
            repository
                .organization_for_assets("project-1", &["ast_a".to_owned()])
                .await
                .unwrap()["ast_a"]
                .is_favorite
        );
        repository
            .set_favorite("project-1", "ast_a", false, Utc::now())
            .await
            .unwrap();
        repository
            .set_favorite("project-1", "ast_a", false, Utc::now())
            .await
            .unwrap();
        assert!(
            !repository
                .organization_for_assets("project-1", &["ast_a".to_owned()])
                .await
                .unwrap()["ast_a"]
                .is_favorite
        );
        assert!(repository
            .set_favorite("project-1", "ast_other", true, Utc::now())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn enforces_twenty_tags_per_asset_and_one_hundred_per_project() {
        let (pool, repository) = setup().await;
        for index in 0..100 {
            sqlx::query("INSERT INTO asset_tags (id, project_id, name, normalized_name, created_at, updated_at) VALUES (?, 'project-1', ?, ?, ?, ?)")
                .bind(format!("tag_{index:03}")).bind(format!("标签{index:03}")).bind(format!("标签{index:03}")).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        }
        assert!(repository
            .create_tag(tag("tag_101", "project-1", "超限"))
            .await
            .is_err());
        for index in 0..20 {
            repository
                .assign_tag("project-1", "ast_a", &format!("tag_{index:03}"), Utc::now())
                .await
                .unwrap();
        }
        assert!(repository
            .assign_tag("project-1", "ast_a", "tag_020", Utc::now())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn project_templates_crud_and_normalized_duplicate() {
        let (_pool, repository) = setup().await;
        let now = Utc::now();
        let template = repository
            .create_template(NewProjectTemplate {
                id: "ptm_one".to_owned(),
                name: "海报".to_owned(),
                normalized_name: "海报".to_owned(),
                description: Some("说明".to_owned()),
                workflow_version_id: "workflow-version-1".to_owned(),
                recipe_id: "recipe-1".to_owned(),
                values: json!({"prompt":{"type":"string","value":"人物"}}),
                now,
            })
            .await
            .unwrap();
        assert!(template.available);
        assert!(repository
            .create_template(NewProjectTemplate {
                id: "ptm_two".to_owned(),
                name: "海报".to_owned(),
                normalized_name: "海报".to_owned(),
                description: None,
                workflow_version_id: "workflow-version-1".to_owned(),
                recipe_id: "recipe-1".to_owned(),
                values: json!({}),
                now
            })
            .await
            .is_err());
        let updated = repository
            .update_template("ptm_one", "新海报", "新海报", None, Utc::now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "新海报");
        assert!(repository.delete_template("ptm_one").await.unwrap());
        assert!(repository.find_template("ptm_one").await.unwrap().is_none());
    }
}
