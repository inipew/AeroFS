use crate::domain::{Actor, ConnectionId, DirectoryListing, FileKind, SortField, SortOrder, VfsPath};
use crate::errors::AppError;
use crate::ports::{authorization::{Authorization, FileAction}, filesystem::FileSystemResolver, settings::FileSettings};
use futures::StreamExt;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ListDirectoryCommand {
    pub connection: ConnectionId, pub path: Option<String>, pub show_hidden: Option<bool>,
    pub sort: Option<SortField>, pub order: Option<SortOrder>, pub cursor: Option<String>, pub limit: Option<usize>,
}

#[derive(Clone)]
pub struct ListDirectory {
    authorization: Arc<dyn Authorization>, filesystem: Arc<dyn FileSystemResolver>, settings: Arc<dyn FileSettings>,
}

impl ListDirectory {
    pub fn new(authorization: Arc<dyn Authorization>, filesystem: Arc<dyn FileSystemResolver>, settings: Arc<dyn FileSettings>) -> Self { Self { authorization, filesystem, settings } }

    pub async fn execute(&self, actor: &Actor, command: ListDirectoryCommand) -> Result<DirectoryListing, AppError> {
        self.authorization.authorize(actor, &command.connection, FileAction::List).await?;
        let provider = self.filesystem.resolve(&command.connection).await?;
        let vfs_path = VfsPath::new(command.connection.as_str(), command.path.unwrap_or_else(|| "/".to_owned()))?;
        let show_hidden = match command.show_hidden { Some(value) => value, None => self.settings.show_hidden_default().await? };
        let offset = decode_cursor(command.cursor.as_deref());
        let limit = command.limit.unwrap_or(self.settings.max_directory_entries()).clamp(1, 1000);
        let mut stream = provider.list_stream(&vfs_path).await?;
        let mut entries = Vec::new(); let mut seen = 0; let mut has_more = false;
        while let Some(result) = stream.next().await {
            let entry = result?;
            if is_internal_entry(&entry.name) || (!show_hidden && entry.is_hidden) { continue; }
            if seen < offset { seen += 1; continue; }
            if entries.len() == limit { has_more = true; break; }
            entries.push(entry); seen += 1;
        }
        let sort = command.sort.unwrap_or(SortField::Name); let order = command.order.unwrap_or(SortOrder::Asc);
        sort_entries(&mut entries, sort, order);
        Ok(DirectoryListing { path: vfs_path.path, connection_id: command.connection.to_string(), total_count: Some(entries.len()), has_more, next_cursor: has_more.then(|| encode_cursor(seen)), entries })
    }
}

fn is_internal_entry(name: &str) -> bool { name.contains(".aerofs-part-") || name.contains(".aerofs.part") || name.contains(".aerofs.tmp") }
fn decode_cursor(cursor: Option<&str>) -> usize { cursor.and_then(|value| base64::Engine::decode(&base64::engine::general_purpose::STANDARD, value).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok().and_then(|value| value["offset"].as_u64()).or_else(|| String::from_utf8_lossy(&bytes).parse().ok())).or_else(|| value.parse().ok())).unwrap_or(0) as usize }
fn encode_cursor(offset: usize) -> String { base64::Engine::encode(&base64::engine::general_purpose::STANDARD, serde_json::json!({"offset": offset}).to_string()) }
fn sort_entries(entries: &mut [crate::domain::FileEntry], field: SortField, order: SortOrder) { entries.sort_by(|a,b| { let dirs = (a.kind == FileKind::Directory).cmp(&(b.kind == FileKind::Directory)); if dirs != std::cmp::Ordering::Equal { return dirs.reverse(); } let value = match field { SortField::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)), SortField::Modified => a.modified_at.map(|d| d.timestamp()).unwrap_or(0).cmp(&b.modified_at.map(|d| d.timestamp()).unwrap_or(0)), SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()) }; if order == SortOrder::Desc { value.reverse() } else { value } }); }
