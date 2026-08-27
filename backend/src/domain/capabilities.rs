use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Describes the operational capabilities supported by a specific VFS provider instance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Capabilities {
    // Navigation & Info
    pub list: bool,
    pub stat: bool,

    // File Operations
    pub read: bool,
    pub write: bool,
    pub create_file: bool,
    pub create_dir: bool,
    pub delete: bool,
    pub rename: bool,
    pub copy: bool,
    pub move_: bool,

    // Transfer
    pub upload: bool,
    pub download: bool,
    pub resume_upload: bool,
    pub resume_download: bool,

    // Advanced & Integrity
    pub atomic_write: bool,
    pub atomic_rename: bool,
    pub server_side_copy: bool,
    pub symlink: bool,
    pub permissions: bool,
    pub watch: bool,
    pub checksum: bool,
    pub range_read: bool,
}

impl Capabilities {
    /// Full capabilities for local filesystem
    pub fn local_default() -> Self {
        Self {
            list: true,
            stat: true,
            read: true,
            write: true,
            create_file: true,
            create_dir: true,
            delete: true,
            rename: true,
            copy: true,
            move_: true,
            upload: true,
            download: true,
            resume_upload: true,
            resume_download: true,
            atomic_write: true,
            atomic_rename: true,
            server_side_copy: true,
            symlink: true,
            permissions: true,
            watch: true,
            checksum: true,
            range_read: true,
        }
    }

    /// Read-only capabilities
    pub fn read_only(mut self) -> Self {
        self.write = false;
        self.create_file = false;
        self.create_dir = false;
        self.delete = false;
        self.rename = false;
        self.move_ = false;
        self.upload = false;
        self.atomic_write = false;
        self.atomic_rename = false;
        self
    }
}
