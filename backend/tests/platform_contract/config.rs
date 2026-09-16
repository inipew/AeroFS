use backend::config::AppConfig;
use std::fs;

#[test]
fn config_file_loading_validation_and_sanitized_export_are_consistent() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("aerofs_custom.toml");
    fs::write(
        &config_path,
        r#"
[server]
host = "0.0.0.0"
port = 9090

[filesystem]
default_local_root = "/tmp/aerofs_test_storage"
show_hidden_default = true
read_only_default = false

[limits]
max_upload_size = 524288000
max_editable_size = 5242880
max_preview_size = 10485760
max_directory_entries = 10000
max_concurrent_transfers = 8

[security]
session_secret = "custom_secret_key_that_is_long_enough_for_security_123"
session_ttl_secs = 3600
allow_symlinks_outside_root = true
allow_private_network_connections = false

[database]
url = "sqlite:///tmp/aerofs_test.db?mode=rwc"
"#,
    )
    .unwrap();

    let config = AppConfig::load(Some(&config_path)).unwrap();
    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.server.port, 9090);
    assert_eq!(
        config.filesystem.default_local_root.to_str().unwrap(),
        "/tmp/aerofs_test_storage"
    );
    assert!(config.filesystem.show_hidden_default);
    assert_eq!(config.limits.max_concurrent_transfers, 8);
    assert!(config.security.allow_symlinks_outside_root);
    assert!(!config.security.allow_private_network_connections);

    let sanitized = config.to_sanitized_toml();
    assert!(sanitized.contains("********"));
    assert!(!sanitized.contains("custom_secret_key_that_is_long_enough"));

    let mut invalid = config;
    invalid.server.port = 0;
    assert!(invalid.validate().is_err());
}

#[test]
fn config_provenance_descriptors_and_key_lookup_match_defaults() {
    let config = AppConfig::default();
    let provenance = config.get_effective_provenance(None);
    assert!(provenance
        .iter()
        .any(|entry| entry.key == "server.port" && entry.value == "8080"));
    assert!(provenance
        .iter()
        .any(|entry| entry.key == "server.host" && entry.value == "127.0.0.1"));

    let descriptor = AppConfig::describe_key("server.port").unwrap();
    assert_eq!(descriptor.key, "server.port");
    assert_eq!(descriptor.value_type, "u16");
    assert_eq!(descriptor.default_value, "8080");
    assert_eq!(config.get_by_key_path("server.port").as_deref(), Some("8080"));
}
