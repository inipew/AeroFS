use std::{fs, path::PathBuf};

pub fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path))
        .unwrap_or_else(|error| panic!("failed to read architecture source {path}: {error}"))
}

pub fn compact(source: &str) -> String {
    source.chars().filter(|character| !character.is_whitespace()).collect()
}

pub fn source_exists(path: &str) -> bool {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path).exists()
}
