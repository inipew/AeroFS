use backend::db::{checkpoint_db, connect_db, init_db, migrate_db, DbPool};
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};
use tokio::sync::OnceCell;

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

struct SeededTemplate {
    _temp: TempDir,
    path: PathBuf,
}

// Each integration-test crate gets its own process and therefore its own template.
// Build it through the real production initializer once, then clone the closed,
// checkpointed SQLite file for hermetic fixtures. This preserves the exact
// migrations/default seed while avoiding repeated Argon2 hashing per TestApp.
static SEEDED_TEMPLATE: OnceCell<SeededTemplate> = OnceCell::const_new();

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
    ///
    /// The production initializer runs once per integration-test process. Each
    /// caller receives an independent copy of that closed seed database, so test
    /// mutations cannot leak between fixtures.
    pub async fn seeded(filename: &str) -> Self {
        let template = seeded_template().await;
        let temp = tempdir().expect("create seeded test database tempdir");
        let path = temp.path().join(filename);
        std::fs::copy(&template.path, &path).expect("clone seeded test database template");
        let url = sqlite_url(&path);
        let pool = connect_db(&url)
            .await
            .expect("connect cloned seeded test database");

        Self {
            pool,
            temp,
            path,
            url,
        }
    }
}

async fn seeded_template() -> &'static SeededTemplate {
    SEEDED_TEMPLATE
        .get_or_init(|| async {
            let temp = tempdir().expect("create seeded database template tempdir");
            let path = temp.path().join("seeded-template.db");
            let url = sqlite_url(&path);
            let pool = init_db(&url)
                .await
                .expect("initialize seeded database template");

            checkpoint_db(&pool)
                .await
                .expect("checkpoint seeded database template");
            pool.close().await;

            SeededTemplate { _temp: temp, path }
        })
        .await
}

fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", path.to_string_lossy())
}
