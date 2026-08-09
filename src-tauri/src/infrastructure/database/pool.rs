use crate::error::AppError;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{path::Path, time::Duration};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn initialize(database_path: &Path) -> Result<SqlitePool, AppError> {
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|_| AppError::database("failed to connect to SQLite"))?;

    configure_pragmas(&pool).await?;
    tracing::info!("database connected");

    MIGRATOR
        .run(&pool)
        .await
        .map_err(|_| AppError::database("database migration failed"))?;
    tracing::info!("database migration completed");

    Ok(pool)
}

async fn configure_pragmas(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await
        .map_err(|_| AppError::database("failed to enable SQLite foreign keys"))?;

    sqlx::query("PRAGMA journal_mode = WAL")
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::database("failed to enable SQLite WAL mode"))?;

    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(pool)
        .await
        .map_err(|_| AppError::database("failed to set SQLite busy timeout"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::initialize;
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    async fn table_count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN \
             ('projects', 'workflows', 'workflow_versions', 'recipes', 'tasks', 'assets', \
              'generation_snapshots', 'task_events', 'presets', 'task_output_assets', \
               'production_batches', 'production_batch_items')",
        )
        .fetch_one(pool)
        .await
        .expect("schema query should succeed")
    }

    #[tokio::test]
    async fn migration_runs_against_temporary_sqlite() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let database_path = temporary_directory.path().join("app.db");

        let pool = initialize(&database_path)
            .await
            .expect("migration should succeed");

        assert_eq!(table_count(&pool).await, 12);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
                .fetch_one(&pool)
                .await
                .expect("foreign keys pragma should be readable"),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
                .fetch_one(&pool)
                .await
                .expect("journal mode pragma should be readable")
                .to_ascii_lowercase(),
            "wal"
        );

        pool.close().await;
        assert!(database_path.is_file());
    }

    #[tokio::test]
    async fn repeated_migration_is_safe() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let database_path = temporary_directory.path().join("app.db");

        let first_pool = initialize(&database_path)
            .await
            .expect("first migration should succeed");
        first_pool.close().await;

        let second_pool = initialize(&database_path)
            .await
            .expect("second migration should succeed");
        assert_eq!(table_count(&second_pool).await, 12);
        second_pool.close().await;
    }
}
