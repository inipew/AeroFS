use crate::auth::password::hash_password;
use chrono::Utc;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use uuid::Uuid;

pub type DbPool = Pool<Sqlite>;

pub async fn init_db(database_url: &str) -> anyhow::Result<DbPool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Seed default admin user if no users exist
    seed_default_admin(&pool).await?;

    // Seed default local connection if no connections exist
    seed_default_connection(&pool).await?;

    Ok(pool)
}

async fn seed_default_admin(pool: &DbPool) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count.0 == 0 {
        let user_id = Uuid::new_v4().to_string();
        let username = "admin";
        let default_password = std::env::var("AEROFS_ADMIN_PASSWORD")
            .unwrap_or_else(|_| "admin12345".to_string());
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

        tracing::info!("Initialized default admin user: 'admin'");
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
