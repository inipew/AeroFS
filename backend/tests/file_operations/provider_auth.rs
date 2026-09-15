use backend::{
    domain::SftpAuth,
    errors::VfsError,
    vfs::opendal::builder::build_sftp_operator_with_config,
};

#[test]
fn sftp_password_authentication_is_rejected_with_actionable_error() {
    let auth = SftpAuth::Password {
        password: "secret-password".into(),
    };

    let error = build_sftp_operator_with_config(
        "127.0.0.1",
        22,
        Some("user"),
        Some(&auth),
        None,
        None,
    )
    .expect_err("password auth must be rejected before building an SFTP operator");

    match error {
        VfsError::NotSupported(message) => {
            assert!(message.contains("SFTP password authentication is not natively supported"));
        }
        other => panic!("expected VfsError::NotSupported, got {other:?}"),
    }
}
