use crate::domain::{FileEntry, FileKind, VfsPath};
use crate::errors::VfsError;
use crate::vfs::FileSystem;
use futures::{future::BoxFuture, FutureExt, StreamExt};
use regex::Regex;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;

const SEARCH_DIRECTORY_CONCURRENCY: usize = 8;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SearchOutput {
    pub results: Vec<FileEntry>,
    pub truncated: bool,
    pub total_scanned: usize,
    pub errors: Vec<String>,
}

#[derive(Clone)]
enum SearchMatcher {
    Regex(Regex),
    Contains(String),
}

impl SearchMatcher {
    fn matches(&self, name: &str) -> bool {
        match self {
            Self::Regex(regex) => regex.is_match(name),
            Self::Contains(query_lower) => name.to_lowercase().contains(query_lower),
        }
    }
}

struct DirectoryScan {
    matches: Vec<FileEntry>,
    children: Vec<(VfsPath, usize)>,
    total_scanned: usize,
    errors: Vec<String>,
    hit_limit: bool,
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
    let root_vfs = VfsPath::new(connection_id, start_path)?;
    let matcher = if is_regex {
        SearchMatcher::Regex(
            Regex::new(query)
                .map_err(|e| VfsError::InvalidPath(format!("Invalid regex: {}", e)))?,
        )
    } else {
        SearchMatcher::Contains(query.to_lowercase())
    };

    let mut matches = Vec::new();
    let mut errors = Vec::new();
    let mut total_scanned = 0usize;
    let mut queue: VecDeque<(VfsPath, usize)> = VecDeque::new();
    let mut active: futures::stream::FuturesUnordered<BoxFuture<'static, DirectoryScan>> =
        futures::stream::FuturesUnordered::new();
    let mut truncated = false;

    queue.push_back((root_vfs, 0));

    loop {
        while active.len() < SEARCH_DIRECTORY_CONCURRENCY {
            let Some((directory, depth)) = queue.pop_front() else {
                break;
            };
            if depth > max_depth {
                continue;
            }

            active.push(
                scan_directory(
                    Arc::clone(provider),
                    connection_id.to_owned(),
                    directory,
                    depth,
                    max_depth,
                    matcher.clone(),
                    limit,
                )
                .boxed(),
            );
        }

        let Some(scan) = active.next().await else {
            break;
        };

        total_scanned = total_scanned.saturating_add(scan.total_scanned);
        errors.extend(scan.errors);

        let remaining = limit.saturating_sub(matches.len());
        let found = scan.matches.len();
        matches.extend(scan.matches.into_iter().take(remaining));

        // Preserve the existing contract: reaching the requested result limit marks
        // the response as truncated immediately. Dropping `active` below cancels
        // unfinished directory streams without spawning detached tasks.
        if scan.hit_limit || found > remaining || matches.len() >= limit {
            truncated = true;
            break;
        }

        // Children are appended only after their parent directory finishes. Since
        // directories themselves are taken from the front of this queue, scheduling
        // remains breadth-first while up to SEARCH_DIRECTORY_CONCURRENCY listings may
        // make progress concurrently.
        queue.extend(scan.children);
    }

    Ok(SearchOutput {
        results: matches,
        truncated,
        total_scanned,
        errors,
    })
}

async fn scan_directory(
    provider: Arc<dyn FileSystem>,
    connection_id: String,
    current_dir: VfsPath,
    depth: usize,
    max_depth: usize,
    matcher: SearchMatcher,
    match_limit: usize,
) -> DirectoryScan {
    let mut matches = Vec::new();
    let mut children = Vec::new();
    let mut errors = Vec::new();
    let mut total_scanned = 0usize;
    let mut hit_limit = false;

    let mut stream = match provider.list_stream(&current_dir).await {
        Ok(stream) => stream,
        Err(error) => {
            push_reportable_error(&mut errors, &current_dir, &error);
            return DirectoryScan {
                matches,
                children,
                total_scanned,
                errors,
                hit_limit,
            };
        }
    };

    while let Some(entry_result) = stream.next().await {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                push_reportable_error(&mut errors, &current_dir, &error);
                continue;
            }
        };

        total_scanned = total_scanned.saturating_add(1);

        if matcher.matches(&entry.name) {
            matches.push(entry.clone());
            if matches.len() >= match_limit {
                hit_limit = true;
                break;
            }
        }

        if entry.kind == FileKind::Directory && depth < max_depth {
            if let Ok(child_vfs) = VfsPath::new(&connection_id, &entry.path) {
                children.push((child_vfs, depth + 1));
            }
        }
    }

    DirectoryScan {
        matches,
        children,
        total_scanned,
        errors,
        hit_limit,
    }
}

fn push_reportable_error(errors: &mut Vec<String>, directory: &VfsPath, error: &VfsError) {
    if matches!(
        error,
        VfsError::ConnectionError(_) | VfsError::PermissionDenied(_)
    ) {
        errors.push(format!("{}: {}", directory.path, error));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_matcher_is_case_insensitive() {
        let matcher = SearchMatcher::Contains("readme".to_string());
        assert!(matcher.matches("README.md"));
        assert!(!matcher.matches("notes.txt"));
    }

    #[test]
    fn regex_matcher_preserves_regex_semantics() {
        let matcher = SearchMatcher::Regex(Regex::new(r"^report-\d+\.pdf$").unwrap());
        assert!(matcher.matches("report-42.pdf"));
        assert!(!matcher.matches("draft-report-42.pdf"));
    }

    #[test]
    fn traversal_concurrency_is_explicitly_bounded() {
        assert!(SEARCH_DIRECTORY_CONCURRENCY > 1);
        assert!(SEARCH_DIRECTORY_CONCURRENCY <= 32);
    }
}
