use super::{map_sqlx_error, parse_json};
use crate::application::ports::{
    PromptEntryRecord, PromptLibraryRepository, PromptVersionRecord, RepositoryError,
};
use async_trait::async_trait;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone)]
pub struct SqlitePromptLibraryRepository {
    pool: SqlitePool,
}

impl SqlitePromptLibraryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PromptLibraryRepository for SqlitePromptLibraryRepository {
    async fn list(
        &self,
        project_id: &str,
        kind: Option<&str>,
        keyword: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<PromptEntryRecord>, RepositoryError> {
        let mut sql = String::from(
            "SELECT e.id, e.project_id, e.kind, e.name, e.normalized_name, e.tags_json,
                    e.created_at, e.updated_at,
                    (SELECT COUNT(*) FROM prompt_versions v WHERE v.prompt_id = e.id) AS version_count
             FROM prompt_entries e WHERE e.project_id = ?",
        );
        if kind.is_some() {
            sql.push_str(" AND e.kind = ?");
        }
        if keyword.is_some() {
            sql.push_str(
                " AND (e.name LIKE ? COLLATE NOCASE OR e.tags_json LIKE ? COLLATE NOCASE)",
            );
        }
        if tag.is_some() {
            sql.push_str(" AND e.tags_json LIKE ? COLLATE NOCASE");
        }
        sql.push_str(" ORDER BY e.updated_at DESC, e.id ASC LIMIT 200");

        let mut query = sqlx::query_as::<_, PromptEntryRow>(&sql).bind(project_id);
        if let Some(kind) = kind {
            query = query.bind(kind);
        }
        if let Some(keyword) = keyword {
            let pattern = format!("%{keyword}%");
            query = query.bind(pattern.clone()).bind(pattern);
        }
        if let Some(tag) = tag {
            let pattern = format!("%{}{}{}%", '"', tag, '"');
            query = query.bind(pattern);
        }
        let rows = query.fetch_all(&self.pool).await.map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(PromptEntryRow::try_into_record)
            .collect()
    }

    async fn find_by_id(
        &self,
        project_id: &str,
        prompt_id: &str,
    ) -> Result<Option<PromptEntryRecord>, RepositoryError> {
        let row = sqlx::query_as::<_, PromptEntryRow>(
            "SELECT e.id, e.project_id, e.kind, e.name, e.normalized_name, e.tags_json,
                    e.created_at, e.updated_at,
                    (SELECT COUNT(*) FROM prompt_versions v WHERE v.prompt_id = e.id) AS version_count
             FROM prompt_entries e WHERE e.project_id = ? AND e.id = ?",
        )
        .bind(project_id)
        .bind(prompt_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(PromptEntryRow::try_into_record).transpose()
    }

    async fn list_versions(
        &self,
        project_id: &str,
        prompt_id: &str,
    ) -> Result<Vec<PromptVersionRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, PromptVersionRow>(
            "SELECT v.id, v.prompt_id, v.version, v.text, v.created_at
             FROM prompt_versions v
             JOIN prompt_entries e ON e.id = v.prompt_id
             WHERE e.project_id = ? AND v.prompt_id = ?
             ORDER BY v.version ASC",
        )
        .bind(project_id)
        .bind(prompt_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(rows
            .into_iter()
            .map(PromptVersionRow::into_record)
            .collect())
    }

    async fn create(
        &self,
        entry: &PromptEntryRecord,
        first_version: &PromptVersionRecord,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "INSERT INTO prompt_entries
             (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&entry.id)
        .bind(&entry.project_id)
        .bind(&entry.kind)
        .bind(&entry.name)
        .bind(&entry.normalized_name)
        .bind(&entry.tags_json)
        .bind(&entry.created_at)
        .bind(&entry.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "INSERT INTO prompt_versions (id, prompt_id, version, text, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&first_version.id)
        .bind(&first_version.prompt_id)
        .bind(first_version.version)
        .bind(&first_version.text)
        .bind(&first_version.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn append_version(
        &self,
        project_id: &str,
        prompt_id: &str,
        version_id: &str,
        text: &str,
        created_at: &str,
    ) -> Result<PromptVersionRecord, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let owned: Option<String> =
            sqlx::query_scalar("SELECT id FROM prompt_entries WHERE project_id = ? AND id = ?")
                .bind(project_id)
                .bind(prompt_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
        if owned.is_none() {
            return Err(RepositoryError::not_found("prompt", prompt_id));
        }
        let latest: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) FROM prompt_versions WHERE prompt_id = ?",
        )
        .bind(prompt_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        let version = latest.saturating_add(1);
        sqlx::query(
            "INSERT INTO prompt_versions (id, prompt_id, version, text, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(version_id)
        .bind(prompt_id)
        .bind(version)
        .bind(text)
        .bind(created_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query("UPDATE prompt_entries SET updated_at = ? WHERE project_id = ? AND id = ?")
            .bind(created_at)
            .bind(project_id)
            .bind(prompt_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(PromptVersionRecord {
            id: version_id.to_owned(),
            prompt_id: prompt_id.to_owned(),
            version,
            text: text.to_owned(),
            created_at: created_at.to_owned(),
        })
    }

    async fn update_metadata(
        &self,
        project_id: &str,
        prompt_id: &str,
        name: &str,
        normalized_name: &str,
        tags_json: &str,
        updated_at: &str,
    ) -> Result<Option<PromptEntryRecord>, RepositoryError> {
        let result = sqlx::query(
            "UPDATE prompt_entries
             SET name = ?, normalized_name = ?, tags_json = ?, updated_at = ?
             WHERE project_id = ? AND id = ?",
        )
        .bind(name)
        .bind(normalized_name)
        .bind(tags_json)
        .bind(updated_at)
        .bind(project_id)
        .bind(prompt_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_by_id(project_id, prompt_id).await
    }

    async fn delete(&self, project_id: &str, prompt_id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM prompt_entries WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(prompt_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(FromRow)]
struct PromptEntryRow {
    id: String,
    project_id: String,
    kind: String,
    name: String,
    normalized_name: String,
    tags_json: String,
    created_at: String,
    updated_at: String,
    version_count: i64,
}

impl PromptEntryRow {
    fn try_into_record(self) -> Result<PromptEntryRecord, RepositoryError> {
        let tags = parse_json("prompt tags", Some(&self.tags_json))?
            .ok_or_else(|| RepositoryError::serialization("prompt tags", "missing value"))?;
        if !tags.is_array() {
            return Err(RepositoryError::serialization(
                "prompt tags",
                "expected array",
            ));
        }
        Ok(PromptEntryRecord {
            id: self.id,
            project_id: self.project_id,
            kind: self.kind,
            name: self.name,
            normalized_name: self.normalized_name,
            tags_json: self.tags_json,
            created_at: self.created_at,
            updated_at: self.updated_at,
            version_count: self.version_count,
        })
    }
}

#[derive(FromRow)]
struct PromptVersionRow {
    id: String,
    prompt_id: String,
    version: i64,
    text: String,
    created_at: String,
}

impl PromptVersionRow {
    fn into_record(self) -> PromptVersionRecord {
        PromptVersionRecord {
            id: self.id,
            prompt_id: self.prompt_id,
            version: self.version,
            text: self.text,
            created_at: self.created_at,
        }
    }
}
