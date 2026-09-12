use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn transfer_port_does_not_depend_on_transfer_implementation() {
    let src = compact(&source("src/ports/transfer.rs"));
    assert!(
        !src.contains("crate::transfer"),
        "transfer port contracts must not import transfer implementation types"
    );
}

#[test]
fn transfer_application_does_not_depend_on_transfer_implementation() {
    let src = compact(&source("src/application/transfers.rs"));
    assert!(
        !src.contains("crate::transfer"),
        "transfer application use cases must depend on ports, not transfer implementation"
    );
}

#[test]
fn transfer_engine_has_no_legacy_manager_escape_hatch() {
    let src = compact(&source("src/transfer/orchestrator.rs"));
    assert!(
        !src.contains("legacy_manager("),
        "TransferEngine must not expose its concrete TransferManager"
    );
}
