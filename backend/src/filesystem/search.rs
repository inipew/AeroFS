use crate::domain::{FileEntry, FileKind, VfsPath};
use crate::errors::VfsError;
use crate::vfs::FileSystem;
use regex::Regex;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SearchOutput {
    pub results: Vec<FileEntry>,
    pub truncated: bool,
    pub total_scanned: usize,
    pub errors: Vec<String>,
}

pub async fn search_recursive(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    start_path: &str,
    query: &str,
    is_regex: bool,
    max_depth: usize,
    limit: usize,
) -> Result<SearchOutput, VfsError> {
    let mut matches = Vec::new();
    let mut errors = Vec::new();
    let mut total_scanned = 0;
    let mut queue: VecDeque<(VfsPath, usize)> = VecDeque::new();

    let root_vfs = VfsPath::new(connection_id, start_path)?;
    queue.push_back((root_vfs, 0));

    let regex_matcher = if is_regex {
        Some(
            Regex::new(query)
                .map_err(|e| VfsError::InvalidPath(format!("Invalid regex: {}", e)))?,
        )
    } else {
        None
    };
    let query_lower = query.to_lowercase();
    let mut truncated = false;

    while let Some((current_dir, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }

        let entries = match provider.list(&current_dir).await {
            Ok(e) => e,
            Err(e) => {
                match e {
                    VfsError::ConnectionError(_) | VfsError::PermissionDenied(_) => {
                        errors.push(format!("{}: {}", current_dir.path, e));
                    }
                    _ => {}
                }
                continue;
            }
        };

        total_scanned += entries.len();

        for entry in entries {
            let is_match = if let Some(re) = &regex_matcher {
                re.is_match(&entry.name)
            } else {
                entry.name.to_lowercase().contains(&query_lower)
            };

            if is_match {
                matches.push(entry.clone());
                if matches.len() >= limit {
                    truncated = true;
                    break;
                }
            }

            // If directory, enqueue child search
            if entry.kind == FileKind::Directory && depth < max_depth {
                if let Ok(child_vfs) = VfsPath::new(connection_id, &entry.path) {
                    queue.push_back((child_vfs, depth + 1));
                }
            }
        }

        if truncated {
            break;
        }
    }

    Ok(SearchOutput {
        results: matches,
        truncated,
        total_scanned,
        errors,
    })
}
