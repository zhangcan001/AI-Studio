//! Own a real per-case migrated SQLite database and its temporary directory.
//! Domain fixtures still seed their own data; no global pool or cached database.
use ai_studio_lib::infrastructure::database::initialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

pub struct ProjectDatabase {
    pub pool: SqlitePool,
    directory: TempDir,
    database_name: &'static str,
}

impl ProjectDatabase {
    pub async fn new(database_name: &'static str) -> Self {
        assert_eq!(Path::new(database_name).file_name().unwrap(), database_name);
        let directory = tempdir().expect("isolated project database directory");
        let pool = initialize(&directory.path().join(database_name))
            .await
            .expect("real migrations must succeed");
        Self {
            pool,
            directory,
            database_name,
        }
    }

    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    #[allow(dead_code)] // Explicit teardown/reopen is used by fixture self-tests.
    pub fn database_path(&self) -> PathBuf {
        self.path().join(self.database_name)
    }

    #[allow(dead_code)]
    pub async fn close(self) {
        self.pool.close().await;
        // Directory removal happens only after the real SQLite connections close.
        drop(self);
    }
}
