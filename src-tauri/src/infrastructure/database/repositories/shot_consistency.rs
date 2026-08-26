use super::{format_datetime, map_domain_error, map_sqlx_error, parse_datetime};
use crate::application::ports::{RepositoryError, ShotConsistencyRepository};
use crate::domain::consistency::{
    BindingRole, InheritanceMode, ProfileType, ShotProfileBinding, ShotReferenceSetBinding,
};
use async_trait::async_trait;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone)]
pub struct SqliteShotConsistencyRepository {
    pool: SqlitePool,
}

impl SqliteShotConsistencyRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ShotProfileBindingRow {
    id: String,
    shot_id: String,
    role: String,
    profile_type: String,
    profile_id: String,
    costume_variant_id: Option<String>,
    ordinal: i64,
    inheritance_mode: String,
    created_at: String,
    updated_at: String,
}

impl ShotProfileBindingRow {
    fn try_into_domain(self) -> Result<ShotProfileBinding, RepositoryError> {
        Ok(ShotProfileBinding {
            id: self.id,
            shot_id: self.shot_id,
            role: BindingRole::try_from_db(&self.role)
                .map_err(|error| map_domain_error("shot profile binding role", error))?,
            profile_type: ProfileType::try_from_db(&self.profile_type)
                .map_err(|error| map_domain_error("shot profile binding profile type", error))?,
            profile_id: self.profile_id,
            costume_variant_id: self.costume_variant_id,
            ordinal: self.ordinal,
            inheritance_mode: InheritanceMode::try_from_db(&self.inheritance_mode).map_err(
                |error| map_domain_error("shot profile binding inheritance mode", error),
            )?,
            created_at: parse_datetime("shot profile binding created_at", &self.created_at)?,
            updated_at: parse_datetime("shot profile binding updated_at", &self.updated_at)?,
        })
    }
}

#[derive(FromRow)]
struct ShotReferenceSetBindingRow {
    id: String,
    shot_id: String,
    role: String,
    reference_set_id: String,
    ordinal: i64,
    required: i64,
    inheritance_mode: String,
    created_at: String,
    updated_at: String,
}

impl ShotReferenceSetBindingRow {
    fn try_into_domain(self) -> Result<ShotReferenceSetBinding, RepositoryError> {
        Ok(ShotReferenceSetBinding {
            id: self.id,
            shot_id: self.shot_id,
            role: BindingRole::try_from_db(&self.role)
                .map_err(|error| map_domain_error("shot reference-set binding role", error))?,
            reference_set_id: self.reference_set_id,
            ordinal: self.ordinal,
            required: parse_sqlite_bool("shot reference-set binding required", self.required)?,
            inheritance_mode: InheritanceMode::try_from_db(&self.inheritance_mode).map_err(
                |error| map_domain_error("shot reference-set binding inheritance mode", error),
            )?,
            created_at: parse_datetime("shot reference-set binding created_at", &self.created_at)?,
            updated_at: parse_datetime("shot reference-set binding updated_at", &self.updated_at)?,
        })
    }
}

#[async_trait]
impl ShotConsistencyRepository for SqliteShotConsistencyRepository {
    async fn list_profile_bindings(
        &self,
        shot_id: &str,
    ) -> Result<Vec<ShotProfileBinding>, RepositoryError> {
        let rows = sqlx::query_as::<_, ShotProfileBindingRow>(
            "SELECT id, shot_id, role, profile_type, profile_id, costume_variant_id,
                    ordinal, inheritance_mode, created_at, updated_at
             FROM shot_profile_bindings
             WHERE shot_id = ?
             ORDER BY role ASC, ordinal ASC, id ASC",
        )
        .bind(shot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ShotProfileBindingRow::try_into_domain)
            .collect()
    }

    async fn replace_profile_bindings(
        &self,
        shot_id: &str,
        bindings: &[ShotProfileBinding],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_shot_exists(&mut transaction, shot_id).await?;
        for binding in bindings {
            if binding.shot_id != shot_id {
                return Err(RepositoryError::integrity(
                    "shot profile binding belongs to a different shot",
                ));
            }
        }

        sqlx::query("DELETE FROM shot_profile_bindings WHERE shot_id = ?")
            .bind(shot_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;

        for binding in bindings {
            sqlx::query(
                "INSERT INTO shot_profile_bindings
                 (id, shot_id, role, profile_type, profile_id, costume_variant_id,
                  ordinal, inheritance_mode, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&binding.id)
            .bind(shot_id)
            .bind(binding.role.as_str())
            .bind(binding.profile_type.as_str())
            .bind(&binding.profile_id)
            .bind(&binding.costume_variant_id)
            .bind(binding.ordinal)
            .bind(binding.inheritance_mode.as_str())
            .bind(format_datetime(binding.created_at))
            .bind(format_datetime(binding.updated_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn list_reference_set_bindings(
        &self,
        shot_id: &str,
    ) -> Result<Vec<ShotReferenceSetBinding>, RepositoryError> {
        let rows = sqlx::query_as::<_, ShotReferenceSetBindingRow>(
            "SELECT id, shot_id, role, reference_set_id, ordinal, required,
                    inheritance_mode, created_at, updated_at
             FROM shot_reference_set_bindings
             WHERE shot_id = ?
             ORDER BY role ASC, ordinal ASC, id ASC",
        )
        .bind(shot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ShotReferenceSetBindingRow::try_into_domain)
            .collect()
    }

    async fn replace_reference_set_bindings(
        &self,
        shot_id: &str,
        bindings: &[ShotReferenceSetBinding],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        ensure_shot_exists(&mut transaction, shot_id).await?;
        for binding in bindings {
            if binding.shot_id != shot_id {
                return Err(RepositoryError::integrity(
                    "shot reference-set binding belongs to a different shot",
                ));
            }
        }

        sqlx::query("DELETE FROM shot_reference_set_bindings WHERE shot_id = ?")
            .bind(shot_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;

        for binding in bindings {
            sqlx::query(
                "INSERT INTO shot_reference_set_bindings
                 (id, shot_id, role, reference_set_id, ordinal, required,
                  inheritance_mode, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&binding.id)
            .bind(shot_id)
            .bind(binding.role.as_str())
            .bind(&binding.reference_set_id)
            .bind(binding.ordinal)
            .bind(i64::from(binding.required))
            .bind(binding.inheritance_mode.as_str())
            .bind(format_datetime(binding.created_at))
            .bind(format_datetime(binding.updated_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)
    }
}

async fn ensure_shot_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    shot_id: &str,
) -> Result<(), RepositoryError> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE id = ?")
        .bind(shot_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    if exists == 0 {
        return Err(RepositoryError::not_found("shot", shot_id));
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
    use super::SqliteShotConsistencyRepository;
    use crate::application::ports::{
        ReferenceSetRepository, RepositoryError, ShotConsistencyRepository, ShotRecord,
        ShotRepository,
    };
    use crate::domain::consistency::{
        BindingRole, InheritanceMode, ProfileType, ReferenceSet, ReferenceSetPurpose,
        ShotReferenceSetBinding,
    };
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn reference_set_binding_replace_rolls_back_on_fk_failure() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let shot_repository =
            crate::infrastructure::database::repositories::SqliteShotRepository::new(pool.clone());
        shot_repository
            .insert(&ShotRecord {
                id: "sht-consistency".to_owned(),
                project_id: "project-1".to_owned(),
                ordinal: 0,
                name: "Shot".to_owned(),
                prompt_text: String::new(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: None,
                selected_video_asset_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await
            .unwrap();

        let reference_repository =
            crate::infrastructure::database::repositories::SqliteReferenceSetRepository::new(
                pool.clone(),
            );
        let now = Utc::now();
        reference_repository
            .insert_reference_set(&ReferenceSet {
                id: "rs-consistency".to_owned(),
                project_id: "project-1".to_owned(),
                name: "References".to_owned(),
                purpose: ReferenceSetPurpose::Shot,
                description: String::new(),
                owner_profile_type: None,
                owner_profile_id: None,
                active_revision_id: None,
                created_at: now,
                updated_at: now,
            })
            .await
            .unwrap();

        let repository = SqliteShotConsistencyRepository::new(pool);
        let original = ShotReferenceSetBinding {
            id: "srb-original".to_owned(),
            shot_id: "sht-consistency".to_owned(),
            role: BindingRole::ShotReference,
            reference_set_id: "rs-consistency".to_owned(),
            ordinal: 0,
            required: true,
            inheritance_mode: InheritanceMode::Explicit,
            created_at: now,
            updated_at: now,
        };
        repository
            .replace_reference_set_bindings("sht-consistency", &[original.clone()])
            .await
            .unwrap();

        let replacement = ShotReferenceSetBinding {
            id: "srb-invalid".to_owned(),
            reference_set_id: "rs-missing".to_owned(),
            ..original.clone()
        };
        let error = repository
            .replace_reference_set_bindings("sht-consistency", &[replacement])
            .await
            .expect_err("missing reference set must fail");
        assert!(matches!(error, RepositoryError::Integrity { .. }));

        assert_eq!(
            repository
                .list_reference_set_bindings("sht-consistency")
                .await
                .unwrap(),
            vec![original]
        );
    }

    #[allow(dead_code)]
    fn _profile_type_contract_is_linked() {
        let _ = ProfileType::Character;
    }
}
