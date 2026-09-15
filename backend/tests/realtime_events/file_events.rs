use axum::extract::FromRef;
use backend::{
    application::files::{ChmodEntryCommand, RenameEntryCommand},
    domain::{Actor, ConnectionId},
    events::{DomainEvent, ReplayOutcome},
    state::{FileApiState, RealtimeState},
};

use crate::support::TestAppBuilder;

fn admin() -> Actor {
    Actor {
        id: "event-admin".into(),
        username: "admin".into(),
        is_admin: true,
    }
}

#[tokio::test]
async fn chmod_emits_file_change_with_parent_metadata() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("test_chmod.txt", b"chmod".to_vec())
        .build()
        .await;
    let realtime = RealtimeState::from_ref(&app.state);
    let files = FileApiState::from_ref(&app.state);
    let mut events = realtime.service.subscribe();

    files
        .files
        .chmod_entry
        .execute(
            &admin(),
            ChmodEntryCommand {
                connection: ConnectionId::local(),
                path: "/test_chmod.txt".into(),
                mode: 0o755,
            },
        )
        .await
        .expect("chmod should succeed");

    let envelope = events.recv().await.expect("expected chmod event");
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
            assert_eq!(old_path, None);
            assert_eq!(parent_path.as_deref(), Some("/"));
        }
        other => panic!("expected FileChange, got {other:?}"),
    }
}

#[tokio::test]
async fn rename_emits_and_replays_source_destination_metadata() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("folder_a/source.txt", b"rename content".to_vec())
        .build()
        .await;
    std::fs::create_dir_all(app.storage_path("folder_b")).unwrap();

    let realtime = RealtimeState::from_ref(&app.state);
    let files = FileApiState::from_ref(&app.state);
    let mut events = realtime.service.subscribe();

    files
        .files
        .rename_entry
        .execute(
            &admin(),
            RenameEntryCommand {
                connection: ConnectionId::local(),
                from: "/folder_a/source.txt".into(),
                to: "/folder_b/dest.txt".into(),
            },
        )
        .await
        .expect("rename should succeed");

    let envelope = events.recv().await.expect("expected rename event");
    assert_rename_metadata(&envelope.event);

    let epoch = realtime.service.epoch_info().epoch;
    let replay = realtime
        .service
        .replay(Some(&epoch), 0, 100)
        .await
        .expect("replay should succeed");
    let replayed = match replay {
        ReplayOutcome::Events(events) => events,
        other => panic!("expected replay events, got {other:?}"),
    };
    let rename = replayed
        .iter()
        .find(|event| {
            matches!(
                &event.event,
                DomainEvent::FileChange { action, .. } if action == "rename"
            )
        })
        .expect("rename event should be replayable");
    assert_rename_metadata(&rename.event);
}

fn assert_rename_metadata(event: &DomainEvent) {
    match event {
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
        other => panic!("expected FileChange, got {other:?}"),
    }
}
