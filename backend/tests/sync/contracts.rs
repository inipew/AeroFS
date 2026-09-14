use async_trait::async_trait;
use backend::{
    domain::{Actor, ConnectionId, FileKind},
    errors::AppError,
    ports::{
        authorization::{Authorization, FileAction},
        sync::SyncControl,
    },
    services::SyncService,
    sync::{
        ConflictResolver, FileManifest, ManifestDiffer, SyncHistoryPage, SyncJob, SyncOpKind,
        SyncOperationRow, SyncPageCursor, SyncStatus, SyncStrategy,
    },
};
use chrono::Utc;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingAuthorization {
    calls: Arc<Mutex<Vec<(String, FileAction)>>>,
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
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Submission {
    user_id: String,
    source_connection: String,
    source_path: String,
    destination_connection: String,
    destination_path: String,
    strategy: SyncStrategy,
}

#[derive(Clone, Default)]
struct RecordingSyncControl {
    submissions: Arc<Mutex<Vec<Submission>>>,
}

#[async_trait]
impl SyncControl for RecordingSyncControl {
    async fn create_job(
        &self,
        user_id: &str,
        source_connection_id: &str,
        source_path: &str,
        destination_connection_id: &str,
        destination_path: &str,
        strategy: SyncStrategy,
    ) -> Result<SyncJob, AppError> {
        self.submissions.lock().unwrap().push(Submission {
            user_id: user_id.to_string(),
            source_connection: source_connection_id.to_string(),
            source_path: source_path.to_string(),
            destination_connection: destination_connection_id.to_string(),
            destination_path: destination_path.to_string(),
            strategy,
        });
        let now = Utc::now();
        Ok(SyncJob {
            id: "fake-sync-job".into(),
            user_id: user_id.into(),
            source_connection_id: source_connection_id.into(),
            source_path: source_path.into(),
            destination_connection_id: destination_connection_id.into(),
            destination_path: destination_path.into(),
            status: SyncStatus::Created,
            strategy,
            total_files: 0,
            synced_files: 0,
            conflict_files: 0,
            created_at: now,
            updated_at: now,
        })
    }

    async fn list_jobs(
        &self,
        _cursor: Option<&SyncPageCursor>,
        _limit: Option<usize>,
    ) -> Result<SyncHistoryPage<SyncJob>, AppError> {
        Ok(SyncHistoryPage {
            items: Vec::new(),
            next_cursor: None,
        })
    }

    async fn list_operations(
        &self,
        _job_id: &str,
        _cursor: Option<&SyncPageCursor>,
        _limit: Option<usize>,
    ) -> Result<SyncHistoryPage<SyncOperationRow>, AppError> {
        Ok(SyncHistoryPage {
            items: Vec::new(),
            next_cursor: None,
        })
    }

    async fn resolve_conflict(
        &self,
        _job_id: &str,
        _op_id: &str,
        _resolution: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

#[tokio::test]
async fn sync_service_authorizes_then_delegates_through_sync_control_port() {
    let authorization = RecordingAuthorization::default();
    let control = RecordingSyncControl::default();
    let service = SyncService::new(Arc::new(authorization.clone()), Arc::new(control.clone()));
    let actor = Actor {
        id: "user-42".into(),
        username: "alice".into(),
        is_admin: false,
    };

    let job = service
        .create_job(
            &actor,
            "local",
            "/source",
            "remote-a",
            "/destination",
            SyncStrategy::SourceWins,
        )
        .await
        .expect("port-backed sync creation should succeed");

    assert_eq!(job.id, "fake-sync-job");
    assert_eq!(
        *authorization.calls.lock().unwrap(),
        vec![
            ("local".to_string(), FileAction::Read),
            ("remote-a".to_string(), FileAction::Write),
        ]
    );
    assert_eq!(
        *control.submissions.lock().unwrap(),
        vec![Submission {
            user_id: "user-42".into(),
            source_connection: "local".into(),
            source_path: "/source".into(),
            destination_connection: "remote-a".into(),
            destination_path: "/destination".into(),
            strategy: SyncStrategy::SourceWins,
        }]
    );
}

#[test]
fn manifest_diff_maps_unchanged_changed_and_new_files_by_strategy() {
    let src = vec![
        FileManifest {
            path: "same.txt".into(),
            kind: FileKind::File,
            size: 100,
            modified_at: None,
            content_hash: Some("hash1".into()),
            etag: None,
        },
        FileManifest {
            path: "modified.txt".into(),
            kind: FileKind::File,
            size: 200,
            modified_at: None,
            content_hash: Some("hash2-src".into()),
            etag: None,
        },
        FileManifest {
            path: "new.txt".into(),
            kind: FileKind::File,
            size: 300,
            modified_at: None,
            content_hash: Some("hash3".into()),
            etag: None,
        },
    ];
    let dst = vec![
        FileManifest {
            path: "same.txt".into(),
            kind: FileKind::File,
            size: 100,
            modified_at: None,
            content_hash: Some("hash1".into()),
            etag: None,
        },
        FileManifest {
            path: "modified.txt".into(),
            kind: FileKind::File,
            size: 250,
            modified_at: None,
            content_hash: Some("hash2-dst".into()),
            etag: None,
        },
    ];

    let source_wins = ManifestDiffer::diff(&src, &dst, SyncStrategy::SourceWins);
    assert_eq!(
        source_wins
            .iter()
            .find(|op| op.relative_path == "same.txt")
            .unwrap()
            .kind,
        SyncOpKind::Noop
    );
    assert_eq!(
        source_wins
            .iter()
            .find(|op| op.relative_path == "modified.txt")
            .unwrap()
            .kind,
        SyncOpKind::Update
    );
    assert_eq!(
        source_wins
            .iter()
            .find(|op| op.relative_path == "new.txt")
            .unwrap()
            .kind,
        SyncOpKind::Create
    );

    let keep_both = ManifestDiffer::diff(&src, &dst, SyncStrategy::KeepBoth);
    assert_eq!(
        keep_both
            .iter()
            .find(|op| op.relative_path == "modified.txt")
            .unwrap()
            .kind,
        SyncOpKind::Conflict
    );
}

#[test]
fn conflict_filename_preserves_stem_and_extension_shape() {
    let with_extension = ConflictResolver::generate_conflict_filename("report.pdf");
    assert!(with_extension.starts_with("report (conflict-"));
    assert!(with_extension.ends_with(").pdf"));

    let without_extension = ConflictResolver::generate_conflict_filename("README");
    assert!(without_extension.starts_with("README (conflict-"));
    assert!(without_extension.ends_with(')'));
}
