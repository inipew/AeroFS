use axum::extract::FromRef;
use backend::{
    events::DomainEvent,
    ports::transfer::TransferType,
    state::RealtimeState,
};
use std::time::Duration;

use crate::support::{create_local_transfer, transfer_admin_actor, TestAppBuilder};

#[tokio::test]
async fn destination_file_change_precedes_transfer_completed() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("order_src.txt", b"ordering test".to_vec())
        .build()
        .await;
    let realtime = RealtimeState::from_ref(&app.state);
    let mut events = realtime.service.subscribe();
    let admin = transfer_admin_actor();

    create_local_transfer(
        &app,
        &admin,
        "event ordering",
        TransferType::Copy,
        "/order_src.txt",
        "/order_dst.txt",
    )
    .await;

    let (file_change_sequence, completed_sequence) = tokio::time::timeout(
        Duration::from_secs(8),
        async {
            let mut file_change_sequence = None;
            loop {
                let envelope = events
                    .recv()
                    .await
                    .expect("realtime event stream should remain open");
                match envelope.event {
                    DomainEvent::FileChange { path, action, .. }
                        if path == "/order_dst.txt" && action == "create" =>
                    {
                        file_change_sequence = Some(envelope.sequence);
                    }
                    DomainEvent::TransferCompleted(job)
                        if job.get("destination_path").and_then(|value| value.as_str())
                            == Some("/order_dst.txt") =>
                    {
                        break (file_change_sequence, envelope.sequence);
                    }
                    _ => {}
                }
            }
        },
    )
    .await
    .expect("transfer terminal event should arrive before deadline");

    let file_change_sequence =
        file_change_sequence.expect("destination FileChange must precede completion");
    assert!(
        file_change_sequence < completed_sequence,
        "destination FileChange must have a lower sequence than TransferCompleted"
    );
}
