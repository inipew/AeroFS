pub mod opendal;
pub mod traits;

pub use opendal::OpenDalFileSystem;
pub use traits::{AsyncReadBox, FileSystem};
