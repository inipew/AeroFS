use super::FileApplicationService;
use crate::auth::UserInfo;
use crate::domain::{DirectoryListing, FileKind, SortField, SortOrder, VfsPath};
use crate::errors::{AppError, VfsError};

#[derive(Debug, Clone)]
pub struct ListOptions {
    pub path: Option<String>,
    pub show_hidden: Option<bool>,
    pub sort: Option<SortField>,
    pub order: Option<SortOrder>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

impl FileApplicationService {
    /// Typed listing — new API boundary (§61, §129).
    /// Wraps legacy stringly service while exposing typed options to handlers.
    /// Incremental: still delegates to FileService via AppState shim; next step removes &AppState.
    pub async fn list_paged_typed(
        &self,
        _state: &crate::state::AppState,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        opts: ListOptions,
    ) -> Result<DirectoryListing, AppError> {
        // Prefer owned path (explicit ports) — state shim kept for backward compat
        self.list_paged_owned(user, connection, opts).await
    }

    /// Owned listing — no AppState god object, explicit ports only (Phase 3.1).
    pub async fn list_paged_owned(
        &self,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        opts: ListOptions,
    ) -> Result<DirectoryListing, AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        use futures::StreamExt;

        check_permission(&self.db, user, connection.as_str(), PermissionAction::Read).await?;

        let provider = self
            .registry
            .get(connection.as_str())
            .await
            .ok_or_else(|| {
                VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
            })?;

        let path_str = opts.path.unwrap_or_else(|| "/".to_string());
        let vfs_path = VfsPath::new(connection.as_str(), path_str)?;

        let show_hidden = match opts.show_hidden {
            Some(val) => val,
            None => {
                // System setting override via DB (explicit port: self.db) instead of AppState
                let sys_val: Option<String> = sqlx::query_scalar(
                    "SELECT value FROM system_settings WHERE key = 'show_hidden_default'",
                )
                .fetch_optional(&self.db)
                .await
                .unwrap_or(None);
                if let Some(v) = sys_val {
                    v == "true"
                } else {
                    self.config.filesystem.show_hidden_default
                }
            }
        };

        let skip_offset = if let Some(cursor_str) = opts.cursor {
            if let Ok(decoded) =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &cursor_str)
            {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                    val["offset"].as_u64().unwrap_or(0) as usize
                } else {
                    String::from_utf8_lossy(&decoded)
                        .parse::<usize>()
                        .unwrap_or_default()
                }
            } else {
                cursor_str.parse::<usize>().unwrap_or(0)
            }
        } else {
            0
        };

        let page_limit = opts
            .limit
            .unwrap_or(self.config.limits.max_directory_entries)
            .clamp(1, 1000);
        let mut stream = provider.list_stream(&vfs_path).await?;
        let mut filtered_entries = Vec::new();
        let mut current_idx = 0;
        let mut has_more = false;

        while let Some(res) = stream.next().await {
            let entry = res?;
            if entry.name.contains(".aerofs-part-")
                || entry.name.contains(".aerofs.part")
                || entry.name.contains(".aerofs.tmp")
            {
                continue;
            }
            if !show_hidden && entry.is_hidden {
                continue;
            }
            if current_idx < skip_offset {
                current_idx += 1;
                continue;
            }
            if filtered_entries.len() < page_limit {
                filtered_entries.push(entry);
                current_idx += 1;
            } else {
                has_more = true;
                break;
            }
        }

        let sort_field = opts.sort.unwrap_or(SortField::Name);
        let sort_order = opts.order.unwrap_or(SortOrder::Asc);

        filtered_entries.sort_by(|a, b| {
            let a_is_dir = a.kind == FileKind::Directory;
            let b_is_dir = b.kind == FileKind::Directory;
            if a_is_dir != b_is_dir {
                return b_is_dir.cmp(&a_is_dir);
            }
            let cmp = match sort_field {
                SortField::Size => {
                    let a_size = a.size.unwrap_or(0);
                    let b_size = b.size.unwrap_or(0);
                    a_size.cmp(&b_size)
                }
                SortField::Modified => {
                    let a_mod = a.modified_at.map(|d| d.timestamp()).unwrap_or(0);
                    let b_mod = b.modified_at.map(|d| d.timestamp()).unwrap_or(0);
                    a_mod.cmp(&b_mod)
                }
                SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            };
            if sort_order == SortOrder::Desc {
                cmp.reverse()
            } else {
                cmp
            }
        });

        let next_cursor = if has_more {
            let cursor_payload = serde_json::json!({ "offset": current_idx });
            Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                cursor_payload.to_string().as_bytes(),
            ))
        } else {
            None
        };

        let total_count = filtered_entries.len();

        Ok(DirectoryListing {
            path: vfs_path.path,
            connection_id: connection.to_string(),
            entries: filtered_entries,
            total_count: Some(total_count),
            has_more,
            next_cursor,
        })
    }
}
