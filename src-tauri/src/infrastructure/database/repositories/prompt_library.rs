use super::{map_sqlx_error, parse_datetime, parse_json};
use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::{
    PromptEntryRecord, PromptLibraryQuery, PromptLibraryRepository, PromptVersionRecord,
    RepositoryError,
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
    async fn list_page(
        &self,
        request: PromptLibraryQuery,
    ) -> Result<PageResult<PromptEntryRecord>, RepositoryError> {
        let mut sql = String::from(
            "SELECT e.id, e.project_id, e.kind, e.name, e.normalized_name, e.tags_json,
                    e.created_at, e.updated_at,
                    (SELECT COUNT(*) FROM prompt_versions v WHERE v.prompt_id = e.id) AS version_count
             FROM prompt_entries e WHERE e.project_id = ?",
        );
        if request.kind.is_some() {
            sql.push_str(" AND e.kind = ?");
        }
        if request.keyword.is_some() {
            sql.push_str(
                " AND (e.name LIKE ? COLLATE NOCASE OR e.tags_json LIKE ? COLLATE NOCASE)",
            );
        }
        if request.tag.is_some() {
            sql.push_str(" AND e.tags_json LIKE ? COLLATE NOCASE");
        }
        if request.cursor.is_some() {
            sql.push_str(" AND (e.updated_at < ? OR (e.updated_at = ? AND e.id < ?))");
        }
        sql.push_str(" ORDER BY e.updated_at DESC, e.id DESC LIMIT ?");

        let mut query = sqlx::query_as::<_, PromptEntryRow>(&sql).bind(&request.project_id);
        if let Some(kind) = request.kind.as_deref() {
            query = query.bind(kind);
        }
        if let Some(keyword) = request.keyword.as_deref() {
            let pattern = format!("%{keyword}%");
            query = query.bind(pattern.clone()).bind(pattern);
        }
        if let Some(tag) = request.tag.as_deref() {
            let pattern = format!("%{}{}{}%", '"', tag, '"');
            query = query.bind(pattern);
        }
        if let Some(cursor) = request.cursor {
            let updated_at = cursor.created_at.to_rfc3339();
            query = query
                .bind(updated_at.clone())
                .bind(updated_at)
                .bind(cursor.id);
        }
        let limit = request.limit.clamp(1, 100);
        query = query.bind(i64::from(limit) + 1);
        let mut rows = query.fetch_all(&self.pool).await.map_err(map_sqlx_error)?;
        let has_more = rows.len() > limit as usize;
        if has_more {
            rows.truncate(limit as usize);
        }
        let next_cursor = if has_more {
            rows.last()
                .map(|row| {
                    Ok(PageCursor::for_item(
                        parse_datetime("prompt updated_at", &row.updated_at)?,
                        row.id.clone(),
                    ))
                })
                .transpose()?
        } else {
            None
        };
        let items = rows
            .into_iter()
            .map(PromptEntryRow::try_into_record)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PageResult { items, next_cursor })
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

#[cfg(test)]
mod tests {
    use super::SqlitePromptLibraryRepository;
    use crate::application::pagination::PageCursor;
    use crate::application::ports::{
        PromptEntryRecord, PromptLibraryQuery, PromptLibraryRepository, PromptVersionRecord,
    };
    use crate::infrastructure::database::initialize;
    use tempfile::tempdir;

    #[tokio::test]
    async fn keyset_pages_are_stable_scoped_and_bounded() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("prompts.db"))
            .await
            .unwrap();
        sqlx::query("INSERT INTO projects (id, name, root_path, created_at, updated_at) VALUES ('prompt-page-project', '分页', 'C:/page', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'), ('prompt-page-other', '其他', 'C:/other', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')")
            .execute(&pool)
            .await
            .unwrap();
        let repository = SqlitePromptLibraryRepository::new(pool);
        for index in 0..31 {
            let id = format!("prm_page_{index:03}");
            repository
                .create(
                    &PromptEntryRecord {
                        id: id.clone(),
                        project_id: "prompt-page-project".to_owned(),
                        kind: "prompt".to_owned(),
                        name: format!("分页 {index:03}"),
                        normalized_name: format!("分页 {index:03}"),
                        tags_json: "[\"分页\"]".to_owned(),
                        created_at: "2026-01-01T00:00:00+00:00".to_owned(),
                        updated_at: "2026-01-01T00:00:00+00:00".to_owned(),
                        version_count: 1,
                    },
                    &PromptVersionRecord {
                        id: format!("prv_page_{index:03}"),
                        prompt_id: id,
                        version: 1,
                        text: format!("text {index}"),
                        created_at: "2026-01-01T00:00:00+00:00".to_owned(),
                    },
                )
                .await
                .unwrap();
        }
        let first = repository
            .list_page(PromptLibraryQuery {
                project_id: "prompt-page-project".to_owned(),
                kind: Some("prompt".to_owned()),
                keyword: Some("分页".to_owned()),
                tag: Some("分页".to_owned()),
                cursor: None,
                limit: 30,
            })
            .await
            .unwrap();
        assert_eq!(first.items.len(), 30);
        assert!(first.next_cursor.is_some());
        let second = repository
            .list_page(PromptLibraryQuery {
                project_id: "prompt-page-project".to_owned(),
                kind: Some("prompt".to_owned()),
                keyword: Some("分页".to_owned()),
                tag: Some("分页".to_owned()),
                cursor: first.next_cursor,
                limit: 30,
            })
            .await
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert!(second.next_cursor.is_none());
        assert!(first
            .items
            .iter()
            .all(|left| second.items.iter().all(|right| left.id != right.id)));
        assert_eq!(first.items[0].id, "prm_page_030");
        assert_eq!(second.items[0].id, "prm_page_000");
        let isolated = repository
            .list_page(PromptLibraryQuery {
                project_id: "prompt-page-other".to_owned(),
                kind: None,
                keyword: None,
                tag: None,
                cursor: Some(PageCursor::for_item(
                    chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00+00:00")
                        .unwrap()
                        .with_timezone(&chrono::Utc),
                    "prm_page_030",
                )),
                limit: 30,
            })
            .await
            .unwrap();
        assert!(isolated.items.is_empty());
    }
}
