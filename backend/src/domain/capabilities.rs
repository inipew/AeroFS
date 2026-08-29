use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Typed granular checksum capabilities for storage backends
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChecksumCapabilities {
    pub md5: bool,
    pub crc32: bool,
    pub crc32c: bool,
    pub sha1: bool,
    pub sha256: bool,
}

impl ChecksumCapabilities {
    pub fn all() -> Self {
        Self {
            md5: true,
            crc32: true,
            crc32c: true,
            sha1: true,
            sha256: true,
        }
    }

    pub fn s3_default() -> Self {
        Self {
            md5: true,
            crc32: true,
            crc32c: true,
            sha1: true,
            sha256: true,
        }
    }

    pub fn none() -> Self {
        Self::default()
    }

    pub fn has_any(&self) -> bool {
        self.md5 || self.crc32 || self.crc32c || self.sha1 || self.sha256
    }
}

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

    // Transfer & Streaming
    pub upload: bool,
    pub download: bool,
    pub resume_upload: bool,
    pub resume_download: bool,
    pub range_read: bool,
    pub resumable_read: bool,
    pub multipart_write: bool,
    pub resumable_write: bool,
    pub write_can_append: bool,
    pub write_can_empty: bool,
    pub write_can_multi: bool,

    // Object Storage & Acceleration
    pub presign_read: bool,
    pub presign_write: bool,
    pub conditional_write: bool,

    // Advanced & Integrity
    pub atomic_write: bool,
    pub atomic_rename: bool,
    pub server_side_copy: bool,
    pub native_copy: bool,
    pub symlink: bool,
    pub permissions: bool,
    pub watch: bool,
    pub checksum: bool,
    pub native_checksum: bool,
    pub checksums: ChecksumCapabilities,
    pub computed_checksums: ChecksumCapabilities,
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
            resume_upload: false,
            resume_download: true,
            range_read: true,
            resumable_read: true,
            multipart_write: false,
            resumable_write: false,
            write_can_append: true,
            write_can_empty: true,
            write_can_multi: false,
            presign_read: false,
            presign_write: false,
            conditional_write: false,
            atomic_write: true,
            atomic_rename: true,
            server_side_copy: false,
            native_copy: true,
            symlink: true,
            permissions: true,
            watch: true,
            checksum: true,
            native_checksum: false,
            checksums: ChecksumCapabilities::none(),
            computed_checksums: ChecksumCapabilities::all(),
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
        self.resume_upload = false;
        self.multipart_write = false;
        self.resumable_write = false;
        self.presign_write = false;
        self.conditional_write = false;
        self.atomic_write = false;
        self.atomic_rename = false;
        self
    }
}
