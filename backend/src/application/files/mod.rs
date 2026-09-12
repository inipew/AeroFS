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
pub use mutation::{
    CopyEntry, CopyEntryCommand, CreateDirectory, CreateDirectoryCommand, DeleteEntries,
    DeleteEntriesCommand, DeleteEntriesResult, RenameEntry, RenameEntryCommand,
};
pub use read::ReadOptions;
pub use read_file::{ReadFile, ReadFileCommand, ReadFileResult};
pub use stat::{StatFile, StatFileCommand};
pub use write::{WriteFile, WriteFileCommand};

use crate::events::EventJournal;
use crate::infrastructure::files::{
    RegistryFileSystemResolver, SqliteAuthorization, SqliteFileMutationEffects, SqliteFileSettings,
};
use crate::ports::{
    authorization::Authorization, effects::FileMutationEffects, filesystem::FileSystemResolver,
    settings::FileSettings,
};
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
    pub write_file: WriteFile,
    pub create_directory: CreateDirectory,
    pub rename_entry: RenameEntry,
    pub copy_entry: CopyEntry,
    pub delete_entries: DeleteEntries,
}

/// Compatibility facade for endpoints not migrated yet. New file operations
/// should be implemented as explicit use-cases and only adapted here while
/// their HTTP handlers are migrated incrementally.
#[derive(Clone)]
pub struct FileApplicationService {
    pub registry: Arc<ProviderRegistry>,
    pub db: crate::db::DbPool,
    pub config: Arc<crate::config::AppConfig>,
    pub metadata_cache: Arc<MetadataCache>,
    pub event_journal: Arc<EventJournal>,
    pub global_io: Arc<Semaphore>,
    pub(crate) authorization: Arc<dyn Authorization>,
    pub(crate) filesystem: Arc<dyn FileSystemResolver>,
    pub(crate) effects: Arc<dyn FileMutationEffects>,
    pub(crate) write_file: WriteFile,
}

impl FileApplicationService {
    pub fn from_state(state: &AppState) -> Self {
        Self::new(
            Arc::clone(&state.registry),
            state.db.clone(),
            Arc::clone(&state.config),
            Arc::clone(&state.metadata_cache),
            Arc::clone(&state.event_journal),
            Arc::clone(&state.global_io_semaphore),
        )
    }

    pub fn new(
        registry: Arc<ProviderRegistry>,
        db: crate::db::DbPool,
        config: Arc<crate::config::AppConfig>,
        metadata_cache: Arc<MetadataCache>,
        event_journal: Arc<EventJournal>,
        global_io: Arc<Semaphore>,
    ) -> Self {
        let authorization: Arc<dyn Authorization> = Arc::new(SqliteAuthorization::new(db.clone()));
        let filesystem: Arc<dyn FileSystemResolver> =
            Arc::new(RegistryFileSystemResolver::new(registry.clone()));
        let settings: Arc<dyn FileSettings> =
            Arc::new(SqliteFileSettings::new(db.clone(), config.clone()));
        let effects: Arc<dyn FileMutationEffects> = Arc::new(SqliteFileMutationEffects::new(
            db.clone(),
            metadata_cache.clone(),
            event_journal.clone(),
        ));
        let write_file = WriteFile::new(
            authorization.clone(),
            filesystem.clone(),
            settings,
            effects.clone(),
        );
        Self {
            registry,
            db,
            config,
            metadata_cache,
            event_journal,
            global_io,
            authorization,
            filesystem,
            effects,
            write_file,
        }
    }
}
