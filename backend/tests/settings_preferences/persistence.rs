use backend::{
    domain::settings::UserPreferences,
    infrastructure::{
        audit::SqliteUserPreferencesRepository,
        settings::SqliteSystemSettingsStore,
    },
    ports::{audit::UserPreferencesRepository, settings::SystemSettingsStore},
    services::PreferencesService,
};
use chrono::Utc;
use std::sync::Arc;

use crate::support::TestDatabase;

#[tokio::test]
async fn system_settings_batch_is_atomic_when_one_write_fails() {
    let database = TestDatabase::migrated("settings_atomicity.db").await;
    sqlx::query(
        "CREATE TRIGGER reject_explode BEFORE INSERT ON system_settings \
         WHEN NEW.key = 'explode' BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
    )
    .execute(&database.pool)
    .await
    .unwrap();

    let store = SqliteSystemSettingsStore::new(database.pool.clone());
    let result = store
        .upsert_many(&[
            ("first".to_string(), "committed-too-early".to_string()),
            ("explode".to_string(), "boom".to_string()),
        ])
        .await;
    assert!(result.is_err());

    let first: Option<String> =
        sqlx::query_scalar("SELECT value FROM system_settings WHERE key = 'first'")
            .fetch_optional(&database.pool)
            .await
            .unwrap();
    assert_eq!(first, None, "failed settings batches must roll back fully");
}

#[tokio::test]
async fn system_settings_upsert_replaces_existing_value() {
    let database = TestDatabase::migrated("settings_upsert.db").await;
    let store = SqliteSystemSettingsStore::new(database.pool.clone());

    store
        .upsert_many(&[("theme".to_string(), "dark".to_string())])
        .await
        .unwrap();
    store
        .upsert_many(&[("theme".to_string(), "light".to_string())])
        .await
        .unwrap();

    assert_eq!(store.get("theme").await.unwrap().as_deref(), Some("light"));
}

#[tokio::test]
async fn user_preferences_are_isolated_and_overwritable_per_user() {
    let database = TestDatabase::seeded("preferences_roundtrip.db").await;
    let admin_id: String = sqlx::query_scalar("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    let second_id = "settings-pref-user";
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at) \
         VALUES (?, 'prefs-user', 'fixture-hash', 0, ?, ?)",
    )
    .bind(second_id)
    .bind(&now)
    .bind(&now)
    .execute(&database.pool)
    .await
    .unwrap();

    let repository = SqliteUserPreferencesRepository::new(database.pool.clone());
    let service = PreferencesService::new(Arc::new(repository.clone()));
    assert_eq!(
        service.get_user_preferences(&admin_id).await.unwrap(),
        UserPreferences::default()
    );

    let first = UserPreferences {
        theme: "dracula".to_string(),
        list_density: "compact".to_string(),
        ..Default::default()
    };
    repository.set(&admin_id, &first).await.unwrap();
    assert_eq!(repository.get(&admin_id).await.unwrap(), Some(first.clone()));
    assert_eq!(repository.get(second_id).await.unwrap(), None);

    let replacement = UserPreferences {
        theme: "light".to_string(),
        default_view: "list".to_string(),
        ..Default::default()
    };
    repository.set(&admin_id, &replacement).await.unwrap();
    assert_eq!(repository.get(&admin_id).await.unwrap(), Some(replacement));
}

#[tokio::test]
async fn malformed_persisted_preferences_are_reported_not_silently_defaulted() {
    let database = TestDatabase::seeded("preferences_corrupt.db").await;
    let admin_id: String = sqlx::query_scalar("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO user_preferences (user_id, preferences_json, updated_at) VALUES (?, ?, ?)",
    )
    .bind(&admin_id)
    .bind("{ definitely-not-json")
    .bind(now)
    .execute(&database.pool)
    .await
    .unwrap();

    let repository = SqliteUserPreferencesRepository::new(database.pool.clone());
    assert!(repository.get(&admin_id).await.is_err());
}
