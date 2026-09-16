use super::performance_support::{section, source};

#[test]
fn inline_uploads_share_transfer_resource_budget() {
    let uploads = source("src/infrastructure/uploads.rs");
    let execute = section(
        &uploads,
        "async fn execute_inline(",
        "async fn complete_inline_job",
    );

    assert!(
        uploads.contains("resource_budget: Arc<ResourceBudget>"),
        "performance regression: inline upload adapter must retain the shared ResourceBudget"
    );
    assert!(
        execute.contains("ResourceClass::TransferMixed"),
        "performance regression: HTTP -> local uploads must consume local + network + transfer capacity"
    );
    assert!(
        execute.contains("ResourceClass::TransferNetwork"),
        "performance regression: HTTP -> remote uploads must consume network + transfer capacity"
    );
    assert!(
        execute.contains("self.resource_budget.acquire(resource_class)"),
        "performance regression: inline upload execution must acquire the shared admission budget"
    );
    assert!(
        execute.contains("cancel_token.cancelled()"),
        "performance regression: waiting for resource capacity must remain cancellation-aware"
    );
}

#[test]
fn bootstrap_wires_the_same_budget_into_queued_and_inline_transfers() {
    let bootstrap = source("src/bootstrap.rs");

    assert!(
        bootstrap.contains(
            "TransferManager::new(\n        registry.providers_map(),\n        db.clone(),\n        config.limits.max_concurrent_transfers,\n        resource_budget.clone(),"
        ),
        "performance regression: queued transfers must use the system ResourceBudget"
    );
    assert!(
        bootstrap.contains(
            "TransferUploadExecution::new(\n        transfer_manager.clone(),\n        resource_budget.clone(),"
        ),
        "performance regression: inline uploads must receive the exact shared ResourceBudget"
    );
}
