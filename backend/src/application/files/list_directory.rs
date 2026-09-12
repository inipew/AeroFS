use crate::domain::{
    Actor, ConnectionId, DirectoryListing, FileEntry, FileKind, SortField, SortOrder, VfsPath,
};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
    settings::FileSettings,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

const CURSOR_VERSION: u8 = 1;

#[derive(Debug, Clone)]
pub struct ListDirectoryCommand {
    pub connection: ConnectionId,
    pub path: Option<String>,
    pub show_hidden: Option<bool>,
    pub sort: Option<SortField>,
    pub order: Option<SortOrder>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone)]
pub struct ListDirectory {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    settings: Arc<dyn FileSettings>,
}

impl ListDirectory {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        settings: Arc<dyn FileSettings>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            settings,
        }
    }

    pub async fn execute(
        &self,
        actor: &Actor,
        command: ListDirectoryCommand,
    ) -> Result<DirectoryListing, AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::List)
            .await?;
        let provider = self.filesystem.resolve(&command.connection).await?;
        let vfs_path = VfsPath::new(
            command.connection.as_str(),
            command.path.unwrap_or_else(|| "/".to_owned()),
        )?;
        let show_hidden = match command.show_hidden {
            Some(value) => value,
            None => self.settings.show_hidden_default().await?,
        };
        let limit = command
            .limit
            .unwrap_or(self.settings.max_directory_entries())
            .clamp(1, 1000);
        let sort = command.sort.unwrap_or(SortField::Name);
        let order = command.order.unwrap_or(SortOrder::Asc);
        let cursor = command
            .cursor
            .as_deref()
            .map(decode_cursor)
            .transpose()?
            .map(|cursor| {
                validate_cursor(
                    cursor,
                    &command.connection,
                    &vfs_path.path,
                    show_hidden,
                    sort,
                    order,
                )
            })
            .transpose()?;

        // Provider streams are not guaranteed to arrive in requested sort order, so the
        // stream still has to be visited once. Keep only the best `limit + 1` entries after
        // the keyset cursor instead of materializing and sorting the whole directory.
        // Memory is therefore O(limit), while page boundaries no longer depend on mutable
        // numeric offsets or provider traversal order.
        let mut page = BinaryHeap::with_capacity(limit.saturating_add(2));
        let mut total_count = 0usize;
        let mut stream = provider.list_stream(&vfs_path).await?;
        while let Some(result) = stream.next().await {
            let entry = result?;
            if is_internal_entry(&entry.name) || (!show_hidden && entry.is_hidden) {
                continue;
            }
            total_count = total_count.saturating_add(1);

            if let Some(cursor) = cursor.as_ref() {
                if compare_entry_to_key(&entry, &cursor.last, sort, order) != Ordering::Greater {
                    continue;
                }
            }

            page.push(PageCandidate { entry, sort, order });
            if page.len() > limit.saturating_add(1) {
                page.pop();
            }
        }

        let mut entries: Vec<FileEntry> = page.into_iter().map(|candidate| candidate.entry).collect();
        sort_entries(&mut entries, sort, order);
        let has_more = entries.len() > limit;
        if has_more {
            entries.truncate(limit);
        }

        let next_cursor = if has_more {
            entries.last().map(|entry| {
                encode_cursor(&DirectoryCursor {
                    version: CURSOR_VERSION,
                    connection_id: command.connection.to_string(),
                    path: vfs_path.path.clone(),
                    show_hidden,
                    sort,
                    order,
                    last: CursorKey::from_entry(entry),
                })
            }).transpose()?
        } else {
            None
        };

        Ok(DirectoryListing {
            path: vfs_path.path,
            connection_id: command.connection.to_string(),
            total_count: Some(total_count),
            has_more,
            next_cursor,
            entries,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirectoryCursor {
    version: u8,
    connection_id: String,
    path: String,
    show_hidden: bool,
    sort: SortField,
    order: SortOrder,
    last: CursorKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CursorKey {
    is_directory: bool,
    size: u64,
    modified: i64,
    name_lower: String,
    name: String,
    path: String,
}

impl CursorKey {
    fn from_entry(entry: &FileEntry) -> Self {
        Self {
            is_directory: entry.kind == FileKind::Directory,
            size: entry.size.unwrap_or(0),
            modified: entry
                .modified_at
                .map(|value| value.timestamp())
                .unwrap_or(0),
            name_lower: entry.name.to_lowercase(),
            name: entry.name.clone(),
            path: entry.path.clone(),
        }
    }
}

struct PageCandidate {
    entry: FileEntry,
    sort: SortField,
    order: SortOrder,
}

impl PartialEq for PageCandidate {
    fn eq(&self, other: &Self) -> bool {
        compare_entries(&self.entry, &other.entry, self.sort, self.order) == Ordering::Equal
    }
}

impl Eq for PageCandidate {}

impl PartialOrd for PageCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PageCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_entries(&self.entry, &other.entry, self.sort, self.order)
    }
}

fn is_internal_entry(name: &str) -> bool {
    name.contains(".aerofs-part-") || name.contains(".aerofs.part") || name.contains(".aerofs.tmp")
}

fn decode_cursor(value: &str) -> Result<DirectoryCursor, AppError> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| AppError::BadRequest("Invalid directory cursor encoding".to_string()))?;
    serde_json::from_slice::<DirectoryCursor>(&bytes)
        .map_err(|_| AppError::BadRequest("Invalid directory cursor payload".to_string()))
}

fn encode_cursor(cursor: &DirectoryCursor) -> Result<String, AppError> {
    use base64::Engine;
    let bytes = serde_json::to_vec(cursor)
        .map_err(|error| AppError::Internal(anyhow::anyhow!("failed to encode directory cursor: {error}")))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn validate_cursor(
    cursor: DirectoryCursor,
    connection: &ConnectionId,
    path: &str,
    show_hidden: bool,
    sort: SortField,
    order: SortOrder,
) -> Result<DirectoryCursor, AppError> {
    if cursor.version != CURSOR_VERSION {
        return Err(AppError::BadRequest("Unsupported directory cursor version".to_string()));
    }
    if cursor.connection_id != connection.as_str()
        || cursor.path != path
        || cursor.show_hidden != show_hidden
        || cursor.sort != sort
        || cursor.order != order
    {
        return Err(AppError::BadRequest(
            "Directory cursor does not match this listing request".to_string(),
        ));
    }
    Ok(cursor)
}

fn compare_entry_to_key(entry: &FileEntry, key: &CursorKey, field: SortField, order: SortOrder) -> Ordering {
    compare_keys(&CursorKey::from_entry(entry), key, field, order)
}

fn compare_entries(a: &FileEntry, b: &FileEntry, field: SortField, order: SortOrder) -> Ordering {
    compare_keys(&CursorKey::from_entry(a), &CursorKey::from_entry(b), field, order)
}

fn compare_keys(a: &CursorKey, b: &CursorKey, field: SortField, order: SortOrder) -> Ordering {
    let dirs = a.is_directory.cmp(&b.is_directory);
    if dirs != Ordering::Equal {
        return dirs.reverse();
    }

    let primary = match field {
        SortField::Size => a.size.cmp(&b.size),
        SortField::Modified => a.modified.cmp(&b.modified),
        SortField::Name => a.name_lower.cmp(&b.name_lower),
    };
    let primary = if order == SortOrder::Desc {
        primary.reverse()
    } else {
        primary
    };
    if primary != Ordering::Equal {
        return primary;
    }

    a.name_lower
        .cmp(&b.name_lower)
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.path.cmp(&b.path))
}

fn sort_entries(entries: &mut [FileEntry], field: SortField, order: SortOrder) {
    entries.sort_by(|a, b| compare_entries(a, b, field, order));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            path: format!("/{name}"),
            kind: FileKind::File,
            size: None,
            modified_at: None,
            permissions: None,
            mime_type: None,
            is_hidden: false,
            symlink_target: None,
        }
    }

    fn cursor_for(entry: &FileEntry) -> DirectoryCursor {
        DirectoryCursor {
            version: CURSOR_VERSION,
            connection_id: "local".to_string(),
            path: "/".to_string(),
            show_hidden: false,
            sort: SortField::Name,
            order: SortOrder::Asc,
            last: CursorKey::from_entry(entry),
        }
    }

    #[test]
    fn keyset_cursor_survives_insert_before_boundary() {
        let boundary = file("b");
        let cursor = cursor_for(&boundary);
        let mut entries = vec![file("aa"), file("a"), file("b"), file("m"), file("z")];
        entries.retain(|entry| {
            compare_entry_to_key(entry, &cursor.last, SortField::Name, SortOrder::Asc)
                == Ordering::Greater
        });
        sort_entries(&mut entries, SortField::Name, SortOrder::Asc);
        let names: Vec<_> = entries.into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["m", "z"]);
    }

    #[test]
    fn malformed_cursor_is_rejected_instead_of_resetting_to_page_one() {
        let error = decode_cursor("definitely-not-a-cursor").unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn cursor_is_bound_to_listing_context() {
        let encoded = encode_cursor(&cursor_for(&file("b"))).unwrap();
        let decoded = decode_cursor(&encoded).unwrap();
        let error = validate_cursor(
            decoded,
            &ConnectionId::local(),
            "/different",
            false,
            SortField::Name,
            SortOrder::Asc,
        )
        .unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn bounded_heap_keeps_only_next_page_candidates() {
        let limit = 100usize;
        let mut heap = BinaryHeap::new();
        for i in (0..2_000).rev() {
            heap.push(PageCandidate {
                entry: file(&format!("file-{i:04}")),
                sort: SortField::Name,
                order: SortOrder::Asc,
            });
            if heap.len() > limit + 1 {
                heap.pop();
            }
        }
        assert_eq!(heap.len(), limit + 1);
        let mut entries: Vec<_> = heap.into_iter().map(|candidate| candidate.entry).collect();
        sort_entries(&mut entries, SortField::Name, SortOrder::Asc);
        assert_eq!(entries.first().unwrap().name, "file-0000");
        assert_eq!(entries.last().unwrap().name, "file-0100");
    }

    #[test]
    fn equal_primary_keys_have_provider_independent_order() {
        let mut first = vec![file("A"), file("a")];
        let mut second = vec![file("a"), file("A")];

        sort_entries(&mut first, SortField::Name, SortOrder::Asc);
        sort_entries(&mut second, SortField::Name, SortOrder::Asc);

        let first_names: Vec<_> = first.into_iter().map(|entry| entry.name).collect();
        let second_names: Vec<_> = second.into_iter().map(|entry| entry.name).collect();
        assert_eq!(first_names, second_names);
    }
}
