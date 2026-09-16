use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::support::{response_json, TestAppBuilder, TestAuth};

#[tokio::test]
async fn settings_require_authentication_and_only_admins_can_update() {
    let app = TestAppBuilder::new().running().build().await;

    let anonymous = app
        .json_request(Method::GET, "/api/v1/settings", TestAuth::Anonymous, None)
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let regular = app.seed_session_user("settings-user", false).await;
    let session = app.session_for_user(&regular).await;

    let readable = app
        .json_request(
            Method::GET,
            "/api/v1/settings",
            TestAuth::Cookie(&session),
            None,
        )
        .await;
    assert_eq!(readable.status(), StatusCode::OK);
    let body = response_json(readable).await;
    assert_eq!(body["settings"]["general"]["theme"], "dark");

    let forbidden = app
        .json_request(
            Method::PUT,
            "/api/v1/settings",
            TestAuth::Cookie(&session),
            Some(json!({"show_hidden_default": true})),
        )
        .await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    let error = response_json(forbidden).await;
    assert_eq!(error["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn admin_settings_update_persists_and_activates_new_local_root() {
    let app = TestAppBuilder::new().running().build().await;
    let admin = app.admin_session().await;
    let new_root = app.temp.path().join("settings-root");
    std::fs::create_dir_all(&new_root).unwrap();
    std::fs::write(new_root.join("root-marker.txt"), b"active root").unwrap();
    let new_root_text = new_root.to_string_lossy().to_string();

    let update = app
        .json_request(
            Method::PUT,
            "/api/v1/settings",
            TestAuth::Cookie(&admin),
            Some(json!({
                "local_root": new_root_text,
                "show_hidden_default": true
            })),
        )
        .await;
    assert_eq!(update.status(), StatusCode::OK);
    assert_eq!(response_json(update).await["success"], true);

    let settings = app
        .json_request(
            Method::GET,
            "/api/v1/settings",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(settings.status(), StatusCode::OK);
    let settings = response_json(settings).await;
    assert_eq!(settings["local_root"], new_root.to_string_lossy().as_ref());
    assert_eq!(settings["show_hidden_default"], true);

    let persisted: String = sqlx::query_scalar(
        "SELECT value FROM system_settings WHERE key = 'local_root'",
    )
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(persisted, new_root.to_string_lossy());

    let listing = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/files?path=/",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(listing.status(), StatusCode::OK);
    let listing = response_json(listing).await;
    assert!(listing["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["name"] == "root-marker.txt"));

    let admin_id: String = sqlx::query_scalar("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE user_id = ? AND action = 'SETTINGS_UPDATED'",
    )
    .bind(admin_id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn user_preferences_round_trip_and_remain_isolated_between_users() {
    let app = TestAppBuilder::new().running().build().await;
    let alice = app.seed_session_user("prefs-alice", false).await;
    let bob = app.seed_session_user("prefs-bob", false).await;
    let alice_session = app.session_for_user(&alice).await;
    let bob_session = app.session_for_user(&bob).await;

    let anonymous = app
        .json_request(
            Method::GET,
            "/api/v1/user/preferences",
            TestAuth::Anonymous,
            None,
        )
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let update = app
        .json_request(
            Method::PUT,
            "/api/v1/user/preferences",
            TestAuth::Cookie(&alice_session),
            Some(json!({
                "theme": "dracula",
                "list_density": "compact",
                "default_view": "list"
            })),
        )
        .await;
    assert_eq!(update.status(), StatusCode::OK);
    let update = response_json(update).await;
    assert_eq!(update["success"], true);
    assert_eq!(update["preferences"]["theme"], "dracula");

    let alice_prefs = app
        .json_request(
            Method::GET,
            "/api/v1/user/preferences",
            TestAuth::Cookie(&alice_session),
            None,
        )
        .await;
    let alice_prefs = response_json(alice_prefs).await;
    assert_eq!(alice_prefs["theme"], "dracula");
    assert_eq!(alice_prefs["list_density"], "compact");
    assert_eq!(alice_prefs["default_view"], "list");

    let bob_prefs = app
        .json_request(
            Method::GET,
            "/api/v1/user/preferences",
            TestAuth::Bearer(&bob_session),
            None,
        )
        .await;
    assert_eq!(bob_prefs.status(), StatusCode::OK);
    let bob_prefs = response_json(bob_prefs).await;
    assert_eq!(bob_prefs["theme"], "dark");
    assert_eq!(bob_prefs["list_density"], "comfortable");
    assert_eq!(bob_prefs["default_view"], "grid");

    let alice_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_preferences WHERE user_id = ?",
    )
    .bind(&alice.id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    let bob_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_preferences WHERE user_id = ?",
    )
    .bind(&bob.id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(alice_rows, 1);
    assert_eq!(bob_rows, 0);
}
