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
use std::cmp::Ordering;
use std::sync::Arc;

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
        let offset = decode_cursor(command.cursor.as_deref());
        let limit = command
            .limit
            .unwrap_or(self.settings.max_directory_entries())
            .clamp(1, 1000);
        let sort = command.sort.unwrap_or(SortField::Name);
        let order = command.order.unwrap_or(SortOrder::Asc);

        // Provider streams are not required to arrive in the requested sort order.
        // Pagination therefore has to operate on the globally filtered + sorted
        // result set; slicing the provider stream first makes page boundaries depend
        // on provider traversal order and can duplicate/skip logical ordering.
        let mut entries = Vec::new();
        let mut stream = provider.list_stream(&vfs_path).await?;
        while let Some(result) = stream.next().await {
            let entry = result?;
            if is_internal_entry(&entry.name) || (!show_hidden && entry.is_hidden) {
                continue;
            }
            entries.push(entry);
        }

        let total_count = entries.len();
        sort_entries(&mut entries, sort, order);

        let page_start = offset.min(total_count);
        let page_end = page_start.saturating_add(limit).min(total_count);
        let has_more = page_end < total_count;
        let entries = entries[page_start..page_end].to_vec();
        let next_cursor = has_more.then(|| encode_cursor(page_end));

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

fn is_internal_entry(name: &str) -> bool {
    name.contains(".aerofs-part-") || name.contains(".aerofs.part") || name.contains(".aerofs.tmp")
}

fn decode_cursor(cursor: Option<&str>) -> usize {
    cursor
        .and_then(|value| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, value)
                .ok()
                .and_then(|bytes| {
                    serde_json::from_slice::<serde_json::Value>(&bytes)
                        .ok()
                        .and_then(|value| value["offset"].as_u64())
                        .or_else(|| String::from_utf8_lossy(&bytes).parse().ok())
                })
                .or_else(|| value.parse().ok())
        })
        .unwrap_or(0) as usize
}

fn encode_cursor(offset: usize) -> String {
    base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        serde_json::json!({"offset": offset}).to_string(),
    )
}

fn sort_entries(entries: &mut [FileEntry], field: SortField, order: SortOrder) {
    entries.sort_by(|a, b| {
        let dirs = (a.kind == FileKind::Directory).cmp(&(b.kind == FileKind::Directory));
        if dirs != Ordering::Equal {
            return dirs.reverse();
        }

        let primary = match field {
            SortField::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortField::Modified => a
                .modified_at
                .map(|d| d.timestamp())
                .unwrap_or(0)
                .cmp(&b.modified_at.map(|d| d.timestamp()).unwrap_or(0)),
            SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        };
        let primary = if order == SortOrder::Desc {
            primary.reverse()
        } else {
            primary
        };
        if primary != Ordering::Equal {
            return primary;
        }

        // Provider iteration order must never decide pagination order for equal
        // primary keys. Use canonical name/path tie-breakers so repeated requests
        // produce the same page boundaries across providers and runs.
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });
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

    fn page(
        mut entries: Vec<FileEntry>,
        offset: usize,
        limit: usize,
    ) -> (Vec<String>, usize, bool, Option<String>) {
        let total_count = entries.len();
        sort_entries(&mut entries, SortField::Name, SortOrder::Asc);
        let start = offset.min(total_count);
        let end = start.saturating_add(limit).min(total_count);
        let has_more = end < total_count;
        let names = entries[start..end]
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        (names, total_count, has_more, has_more.then(|| encode_cursor(end)))
    }

    #[test]
    fn pagination_slices_after_global_sort() {
        let provider_order = vec![file("z"), file("a"), file("m"), file("b")];

        let (first, total, has_more, cursor) = page(provider_order.clone(), 0, 2);
        assert_eq!(first, vec!["a", "b"]);
        assert_eq!(total, 4);
        assert!(has_more);

        let offset = decode_cursor(cursor.as_deref());
        let (second, total, has_more, cursor) = page(provider_order, offset, 2);
        assert_eq!(second, vec!["m", "z"]);
        assert_eq!(total, 4);
        assert!(!has_more);
        assert!(cursor.is_none());
    }

    #[test]
    fn total_count_is_not_page_length() {
        let entries = (0..2_000)
            .rev()
            .map(|i| file(&format!("file-{i:04}")))
            .collect();

        let (page, total, has_more, _) = page(entries, 0, 100);
        assert_eq!(page.len(), 100);
        assert_eq!(total, 2_000);
        assert!(has_more);
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
