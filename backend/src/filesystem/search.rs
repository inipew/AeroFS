use crate::domain::{FileEntry, FileKind, VfsPath};
use crate::errors::VfsError;
use crate::vfs::FileSystem;
use regex::Regex;
use std::collections::VecDeque;
use std::sync::Arc;

pub async fn search_recursive(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    start_path: &str,
    query: &str,
    is_regex: bool,
    max_depth: usize,
) -> Result<Vec<FileEntry>, VfsError> {
    let mut matches = Vec::new();
    let mut queue: VecDeque<(VfsPath, usize)> = VecDeque::new();

    let root_vfs = VfsPath::new(connection_id, start_path);
    queue.push_back((root_vfs, 0));

    let regex_matcher = if is_regex {
        Some(Regex::new(query).map_err(|e| VfsError::InvalidPath(format!("Invalid regex: {}", e)))?)
    } else {
        None
    };
    let query_lower = query.to_lowercase();

    while let Some((current_dir, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }

        let entries = match provider.list(&current_dir).await {
            Ok(e) => e,
            Err(_) => continue, // Skip unreadable directories
        };

        for entry in entries {
            let is_match = if let Some(re) = &regex_matcher {
                re.is_match(&entry.name)
            } else {
                entry.name.to_lowercase().contains(&query_lower)
            };

            if is_match {
                matches.push(entry.clone());
            }

            // If directory, enqueue child search
            if entry.kind == FileKind::Directory && depth < max_depth {
                let child_vfs = VfsPath::new(connection_id, &entry.path);
                queue.push_back((child_vfs, depth + 1));
            }
        }
    }

    Ok(matches)
}
