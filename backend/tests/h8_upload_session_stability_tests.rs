use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn admitted_upload_session_pins_the_provider_generation() {
    let port = compact(&source("src/ports/upload.rs"));
    let upload = compact(&source("src/application/upload.rs"));

    assert!(
        port.contains("provider:Arc<dynFileSystem>"),
        "UploadSession must retain the exact provider used during admission"
    );
    assert!(upload.contains("provider,"));
    assert!(
        upload.contains("execute_inline(session.provider.clone(),"),
        "reserved uploads must execute against their admitted provider generation"
    );

    let execute_reserved = upload
        .split("pubasyncfnexecute_session_stream")
        .nth(1)
        .expect("reserved upload execution must exist")
        .split("pubasyncfnexecute_inline_stream")
        .next()
        .unwrap();
    assert!(
        !execute_reserved.contains("self.filesystem.resolve("),
        "reserved upload execution must not re-resolve a possibly replaced provider"
    );
}
