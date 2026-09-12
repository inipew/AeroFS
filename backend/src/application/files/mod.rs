//! File application use-cases.
//! Application depends on domain contracts and ports only; concrete adapters are
//! assembled by `crate::bootstrap`.

mod chmod;
mod list_directory;
mod listing;
mod mutation;
mod presign;
mod read;
mod read_file;
mod stat;
mod write;

pub use chmod::{ChmodEntry, ChmodEntryCommand};
pub use list_directory::{ListDirectory, ListDirectoryCommand};
pub use listing::ListOptions;
pub use mutation::{
    CopyEntry, CopyEntryCommand, CreateDirectory, CreateDirectoryCommand, DeleteEntries,
    DeleteEntriesCommand, DeleteEntriesResult, RenameEntry, RenameEntryCommand,
};
pub use presign::{
    CompletePresigned, CompletePresignedCommand, PresignCommand, PresignDownload, PresignUpload,
};
pub use read::ReadOptions;
pub use read_file::{ReadFile, ReadFileCommand, ReadFileResult};
pub use stat::{StatFile, StatFileCommand};
pub use write::{WriteFile, WriteFileCommand};

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
    pub presign_download: PresignDownload,
    pub presign_upload: PresignUpload,
    pub complete_presigned: CompletePresigned,
    pub chmod_entry: ChmodEntry,
}
