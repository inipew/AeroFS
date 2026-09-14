mod support;

use axum::extract::FromRef;
use backend::{
    domain::ProviderKind,
    events::DomainEvent,
    ports::transfer::{TransferPhase, TransferStatus, TransferType},
    services::CreateConnectionRequest,
    state::{ConnectionState, FileApiState, RealtimeState, TransferState},
};
use support::{
    create_local_transfer, create_transfer, list_transfers, transfer_actor, transfer_admin_actor,
    wait_completed, wait_failed, wait_for_status, TestAppBuilder,
};
use std::time::Duration;

#[tokio::test]
async fn copy_completion_is_durable_and_queryable_after_execution_finishes() {
    let payload = vec![b'A'; 256 * 1024];
    let app = TestAppBuilder::new()
        .running()
        .with_file("source.bin", payload.clone())
        .build()
        .await;
    let admin = transfer_admin_actor();

    let job_id = create_local_transfer(
        &app,
        &admin,
        "copy source",
        TransferType::Copy,
        "/source.bin",
        "/destination.bin",
    )
    .await;

    let completed = wait_completed(&app, &admin, &job_id).await;
    assert_eq!(completed.phase, TransferPhase::Completed);
    assert_eq!(completed.transferred_bytes, payload.len() as u64);
    assert_eq!(completed.total_bytes, payload.len() as u64);
    assert!(completed.checksum.is_some());
    assert!(!completed.capabilities.can_cancel);
    assert_eq!(std::fs::read(app.storage_path("destination.bin")).unwrap(), payload);

    let row: (String, String, i64, i64) = sqlx::query_as(
        "SELECT status, phase, transferred_bytes, total_bytes FROM transfer_jobs WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(row.0, "completed");
    assert_eq!(row.1, "completed");
    assert_eq!(row.2, payload.len() as i64);
    assert_eq!(row.3, payload.len() as i64);

    let history = list_transfers(&app, &admin).await;
    assert!(history.iter().any(|job| job.id == job_id));
}

#[tokio::test]
async fn move_deletes_source_only_after_destination_commits() {
    let payload = b"transactional move payload".to_vec();
    let app = TestAppBuilder::new()
        .running()
        .with_file("move-source.txt", payload.clone())
        .build()
        .await;
    let admin = transfer_admin_actor();

    let job_id = create_local_transfer(
        &app,
        &admin,
        "move source",
        TransferType::Move,
        "/move-source.txt",
        "/move-destination.txt",
    )
    .await;
    let completed = wait_completed(&app, &admin, &job_id).await;

    assert_eq!(completed.phase, TransferPhase::Completed);
    assert!(!app.storage_path("move-source.txt").exists());
    assert_eq!(
        std::fs::read(app.storage_path("move-destination.txt")).unwrap(),
        payload
    );
}

#[tokio::test]
async fn recursive_directory_copy_preserves_nested_payloads() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("tree/file1.txt", b"file one".to_vec())
        .with_file("tree/nested/file2.txt", b"file two".to_vec())
        .build()
        .await;
    let admin = transfer_admin_actor();

    let job_id = create_local_transfer(
        &app,
        &admin,
        "copy tree",
        TransferType::Copy,
        "/tree",
        "/tree-copy",
    )
    .await;
    wait_completed(&app, &admin, &job_id).await;

    assert_eq!(
        std::fs::read(app.storage_path("tree-copy/file1.txt")).unwrap(),
        b"file one"
    );
    assert_eq!(
        std::fs::read(app.storage_path("tree-copy/nested/file2.txt")).unwrap(),
        b"file two"
    );
}

#[tokio::test]
async fn failed_transfer_retries_from_durable_history_when_source_appears() {
    let app = TestAppBuilder::new().running().build().await;
    let admin = transfer_admin_actor();

    let job_id = create_local_transfer(
        &app,
        &admin,
        "late source",
        TransferType::Copy,
        "/late-source.txt",
        "/late-destination.txt",
    )
    .await;

    let failed = wait_failed(&app, &admin, &job_id).await;
    assert!(failed.capabilities.can_retry);
    assert!(failed.error_message.is_some());

    let payload = b"source became available before retry";
    std::fs::write(app.storage_path("late-source.txt"), payload).unwrap();

    let transfers = TransferState::from_ref(&app.state);
    transfers.use_cases.retry(&admin, &job_id).await.unwrap();

    let completed = wait_completed(&app, &admin, &job_id).await;
    assert_eq!(completed.phase, TransferPhase::Completed);
    assert_eq!(
        std::fs::read(app.storage_path("late-destination.txt")).unwrap(),
        payload
    );
}

#[tokio::test]
async fn concurrent_retry_admits_exactly_one_attempt() {
    let app = TestAppBuilder::new().running().build().await;
    let admin = transfer_admin_actor();

    let job_id = create_local_transfer(
        &app,
        &admin,
        "concurrent retry",
        TransferType::Copy,
        "/retry-source.txt",
        "/retry-destination.txt",
    )
    .await;
    wait_failed(&app, &admin, &job_id).await;
    std::fs::write(app.storage_path("retry-source.txt"), b"retry payload").unwrap();

    let transfers = TransferState::from_ref(&app.state);
    let first = transfers.use_cases.clone();
    let second = transfers.use_cases.clone();
    let first_actor = admin.clone();
    let second_actor = admin.clone();
    let first_id = job_id.clone();
    let second_id = job_id.clone();

    let first_retry = tokio::spawn(async move { first.retry(&first_actor, &first_id).await });
    let second_retry = tokio::spawn(async move { second.retry(&second_actor, &second_id).await });
    let (first_result, second_result) = tokio::join!(first_retry, second_retry);
    let first_result = first_result.unwrap();
    let second_result = second_result.unwrap();

    assert_eq!(
        usize::from(first_result.is_ok()) + usize::from(second_result.is_ok()),
        1
    );
    let rejected = if let Err(error) = first_result {
        error
    } else {
        second_result.unwrap_err()
    };
    assert!(matches!(rejected, backend::errors::AppError::BadRequest(_)));

    wait_completed(&app, &admin, &job_id).await;
}

#[tokio::test]
async fn dismissed_failed_transfer_is_hidden_and_cannot_be_retried() {
    let app = TestAppBuilder::new().running().build().await;
    let admin = transfer_admin_actor();

    let job_id = create_local_transfer(
        &app,
        &admin,
        "dismiss failure",
        TransferType::Copy,
        "/missing.txt",
        "/never-created.txt",
    )
    .await;
    wait_failed(&app, &admin, &job_id).await;

    let transfers = TransferState::from_ref(&app.state);
    transfers.use_cases.dismiss(&admin, &job_id).await.unwrap();
    assert!(list_transfers(&app, &admin)
        .await
        .iter()
        .all(|job| job.id != job_id));

    let dismissed_at: (Option<String>,) =
        sqlx::query_as("SELECT dismissed_at FROM transfer_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(dismissed_at.0.is_some());

    std::fs::write(app.storage_path("missing.txt"), b"too late").unwrap();
    assert!(matches!(
        transfers.use_cases.retry(&admin, &job_id).await,
        Err(backend::errors::AppError::BadRequest(_))
    ));
}

#[tokio::test]
async fn clear_finished_persists_history_dismissal() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("clear-source.txt", b"clear me".to_vec())
        .build()
        .await;
    let admin = transfer_admin_actor();

    let job_id = create_local_transfer(
        &app,
        &admin,
        "clear finished",
        TransferType::Copy,
        "/clear-source.txt",
        "/clear-destination.txt",
    )
    .await;
    wait_completed(&app, &admin, &job_id).await;

    let transfers = TransferState::from_ref(&app.state);
    let cleared = transfers.use_cases.clear_finished(&admin).await.unwrap();
    assert_eq!(cleared, 1);
    assert!(list_transfers(&app, &admin).await.is_empty());

    let dismissed_at: (Option<String>,) =
        sqlx::query_as("SELECT dismissed_at FROM transfer_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(dismissed_at.0.is_some());
}

#[tokio::test]
async fn non_owner_cannot_observe_cancel_or_retry_an_owned_transfer() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("owned-source.txt", b"owned payload".to_vec())
        .build()
        .await;
    let owner = transfer_admin_actor();
    let other = transfer_actor("other-user", "other", false);

    let job_id = create_local_transfer(
        &app,
        &owner,
        "owned transfer",
        TransferType::Copy,
        "/owned-source.txt",
        "/owned-destination.txt",
    )
    .await;
    wait_completed(&app, &owner, &job_id).await;

    assert!(list_transfers(&app, &other)
        .await
        .iter()
        .all(|job| job.id != job_id));

    let transfers = TransferState::from_ref(&app.state);
    assert!(matches!(
        transfers.use_cases.cancel(&other, &job_id).await,
        Err(backend::errors::AppError::Forbidden(_))
    ));
    assert!(matches!(
        transfers.use_cases.retry(&other, &job_id).await,
        Err(backend::errors::AppError::Forbidden(_))
    ));
}

#[tokio::test]
async fn cancellation_emits_terminal_event_and_removes_staging_file() {
    let app = TestAppBuilder::new().running().build().await;
    let admin = transfer_admin_actor();

    let source = std::fs::File::create(app.storage_path("cancel-source.bin")).unwrap();
    source.set_len(64 * 1024 * 1024).unwrap();
    drop(source);

    let realtime = RealtimeState::from_ref(&app.state);
    let mut events = realtime.service.subscribe();
    let job_id = create_local_transfer(
        &app,
        &admin,
        "cancel active copy",
        TransferType::Copy,
        "/cancel-source.bin",
        "/cancel-destination.bin",
    )
    .await;

    let transfers = TransferState::from_ref(&app.state);
    transfers.use_cases.cancel(&admin, &job_id).await.unwrap();

    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let envelope = events
                .recv()
                .await
                .expect("realtime event stream should stay open");
            if let DomainEvent::TransferCancelled(value) = envelope.event {
                if value.get("id").and_then(|id| id.as_str()) == Some(job_id.as_str()) {
                    break;
                }
            }
        }
    })
    .await
    .expect("cancelled transfer event should arrive before deadline");

    let cancelled = wait_for_status(&app, &admin, &job_id, &[TransferStatus::Cancelled]).await;
    assert_eq!(cancelled.status, TransferStatus::Cancelled);

    let staging = app.storage_path(format!(
        ".cancel-destination.bin.aerofs-part-{job_id}"
    ));
    assert!(!staging.exists(), "cancelled transfer must not leave staging data");
}

#[tokio::test]
async fn deleting_connection_settles_transfer_using_that_provider() {
    let app = TestAppBuilder::new().running().build().await;
    let admin = transfer_admin_actor();
    let connections = ConnectionState::from_ref(&app.state);

    let remote_root = app.temp.path().join("secondary-storage");
    std::fs::create_dir_all(&remote_root).unwrap();
    let connection_id = connections
        .service
        .create_connection(
            &admin,
            CreateConnectionRequest {
                name: "Secondary local provider".to_string(),
                provider: ProviderKind::Local,
                host: None,
                port: None,
                username: None,
                secret: None,
                base_path: Some(remote_root.to_string_lossy().into_owned()),
                read_only: None,
            },
        )
        .await
        .unwrap();

    let source = std::fs::File::create(app.storage_path("drain-source.bin")).unwrap();
    source.set_len(64 * 1024 * 1024).unwrap();
    drop(source);

    let job_id = create_transfer(
        &app,
        &admin,
        "connection drain",
        TransferType::Copy,
        "local",
        "/drain-source.bin",
        &connection_id,
        "/drain-destination.bin",
    )
    .await;

    connections
        .service
        .delete_connection(&admin, &connection_id)
        .await
        .unwrap();

    let settled = wait_for_status(
        &app,
        &admin,
        &job_id,
        &[
            TransferStatus::Cancelled,
            TransferStatus::CancellationRequested,
            TransferStatus::Failed,
        ],
    )
    .await;
    assert!(matches!(
        settled.status,
        TransferStatus::Cancelled
            | TransferStatus::CancellationRequested
            | TransferStatus::Failed
    ));
}

#[tokio::test]
async fn completed_destination_is_visible_through_file_application_boundary() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("visible-source.txt", b"visible".to_vec())
        .build()
        .await;
    let admin = transfer_admin_actor();
    let job_id = create_local_transfer(
        &app,
        &admin,
        "visible destination",
        TransferType::Copy,
        "/visible-source.txt",
        "/visible-destination.txt",
    )
    .await;
    wait_completed(&app, &admin, &job_id).await;

    let file_api = FileApiState::from_ref(&app.state);
    let metadata = file_api
        .files
        .stat_file
        .execute(
            &admin,
            backend::application::files::StatFileCommand {
                connection: backend::domain::ConnectionId::local(),
                path: "/visible-destination.txt".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(metadata.size, 7);
}
