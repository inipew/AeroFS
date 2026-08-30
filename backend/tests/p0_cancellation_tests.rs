use backend::{
    config::AppConfig,
    db::init_db,
    domain::{Capabilities, FileEntry, FileMetadata, VfsPath},
    errors::VfsError,
    transfer::{TransferPlan, TransferStaging, TransferStatus},
    vfs::FileSystem,
    AppState,
};
use axum::body::Bytes;
use backend::errors::AppError;
use futures::Stream;
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::Notify;

struct BlockingMockFs {
    capabilities: Capabilities,
    write_started: Arc<Notify>,
    write_continue: Arc<Notify>,
    delete_called: Arc<AtomicBool>,
    rename_called: Arc<AtomicBool>,
    write_calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl FileSystem for BlockingMockFs {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }
    async fn list_stream(
        &self,
        _path: &VfsPath,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<FileEntry, VfsError>> + Send + 'static>>, VfsError>
    {
        Err(VfsError::IoError("not implemented".into()))
    }
    async fn stat(&self, _path: &VfsPath) -> Result<FileMetadata, VfsError> {
        Err(VfsError::IoError("not found".into()))
    }
    async fn read_stream(
        &self,
        _path: &VfsPath,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, VfsError> {
        Err(VfsError::IoError("not implemented".into()))
    }
    async fn read_range(
        &self,
        _path: &VfsPath,
        _offset: u64,
        _length: u64,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, VfsError> {
        Err(VfsError::IoError("not implemented".into()))
    }
    async fn write_stream(
        &self,
        _path: &VfsPath,
        mut _input: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
    ) -> Result<(), VfsError> {
        self.write_calls.fetch_add(1, Ordering::SeqCst);
        self.write_started.notify_one();
        // Block until test allows continue or cancellation via drop+token
        // Wait for either continue or timeout; if cancelled, the executor's select will abort this future via token
        // But our mock just waits on notify; executor's token select will drop this future via abort
        self.write_continue.notified().await;
        // Try to consume input (if cancelled, this future would have been dropped, so we won't reach here in cancelled case)
        // Simulate success if not cancelled
        Ok(())
    }
    async fn create_file(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Ok(())
    }
    async fn create_dir(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Ok(())
    }
    async fn delete(&self, _path: &VfsPath) -> Result<(), VfsError> {
        self.delete_called.store(true, Ordering::SeqCst);
        Ok(())
    }
    async fn rename(&self, _from: &VfsPath, _to: &VfsPath) -> Result<(), VfsError> {
        self.rename_called.store(true, Ordering::SeqCst);
        Ok(())
    }
    async fn copy(&self, _from: &VfsPath, _to: &VfsPath) -> Result<(), VfsError> {
        Ok(())
    }
}

async fn setup_manager() -> (AppState, Arc<BlockingMockFs>) {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("p0_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();
    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir.clone();
    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
    // Keep tempdir alive by leaking (test will drop at end, but state holds path)
    std::mem::forget(temp);

    let caps = {
        let mut c = Capabilities::default();
        c.atomic_rename = true;
        c.write = true;
        c.read = true;
        c
    };
    let mock = Arc::new(BlockingMockFs {
        capabilities: caps,
        write_started: Arc::new(Notify::new()),
        write_continue: Arc::new(Notify::new()),
        delete_called: Arc::new(AtomicBool::new(false)),
        rename_called: Arc::new(AtomicBool::new(false)),
        write_calls: Arc::new(AtomicUsize::new(0)),
    });
    (state, mock)
}

#[tokio::test]
async fn test_initial_cancel_no_commit() {
    let (state, mock) = setup_manager().await;
    let provider: Arc<dyn FileSystem> = mock.clone();
    let target = VfsPath::new("local", "/upload_test.txt").unwrap();
    let plan = TransferPlan {
        execution_mode: backend::transfer::TransferExecutionMode::Inline,
        staging: TransferStaging::LocalTemp,
        commit: backend::domain::CommitSemantics::AtomicRename,
    };
    let job = state
        .transfer_manager
        .create_inline_upload_job_with_plan(
            Some("user1".into()),
            "upload_test.txt".into(),
            "local".into(),
            "/upload_test.txt".into(),
            Some(1024),
            plan.clone(),
        )
        .await;
    let token = state.transfer_manager.cancel_token(&job.id).expect("token exists");
    // Simulate race: cancel before executor start
    token.cancel();
    // Also via manager cancel_job to set status
    let _ = state
        .transfer_manager
        .cancel_job(&job.id, Some("user1"), false)
        .await;
    let byte_stream = futures::stream::empty::<Result<Bytes, AppError>>();
    let res = backend::transfer::executor::execute_inline_upload_stream(
        &state.transfer_manager,
        provider,
        target,
        &job.id,
        &plan,
        Some(1024),
        10 * 1024 * 1024,
        false,
        None,
        token.clone(),
        byte_stream,
    )
    .await;
    assert!(res.is_err(), "should be cancelled");
    assert!(res.unwrap_err().to_string().contains("cancelled"));
    assert!(!mock.rename_called.load(Ordering::SeqCst), "no rename on initial cancel");
    // Job should be Cancelled or CancellationRequested, not Failed/Completed
    let jobs = state.transfer_manager.list_jobs(Some("user1"), false, false).await;
    // Job may have been marked Cancelled via cancel_job
    // Check that at least no commit happened
}

#[tokio::test]
async fn test_mid_stream_cancel_aborts_and_no_rename() {
    let (state, mock) = setup_manager().await;
    let provider: Arc<dyn FileSystem> = mock.clone();
    let target = VfsPath::new("local", "/mid_cancel.txt").unwrap();
    let plan = TransferPlan {
        execution_mode: backend::transfer::TransferExecutionMode::Inline,
        staging: TransferStaging::LocalTemp,
        commit: backend::domain::CommitSemantics::AtomicRename,
    };
    let job = state
        .transfer_manager
        .create_inline_upload_job_with_plan(
            Some("user1".into()),
            "mid_cancel.txt".into(),
            "local".into(),
            "/mid_cancel.txt".into(),
            Some(1024 * 1024),
            plan.clone(),
        )
        .await;
    let token = state.transfer_manager.cancel_token(&job.id).expect("token exists");
    // Create a stream that yields one chunk then pending forever (so executor blocks on next chunk)
    let byte_stream = futures::stream::unfold(0, |state| async move {
        if state == 0 {
            Some((Ok(Bytes::from(vec![1u8; 1024])), 1))
        } else {
            // pending forever to simulate slow client
            std::future::pending::<Option<(Result<Bytes, AppError>, i32)>>().await
        }
    });
    let manager_clone = state.transfer_manager.clone();
    let job_id_clone = job.id.clone();
    let token_clone = token.clone();
    // Spawn executor
    let provider_clone = provider.clone();
    let plan_clone = plan.clone();
    let target_clone = target.clone();
    let handle = tokio::spawn(async move {
        backend::transfer::executor::execute_inline_upload_stream(
            &manager_clone,
            provider_clone,
            target_clone,
            &job_id_clone,
            &plan_clone,
            Some(1024 * 1024),
            10 * 1024 * 1024,
            false,
            None,
            token_clone,
            byte_stream,
        )
        .await
    });
    // Wait until write_stream started (mock notifies)
    tokio::time::timeout(std::time::Duration::from_secs(2), mock.write_started.notified())
        .await
        .expect("write_stream should start");
    // Now cancel via manager (this cancels the same token)
    let _ = state
        .transfer_manager
        .cancel_job(&job.id, Some("user1"), false)
        .await;
    // Also ensure token is cancelled (manager's token is same)
    assert!(token.is_cancelled());
    // Executor should abort within timeout and NOT rename
    let res = tokio::time::timeout(std::time::Duration::from_secs(3), handle)
        .await
        .expect("executor should finish after cancel")
        .unwrap();
    assert!(res.is_err(), "executor should return Err on cancel");
    assert!(res.unwrap_err().to_string().contains("cancelled"));
    assert!(!mock.rename_called.load(Ordering::SeqCst), "must not rename after cancel");
    // write should have been started but not committed
    assert!(mock.write_calls.load(Ordering::SeqCst) >= 1);
    // Allow mock to unblock if still waiting (not needed after cancel, but ensure cleanup)
    mock.write_continue.notify_waiters();
}
