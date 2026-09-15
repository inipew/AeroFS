mod support;

use backend::{
    db::init_db,
    events::EventJournal,
    transfer::{
        TransferExecutionMode, TransferJob, TransferManager, TransferPhase, TransferStatus,
        TransferType,
    },
    vfs::{
        opendal::{
            build_fs_operator, capabilities::map_opendal_capabilities_for_scheme,
            OpenDalFileSystem,
        },
        FileSystem,
    },
};
use chrono::Utc;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn streaming_transfer_succeeds_when_destination_has_no_atomic_commit_capabilities() {
    let root = tempfile::tempdir().unwrap();
    let source_root = root.path().join("source");
    let destination_root = root.path().join("destination");
    std::fs::create_dir_all(&source_root).unwrap();
    std::fs::create_dir_all(&destination_root).unwrap();
    let payload = b"fallback transfer payload for a non-atomic destination";
    std::fs::write(source_root.join("source.txt"), payload).unwrap();

    let source_operator = build_fs_operator(&source_root.to_string_lossy()).unwrap();
    let source_capabilities = map_opendal_capabilities_for_scheme(
        source_operator.info().capability(),
        source_operator.info().scheme(),
    );
    let source: Arc<dyn FileSystem> = Arc::new(OpenDalFileSystem::new_with_capabilities(
        "local",
        source_operator,
        source_capabilities,
    ));

    let destination_operator = build_fs_operator(&destination_root.to_string_lossy()).unwrap();
    let mut destination_capabilities = map_opendal_capabilities_for_scheme(
        destination_operator.info().capability(),
        destination_operator.info().scheme(),
    );
    destination_capabilities.atomic_rename = false;
    destination_capabilities.atomic_write = false;
    let destination: Arc<dyn FileSystem> = Arc::new(OpenDalFileSystem::new_with_capabilities(
        "non-atomic",
        destination_operator,
        destination_capabilities,
    ));

    let database_path = root.path().join("transfer.db");
    let database_url = format!("sqlite://{}?mode=rwc", database_path.to_string_lossy());
    let db = init_db(&database_url).await.unwrap();
    let journal = Arc::new(EventJournal::init(db.clone()).await.unwrap());

    let now = Utc::now();
    let mut job = TransferJob {
        id: "non-atomic-destination".into(),
        user_id: Some("admin-user".into()),
        name: "non-atomic destination".into(),
        transfer_type: TransferType::Copy,
        source_connection_id: "local".into(),
        source_path: "/source.txt".into(),
        destination_connection_id: "non-atomic".into(),
        destination_path: "/destination.txt".into(),
        status: TransferStatus::Queued,
        phase: TransferPhase::Preparing,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: payload.len() as u64,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: None,
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };

    let cancellation = tokio_util::sync::CancellationToken::new();
    let jobs = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    jobs.write().await.insert(job.id.clone(), job.clone());

    let mut providers = std::collections::HashMap::new();
    providers.insert("local".to_string(), source);
    providers.insert("non-atomic".to_string(), destination.clone());
    let providers = Arc::new(tokio::sync::RwLock::new(providers));

    TransferManager::execute_job(
        &mut job,
        &cancellation,
        &providers,
        &jobs,
        &journal,
        &db,
    )
    .await
    .expect("non-atomic destination must fall back to a supported commit path");

    let target = backend::domain::VfsPath::new("non-atomic", "/destination.txt").unwrap();
    let mut reader = destination.read_stream(&target).await.unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    assert_eq!(bytes, payload);
}
