use async_trait::async_trait;
use backend::{
    domain::{Actor, Capabilities, ConnectionId},
    errors::AppError,
    ports::authorization::{Authorization, FileAction},
    runtime::ResourceBudget,
    services::SearchService,
    vfs::FileSystem,
};
use std::sync::{Arc, Mutex};

use crate::support::{MemoryFileSystem, SwappableFileSystemResolver};

struct RecordingAuthorization {
    allow: bool,
    calls: Mutex<Vec<(String, FileAction)>>,
}

impl RecordingAuthorization {
    fn allow() -> Self {
        Self {
            allow: true,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn deny() -> Self {
        Self {
            allow: false,
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Authorization for RecordingAuthorization {
    async fn authorize(
        &self,
        _actor: &Actor,
        connection: &ConnectionId,
        action: FileAction,
    ) -> Result<(), AppError> {
        self.calls
            .lock()
            .unwrap()
            .push((connection.as_str().to_string(), action));
        if self.allow {
            Ok(())
        } else {
            Err(AppError::Forbidden("search denied".to_string()))
        }
    }
}

fn actor() -> Actor {
    Actor {
        id: "search-user".to_string(),
        username: "searcher".to_string(),
        is_admin: false,
    }
}

fn provider() -> Arc<MemoryFileSystem> {
    let fs = Arc::new(MemoryFileSystem::new(Capabilities {
        read: true,
        atomic_write: true,
        atomic_rename: true,
        native_copy: true,
        ..Default::default()
    }));
    fs.insert_dir("/docs");
    fs.insert_dir("/docs/deep");
    fs.insert_dir("/other");
    fs.insert_file("/README.md", b"root".to_vec());
    fs.insert_file("/docs/Quarterly-REPORT.txt", b"report".to_vec());
    fs.insert_file("/docs/notes.md", b"notes".to_vec());
    fs.insert_file("/docs/deep/report-42.pdf", b"pdf".to_vec());
    fs.insert_file("/other/report-draft.txt", b"draft".to_vec());
    fs
}

fn service(
    authorization: Arc<RecordingAuthorization>,
    fs: Arc<MemoryFileSystem>,
    search_permits: usize,
) -> SearchService {
    let provider: Arc<dyn FileSystem> = fs;
    SearchService::new(
        authorization,
        Arc::new(SwappableFileSystemResolver::new(provider)),
        Arc::new(ResourceBudget::with_limits(8, 8, 8, 4, 4, search_permits)),
    )
}

#[tokio::test]
async fn plain_search_is_case_insensitive_recursive_and_scoped_by_path() {
    let authorization = Arc::new(RecordingAuthorization::allow());
    let service = service(authorization.clone(), provider(), 2);
    let connection = ConnectionId::new("memory").unwrap();

    let all = service
        .search_files(&actor(), &connection, None, "report", false, None, None)
        .await
        .unwrap();
    let mut paths: Vec<_> = all.results.iter().map(|entry| entry.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "/docs/Quarterly-REPORT.txt",
            "/docs/deep/report-42.pdf",
            "/other/report-draft.txt",
        ]
    );
    assert!(!all.truncated);
    assert!(all.total_scanned >= 7);

    let scoped = service
        .search_files(
            &actor(),
            &connection,
            Some("/docs"),
            "report",
            false,
            Some(0),
            Some(50),
        )
        .await
        .unwrap();
    assert_eq!(scoped.results.len(), 1);
    assert_eq!(scoped.results[0].path, "/docs/Quarterly-REPORT.txt");

    assert_eq!(
        authorization.calls.lock().unwrap().as_slice(),
        [
            ("memory".to_string(), FileAction::Read),
            ("memory".to_string(), FileAction::Read),
        ]
    );
}

#[tokio::test]
async fn regex_search_and_invalid_regex_preserve_contract() {
    let service = service(Arc::new(RecordingAuthorization::allow()), provider(), 2);
    let connection = ConnectionId::new("memory").unwrap();

    let output = service
        .search_files(
            &actor(),
            &connection,
            Some("/docs"),
            r"^report-\d+\.pdf$",
            true,
            Some(4),
            Some(50),
        )
        .await
        .unwrap();
    assert_eq!(output.results.len(), 1);
    assert_eq!(output.results[0].path, "/docs/deep/report-42.pdf");

    let invalid = service
        .search_files(
            &actor(),
            &connection,
            None,
            "[unterminated",
            true,
            None,
            None,
        )
        .await;
    assert!(invalid.is_err());
    assert!(invalid.unwrap_err().to_string().contains("Invalid regex"));
}

#[tokio::test]
async fn result_limit_truncates_and_search_capacity_returns_to_baseline() {
    let service = service(Arc::new(RecordingAuthorization::allow()), provider(), 2);
    let connection = ConnectionId::new("memory").unwrap();
    assert_eq!(service.available_capacity(), 2);

    let output = service
        .search_files(&actor(), &connection, None, "report", false, None, Some(1))
        .await
        .unwrap();
    assert_eq!(output.results.len(), 1);
    assert!(output.truncated);
    assert_eq!(service.available_capacity(), 2);
}

#[tokio::test]
async fn authorization_failure_stops_search_before_provider_work() {
    let authorization = Arc::new(RecordingAuthorization::deny());
    let service = service(authorization.clone(), provider(), 1);
    let connection = ConnectionId::new("memory").unwrap();

    let error = service
        .search_files(&actor(), &connection, None, "report", false, None, None)
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Forbidden(_)));
    assert_eq!(service.available_capacity(), 1);
    assert_eq!(
        authorization.calls.lock().unwrap().as_slice(),
        [("memory".to_string(), FileAction::Read)]
    );
}
