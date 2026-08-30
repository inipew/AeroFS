//! Services/files split target (67.md §6) — incremental extraction from `file_service.rs` 823L.
//! Current file_service remains canonical; this module exposes typed sub-services
//! that will take explicit `&self` DI instead of `&AppState` (§3-4).

pub mod delete;
pub mod listing;
pub mod metadata;
pub mod presign;
pub mod read;
pub mod write;

// Re-export for future `use crate::services::files::*`
pub use listing::ListOptions;
