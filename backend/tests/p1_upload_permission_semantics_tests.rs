use std::fs;

#[test]
fn staged_upload_applies_permissions_before_commit() {
    let source = fs::read_to_string("src/transfer/executor.rs").expect("read upload executor");
    let permission = source
        .find("provider.set_permissions(&write_target, perms)")
        .expect("staging permissions must be applied to staging target");
    let rename = source
        .find("provider.rename(&write_target, &context.target)")
        .expect("staging target must be promoted by rename");

    assert!(
        permission < rename,
        "staged upload must apply inherited permissions before the final rename"
    );
    assert!(
        source.contains("cleanup_upload_target(provider.as_ref(), &write_target, &context.job_id).await"),
        "permission failure on staging must clean the staging artifact"
    );
}

#[test]
fn direct_upload_permission_failure_is_not_silent_success() {
    let source = fs::read_to_string("src/transfer/executor.rs").expect("read upload executor");

    assert!(
        !source.contains("upload committed but inherited permissions could not be applied"),
        "upload executor must not reduce permission failure to a warning"
    );
    assert!(
        source.contains("Filesystem mutation committed; recovery required"),
        "direct upload must expose partial-commit recovery semantics when chmod fails"
    );
}
