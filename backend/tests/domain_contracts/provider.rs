use backend::domain::Capabilities;

#[test]
fn local_provider_defaults_expose_required_filesystem_capabilities() {
    let capabilities = Capabilities::local_default();

    assert!(capabilities.atomic_write);
    assert!(capabilities.atomic_rename);
    assert!(capabilities.permissions);
    assert!(capabilities.range_read);
}
