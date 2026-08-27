use super::{format_datetime, map_domain_error, map_sqlx_error, parse_datetime};
use crate::application::ports::{ConsistencyScopeRepository, RepositoryError};
use crate::domain::consistency::{
    BindingRole, ConsistencyScopeType, InheritanceMode, ProfileType, ScopedProfileBinding,
    ScopedReferenceSetBinding,
};
use async_trait::async_trait;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[derive(Clone)]
pub struct SqliteConsistencyScopeRepository {
    pool: SqlitePool,
}

impl SqliteConsistencyScopeRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ScopedProfileBindingRow {
    id: String,
    project_id: String,
    scope_type: String,
    scope_id: String,
    role: String,
    profile_type: String,
    profile_id: String,
    costume_variant_id: Option<String>,
    ordinal: i64,
    inheritance_mode: String,
    created_at: String,
    updated_at: String,
}

impl ScopedProfileBindingRow {
    fn into_domain(self) -> Result<ScopedProfileBinding, RepositoryError> {
        Ok(ScopedProfileBinding {
            id: self.id,
            project_id: self.project_id,
            scope_type: ConsistencyScopeType::try_from_db(&self.scope_type)
                .map_err(|error| map_domain_error("scope profile scope_type", error))?,
            scope_id: self.scope_id,
            role: BindingRole::try_from_db(&self.role)
                .map_err(|error| map_domain_error("scope profile role", error))?,
            profile_type: ProfileType::try_from_db(&self.profile_type)
                .map_err(|error| map_domain_error("scope profile profile_type", error))?,
            profile_id: self.profile_id,
            costume_variant_id: self.costume_variant_id,
            ordinal: self.ordinal,
            inheritance_mode: InheritanceMode::try_from_db(&self.inheritance_mode)
                .map_err(|error| map_domain_error("scope profile inheritance_mode", error))?,
            created_at: parse_datetime("scope profile created_at", &self.created_at)?,
            updated_at: parse_datetime("scope profile updated_at", &self.updated_at)?,
        })
    }
}

#[derive(FromRow)]
struct ScopedReferenceSetBindingRow {
    id: String,
    project_id: String,
    scope_type: String,
    scope_id: String,
    role: String,
    reference_set_id: String,
    ordinal: i64,
    required: i64,
    inheritance_mode: String,
    created_at: String,
    updated_at: String,
}

impl ScopedReferenceSetBindingRow {
    fn into_domain(self) -> Result<ScopedReferenceSetBinding, RepositoryError> {
        Ok(ScopedReferenceSetBinding {
            id: self.id,
            project_id: self.project_id,
            scope_type: ConsistencyScopeType::try_from_db(&self.scope_type)
                .map_err(|error| map_domain_error("scope reference scope_type", error))?,
            scope_id: self.scope_id,
            role: BindingRole::try_from_db(&self.role)
                .map_err(|error| map_domain_error("scope reference role", error))?,
            reference_set_id: self.reference_set_id,
            ordinal: self.ordinal,
            required: parse_sqlite_bool("scope reference required", self.required)?,
            inheritance_mode: InheritanceMode::try_from_db(&self.inheritance_mode)
                .map_err(|error| map_domain_error("scope reference inheritance_mode", error))?,
            created_at: parse_datetime("scope reference created_at", &self.created_at)?,
            updated_at: parse_datetime("scope reference updated_at", &self.updated_at)?,
        })
    }
}

#[async_trait]
impl ConsistencyScopeRepository for SqliteConsistencyScopeRepository {
    async fn list_profile_bindings_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ScopedProfileBinding>, RepositoryError> {
        let rows = sqlx::query_as::<_, ScopedProfileBindingRow>(
            "SELECT id, project_id, scope_type, scope_id, role, profile_type, profile_id,
                    costume_variant_id, ordinal, inheritance_mode, created_at, updated_at
             FROM consistency_scope_profile_bindings
             WHERE project_id = ?
             ORDER BY CASE scope_type
                        WHEN 'PROJECT' THEN 0
                        WHEN 'SERIES' THEN 1
                        WHEN 'EPISODE' THEN 2
                        WHEN 'SCENE' THEN 3
                      END,
                      scope_id ASC, role ASC, ordinal ASC, profile_id ASC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ScopedProfileBindingRow::into_domain)
            .collect()
    }

    async fn list_reference_set_bindings_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ScopedReferenceSetBinding>, RepositoryError> {
        let rows = sqlx::query_as::<_, ScopedReferenceSetBindingRow>(
            "SELECT id, project_id, scope_type, scope_id, role, reference_set_id,
                    ordinal, required, inheritance_mode, created_at, updated_at
             FROM consistency_scope_reference_set_bindings
             WHERE project_id = ?
             ORDER BY CASE scope_type
                        WHEN 'PROJECT' THEN 0
                        WHEN 'SERIES' THEN 1
                        WHEN 'EPISODE' THEN 2
                        WHEN 'SCENE' THEN 3
                      END,
                      scope_id ASC, role ASC, ordinal ASC, reference_set_id ASC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ScopedReferenceSetBindingRow::into_domain)
            .collect()
    }

    async fn replace_profile_bindings(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        bindings: &[ScopedProfileBinding],
    ) -> Result<(), RepositoryError> {
        validate_profile_binding_scope(project_id, scope_type, scope_id, bindings)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "DELETE FROM consistency_scope_profile_bindings
             WHERE project_id = ? AND scope_type = ? AND scope_id = ?",
        )
        .bind(project_id)
        .bind(scope_type.as_str())
        .bind(scope_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        insert_profile_bindings(&mut transaction, bindings).await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn replace_reference_set_bindings(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        bindings: &[ScopedReferenceSetBinding],
    ) -> Result<(), RepositoryError> {
        validate_reference_binding_scope(project_id, scope_type, scope_id, bindings)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "DELETE FROM consistency_scope_reference_set_bindings
             WHERE project_id = ? AND scope_type = ? AND scope_id = ?",
        )
        .bind(project_id)
        .bind(scope_type.as_str())
        .bind(scope_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        insert_reference_bindings(&mut transaction, bindings).await?;
        transaction.commit().await.map_err(map_sqlx_error)
    }

    async fn replace_binding_pack(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        profile_bindings: &[ScopedProfileBinding],
        reference_set_bindings: &[ScopedReferenceSetBinding],
    ) -> Result<(), RepositoryError> {
        validate_profile_binding_scope(project_id, scope_type, scope_id, profile_bindings)?;
        validate_reference_binding_scope(project_id, scope_type, scope_id, reference_set_bindings)?;

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "DELETE FROM consistency_scope_profile_bindings
             WHERE project_id = ? AND scope_type = ? AND scope_id = ?",
        )
        .bind(project_id)
        .bind(scope_type.as_str())
        .bind(scope_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        insert_profile_bindings(&mut transaction, profile_bindings).await?;

        sqlx::query(
            "DELETE FROM consistency_scope_reference_set_bindings
             WHERE project_id = ? AND scope_type = ? AND scope_id = ?",
        )
        .bind(project_id)
        .bind(scope_type.as_str())
        .bind(scope_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        insert_reference_bindings(&mut transaction, reference_set_bindings).await?;

        transaction.commit().await.map_err(map_sqlx_error)
    }
}

fn validate_profile_binding_scope(
    project_id: &str,
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    bindings: &[ScopedProfileBinding],
) -> Result<(), RepositoryError> {
    for binding in bindings {
        if binding.project_id != project_id
            || binding.scope_type != scope_type
            || binding.scope_id != scope_id
        {
            return Err(RepositoryError::integrity(
                "scope profile binding does not match replacement scope",
            ));
        }
        if binding.role == BindingRole::ShotReference {
            return Err(RepositoryError::integrity(
                "SHOT_REFERENCE is not valid for a profile binding",
            ));
        }
    }
    Ok(())
}

fn validate_reference_binding_scope(
    project_id: &str,
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    bindings: &[ScopedReferenceSetBinding],
) -> Result<(), RepositoryError> {
    for binding in bindings {
        if binding.project_id != project_id
            || binding.scope_type != scope_type
            || binding.scope_id != scope_id
        {
            return Err(RepositoryError::integrity(
                "scope reference-set binding does not match replacement scope",
            ));
        }
    }
    Ok(())
}

async fn insert_profile_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    bindings: &[ScopedProfileBinding],
) -> Result<(), RepositoryError> {
    for binding in bindings {
        sqlx::query(
            "INSERT INTO consistency_scope_profile_bindings
             (id, project_id, scope_type, scope_id, role, profile_type, profile_id,
              costume_variant_id, ordinal, inheritance_mode, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&binding.id)
        .bind(&binding.project_id)
        .bind(binding.scope_type.as_str())
        .bind(&binding.scope_id)
        .bind(binding.role.as_str())
        .bind(binding.profile_type.as_str())
        .bind(&binding.profile_id)
        .bind(&binding.costume_variant_id)
        .bind(binding.ordinal)
        .bind(binding.inheritance_mode.as_str())
        .bind(format_datetime(binding.created_at))
        .bind(format_datetime(binding.updated_at))
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }
    Ok(())
}

async fn insert_reference_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    bindings: &[ScopedReferenceSetBinding],
) -> Result<(), RepositoryError> {
    for binding in bindings {
        sqlx::query(
            "INSERT INTO consistency_scope_reference_set_bindings
             (id, project_id, scope_type, scope_id, role, reference_set_id, ordinal,
              required, inheritance_mode, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&binding.id)
        .bind(&binding.project_id)
        .bind(binding.scope_type.as_str())
        .bind(&binding.scope_id)
        .bind(binding.role.as_str())
        .bind(&binding.reference_set_id)
        .bind(binding.ordinal)
        .bind(i64::from(binding.required))
        .bind(binding.inheritance_mode.as_str())
        .bind(format_datetime(binding.created_at))
        .bind(format_datetime(binding.updated_at))
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
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
