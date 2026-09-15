use backend::vfs::{factory::ProviderFactory, registry::ProviderRegistry};
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn concurrent_permits_share_one_registered_storage_runtime() {
    let temp = tempfile::tempdir().unwrap();
    let provider = ProviderFactory::build_local("local", temp.path().to_path_buf()).unwrap();
    let registry = ProviderRegistry::new();
    registry.register("local".into(), provider).await;

    let runtime = registry.get_runtime("local").await.unwrap();
    assert_eq!(runtime.connection_id, "local");
    assert!(runtime.capabilities().read);

    let mut tasks = Vec::new();
    for _ in 0..10 {
        let runtime = Arc::clone(&runtime);
        tasks.push(tokio::spawn(async move {
            let _permit = runtime.acquire_permit().await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }

    let same_runtime = registry.get_runtime("local").await.unwrap();
    assert!(Arc::ptr_eq(&runtime, &same_runtime));
}
