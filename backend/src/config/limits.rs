use serde::{Deserialize, Serialize};

use super::{
    DEFAULT_ARCHIVE_CONCURRENCY, DEFAULT_GLOBAL_IO_CONCURRENCY, DEFAULT_SEARCH_CONCURRENCY,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LimitsConfig {
    pub max_upload_size: u64,
    pub max_editable_size: u64,
    pub max_preview_size: u64,
    pub max_directory_entries: usize,
    pub max_concurrent_transfers: usize,
    pub global_io_concurrency: usize,
    pub archive_concurrency: usize,
    pub search_concurrency: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_upload_size: 1024 * 1024 * 1024,
            max_editable_size: 10 * 1024 * 1024,
            max_preview_size: 25 * 1024 * 1024,
            max_directory_entries: 50_000,
            max_concurrent_transfers: 4,
            global_io_concurrency: DEFAULT_GLOBAL_IO_CONCURRENCY,
            archive_concurrency: DEFAULT_ARCHIVE_CONCURRENCY,
            search_concurrency: DEFAULT_SEARCH_CONCURRENCY,
        }
    }
}
