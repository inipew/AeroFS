use backend::{
    domain::{OperationKind, RetryPolicy},
    errors::{AppError, VfsError},
};

#[test]
fn retry_policy_respects_operation_idempotency_and_checksum_retry_budget() {
    let policy = RetryPolicy::new(3);
    let timeout = AppError::Vfs(VfsError::Timeout("connection timed out".into()));
    let checksum = AppError::ChecksumMismatch("mismatched sha256".into());

    for operation in [OperationKind::Read, OperationKind::Stat, OperationKind::List] {
        assert!(policy.is_retryable_for_operation(operation, &timeout, 1));
    }
    assert!(!policy.is_retryable_for_operation(
        OperationKind::Append,
        &timeout,
        1
    ));

    assert!(policy.is_retryable_for_operation(OperationKind::Read, &checksum, 1));
    assert!(!policy.is_retryable_for_operation(OperationKind::Read, &checksum, 2));
    assert!(!policy.is_retryable_for_operation(OperationKind::Read, &checksum, 3));
}
