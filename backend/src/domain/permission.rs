use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct Permissions: u16 {
        const READ     = 1 << 0;
        const WRITE    = 1 << 1;
        const CREATE   = 1 << 2;
        const DELETE   = 1 << 3;
        const RENAME   = 1 << 4;
        const UPLOAD   = 1 << 5;
        const DOWNLOAD = 1 << 6;
        const SEARCH   = 1 << 7;
        const ADMIN    = 1 << 8;
    }
}

impl Permissions {
    pub fn all_rw() -> Self {
        Self::READ
            | Self::WRITE
            | Self::CREATE
            | Self::DELETE
            | Self::RENAME
            | Self::UPLOAD
            | Self::DOWNLOAD
            | Self::SEARCH
    }
    pub fn from_set(set: &PermissionSet) -> Self {
        let mut p = Permissions::empty();
        if set.read {
            p |= Permissions::READ;
        }
        if set.write {
            p |= Permissions::WRITE;
        }
        if set.create {
            p |= Permissions::CREATE;
        }
        if set.delete {
            p |= Permissions::DELETE;
        }
        if set.rename {
            p |= Permissions::RENAME;
        }
        if set.upload {
            p |= Permissions::UPLOAD;
        }
        if set.download {
            p |= Permissions::DOWNLOAD;
        }
        if set.search {
            p |= Permissions::SEARCH;
        }
        if set.admin {
            p |= Permissions::ADMIN;
        }
        p
    }
    pub fn to_set(self) -> PermissionSet {
        PermissionSet {
            read: self.contains(Permissions::READ),
            write: self.contains(Permissions::WRITE),
            create: self.contains(Permissions::CREATE),
            delete: self.contains(Permissions::DELETE),
            rename: self.contains(Permissions::RENAME),
            upload: self.contains(Permissions::UPLOAD),
            download: self.contains(Permissions::DOWNLOAD),
            search: self.contains(Permissions::SEARCH),
            admin: self.contains(Permissions::ADMIN),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PermissionSet {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub delete: bool,
    pub rename: bool,
    pub upload: bool,
    pub download: bool,
    pub search: bool,
    pub admin: bool,
}

impl Default for PermissionSet {
    fn default() -> Self {
        Self {
            read: true,
            write: true,
            create: true,
            delete: true,
            rename: true,
            upload: true,
            download: true,
            search: true,
            admin: false,
        }
    }
}

impl PermissionSet {
    pub fn admin() -> Self {
        Self {
            read: true,
            write: true,
            create: true,
            delete: true,
            rename: true,
            upload: true,
            download: true,
            search: true,
            admin: true,
        }
    }

    pub fn read_only() -> Self {
        Self {
            read: true,
            write: false,
            create: false,
            delete: false,
            rename: false,
            upload: false,
            download: true,
            search: true,
            admin: false,
        }
    }
}
