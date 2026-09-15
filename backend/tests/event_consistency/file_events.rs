use axum::extract::FromRef;
use backend::{
    application::files::{ChmodEntryCommand, RenameEntryCommand},
    domain::ConnectionId,
    events::{DomainEvent, ReplayOutcome},
    state::{FileApiState, RealtimeState},
};
use std::time::Duration;

use crate::support::{transfer_admin_actor, TestAppBuilder};

#[tokio::test]
async fn chmod_broadcasts_structured_file_change_metadata() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("test_chmod.txt", b"chmod payload".to_vec())
        .build()
        .await;
    let actor = transfer_admin_actor();
    let realtime = RealtimeState::from_ref(&app.state);
    let files = FileApiState::from_ref(&app.state);
    let mut rx = realtime.service.subscribe();

    files
        .files
        .chmod_entry
        .execute(
            &actor,
            ChmodEntryCommand {
                connection: ConnectionId::local(),
                path: "/test_chmod.txt".to_string(),
                mode: 0o755,
            },
        )
        .await
        .expect("chmod should succeed");

    let envelope = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for chmod event")
        .expect("realtime channel closed before chmod event");

    match envelope.event {
        DomainEvent::FileChange {
            connection_id,
            path,
            action,
            old_path,
            parent_path,
            ..
        } => {
            assert_eq!(connection_id, "local");
            assert_eq!(path, "/test_chmod.txt");
            assert_eq!(action, "chmod");
            assert!(old_path.is_none());
            assert_eq!(parent_path.as_deref(), Some("/"));
        }
        other => panic!("expected chmod FileChange event, got {other:?}"),
    }
}

#[tokio::test]
async fn rename_broadcast_and_replay_preserve_source_and_destination_metadata() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("folder_a/source.txt", b"rename payload".to_vec())
        .build()
        .await;
    std::fs::create_dir_all(app.storage_path("folder_b")).unwrap();

    let actor = transfer_admin_actor();
    let realtime = RealtimeState::from_ref(&app.state);
    let files = FileApiState::from_ref(&app.state);
    let mut rx = realtime.service.subscribe();

    files
        .files
        .rename_entry
        .execute(
            &actor,
            RenameEntryCommand {
                connection: ConnectionId::local(),
                from: "/folder_a/source.txt".to_string(),
                to: "/folder_b/dest.txt".to_string(),
            },
        )
        .await
        .expect("rename should succeed");

    let envelope = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for rename event")
        .expect("realtime channel closed before rename event");

    match envelope.event {
        DomainEvent::FileChange {
            connection_id,
            path,
            action,
            old_path,
            parent_path,
            old_parent_path,
        } => {
            assert_eq!(connection_id, "local");
            assert_eq!(path, "/folder_b/dest.txt");
            assert_eq!(action, "rename");
            assert_eq!(old_path.as_deref(), Some("/folder_a/source.txt"));
            assert_eq!(parent_path.as_deref(), Some("/folder_b"));
            assert_eq!(old_parent_path.as_deref(), Some("/folder_a"));
        }
        other => panic!("expected rename FileChange event, got {other:?}"),
    }

    let epoch = realtime.service.epoch_info().epoch;
    let replay = realtime
        .service
        .replay(Some(&epoch), 0, 100)
        .await
        .expect("replay should succeed");

    let events = match replay {
        ReplayOutcome::Events(events) => events,
        other => panic!("expected replay events, got {other:?}"),
    };
    let replayed = events
        .iter()
        .find(|event| {
            matches!(
                &event.event,
                DomainEvent::FileChange { action, .. } if action == "rename"
            )
        })
        .expect("rename event should be durable and replayable");

    match &replayed.event {
        DomainEvent::FileChange {
            path,
            old_path,
            parent_path,
            old_parent_path,
            ..
        } => {
            assert_eq!(path, "/folder_b/dest.txt");
            assert_eq!(old_path.as_deref(), Some("/folder_a/source.txt"));
            assert_eq!(parent_path.as_deref(), Some("/folder_b"));
            assert_eq!(old_parent_path.as_deref(), Some("/folder_a"));
        }
        other => panic!("expected replayed FileChange event, got {other:?}"),
    }
}
