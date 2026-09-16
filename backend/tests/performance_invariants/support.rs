use std::{fs, path::PathBuf};

pub fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path))
        .unwrap_or_else(|error| panic!("failed to read performance invariant source '{path}': {error}"))
}

pub fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("performance invariant start marker missing: {start}"));
    let rest = &source[start_index..];
    let end_index = rest
        .find(end)
        .unwrap_or_else(|| panic!("performance invariant end marker missing: {end}"));
    &rest[..end_index]
}

pub fn section_from<'a>(source: &'a str, start: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("performance invariant start marker missing: {start}"));
    &source[start_index..]
}
