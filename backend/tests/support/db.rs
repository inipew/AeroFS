use backend::db::{connect_db, init_db, migrate_db, DbPool};
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

/// Isolated SQLite database owned by one integration-test fixture.
///
/// File-backed databases are intentional: SQLite `:memory:` databases are scoped
/// to a connection and become surprising as soon as a pool opens more than one
/// connection. The fixture therefore mirrors production's pooled SQLite shape
/// while keeping every test hermetic.
pub struct TestDatabase {
    pub pool: DbPool,
    pub temp: TempDir,
    pub path: PathBuf,
    pub url: String,
}

impl TestDatabase {
    /// Create a database with the real migration set, but without default seed data.
    pub async fn migrated(filename: &str) -> Self {
        let temp = tempdir().expect("create test database tempdir");
        let path = temp.path().join(filename);
        let url = sqlite_url(&path);
        let pool = connect_db(&url).await.expect("connect test database");
        migrate_db(&pool).await.expect("migrate test database");

        Self {
            pool,
            temp,
            path,
            url,
        }
    }

    /// Create a database exactly as the application does: migrations plus defaults.
    pub async fn seeded(filename: &str) -> Self {
        let temp = tempdir().expect("create test database tempdir");
        let path = temp.path().join(filename);
        let url = sqlite_url(&path);
        let pool = init_db(&url).await.expect("initialize test database");

        Self {
            pool,
            temp,
            path,
            url,
        }
    }
}

fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", path.to_string_lossy())
}
