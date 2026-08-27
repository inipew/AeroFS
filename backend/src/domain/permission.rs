use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
