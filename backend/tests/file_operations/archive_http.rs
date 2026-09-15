use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::support::{response_json, TestAppBuilder, TestAuth};

#[tokio::test]
async fn archive_http_round_trip_virtual_read_selective_extract_and_audit() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("documents/report.txt", b"Quarterly Financial Report".to_vec())
        .with_file("root.txt", b"Root Level File".to_vec())
        .build()
        .await;
    let admin = app.login_session("admin", "admin12345").await;

    let compressed = app
        .json_request(
            Method::POST,
            "/api/v1/connections/local/archive/compress",
            TestAuth::Cookie(&admin),
            Some(json!({
                "base_path": "/",
                "relative_paths": ["/documents/report.txt", "/root.txt"],
                "destination_file": "/backup.zip",
                "format": "zip"
            })),
        )
        .await;
    assert_eq!(compressed.status(), StatusCode::CREATED);

    let extracted = app
        .json_request(
            Method::POST,
            "/api/v1/connections/local/archive/extract",
            TestAuth::Cookie(&admin),
            Some(json!({
                "archive_path": "/backup.zip",
                "destination_dir": "/extracted",
                "format": "zip"
            })),
        )
        .await;
    assert_eq!(extracted.status(), StatusCode::OK);
    assert_eq!(
        std::fs::read(app.storage_path("extracted/root.txt")).unwrap(),
        b"Root Level File"
    );

    // Preserve the legacy cross-feature contract: freshly extracted content must be
    // immediately visible to the normal search API, not just present on disk.
    let search = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/search?path=/extracted&query=report",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(search.status(), StatusCode::OK);
    let search = response_json(search).await;
    assert!(search["results"].as_array().unwrap().iter().any(|entry| {
        entry["path"] == "/extracted/documents/report.txt"
    }));

    let entries = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/archive/entries?archive_path=/backup.zip",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(entries.status(), StatusCode::OK);
    let entries = response_json(entries).await;
    let entries = entries.as_array().unwrap();
    assert!(entries
        .iter()
        .any(|entry| entry["name"] == "documents" && entry["kind"] == "directory"));
    assert!(entries
        .iter()
        .any(|entry| entry["name"] == "root.txt" && entry["kind"] == "file"));

    let read = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/archive/read?archive_path=/backup.zip&entry_path=root.txt",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(read.status(), StatusCode::OK);
    let body = axum::body::to_bytes(read.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), b"Root Level File");

    let selected = app
        .json_request(
            Method::POST,
            "/api/v1/connections/local/archive/extract-selected",
            TestAuth::Cookie(&admin),
            Some(json!({
                "archive_path": "/backup.zip",
                "destination_dir": "/selected",
                "entries": ["root.txt"]
            })),
        )
        .await;
    assert_eq!(selected.status(), StatusCode::OK);
    assert_eq!(
        std::fs::read(app.storage_path("selected/root.txt")).unwrap(),
        b"Root Level File"
    );

    let audit = app
        .json_request(
            Method::GET,
            "/api/v1/audit-logs",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(audit.status(), StatusCode::OK);
    let logs = response_json(audit).await;
    let logs = logs.as_array().unwrap();
    for action in [
        "ARCHIVE_COMPRESS",
        "ARCHIVE_EXTRACT",
        "ARCHIVE_EXTRACT_SELECTED",
    ] {
        assert!(
            logs.iter().any(|log| log["action"] == action),
            "missing archive audit action {action}"
        );
    }
}

#[tokio::test]
async fn archive_http_preserves_skip_and_keep_both_overwrite_modes() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("collision.txt", b"Original Content in Zip".to_vec())
        .build()
        .await;
    let admin = app.login_session("admin", "admin12345").await;

    let compressed = app
        .json_request(
            Method::POST,
            "/api/v1/connections/local/archive/compress",
            TestAuth::Cookie(&admin),
            Some(json!({
                "base_path": "/",
                "relative_paths": ["/collision.txt"],
                "destination_file": "/collision.zip",
                "format": "zip"
            })),
        )
        .await;
    assert_eq!(compressed.status(), StatusCode::CREATED);

    std::fs::create_dir_all(app.storage_path("target")).unwrap();
    std::fs::write(
        app.storage_path("target/collision.txt"),
        b"Existing Content in Target",
    )
    .unwrap();

    let skipped = app
        .json_request(
            Method::POST,
            "/api/v1/connections/local/archive/extract",
            TestAuth::Cookie(&admin),
            Some(json!({
                "archive_path": "/collision.zip",
                "destination_dir": "/target",
                "format": "zip",
                "overwrite_mode": "skip"
            })),
        )
        .await;
    assert_eq!(skipped.status(), StatusCode::OK);
    assert_eq!(response_json(skipped).await["skipped_count"], 1);
    assert_eq!(
        std::fs::read(app.storage_path("target/collision.txt")).unwrap(),
        b"Existing Content in Target"
    );

    let kept = app
        .json_request(
            Method::POST,
            "/api/v1/connections/local/archive/extract",
            TestAuth::Cookie(&admin),
            Some(json!({
                "archive_path": "/collision.zip",
                "destination_dir": "/target",
                "format": "zip",
                "overwrite_mode": "keep_both"
            })),
        )
        .await;
    assert_eq!(kept.status(), StatusCode::OK);
    assert_eq!(response_json(kept).await["entries_count"], 1);
    assert_eq!(
        std::fs::read(app.storage_path("target/collision (1).txt")).unwrap(),
        b"Original Content in Zip"
    );
}
