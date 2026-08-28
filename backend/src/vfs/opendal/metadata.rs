use crate::domain::{FileEntry, FileKind, FileMetadata, VfsPath};
use chrono::{DateTime, Utc};
use std::time::SystemTime;

/// Map OpenDAL metadata to AeroFS FileMetadata
pub fn map_opendal_metadata(
    meta: &opendal::Metadata,
    vfs_path: &VfsPath,
    is_root: bool,
) -> FileMetadata {
    let name = if is_root {
        "root".to_string()
    } else {
        vfs_path.file_name().unwrap_or("entry").to_string()
    };

    let is_dir = meta.is_dir() || is_root;
    let kind = if is_dir {
        FileKind::Directory
    } else {
        FileKind::File
    };

    let size = if is_dir { 0 } else { meta.content_length() };
    let modified_at: Option<DateTime<Utc>> = meta.last_modified().map(|dt| {
        let st: SystemTime = dt.into();
        DateTime::<Utc>::from(st)
    });
    let mtime_ts = modified_at.map(|m| m.timestamp()).unwrap_or(0);

    let etag = if let Some(e) = meta.etag() {
        if e.starts_with('"') && e.ends_with('"') {
            e.to_string()
        } else {
            format!("\"{}\"", e)
        }
    } else if is_dir {
        format!("\"od-dir-{}\"", vfs_path.path)
    } else {
        format!("\"od-{}-{}-{}\"", vfs_path.path, size, mtime_ts)
    };

    let mime_type = if is_dir {
        None
    } else {
        Some(
            mime_guess::from_path(&name)
                .first_or_octet_stream()
                .to_string(),
        )
    };

    FileMetadata {
        name: name.clone(),
        path: vfs_path.path.clone(),
        kind,
        size,
        modified_at,
        created_at: None,
        permissions: None,
        mime_type,
        etag,
        is_readonly: false,
        is_hidden: name.starts_with('.'),
        symlink_target: None,
    }
}

/// Map OpenDAL Entry to AeroFS FileEntry
pub fn map_opendal_entry(entry: &opendal::Entry, parent_vfs: &VfsPath) -> Option<FileEntry> {
    let entry_path_clean = entry.path().trim_matches('/');
    let parent_path_clean = parent_vfs.path.trim_matches('/');

    // Ignore self-directory marker and empty/dot names
    if !parent_path_clean.is_empty() && entry_path_clean == parent_path_clean {
        return None;
    }

    let raw_name = entry.name().trim_end_matches('/');
    if raw_name.is_empty() || raw_name == "." || raw_name == ".." {
        return None;
    }

    let is_dir = entry.metadata().is_dir() || entry.name().ends_with('/');
    let kind = if is_dir {
        FileKind::Directory
    } else {
        FileKind::File
    };

    let child_vfs = parent_vfs.join(raw_name).ok()?;
    let size = if is_dir {
        None
    } else {
        Some(entry.metadata().content_length())
    };

    let modified_at: Option<DateTime<Utc>> = entry.metadata().last_modified().map(|dt| {
        let st: SystemTime = dt.into();
        DateTime::<Utc>::from(st)
    });

    let mime_type = if is_dir {
        None
    } else {
        Some(
            mime_guess::from_path(raw_name)
                .first_or_octet_stream()
                .to_string(),
        )
    };

    Some(FileEntry {
        name: raw_name.to_string(),
        path: child_vfs.path,
        kind,
        size,
        modified_at,
        permissions: None,
        mime_type,
        is_hidden: raw_name.starts_with('.'),
        symlink_target: None,
    })
}
