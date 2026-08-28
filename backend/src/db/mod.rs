use crate::auth::password::hash_password;
use chrono::Utc;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Pool, Row, Sqlite,
};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

pub type DbPool = Pool<Sqlite>;

pub async fn init_db(database_url: &str) -> anyhow::Result<DbPool> {
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

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Seed default admin user if no users exist
    seed_default_admin(&pool).await?;

    // Seed default local connection if no connections exist
    seed_default_connection(&pool).await?;

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

/// Create a consistent online backup snapshot using SQLite VACUUM INTO
pub async fn backup_db(pool: &DbPool, target_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
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
