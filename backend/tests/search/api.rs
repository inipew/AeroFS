use axum::http::{Method, StatusCode};

use crate::support::{response_json, TestAppBuilder, TestAuth};

#[tokio::test]
async fn search_requires_authentication_and_returns_recursive_matches() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("README.md", b"root".to_vec())
        .with_file("documents/report.txt", b"report".to_vec())
        .with_file("documents/notes.md", b"notes".to_vec())
        .with_file("documents/deep/report-42.pdf", b"pdf".to_vec())
        .build()
        .await;

    let anonymous = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/search?query=report",
            TestAuth::Anonymous,
            None,
        )
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let admin = app.login_session("admin", "admin12345").await;
    let response = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/search?query=report",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|entry| entry["path"] == "/documents/report.txt"));
    assert!(results
        .iter()
        .any(|entry| entry["path"] == "/documents/deep/report-42.pdf"));
    assert_eq!(body["truncated"], false);
    assert!(body["total_scanned"].as_u64().unwrap() >= 5);
}

#[tokio::test]
async fn search_honors_path_depth_regex_and_limit_parameters() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("report-root.txt", b"root".to_vec())
        .with_file("documents/report-direct.txt", b"direct".to_vec())
        .with_file("documents/deep/report-deep.txt", b"deep".to_vec())
        .with_file("documents/deep/notes.md", b"notes".to_vec())
        .build()
        .await;
    let admin = app.login_session("admin", "admin12345").await;

    let shallow = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/search?path=/documents&query=report&max_depth=0",
            TestAuth::Bearer(&admin),
            None,
        )
        .await;
    assert_eq!(shallow.status(), StatusCode::OK);
    let shallow = response_json(shallow).await;
    assert_eq!(shallow["results"].as_array().unwrap().len(), 1);
    assert_eq!(shallow["results"][0]["path"], "/documents/report-direct.txt");

    let regex = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/search?path=/documents&query=%5Ereport-&regex=true",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(regex.status(), StatusCode::OK);
    let regex = response_json(regex).await;
    assert_eq!(regex["results"].as_array().unwrap().len(), 2);

    let limited = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/search?query=report&limit=1",
            TestAuth::Cookie(&admin),
            None,
        )
        .await;
    assert_eq!(limited.status(), StatusCode::OK);
    let limited = response_json(limited).await;
    assert_eq!(limited["results"].as_array().unwrap().len(), 1);
    assert_eq!(limited["truncated"], true);
}
