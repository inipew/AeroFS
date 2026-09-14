use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{
    application::upload::UploadApplicationService,
    domain::{Actor, Capabilities, CommitSemantics, ConnectionId, VfsPath},
    errors::AppError,
    ports::{
        authorization::{Authorization, FileAction},
        effects::FileMutationEffects,
        mutation::{MutationCoordinator, MutationLease},
        settings::FileSettings,
        upload::{
            CreateInlineUploadJob, InlineUploadContext, PreparedInlineUpload, UploadByteStream,
            UploadClaim, UploadExecution, UploadPlan, UploadReservationStore, UploadSession,
            UploadStaging,
        },
    },
    vfs::FileSystem,
};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};
use tower::ServiceExt;

use crate::support::{MemoryFileSystem, SwappableFileSystemResolver, TestAppBuilder};

#[derive(Default)]
struct AllowAllAuthorization;

#[async_trait]
impl Authorization for AllowAllAuthorization {
    async fn authorize(
        &self,
        _actor: &Actor,
        _connection: &ConnectionId,
        _action: FileAction,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Default)]
struct NoopMutationEffects;

#[async_trait]
impl FileMutationEffects for NoopMutationEffects {
    async fn invalidate(&self, _connection: &ConnectionId, _path: &str) {}
    async fn invalidate_prefix(&self, _connection: &ConnectionId, _path: &str) {}

    async fn file_changed(
        &self,
        _actor: &Actor,
        _connection: &ConnectionId,
        _path: &str,
        _audit_action: &'static str,
        _event_action: &'static str,
        _details: Option<String>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn file_renamed(
        &self,
        _actor: &Actor,
        _connection: &ConnectionId,
        _from: &str,
        _to: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn file_copied(
        &self,
        _actor: &Actor,
        _connection: &ConnectionId,
        _from: &str,
        _to: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Default)]
struct NoopMutationCoordinator;

#[async_trait]
impl MutationCoordinator for NoopMutationCoordinator {
    async fn try_acquire(
        &self,
        _connection: &ConnectionId,
        _path: &str,
    ) -> Result<Box<dyn MutationLease>, AppError> {
        Ok(Box::new(()))
    }
}

struct MutableFileSettings {
    max_editable_size: RwLock<u64>,
    local_root: PathBuf,
}

impl MutableFileSettings {
    fn new(max_editable_size: u64) -> Self {
        Self {
            max_editable_size: RwLock::new(max_editable_size),
            local_root: std::env::temp_dir(),
        }
    }

    fn set_max_editable_size(&self, value: u64) {
        *self.max_editable_size.write().unwrap() = value;
    }
}

#[async_trait]
impl FileSettings for MutableFileSettings {
    async fn show_hidden_default(&self) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn max_editable_size(&self) -> Result<u64, AppError> {
        Ok(*self.max_editable_size.read().unwrap())
    }

    async fn local_root(&self) -> Result<PathBuf, AppError> {
        Ok(self.local_root.clone())
    }

    async fn allow_symlinks_outside_root(&self) -> Result<bool, AppError> {
        Ok(false)
    }

    fn max_directory_entries(&self) -> usize {
        10_000
    }
}

struct ClaimedUpload {
    session: UploadSession,
}

impl UploadClaim for ClaimedUpload {
    fn session(&self) -> &UploadSession {
        &self.session
    }
}

#[derive(Default)]
struct MemoryReservations {
    session: Mutex<Option<UploadSession>>,
    lease: Mutex<Option<Box<dyn MutationLease>>>,
}

#[async_trait]
impl UploadReservationStore for MemoryReservations {
    async fn reserve(
        &self,
        session: UploadSession,
        lease: Box<dyn MutationLease>,
    ) -> Result<(), AppError> {
        *self.session.lock().unwrap() = Some(session);
        *self.lease.lock().unwrap() = Some(lease);
        Ok(())
    }

    async fn claim(
        &self,
        job_id: &str,
        user_id: &str,
    ) -> Result<Box<dyn UploadClaim>, AppError> {
        let session = self
            .session
            .lock()
            .unwrap()
            .clone()
            .filter(|session| session.job_id == job_id && session.user_id == user_id)
            .ok_or_else(|| AppError::NotFound(format!("upload session {job_id}")))?;
        Ok(Box::new(ClaimedUpload { session }))
    }

    async fn release(&self, _job_id: &str) {
        self.session.lock().unwrap().take();
        self.lease.lock().unwrap().take();
    }
}

#[derive(Default)]
struct RecordingUploadExecution {
    requests: Mutex<Vec<CreateInlineUploadJob>>,
    providers: Mutex<Vec<Arc<dyn FileSystem>>>,
    completed: Mutex<Vec<String>>,
    next_id: Mutex<usize>,
}

#[async_trait]
impl UploadExecution for RecordingUploadExecution {
    async fn create_inline_job(
        &self,
        request: CreateInlineUploadJob,
    ) -> Result<PreparedInlineUpload, AppError> {
        self.requests.lock().unwrap().push(request);
        let mut next = self.next_id.lock().unwrap();
        *next += 1;
        Ok(PreparedInlineUpload {
            job_id: format!("fake-upload-{next}"),
            plan: UploadPlan {
                staging: UploadStaging::None,
                commit: CommitSemantics::DirectWrite,
            },
        })
    }

    async fn execute_inline(
        &self,
        provider: Arc<dyn FileSystem>,
        _context: InlineUploadContext,
        mut stream: UploadByteStream<'_>,
    ) -> Result<u64, AppError> {
        self.providers.lock().unwrap().push(provider);
        let mut total = 0u64;
        while let Some(chunk) = stream.next().await {
            total += chunk?.len() as u64;
        }
        Ok(total)
    }

    async fn complete_inline_job(&self, job_id: &str) {
        self.completed.lock().unwrap().push(job_id.to_string());
    }

    async fn fail_inline_job(&self, _job_id: &str, _error: String) {}
    async fn cancel_inline_job(&self, _job_id: &str) {}
}

fn upload_service(
    resolver: Arc<SwappableFileSystemResolver>,
    settings: Arc<MutableFileSettings>,
    reservations: Arc<MemoryReservations>,
    execution: Arc<RecordingUploadExecution>,
) -> UploadApplicationService {
    UploadApplicationService::new(
        Arc::new(AllowAllAuthorization),
        resolver,
        Arc::new(NoopMutationEffects),
        Arc::new(NoopMutationCoordinator),
        reservations,
        execution,
        settings,
        1024 * 1024,
    )
}

fn actor() -> Actor {
    Actor {
        id: "upload-user".into(),
        username: "upload-user".into(),
        is_admin: false,
    }
}

fn provider() -> Arc<MemoryFileSystem> {
    Arc::new(MemoryFileSystem::new(Capabilities {
        stat: true,
        read: true,
        write: true,
        upload: true,
        ..Default::default()
    }))
}

#[tokio::test]
async fn admitted_upload_session_executes_against_the_pinned_provider_generation() {
    let provider_a = provider();
    let provider_b = provider();
    let resolver = Arc::new(SwappableFileSystemResolver::new(provider_a.clone()));
    let settings = Arc::new(MutableFileSettings::new(4096));
    let reservations = Arc::new(MemoryReservations::default());
    let execution = Arc::new(RecordingUploadExecution::default());
    let service = upload_service(
        resolver.clone(),
        settings,
        reservations,
        execution.clone(),
    );
    let connection = ConnectionId::new("remote-a").unwrap();
    let target = VfsPath::new("remote-a", "/session.txt").unwrap();

    let session = service
        .create_session(
            &actor(),
            &connection,
            target,
            "session.txt".into(),
            Some(5),
        )
        .await
        .unwrap();
    resolver.replace(provider_b.clone());

    service
        .execute_session_stream(
            &actor(),
            &connection,
            &session.job_id,
            futures::stream::iter(vec![Ok(Bytes::from_static(b"hello"))]),
        )
        .await
        .unwrap();

    let providers = execution.providers.lock().unwrap();
    assert_eq!(providers.len(), 1);
    let admitted: Arc<dyn FileSystem> = provider_a;
    let replaced: Arc<dyn FileSystem> = provider_b;
    assert!(Arc::ptr_eq(&providers[0], &admitted));
    assert!(!Arc::ptr_eq(&providers[0], &replaced));
}

#[tokio::test]
async fn upload_admission_reads_mutable_file_settings_for_each_session() {
    let resolver = Arc::new(SwappableFileSystemResolver::new(provider()));
    let settings = Arc::new(MutableFileSettings::new(64));
    let reservations = Arc::new(MemoryReservations::default());
    let execution = Arc::new(RecordingUploadExecution::default());
    let service = upload_service(
        resolver,
        settings.clone(),
        reservations,
        execution.clone(),
    );
    let connection = ConnectionId::new("remote-a").unwrap();

    service
        .create_session(
            &actor(),
            &connection,
            VfsPath::new("remote-a", "/first.txt").unwrap(),
            "first.txt".into(),
            Some(1),
        )
        .await
        .unwrap();
    settings.set_max_editable_size(4096);
    service
        .create_session(
            &actor(),
            &connection,
            VfsPath::new("remote-a", "/second.txt").unwrap(),
            "second.txt".into(),
            Some(1),
        )
        .await
        .unwrap();

    let requests = execution.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].inline_threshold, 64);
    assert_eq!(requests[1].inline_threshold, 4096);
}

#[tokio::test]
async fn http_upload_session_admits_streams_and_completes_one_durable_transfer_job() {
    let app = TestAppBuilder::new().running().build().await;
    let cookie = app.login_admin().await;

    let admitted = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/uploads")
                .method("POST")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "path": "/session.txt",
                        "file_name": "session.txt",
                        "total_bytes": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admitted.status(), StatusCode::ACCEPTED);
    let body = to_bytes(admitted.into_body(), usize::MAX).await.unwrap();
    let admitted: Value = serde_json::from_slice(&body).unwrap();
    let job_id = admitted["job_id"].as_str().unwrap().to_string();
    let upload_url = admitted["upload_url"].as_str().unwrap().to_string();

    let streamed = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(upload_url)
                .method("PUT")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("hello"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(streamed.status(), StatusCode::OK);

    let jobs = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/transfers")
                .method("GET")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(jobs.status(), StatusCode::OK);
    let jobs: Value = serde_json::from_slice(
        &to_bytes(jobs.into_body(), usize::MAX).await.unwrap(),
    )
    .unwrap();
    let job = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|job| job["id"] == job_id)
        .expect("admitted upload transfer must remain queryable");
    assert_eq!(job["status"], "completed");
    assert_eq!(std::fs::read(app.storage_path("session.txt")).unwrap(), b"hello");
}
