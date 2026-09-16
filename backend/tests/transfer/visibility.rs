use backend::{
    auth::{AuthenticatedUser, UserInfo},
    ports::transfer::{TransferJob, TransferPhase, TransferStatus, TransferType},
    services::TransferService,
};
use chrono::Utc;
use std::collections::HashSet;

fn user(id: &str, username: &str, is_admin: bool) -> AuthenticatedUser {
    AuthenticatedUser(UserInfo {
        id: id.to_string(),
        username: username.to_string(),
        is_admin,
    })
}

fn interrupted_job() -> TransferJob {
    let now = Utc::now();
    TransferJob {
        id: "job-123".into(),
        user_id: Some("owner-user".into()),
        name: "test-transfer".into(),
        transfer_type: TransferType::Copy,
        status: TransferStatus::Interrupted,
        phase: TransferPhase::Finalizing,
        execution_mode: Default::default(),
        staging: Default::default(),
        source_connection_id: "local".into(),
        source_path: "/source".into(),
        destination_connection_id: "remote_s3".into(),
        destination_path: "/dest".into(),
        total_bytes: 1000,
        transferred_bytes: 500,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("Transfer interrupted by server restart".into()),
        created_at: now,
        updated_at: now,
        dismissed_at: None,
    }
}

#[test]
fn interrupted_status_has_stable_wire_value() {
    assert_eq!(TransferStatus::Interrupted.as_str(), "interrupted");
}

#[test]
fn transfer_visibility_allows_admin_owner_or_access_to_both_connections() {
    let admin = user("admin-user", "admin", true);
    let owner = user("owner-user", "owner", false);
    let stranger = user("stranger-user", "stranger", false);
    let job = interrupted_job();

    let mut allowed_connections = HashSet::new();
    assert!(TransferService::authorize_transfer_visibility(
        &admin,
        &job,
        &allowed_connections
    ));
    assert!(TransferService::authorize_transfer_visibility(
        &owner,
        &job,
        &allowed_connections
    ));
    assert!(!TransferService::authorize_transfer_visibility(
        &stranger,
        &job,
        &allowed_connections
    ));

    allowed_connections.insert("local".into());
    assert!(!TransferService::authorize_transfer_visibility(
        &stranger,
        &job,
        &allowed_connections
    ));

    allowed_connections.insert("remote_s3".into());
    assert!(TransferService::authorize_transfer_visibility(
        &stranger,
        &job,
        &allowed_connections
    ));
}
