pub mod builder;
pub mod capabilities;
pub mod error;
pub mod filesystem;
pub mod metadata;

pub use builder::{build_fs_operator, build_ftp_operator, build_s3_operator, build_sftp_operator};
pub use filesystem::OpenDalFileSystem;
