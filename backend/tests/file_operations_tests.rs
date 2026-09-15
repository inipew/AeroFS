mod support;

#[path = "file_operations/archive.rs"]
mod archive;
#[path = "file_operations/archive_http.rs"]
mod archive_http;
#[path = "file_operations/archive_roundtrip.rs"]
mod archive_roundtrip;
#[path = "file_operations/cache.rs"]
mod cache;
#[path = "file_operations/cache_singleflight.rs"]
mod cache_singleflight;
#[path = "file_operations/contracts.rs"]
mod contracts;
#[path = "file_operations/delete.rs"]
mod delete;
#[path = "file_operations/editor.rs"]
mod editor;
#[path = "file_operations/editor_permissions.rs"]
mod editor_permissions;
#[path = "file_operations/preconditions.rs"]
mod preconditions;
#[path = "file_operations/presign.rs"]
mod presign;
#[path = "file_operations/provider_auth.rs"]
mod provider_auth;
#[path = "file_operations/staging.rs"]
mod staging;
#[path = "file_operations/staging_cleanup.rs"]
mod staging_cleanup;
#[path = "file_operations/upload_lock.rs"]
mod upload_lock;
#[path = "file_operations/uploads.rs"]
mod uploads;
#[path = "file_operations/vfs.rs"]
mod vfs;
