use crate::auth::password::hash_password;
use chrono::Utc;
use serde::Serialize;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Pool, Row, Sqlite,
};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

pub type DbPool = Pool<Sqlite>;

#[derive(Debug, Clone, Serialize)]
pub struct DbStats {
    #[serde(skip)]
    pub database_url: String,
    pub sanitized_url: String,
    pub users_count: i64,
    pub connections_count: i64,
    pub transfer_jobs_count: i64,
    pub journal_mode: String,
    pub foreign_keys: bool,
    pub busy_timeout_ms: u64,
    pub page_count: i64,
    pub page_size: i64,
    pub total_size_bytes: i64,
}

/// Connect to SQLite without running migrations or seeding defaults.
/// Safe for read-only CLI commands and diagnostics.
pub async fn connect_db(database_url: &str) -> anyhow::Result<DbPool> {
    let connect_options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_millis(5000));

    let max_conns = std::env::var("AEROFS_DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(if cfg!(target_os = "android") { 4 } else { 8 });

    let pool = SqlitePoolOptions::new()
        .max_connections(max_conns)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(connect_options)
        .await?;

    Ok(pool)
}

/// Run pending migrations on the database and return applied migration names
pub async fn migrate_db(pool: &DbPool) -> anyhow::Result<Vec<String>> {
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(pool).await?;

    // Query applied migrations from _sqlx_migrations table
    let rows = sqlx::query("SELECT version, description, installed_on FROM _sqlx_migrations ORDER BY version ASC")
        .fetch_all(pool)
        .await;

    let mut applied = Vec::new();
    if let Ok(rows) = rows {
        for r in rows {
            let ver: i64 = r.get("version");
            let desc: String = r.get("description");
            applied.push(format!("{:04}_{}", ver, desc));
        }
    }

    Ok(applied)
}

/// Seed default administrative user and local filesystem connection if table is empty
pub async fn seed_defaults(pool: &DbPool) -> anyhow::Result<()> {
    seed_default_admin(pool).await?;
    seed_default_connection(pool).await?;
    Ok(())
}

/// Complete initialization: connect + migrate + seed (used by `serve` daemon)
pub async fn init_db(database_url: &str) -> anyhow::Result<DbPool> {
    let pool = connect_db(database_url).await?;
    migrate_db(&pool).await?;
    seed_defaults(&pool).await?;
    Ok(pool)
}

/// Run PRAGMA integrity_check and foreign_key_check on SQLite
pub async fn check_integrity(pool: &DbPool) -> anyhow::Result<Vec<String>> {
    let mut reports = Vec::new();

    let rows = sqlx::query("PRAGMA integrity_check;")
        .fetch_all(pool)
        .await?;

    for row in rows {
        let result: String = row.get(0);
        reports.push(format!("integrity_check: {}", result));
    }

    let fk_rows = sqlx::query("PRAGMA foreign_key_check;")
        .fetch_all(pool)
        .await?;

    if fk_rows.is_empty() {
        reports.push("foreign_key_check: ok".to_string());
    } else {
        for row in fk_rows {
            let table: String = row.get(0);
            reports.push(format!("foreign_key_violation in table: {}", table));
        }
    }

    Ok(reports)
}

/// Run SQLite VACUUM to reclaim space and defragment database file
pub async fn vacuum_db(pool: &DbPool) -> anyhow::Result<()> {
    sqlx::query("VACUUM;").execute(pool).await?;
    Ok(())
}

/// Run SQLite WAL checkpoint to truncate journal and flush data to DB file
pub async fn checkpoint_db(pool: &DbPool) -> anyhow::Result<()> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE);")
        .execute(pool)
        .await?;
    Ok(())
}

/// Create a consistent online backup snapshot using SQLite VACUUM INTO
pub async fn backup_db(pool: &DbPool, target_path: &Path, force: bool) -> anyhow::Result<()> {
    // Check if target file already exists and force flag is not set
    if target_path.exists() {
        if !force {
            anyhow::bail!(
                "Target backup file '{}' already exists. Use --force to overwrite.",
                target_path.display()
            );
        }
        // Remove existing file if force is true, because VACUUM INTO requires target to not pre-exist in SQLite
        std::fs::remove_file(target_path)?;
    }

    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let target_str = target_path.to_string_lossy();
    sqlx::query(&format!(
        "VACUUM INTO '{}';",
        target_str.replace('\'', "''")
    ))
    .execute(pool)
    .await?;
    Ok(())
}

/// Gather database runtime and table statistics with sanitized database URL
pub async fn get_db_stats(pool: &DbPool, database_url: &str) -> anyhow::Result<DbStats> {
    let count_users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let count_connections: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM connections")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let count_transfers: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transfer_jobs")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let journal_mode_row: (String,) = sqlx::query_as("PRAGMA journal_mode;")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| ("wal".into(),));

    let foreign_keys_row: (i64,) = sqlx::query_as("PRAGMA foreign_keys;")
        .fetch_one(pool)
        .await
        .unwrap_or((1,));

    let page_count_row: (i64,) = sqlx::query_as("PRAGMA page_count;")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let page_size_row: (i64,) = sqlx::query_as("PRAGMA page_size;")
        .fetch_one(pool)
        .await
        .unwrap_or((4096,));

    let total_size_bytes = page_count_row.0 * page_size_row.0;
    let sanitized_url = sanitize_db_url(database_url);

    Ok(DbStats {
        database_url: database_url.to_string(),
        sanitized_url,
        users_count: count_users.0,
        connections_count: count_connections.0,
        transfer_jobs_count: count_transfers.0,
        journal_mode: journal_mode_row.0.to_uppercase(),
        foreign_keys: foreign_keys_row.0 == 1,
        busy_timeout_ms: 5000,
        page_count: page_count_row.0,
        page_size: page_size_row.0,
        total_size_bytes,
    })
}

/// Redact credentials or internal token from database URL string
pub fn sanitize_db_url(url: &str) -> String {
    if let Some(at_idx) = url.find('@') {
        if let Some(proto_idx) = url.find("://") {
            let prefix = &url[..proto_idx + 3];
            let rest = &url[at_idx + 1..];
            return format!("{}***:***@{}", prefix, rest);
        }
    }
    url.to_string()
}

async fn seed_default_admin(pool: &DbPool) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count.0 == 0 {
        let user_id = Uuid::new_v4().to_string();
        let username = "admin";
        let default_password = match std::env::var("AEROFS_ADMIN_PASSWORD") {
            Ok(p) => p,
            Err(_) => {
                if std::env::var("AEROFS_ENV").unwrap_or_else(|_| "development".into())
                    == "development"
                    || cfg!(test)
                {
                    "admin12345".to_string()
                } else {
                    let random_pass = format!(
                        "aerofs_{}",
                        &Uuid::new_v4().to_string().replace('-', "")[..16]
                    );
                    eprintln!("============================================================");
                    eprintln!(" AeroFS First Boot Admin Password Generated:");
                    eprintln!(" Username: admin");
                    eprintln!(" Password: {}", random_pass);
                    eprintln!("============================================================");
                    tracing::warn!(
                        "Generated random bootstrap admin credentials: {}",
                        random_pass
                    );
                    random_pass
                }
            }
        };
        let password_hash = hash_password(&default_password)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at) VALUES (?, ?, ?, 1, ?, ?)"
        )
        .bind(&user_id)
        .bind(username)
        .bind(&password_hash)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        tracing::info!("Initialized admin user: 'admin'");
    }

    Ok(())
}

async fn seed_default_connection(pool: &DbPool) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM connections")
        .fetch_one(pool)
        .await?;

    if count.0 == 0 {
        let conn_id = "local";
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO connections (id, name, provider, base_path, read_only, enabled, created_at, updated_at)
             VALUES (?, 'Local Storage', 'local', '/', 0, 1, ?, ?)"
        )
        .bind(conn_id)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        tracing::info!("Initialized default Local connection in database");
    }

    Ok(())
}
