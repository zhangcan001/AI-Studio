use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    ReferenceAnchorRecord, ReferenceAnchorRepository, RepositoryError,
};
use crate::domain::{
    AssetId, ReferenceAnchor, ReferenceAnchorAsset, ReferenceAnchorId, ReferenceAnchorKind,
};
use async_trait::async_trait;
use sqlx::{SqlitePool, Transaction};
use std::collections::HashMap;

#[derive(Clone)]
pub struct SqliteReferenceAnchorRepository {
    pool: SqlitePool,
}

impl SqliteReferenceAnchorRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct AnchorRow {
    id: String,
    project_id: String,
    kind: String,
    name: String,
    normalized_name: String,
    description: String,
    created_at: String,
    updated_at: String,
}

impl AnchorRow {
    fn into_domain(self) -> Result<ReferenceAnchor, RepositoryError> {
        Ok(ReferenceAnchor {
            id: ReferenceAnchorId::parse(self.id).map_err(|error| {
                RepositoryError::serialization("reference anchor id", error.to_string())
            })?,
            project_id: self.project_id,
            kind: ReferenceAnchorKind::try_from_db(&self.kind).map_err(|error| {
                RepositoryError::serialization("reference anchor kind", error.to_string())
            })?,
            name: self.name,
            normalized_name: self.normalized_name,
            description: self.description,
            created_at: parse_datetime("reference anchor created_at", &self.created_at)?,
            updated_at: parse_datetime("reference anchor updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct AnchorAssetRow {
    anchor_id: String,
    asset_id: String,
    ordinal: i64,
    created_at: String,
}

impl AnchorAssetRow {
    fn into_domain(self) -> Result<ReferenceAnchorAsset, RepositoryError> {
        Ok(ReferenceAnchorAsset {
            anchor_id: ReferenceAnchorId::parse(self.anchor_id).map_err(|error| {
                RepositoryError::serialization(
                    "reference anchor asset anchor_id",
                    error.to_string(),
                )
            })?,
            asset_id: AssetId::parse(self.asset_id).map_err(|error| {
                RepositoryError::serialization("reference anchor asset asset_id", error.to_string())
            })?,
            ordinal: u32::try_from(self.ordinal).map_err(|_| {
                RepositoryError::serialization(
                    "reference anchor asset ordinal",
                    format!("invalid value {}", self.ordinal),
                )
            })?,
            created_at: parse_datetime("reference anchor asset created_at", &self.created_at)?,
        })
    }
}

#[async_trait]
impl ReferenceAnchorRepository for SqliteReferenceAnchorRepository {
    async fn list(&self, project_id: &str) -> Result<Vec<ReferenceAnchorRecord>, RepositoryError> {
        let anchors = sqlx::query_as::<_, AnchorRow>(
            "SELECT id, project_id, kind, name, normalized_name, description, created_at, updated_at
             FROM reference_anchors
             WHERE project_id = ?
             ORDER BY kind ASC, name ASC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let memberships = sqlx::query_as::<_, AnchorAssetRow>(
            "SELECT aa.anchor_id, aa.asset_id, aa.ordinal, aa.created_at
             FROM reference_anchor_assets aa
             INNER JOIN reference_anchors a ON a.id = aa.anchor_id
             WHERE a.project_id = ?
             ORDER BY aa.anchor_id ASC, aa.ordinal ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        assemble_records(anchors, memberships)
    }

    async fn find(
        &self,
        project_id: &str,
        anchor_id: &ReferenceAnchorId,
    ) -> Result<Option<ReferenceAnchorRecord>, RepositoryError> {
        let Some(anchor) = sqlx::query_as::<_, AnchorRow>(
            "SELECT id, project_id, kind, name, normalized_name, description, created_at, updated_at
             FROM reference_anchors
             WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(anchor_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        else {
            return Ok(None);
        };

        let memberships = sqlx::query_as::<_, AnchorAssetRow>(
            "SELECT anchor_id, asset_id, ordinal, created_at
             FROM reference_anchor_assets
             WHERE anchor_id = ?
             ORDER BY ordinal ASC",
        )
        .bind(anchor_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(Some(assemble_records(vec![anchor], memberships)?.remove(0)))
    }

    async fn create_atomic(
        &self,
        anchor: &ReferenceAnchor,
        asset_ids: &[AssetId],
    ) -> Result<ReferenceAnchorRecord, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        insert_anchor(&mut transaction, anchor).await?;
        insert_memberships(&mut transaction, anchor, asset_ids).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(record_from_input(anchor, asset_ids))
    }

    async fn update_atomic(
        &self,
        anchor: &ReferenceAnchor,
        asset_ids: &[AssetId],
    ) -> Result<ReferenceAnchorRecord, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let result = sqlx::query(
            "UPDATE reference_anchors
             SET kind = ?, name = ?, normalized_name = ?, description = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(anchor.kind.as_str())
        .bind(&anchor.name)
        .bind(&anchor.normalized_name)
        .bind(&anchor.description)
        .bind(format_datetime(anchor.updated_at))
        .bind(anchor.id.as_str())
        .bind(&anchor.project_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::not_found(
                "reference anchor",
                anchor.id.as_str(),
            ));
        }

        sqlx::query("DELETE FROM reference_anchor_assets WHERE anchor_id = ?")
            .bind(anchor.id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        insert_memberships(&mut transaction, anchor, asset_ids).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(record_from_input(anchor, asset_ids))
    }

    async fn delete(
        &self,
        project_id: &str,
        anchor_id: &ReferenceAnchorId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM reference_anchors WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(anchor_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }
}

async fn insert_anchor(
    transaction: &mut Transaction<'_, sqlx::Sqlite>,
    anchor: &ReferenceAnchor,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO reference_anchors
         (id, project_id, kind, name, normalized_name, description, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(anchor.id.as_str())
    .bind(&anchor.project_id)
    .bind(anchor.kind.as_str())
    .bind(&anchor.name)
    .bind(&anchor.normalized_name)
    .bind(&anchor.description)
    .bind(format_datetime(anchor.created_at))
    .bind(format_datetime(anchor.updated_at))
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn insert_memberships(
    transaction: &mut Transaction<'_, sqlx::Sqlite>,
    anchor: &ReferenceAnchor,
    asset_ids: &[AssetId],
) -> Result<(), RepositoryError> {
    for (ordinal, asset_id) in asset_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO reference_anchor_assets
             (anchor_id, asset_id, ordinal, created_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(anchor.id.as_str())
        .bind(asset_id.as_str())
        .bind(i64::try_from(ordinal).map_err(|_| {
            RepositoryError::integrity("reference anchor ordinal exceeds SQLite integer range")
        })?)
        .bind(format_datetime(anchor.updated_at))
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }
    Ok(())
}

fn record_from_input(anchor: &ReferenceAnchor, asset_ids: &[AssetId]) -> ReferenceAnchorRecord {
    ReferenceAnchorRecord {
        anchor: anchor.clone(),
        assets: asset_ids
            .iter()
            .enumerate()
            .map(|(ordinal, asset_id)| ReferenceAnchorAsset {
                anchor_id: anchor.id.clone(),
                asset_id: asset_id.clone(),
                ordinal: ordinal as u32,
                created_at: anchor.updated_at,
            })
            .collect(),
    }
}

fn assemble_records(
    anchors: Vec<AnchorRow>,
    memberships: Vec<AnchorAssetRow>,
) -> Result<Vec<ReferenceAnchorRecord>, RepositoryError> {
    let mut memberships_by_anchor: HashMap<String, Vec<ReferenceAnchorAsset>> = HashMap::new();
    for membership in memberships {
        let domain = membership.into_domain()?;
        memberships_by_anchor
            .entry(domain.anchor_id.as_str().to_owned())
            .or_default()
            .push(domain);
    }
    anchors
        .into_iter()
        .map(|row| {
            let anchor_id = row.id.clone();
            Ok(ReferenceAnchorRecord {
                anchor: row.into_domain()?,
                assets: memberships_by_anchor.remove(&anchor_id).unwrap_or_default(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::SqliteReferenceAnchorRepository;
    use crate::application::ports::{ReferenceAnchorRepository, RepositoryError};
    use crate::domain::{AssetId, ReferenceAnchor, ReferenceAnchorId, ReferenceAnchorKind};
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::{TimeZone, Utc};
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, SqliteReferenceAnchorRepository) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        (
            directory,
            pool.clone(),
            SqliteReferenceAnchorRepository::new(pool),
        )
    }

    async fn seed_assets(pool: &SqlitePool, ids: &[&str]) {
        for id in ids {
            sqlx::query(
                "INSERT INTO assets
                 (id, project_id, type, category, name, original_name, storage_path,
                  sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at)
                 VALUES (?, 'project-1', 'image', 'source_image', ?, ?, ?, ?, 'image/png', 2, 2, 1, '{}', ?, ?)",
            )
            .bind(*id)
            .bind(*id)
            .bind(format!("{id}.png"))
            .bind(format!("C:/{id}.png"))
            .bind("sha")
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(pool)
            .await
            .unwrap();
        }
    }

    fn anchor(id: &str, kind: ReferenceAnchorKind) -> ReferenceAnchor {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        ReferenceAnchor {
            id: ReferenceAnchorId::parse(id).unwrap(),
            project_id: "project-1".to_owned(),
            kind,
            name: "Anchor".to_owned(),
            normalized_name: "anchor".to_owned(),
            description: String::new(),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn list_batch_loads_ordered_memberships_and_empty_anchor() {
        let (_directory, pool, repository) = setup().await;
        seed_assets(&pool, &["ast_a", "ast_b", "ast_c"]).await;
        let first = anchor("anc_first", ReferenceAnchorKind::Character);
        let second = ReferenceAnchor {
            id: ReferenceAnchorId::parse("anc_second").unwrap(),
            name: "Empty".to_owned(),
            normalized_name: "empty".to_owned(),
            kind: ReferenceAnchorKind::Scene,
            ..first.clone()
        };
        repository
            .create_atomic(
                &first,
                &[
                    AssetId::parse("ast_b").unwrap(),
                    AssetId::parse("ast_a").unwrap(),
                ],
            )
            .await
            .unwrap();
        repository.create_atomic(&second, &[]).await.unwrap();

        let records = repository.list("project-1").await.unwrap();
        assert_eq!(records.len(), 2);
        let record = records
            .iter()
            .find(|record| record.anchor.id.as_str() == "anc_first")
            .unwrap();
        assert_eq!(
            record
                .assets
                .iter()
                .map(|asset| asset.asset_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ast_b", "ast_a"]
        );
        assert!(records
            .iter()
            .any(|record| record.anchor.id.as_str() == "anc_second" && record.assets.is_empty()));
    }

    #[tokio::test]
    async fn update_is_atomic_and_delete_cascades_memberships_only() {
        let (_directory, pool, repository) = setup().await;
        seed_assets(&pool, &["ast_a", "ast_b"]).await;
        let original = anchor("anc_atomic", ReferenceAnchorKind::Prop);
        repository
            .create_atomic(&original, &[AssetId::parse("ast_a").unwrap()])
            .await
            .unwrap();
        let updated = ReferenceAnchor {
            name: "Updated".to_owned(),
            normalized_name: "updated".to_owned(),
            updated_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
            ..original.clone()
        };
        repository
            .update_atomic(
                &updated,
                &[
                    AssetId::parse("ast_b").unwrap(),
                    AssetId::parse("ast_a").unwrap(),
                ],
            )
            .await
            .unwrap();
        let found = repository
            .find("project-1", &original.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.anchor.name, "Updated");
        assert_eq!(found.assets[0].ordinal, 0);
        assert_eq!(found.assets[0].asset_id.as_str(), "ast_b");

        assert!(repository.delete("project-1", &original.id).await.unwrap());
        assert!(repository
            .find("project-1", &original.id)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM assets WHERE id IN ('ast_a', 'ast_b')"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn duplicate_kind_name_rolls_back_create() {
        let (_directory, pool, repository) = setup().await;
        seed_assets(&pool, &["ast_a"]).await;
        let first = anchor("anc_duplicate_a", ReferenceAnchorKind::Style);
        repository
            .create_atomic(&first, &[AssetId::parse("ast_a").unwrap()])
            .await
            .unwrap();
        let duplicate = ReferenceAnchor {
            id: ReferenceAnchorId::parse("anc_duplicate_b").unwrap(),
            ..first.clone()
        };
        let error = repository
            .create_atomic(&duplicate, &[AssetId::parse("ast_a").unwrap()])
            .await
            .unwrap_err();
        assert!(matches!(error, RepositoryError::Integrity { .. }));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reference_anchors")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }
}
