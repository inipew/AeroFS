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

pub fn async_function<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("pub async fn {name}");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("async function `{name}` not found"));
    let item = &source[start..];
    let body_start = item
        .find('{')
        .unwrap_or_else(|| panic!("async function `{name}` has no body"));

    let mut depth = 0usize;
    for (offset, byte) in item.as_bytes()[body_start..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth
                    .checked_sub(1)
                    .unwrap_or_else(|| panic!("async function `{name}` has unbalanced braces"));
                if depth == 0 {
                    return &item[..body_start + offset + 1];
                }
            }
            _ => {}
        }
    }

    panic!("async function `{name}` has an unterminated body")
}
