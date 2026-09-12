//! File application service — typed boundary (§3-4, §85).
//! Replaces universal `&AppState` DI with explicit ports.

mod list_directory;
mod listing;
mod mutation;
mod presign;
mod read;
mod read_file;
mod stat;
mod write;

pub use list_directory::{ListDirectory, ListDirectoryCommand};
pub use listing::ListOptions;
pub use read::ReadOptions;
pub use read_file::{ReadFile, ReadFileCommand, ReadFileResult};
pub use stat::{StatFile, StatFileCommand};

use crate::events::EventJournal;
use crate::services::cache::MetadataCache;
use crate::state::AppState;
use crate::vfs::registry::ProviderRegistry;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// File use-cases exposed to HTTP/CLI adapters. Concrete dependencies are
/// assembled once in the composition root, never reconstructed per request.
#[derive(Clone)]
pub struct FileUseCases {
    pub list_directory: ListDirectory,
    pub stat_file: StatFile,
    pub read_file: ReadFile,
}

/// Explicit dependencies — no god context.
#[derive(Clone)]
pub struct FileApplicationService {
    pub registry: Arc<ProviderRegistry>,
    pub db: crate::db::DbPool,
    pub config: Arc<crate::config::AppConfig>,
    pub metadata_cache: Arc<MetadataCache>,
    pub event_journal: Arc<EventJournal>,
    pub global_io: Arc<Semaphore>,
}

impl FileApplicationService {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            registry: Arc::clone(&state.registry),
            db: state.db.clone(),
            config: Arc::clone(&state.config),
            metadata_cache: Arc::clone(&state.metadata_cache),
            event_journal: Arc::clone(&state.event_journal),
            global_io: Arc::clone(&state.global_io_semaphore),
        }
    }

    pub fn new(
        registry: Arc<ProviderRegistry>,
        db: crate::db::DbPool,
        config: Arc<crate::config::AppConfig>,
        metadata_cache: Arc<MetadataCache>,
        event_journal: Arc<EventJournal>,
        global_io: Arc<Semaphore>,
    ) -> Self {
        Self {
            registry,
            db,
            config,
            metadata_cache,
            event_journal,
            global_io,
        }
    }
}
