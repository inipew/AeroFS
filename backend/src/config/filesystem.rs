use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FilesystemConfig {
    pub default_local_root: PathBuf,
    pub temp_dir: Option<PathBuf>,
    pub show_hidden_default: bool,
    pub read_only_default: bool,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            default_local_root: PathBuf::from("./storage"),
            temp_dir: Some(PathBuf::from("./storage/temp")),
            show_hidden_default: false,
            read_only_default: false,
        }
    }
}
