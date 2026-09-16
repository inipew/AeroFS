use backend::{
    infrastructure::CredentialStore,
    vfs::{factory::ProviderFactory, registry::ProviderRegistry},
};

#[tokio::test]
async fn credential_store_round_trips_without_persisting_plaintext() {
    let store = CredentialStore::new("test_secret_passphrase_1234567890");
    let secret = "s3_access_secret_key_123456";

    let encrypted = store.encrypt(secret).unwrap();
    assert_ne!(encrypted, secret);
    assert_eq!(store.decrypt(&encrypted).unwrap(), secret);
}

#[tokio::test]
async fn local_provider_factory_and_registry_support_runtime_lifecycle() {
    let registry = ProviderRegistry::new();
    let temp = tempfile::tempdir().unwrap();
    let provider = ProviderFactory::build_local("local-contract", temp.path().to_path_buf()).unwrap();
    assert!(provider.capabilities().read);

    assert!(!registry.contains("local-contract").await);
    registry
        .register("local-contract".to_string(), provider)
        .await;
    assert!(registry.contains("local-contract").await);
    assert_eq!(registry.list_ids().await, vec!["local-contract"]);

    registry
        .set_connection_error("local-contract", "Connection timed out")
        .await;
    assert_eq!(
        registry.get_connection_error("local-contract").await.as_deref(),
        Some("Connection timed out")
    );

    registry.remove("local-contract").await;
    assert!(!registry.contains("local-contract").await);
}
