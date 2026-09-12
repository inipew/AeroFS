use backend::domain::RetryPolicy;
use std::fs;

#[test]
fn partial_commit_recovery_errors_are_never_retried() {
    let error = anyhow::anyhow!(
        "Transfer content was written, but chmod failed. Filesystem mutation committed; recovery required"
    );
    assert!(!RetryPolicy::is_anyhow_retryable(&error));
}

#[test]
fn transfer_permission_snapshot_is_strict_and_staging_is_chmodded_before_rename() {
    let source = fs::read_to_string("src/transfer/engine.rs").expect("read transfer engine");

    assert!(
        source.contains("resolve_destination_permissions_strict"),
        "transfer engine must use strict destination permission lookup"
    );
    assert!(
        !source.contains("let _ = dst_fs.set_permissions(&dst_vfs, &perms).await"),
        "transfer engine must not swallow destination permission failures"
    );

    let chmod = source
        .find("dst_fs.set_permissions(&write_target_vfs, perms)")
        .expect("staged transfer must chmod staging target");
    let rename = source
        .find("dst_fs.rename(&write_target_vfs, &dst_vfs)")
        .expect("staged transfer must promote staging target");
    assert!(chmod < rename, "staging permission must be applied before rename");

    assert!(
        source.contains("Filesystem mutation committed; recovery required"),
        "direct transfer chmod failure must expose partial-commit recovery semantics"
    );
}

#[test]
fn archive_extraction_does_not_use_best_effort_permission_calls() {
    let source = fs::read_to_string("src/filesystem/archive.rs").expect("read archive implementation");

    assert!(
        source.contains("resolve_destination_permissions_strict"),
        "archive extraction must use strict permission lookup"
    );
    assert!(
        !source.contains("let _ = provider.set_permissions"),
        "archive extraction must not swallow permission application failures"
    );
}
