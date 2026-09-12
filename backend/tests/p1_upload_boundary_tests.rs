use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn upload_application_depends_on_ports_not_service_or_transfer_implementation() {
    let upload = source("src/application/upload.rs");
    let compact = compact(&upload);

    for forbidden in [
        "crate::services::",
        "crate::transfer::",
        "TransferManager",
        "TransferPlanner",
        "execute_inline_upload_stream",
        "UploadLockManager",
    ] {
        assert!(
            !upload.contains(forbidden),
            "upload application must not depend on implementation detail `{forbidden}`"
        );
    }

    assert!(compact.contains("mutations:Arc<dynMutationCoordinator>"));
    assert!(compact.contains("reservations:Arc<dynUploadReservationStore>"));
    assert!(compact.contains("execution:Arc<dynUploadExecution>"));
}

#[test]
fn transfer_specific_upload_planning_lives_in_infrastructure_adapter() {
    let adapter = source("src/infrastructure/uploads.rs");
    assert!(adapter.contains("TransferPlanner"));
    assert!(adapter.contains("TransferManager"));
    assert!(adapter.contains("execute_inline_upload_stream"));
}
