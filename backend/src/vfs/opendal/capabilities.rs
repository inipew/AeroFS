use crate::domain::{Capabilities, ChecksumCapabilities};

/// Map OpenDAL Capability into AeroFS Capabilities domain struct with provider-specific policy
pub fn map_opendal_capabilities(cap: opendal::Capability) -> Capabilities {
    map_opendal_capabilities_for_scheme(cap, "generic")
}

/// Map OpenDAL Capability with provider scheme awareness and granular typed flags
pub fn map_opendal_capabilities_for_scheme(cap: opendal::Capability, scheme: &str) -> Capabilities {
    let (
        permissions,
        symlink,
        server_side_copy,
        checksums,
        presign_read,
        presign_write,
        conditional_write,
        multipart_write,
        atomic_rename,
        atomic_write,
    ) = match scheme {
        "fs" => {
            #[cfg(unix)]
            let perms = true;
            #[cfg(not(unix))]
            let perms = false;
            (
                perms,
                cfg!(unix),
                true, // local fs supports instant copy
                ChecksumCapabilities::all(),
                false,
                false,
                false,
                false,
                false,
                false,
            )
        }
        "s3" => {
            (
                false,
                false,
                cap.copy, // S3 natively supports Server-Side Copy (CopyObject)
                ChecksumCapabilities::s3_default(),
                true,  // S3 supports presigned GET
                true,  // S3 supports presigned PUT
                true,  // S3 supports If-Match / If-None-Match conditional operations
                true,  // S3 supports multipart upload
                false, // S3 does not have POSIX atomic rename
                false,
            )
        }
        "sftp" => {
            (
                true, // SFTP supports remote POSIX file permissions
                false,
                false,
                ChecksumCapabilities::default(),
                false,
                false,
                false,
                false,
                cap.rename,
                false,
            )
        }
        "ftp" => (
            false,
            false,
            false,
            ChecksumCapabilities::default(),
            false,
            false,
            false,
            false,
            cap.rename,
            false,
        ),
        _ => (
            false,
            false,
            cap.copy,
            ChecksumCapabilities::default(),
            false,
            false,
            false,
            false,
            cap.rename,
            false,
        ),
    };

    let has_checksum = checksums.has_any();

    Capabilities {
        list: cap.list,
        stat: cap.stat,
        read: cap.read,
        write: cap.write,
        create_file: cap.write,
        create_dir: cap.create_dir,
        delete: cap.delete,
        rename: cap.rename,
        copy: cap.copy || server_side_copy,
        move_: cap.rename,
        upload: cap.write,
        download: cap.read,
        resume_upload: false,
        resume_download: false,
        range_read: cap.read,
        resumable_read: cap.read,
        multipart_write,
        resumable_write: multipart_write,
        presign_read,
        presign_write,
        conditional_write,
        atomic_write,
        atomic_rename,
        server_side_copy,
        symlink,
        permissions,
        watch: false,
        checksum: has_checksum,
        checksums,
    }
}
