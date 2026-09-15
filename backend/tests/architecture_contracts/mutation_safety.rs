use backend::domain::RetryPolicy;

use crate::support::{architecture_source as source, compact_source as compact};

#[test]
fn transfer_and_upload_application_boundaries_do_not_depend_on_implementation_types() {
    let transfer_port = compact(&source("src/ports/transfer.rs"));
    assert!(
        !transfer_port.contains("crate::transfer"),
        "transfer port contracts must not import transfer implementation types"
    );

    let transfer_application = compact(&source("src/application/transfers.rs"));
    assert!(
        !transfer_application.contains("crate::transfer"),
        "transfer application use cases must depend on ports, not transfer implementation"
    );

    let orchestrator = compact(&source("src/transfer/orchestrator.rs"));
    assert!(
        !orchestrator.contains("legacy_manager("),
        "TransferEngine must not expose its concrete TransferManager"
    );

    let upload = source("src/application/upload.rs");
    let compact_upload = compact(&upload);
    for forbidden in [
        "crate::services::",
        "crate::transfer::",
        "TransferManager",
        "TransferPlanner",
        "execute_inline_upload_stream",
        "UploadLockManager",
    ] {
        assert!(
            !upload.contains(forbidden),
            "upload application must not depend on implementation detail `{forbidden}`"
        );
    }
    assert!(compact_upload.contains("mutations:Arc<dynMutationCoordinator>"));
    assert!(compact_upload.contains("reservations:Arc<dynUploadReservationStore>"));
    assert!(compact_upload.contains("execution:Arc<dynUploadExecution>"));

    let upload_adapter = source("src/infrastructure/uploads.rs");
    assert!(upload_adapter.contains("TransferPlanner"));
    assert!(upload_adapter.contains("TransferManager"));
    assert!(upload_adapter.contains("execute_inline_upload_stream"));
}

#[test]
fn partial_commit_recovery_errors_are_not_retryable() {
    let error = anyhow::anyhow!(
        "Transfer content was written, but chmod failed. Filesystem mutation committed; recovery required"
    );
    assert!(!RetryPolicy::is_anyhow_retryable(&error));
}

#[test]
fn transfer_permissions_are_resolved_strictly_and_applied_before_staged_commit() {
    let engine = source("src/transfer/engine.rs");
    assert!(engine.contains("resolve_destination_permissions_strict"));
    assert!(
        !engine.contains("let _ = dst_fs.set_permissions(&dst_vfs, &perms).await"),
        "destination permission failures must never be swallowed"
    );

    let chmod = engine
        .find("dst_fs.set_permissions(&write_target_vfs, perms)")
        .expect("staged transfer must chmod staging target");
    let rename = engine
        .find("dst_fs.rename(&write_target_vfs, &dst_vfs)")
        .expect("staged transfer must promote staging target");
    assert!(chmod < rename, "staging permission must be applied before rename");
    assert!(engine.contains("Filesystem mutation committed; recovery required"));
}

#[test]
fn archive_extraction_uses_strict_permission_application() {
    let archive = source("src/filesystem/archive.rs");
    assert!(archive.contains("resolve_destination_permissions_strict"));
    assert!(
        !archive.contains("let _ = provider.set_permissions"),
        "archive extraction must not swallow permission application failures"
    );
}

#[test]
fn staged_upload_permissions_precede_commit_and_failure_cleans_staging() {
    let executor = source("src/transfer/executor.rs");
    let permission = executor
        .find("provider.set_permissions(&write_target, perms)")
        .expect("staging permissions must be applied to staging target");
    let rename = executor
        .find("provider.rename(&write_target, &context.target)")
        .expect("staging target must be promoted by rename");

    assert!(
        permission < rename,
        "staged upload must apply inherited permissions before final rename"
    );
    assert!(executor.contains(
        "cleanup_upload_target(provider.as_ref(), &write_target, &context.job_id).await"
    ));
}

#[test]
fn direct_upload_permission_failure_exposes_partial_commit_recovery() {
    let executor = source("src/transfer/executor.rs");
    assert!(
        !executor.contains("upload committed but inherited permissions could not be applied"),
        "permission failure must not be reduced to a warning"
    );
    assert!(executor.contains("Filesystem mutation committed; recovery required"));
}
