use super::{format_datetime, map_domain_error, map_sqlx_error, parse_datetime};
use crate::application::ports::{ReferenceSetRepository, RepositoryError};
use crate::domain::consistency::{ReferenceSet, ReferenceSetItem, ReferenceSetPurpose};
use async_trait::async_trait;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[derive(Clone)]
pub struct SqliteReferenceSetRepository {
    pool: SqlitePool,
}

impl SqliteReferenceSetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

const REFERENCE_SET_COLUMNS: &str =
    "id, project_id, name, purpose, description, owner_profile_type, owner_profile_id, active_revision_id, created_at, updated_at";

#[derive(FromRow)]
struct ReferenceSetRow {
    id: String,
    project_id: String,
    name: String,
    purpose: String,
    description: String,
    owner_profile_type: Option<String>,
    owner_profile_id: Option<String>,
    active_revision_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl ReferenceSetRow {
    fn try_into_domain(self) -> Result<ReferenceSet, RepositoryError> {
        Ok(ReferenceSet {
            id: self.id,
            project_id: self.project_id,
            name: self.name,
            purpose: ReferenceSetPurpose::try_from_db(&self.purpose)
                .map_err(|error| map_domain_error("reference set purpose", error))?,
            description: self.description,
            owner_profile_type: self
                .owner_profile_type
                .as_deref()
                .map(|value| {
                    crate::domain::consistency::ProfileType::try_from_db(value).map_err(|error| {
                        map_domain_error("reference set owner profile type", error)
                    })
                })
                .transpose()?,
            owner_profile_id: self.owner_profile_id,
            active_revision_id: self.active_revision_id,
            created_at: parse_datetime("reference set created_at", &self.created_at)?,
            updated_at: parse_datetime("reference set updated_at", &self.updated_at)?,
        })
    }
}

#[derive(FromRow)]
struct ReferenceSetItemRow {
    reference_set_id: String,
    asset_id: String,
    ordinal: i64,
    role: Option<String>,
    is_primary: i64,
    created_at: String,
}

impl ReferenceSetItemRow {
    fn try_into_domain(self) -> Result<ReferenceSetItem, RepositoryError> {
        Ok(ReferenceSetItem {
            reference_set_id: self.reference_set_id,
            asset_id: self.asset_id,
            ordinal: self.ordinal,
            role: self.role,
            is_primary: parse_sqlite_bool("reference set item is_primary", self.is_primary)?,
            created_at: parse_datetime("reference set item created_at", &self.created_at)?,
        })
    }
}

#[async_trait]
impl ReferenceSetRepository for SqliteReferenceSetRepository {
    async fn list_reference_sets(
        &self,
        project_id: &str,
        purpose: Option<ReferenceSetPurpose>,
    ) -> Result<Vec<ReferenceSet>, RepositoryError> {
        let rows = if let Some(purpose) = purpose {
            sqlx::query_as::<_, ReferenceSetRow>(&format!(
                "SELECT {REFERENCE_SET_COLUMNS}
                 FROM reference_sets
                 WHERE project_id = ? AND purpose = ?
                 ORDER BY name COLLATE NOCASE ASC, id ASC"
            ))
            .bind(project_id)
            .bind(purpose.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
        } else {
            sqlx::query_as::<_, ReferenceSetRow>(&format!(
                "SELECT {REFERENCE_SET_COLUMNS}
                 FROM reference_sets
                 WHERE project_id = ?
                 ORDER BY name COLLATE NOCASE ASC, id ASC"
            ))
            .bind(project_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
        };

        rows.into_iter()
            .map(ReferenceSetRow::try_into_domain)
            .collect()
    }

    async fn find_reference_set(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<Option<ReferenceSet>, RepositoryError> {
        sqlx::query_as::<_, ReferenceSetRow>(&format!(
            "SELECT {REFERENCE_SET_COLUMNS}
             FROM reference_sets
             WHERE project_id = ? AND id = ?"
        ))
        .bind(project_id)
        .bind(reference_set_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(ReferenceSetRow::try_into_domain)
        .transpose()
    }

    async fn insert_reference_set(
        &self,
        reference_set: &ReferenceSet,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO reference_sets
             (id, project_id, name, purpose, description, owner_profile_type,
              owner_profile_id, active_revision_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&reference_set.id)
        .bind(&reference_set.project_id)
        .bind(&reference_set.name)
        .bind(reference_set.purpose.as_str())
        .bind(&reference_set.description)
        .bind(reference_set.owner_profile_type.map(|value| value.as_str()))
        .bind(&reference_set.owner_profile_id)
        .bind(&reference_set.active_revision_id)
        .bind(format_datetime(reference_set.created_at))
        .bind(format_datetime(reference_set.updated_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn update_reference_set(
        &self,
        reference_set: &ReferenceSet,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE reference_sets
             SET name = ?, purpose = ?, description = ?, owner_profile_type = ?,
                 owner_profile_id = ?, active_revision_id = ?, updated_at = ?
             WHERE project_id = ? AND id = ?",
        )
        .bind(&reference_set.name)
        .bind(reference_set.purpose.as_str())
        .bind(&reference_set.description)
        .bind(reference_set.owner_profile_type.map(|value| value.as_str()))
        .bind(&reference_set.owner_profile_id)
        .bind(&reference_set.active_revision_id)
        .bind(format_datetime(reference_set.updated_at))
        .bind(&reference_set.project_id)
        .bind(&reference_set.id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn delete_reference_set(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM reference_sets WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(reference_set_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if exists == 0 {
            transaction.commit().await.map_err(map_sqlx_error)?;
            return Ok(false);
        }

        ensure_not_referenced(&mut transaction, reference_set_id).await?;

        let result = sqlx::query("DELETE FROM reference_sets WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(reference_set_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn list_items(
        &self,
        reference_set_id: &str,
    ) -> Result<Vec<ReferenceSetItem>, RepositoryError> {
        let rows = sqlx::query_as::<_, ReferenceSetItemRow>(
            "SELECT reference_set_id, asset_id, ordinal, role, is_primary, created_at
             FROM reference_set_items
             WHERE reference_set_id = ?
             ORDER BY ordinal ASC, asset_id ASC",
        )
        .bind(reference_set_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ReferenceSetItemRow::try_into_domain)
            .collect()
    }

    async fn replace_items(
        &self,
        reference_set_id: &str,
        items: &[ReferenceSetItem],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reference_sets WHERE id = ?")
                .bind(reference_set_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
        if exists == 0 {
            return Err(RepositoryError::not_found(
                "reference set",
                reference_set_id,
            ));
        }

        for item in items {
            if item.reference_set_id != reference_set_id {
                return Err(RepositoryError::integrity(
                    "reference set item belongs to a different reference set",
                ));
            }
        }

        sqlx::query("DELETE FROM reference_set_items WHERE reference_set_id = ?")
            .bind(reference_set_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;

        insert_items(&mut transaction, reference_set_id, items).await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }
}

async fn insert_items(
    transaction: &mut Transaction<'_, Sqlite>,
    reference_set_id: &str,
    items: &[ReferenceSetItem],
) -> Result<(), RepositoryError> {
    for item in items {
        sqlx::query(
            "INSERT INTO reference_set_items
             (reference_set_id, asset_id, ordinal, role, is_primary, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(reference_set_id)
        .bind(&item.asset_id)
        .bind(item.ordinal)
        .bind(&item.role)
        .bind(i64::from(item.is_primary))
        .bind(format_datetime(item.created_at))
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }
    Ok(())
}

async fn ensure_not_referenced(
    transaction: &mut Transaction<'_, Sqlite>,
    reference_set_id: &str,
) -> Result<(), RepositoryError> {
    let references = [
        (
            "shot_reference_set_bindings",
            "SELECT COUNT(*) FROM shot_reference_set_bindings WHERE reference_set_id = ?",
        ),
        (
            "costume_variants",
            "SELECT COUNT(*) FROM costume_variants WHERE reference_set_id = ?",
        ),
        (
            "character_profiles",
            "SELECT COUNT(*) FROM character_profiles WHERE default_reference_set_id = ?",
        ),
        (
            "scene_profiles",
            "SELECT COUNT(*) FROM scene_profiles WHERE default_reference_set_id = ?",
        ),
        (
            "prop_profiles",
            "SELECT COUNT(*) FROM prop_profiles WHERE default_reference_set_id = ?",
        ),
    ];

    for (relation, query) in references {
        let count = sqlx::query_scalar::<_, i64>(query)
            .bind(reference_set_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
        if count > 0 {
            return Err(RepositoryError::integrity(format!(
                "reference set {reference_set_id} is still referenced by {relation}"
            )));
        }
    }
    Ok(())
}

fn parse_sqlite_bool(field: &str, value: i64) -> Result<bool, RepositoryError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(RepositoryError::serialization(
            field,
            format!("expected 0 or 1, got {other}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteReferenceSetRepository;
    use crate::application::ports::{ReferenceSetRepository, RepositoryError};
    use crate::domain::consistency::{ReferenceSet, ReferenceSetItem, ReferenceSetPurpose};
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::{TimeZone, Utc};
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, SqliteReferenceSetRepository) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        (
            directory,
            pool.clone(),
            SqliteReferenceSetRepository::new(pool),
        )
    }

    async fn seed_image(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path,
              sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at)
             VALUES (?, 'project-1', 'image', 'source_image', ?, ?, ?, 'sha', 'image/png',
                     2, 2, 1, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(id)
        .bind(id)
        .bind(format!("{id}.png"))
        .bind(format!("C:/{id}.png"))
        .execute(pool)
        .await
        .unwrap();
    }

    fn reference_set(id: &str) -> ReferenceSet {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        ReferenceSet {
            id: id.to_owned(),
            project_id: "project-1".to_owned(),
            name: "References".to_owned(),
            purpose: ReferenceSetPurpose::Character,
            description: String::new(),
            owner_profile_type: None,
            owner_profile_id: None,
            active_revision_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn item(reference_set_id: &str, asset_id: &str, ordinal: i64) -> ReferenceSetItem {
        ReferenceSetItem {
            reference_set_id: reference_set_id.to_owned(),
            asset_id: asset_id.to_owned(),
            ordinal,
            role: None,
            is_primary: ordinal == 0,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        }
    }

    #[tokio::test]
    async fn replace_items_rolls_back_when_asset_fk_fails() {
        let (_directory, pool, repository) = setup().await;
        seed_image(&pool, "ast-existing").await;
        repository
            .insert_reference_set(&reference_set("rs-test"))
            .await
            .unwrap();
        repository
            .replace_items("rs-test", &[item("rs-test", "ast-existing", 0)])
            .await
            .unwrap();

        let error = repository
            .replace_items(
                "rs-test",
                &[
                    item("rs-test", "ast-existing", 0),
                    item("rs-test", "ast-missing", 1),
                ],
            )
            .await
            .expect_err("missing asset must fail the transaction");
        assert!(matches!(error, RepositoryError::Integrity { .. }));

        let items = repository.list_items("rs-test").await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].asset_id, "ast-existing");
        assert_eq!(items[0].ordinal, 0);
    }
}
