use crate::application::ports::DatabaseHealthProbe;
use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteDatabaseHealthProbe {
    pool: SqlitePool,
}

impl SqliteDatabaseHealthProbe {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DatabaseHealthProbe for SqliteDatabaseHealthProbe {
    async fn is_healthy(&self) -> bool {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .is_ok()
    }
}
