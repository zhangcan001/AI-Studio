use crate::application::ports::{RepositoryError, StoryEntityRepository};
use crate::domain::story_entity::{StoryEntity, StoryEntityKind, StoryEntityUpsert};
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

pub struct SqliteStoryEntityRepository {
    pool: SqlitePool,
}

impl SqliteStoryEntityRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn load_images(&self, entity_id: &str) -> Result<Vec<String>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT asset_id FROM story_entity_images WHERE entity_id = ? ORDER BY ordinal ASC, asset_id ASC",
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database { message: e.to_string() })?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("asset_id"))
            .collect())
    }

    async fn replace_images(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        entity_id: &str,
        asset_ids: &[String],
        now: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM story_entity_images WHERE entity_id = ?")
            .bind(entity_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
        for (ordinal, asset_id) in asset_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO story_entity_images (entity_id, asset_id, ordinal, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(entity_id)
            .bind(asset_id)
            .bind(ordinal as i64)
            .bind(now)
            .execute(&mut **tx)
            .await
            .map_err(|e| RepositoryError::Database { message: e.to_string() })?;
        }
        Ok(())
    }

    fn map_row(
        row: &sqlx::sqlite::SqliteRow,
        images: Vec<String>,
    ) -> Result<StoryEntity, RepositoryError> {
        let kind = StoryEntityKind::parse(&row.get::<String, _>("kind"))
            .map_err(|message| RepositoryError::Integrity { message })?;
        Ok(StoryEntity {
            id: row.get("id"),
            project_id: row.get("project_id"),
            kind,
            name: row.get("name"),
            description: row.get("description"),
            cover_asset_id: row.get("cover_asset_id"),
            image_asset_ids: images,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

#[async_trait]
impl StoryEntityRepository for SqliteStoryEntityRepository {
    async fn list(
        &self,
        project_id: &str,
        kind: Option<StoryEntityKind>,
    ) -> Result<Vec<StoryEntity>, RepositoryError> {
        let rows = if let Some(kind) = kind {
            sqlx::query(
                "SELECT id, project_id, kind, name, description, cover_asset_id, created_at, updated_at
                 FROM story_entities WHERE project_id = ? AND kind = ?
                 ORDER BY updated_at DESC, id ASC",
            )
            .bind(project_id)
            .bind(kind.as_str())
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                "SELECT id, project_id, kind, name, description, cover_asset_id, created_at, updated_at
                 FROM story_entities WHERE project_id = ?
                 ORDER BY kind ASC, updated_at DESC, id ASC",
            )
            .bind(project_id)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| RepositoryError::Database { message: e.to_string() })?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("id");
            let images = self.load_images(&id).await?;
            out.push(Self::map_row(&row, images)?);
        }
        Ok(out)
    }

    async fn get(
        &self,
        project_id: &str,
        id: &str,
    ) -> Result<Option<StoryEntity>, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, project_id, kind, name, description, cover_asset_id, created_at, updated_at
             FROM story_entities WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database {
            message: e.to_string(),
        })?;
        let Some(row) = row else {
            return Ok(None);
        };
        let images = self.load_images(id).await?;
        Ok(Some(Self::map_row(&row, images)?))
    }

    async fn create(
        &self,
        project_id: &str,
        kind: StoryEntityKind,
        input: StoryEntityUpsert,
    ) -> Result<StoryEntity, RepositoryError> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(RepositoryError::Integrity {
                message: "名称不能为空".into(),
            });
        }
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
        sqlx::query(
            "INSERT INTO story_entities (id, project_id, kind, name, description, cover_asset_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(project_id)
        .bind(kind.as_str())
        .bind(name)
        .bind(input.description.trim())
        .bind(&input.cover_asset_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                RepositoryError::Integrity { message: "同类型下名称已存在".into() }
            } else {
                RepositoryError::database(msg)
            }
        })?;
        self.replace_images(&mut tx, &id, &input.image_asset_ids, &now)
            .await?;
        tx.commit().await.map_err(|e| RepositoryError::Database {
            message: e.to_string(),
        })?;
        self.get(project_id, &id)
            .await?
            .ok_or_else(|| RepositoryError::Integrity {
                message: "story entity missing after create".into(),
            })
    }

    async fn update(
        &self,
        project_id: &str,
        id: &str,
        input: StoryEntityUpsert,
    ) -> Result<StoryEntity, RepositoryError> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(RepositoryError::Integrity {
                message: "名称不能为空".into(),
            });
        }
        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
        let result = sqlx::query(
            "UPDATE story_entities SET name = ?, description = ?, cover_asset_id = ?, updated_at = ?
             WHERE project_id = ? AND id = ?",
        )
        .bind(name)
        .bind(input.description.trim())
        .bind(&input.cover_asset_id)
        .bind(&now)
        .bind(project_id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                RepositoryError::Integrity { message: "同类型下名称已存在".into() }
            } else {
                RepositoryError::database(msg)
            }
        })?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound {
                entity: "story_entity".into(),
                id: id.to_string(),
            });
        }
        self.replace_images(&mut tx, id, &input.image_asset_ids, &now)
            .await?;
        tx.commit().await.map_err(|e| RepositoryError::Database {
            message: e.to_string(),
        })?;
        self.get(project_id, id)
            .await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity: "story_entity".into(),
                id: id.to_string(),
            })
    }

    async fn delete(&self, project_id: &str, id: &str) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM story_entities WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound {
                entity: "story_entity".into(),
                id: id.to_string(),
            });
        }
        Ok(())
    }
}
