use super::db::TestDatabase;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use backend::{
    bootstrap::build_application,
    config::AppConfig,
    create_router,
    db::DbPool,
    state::{RuntimeOwner, RuntimePhase, ShutdownReason},
    AppState,
};
use serde_json::json;
use std::path::{Component, Path, PathBuf};
use tempfile::TempDir;
use tower::ServiceExt;

pub struct TestApp {
    pub router: axum::Router,
    pub state: AppState,
    pub db: DbPool,
    pub runtime: RuntimeOwner,
    pub temp: TempDir,
    pub database_path: PathBuf,
    pub storage_root: PathBuf,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.runtime.request_shutdown(ShutdownReason::Manual);
    }
}

impl TestApp {
    pub async fn login(&self, username: &str, password: &str) -> String {
        let request = Request::builder()
            .uri("/api/v1/auth/login")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "username": username, "password": password }).to_string(),
            ))
            .expect("build login request");

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("execute login request");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "test fixture login failed for {username}"
        );

        response
            .headers()
            .get(header::SET_COOKIE)
            .expect("login response must set a session cookie")
            .to_str()
            .expect("session cookie must be valid header text")
            .split(';')
            .next()
            .expect("session cookie must contain a cookie pair")
            .to_string()
    }

    pub async fn login_admin(&self) -> String {
        self.login("admin", "admin12345").await
    }

    pub fn storage_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.storage_root.join(normalize_fixture_path(path.as_ref()))
    }
}

pub struct TestAppBuilder {
    database_filename: String,
    runtime_phase: Option<RuntimePhase>,
    initial_files: Vec<(PathBuf, Vec<u8>)>,
}

impl Default for TestAppBuilder {
    fn default() -> Self {
        Self {
            database_filename: "test.db".to_string(),
            runtime_phase: None,
            initial_files: Vec::new(),
        }
    }
}

impl TestAppBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn database_filename(mut self, filename: impl Into<String>) -> Self {
        self.database_filename = filename.into();
        self
    }

    pub fn runtime_phase(mut self, phase: RuntimePhase) -> Self {
        self.runtime_phase = Some(phase);
        self
    }

    pub fn running(self) -> Self {
        self.runtime_phase(RuntimePhase::Running)
    }

    pub fn with_file(
        mut self,
        path: impl Into<PathBuf>,
        contents: impl Into<Vec<u8>>,
    ) -> Self {
        self.initial_files.push((path.into(), contents.into()));
        self
    }

    pub async fn build(self) -> TestApp {
        let TestDatabase {
            pool,
            temp,
            path: database_path,
            url,
        } = TestDatabase::seeded(&self.database_filename).await;

        let storage_root = temp.path().join("storage");
        std::fs::create_dir_all(&storage_root).expect("create test storage root");

        for (path, contents) in self.initial_files {
            let destination = storage_root.join(normalize_fixture_path(&path));
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).expect("create test fixture parent directory");
            }
            std::fs::write(&destination, contents).expect("write test fixture file");
        }

        let mut config = AppConfig::default();
        config.database.url = url;
        config.filesystem.default_local_root = storage_root.clone();

        let built = build_application(config, pool.clone()).await;
        if let Some(phase) = self.runtime_phase {
            built.runtime.set_phase(phase);
        }

        let state = built.state;
        let router = create_router(state.clone());

        TestApp {
            router,
            state,
            db: pool,
            runtime: built.runtime,
            temp,
            database_path,
            storage_root,
        }
    }
}

fn normalize_fixture_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir | Component::RootDir => {}
            Component::ParentDir | Component::Prefix(_) => {
                panic!("test fixture paths must stay inside the storage root: {path:?}")
            }
        }
    }
    normalized
}
