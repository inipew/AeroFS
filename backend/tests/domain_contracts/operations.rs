use backend::domain::{
    operation::{
        FailureStrategy, OperationExecutionResult, OperationIntentType, OperationPlan,
        OperationStatus,
    },
    policy::PermissionInheritanceMode,
};

#[test]
fn operation_execution_result_finalizes_partial_when_some_items_fail() {
    let plan = OperationPlan {
        id: "plan-123".to_string(),
        intent_type: OperationIntentType::Copy,
        source_connection_id: "local".to_string(),
        source_paths: vec![],
        destination_connection_id: Some("local".to_string()),
        destination_path: None,
        failure_strategy: FailureStrategy::ContinueOnFailure,
        permission_mode: PermissionInheritanceMode::InheritParent,
        overwrite_mode: None,
    };
    assert_eq!(plan.failure_strategy, FailureStrategy::ContinueOnFailure);

    let mut result = OperationExecutionResult::new(plan.id.clone(), 3);
    assert_eq!(result.status, OperationStatus::Executing);
    result.succeeded_items.push("file1.txt".to_string());
    result
        .failed_items
        .push(("file2.txt".to_string(), "Permission denied".to_string()));
    result.succeeded_items.push("file3.txt".to_string());
    assert!(!result.is_success());

    result.finalize();
    assert_eq!(result.status, OperationStatus::Partial);
}
