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
        native_copy,
        native_checksum,
        checksums,
        computed_checksums,
        presign_read,
        presign_write,
        conditional_write,
        multipart_write,
        write_can_append,
        write_can_empty,
        write_can_multi,
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
                false, // local fs is not cloud server-side copy
                true,  // local fs supports native fast copy
                false, // local files do not have server metadata checksums
                ChecksumCapabilities::none(),
                ChecksumCapabilities::all(),
                false,
                false,
                false,
                false,
                cap.write_can_append,
                cap.write_can_empty,
                cap.write_can_multi,
                cap.rename,
                true,
            )
        }
        "s3" => (
            false,
            false,
            cap.copy, // S3 natively supports Server-Side Copy (CopyObject)
            false,
            true, // S3 metadata contains ETag / server-side hashes
            ChecksumCapabilities::s3_default(),
            ChecksumCapabilities::all(),
            cap.presign_read,
            cap.presign_write,
            cap.write_with_if_match || cap.write_with_if_none_match || cap.write_with_if_not_exists,
            cap.write_can_multi,
            cap.write_can_append,
            cap.write_can_empty,
            cap.write_can_multi,
            false, // S3 does not have POSIX atomic rename
            false,
        ),
        "sftp" => (
            true, // SFTP supports remote POSIX file permissions
            false,
            false,
            false,
            false,
            ChecksumCapabilities::none(),
            ChecksumCapabilities::all(),
            false,
            false,
            false,
            false,
            cap.write_can_append,
            cap.write_can_empty,
            cap.write_can_multi,
            cap.rename,
            false,
        ),
        "ftp" => (
            false,
            false,
            false,
            false,
            false,
            ChecksumCapabilities::none(),
            ChecksumCapabilities::all(),
            false,
            false,
            false,
            false,
            cap.write_can_append,
            cap.write_can_empty,
            cap.write_can_multi,
            cap.rename,
            false,
        ),
        _ => (
            false,
            false,
            cap.copy,
            false,
            false,
            ChecksumCapabilities::none(),
            ChecksumCapabilities::all(),
            cap.presign_read,
            cap.presign_write,
            cap.write_with_if_match,
            cap.write_can_multi,
            cap.write_can_append,
            cap.write_can_empty,
            cap.write_can_multi,
            cap.rename,
            false,
        ),
    };

    let has_checksum = checksums.has_any() || computed_checksums.has_any();

    Capabilities {
        list: cap.list,
        stat: cap.stat,
        read: cap.read,
        write: cap.write,
        create_file: cap.write,
        create_dir: cap.create_dir,
        delete: cap.delete,
        rename: cap.rename,
        copy: cap.copy || server_side_copy || native_copy,
        move_: cap.rename,
        upload: cap.write,
        download: cap.read,
        resume_upload: false,
        resume_download: cap.read,
        range_read: cap.read,
        resumable_read: cap.read,
        multipart_write,
        resumable_write: write_can_append,
        write_can_append,
        write_can_empty,
        write_can_multi,
        presign_read,
        presign_write,
        conditional_write,
        atomic_write,
        atomic_rename,
        server_side_copy,
        native_copy,
        symlink,
        permissions,
        watch: false,
        checksum: has_checksum,
        native_checksum,
        checksums,
        computed_checksums,
    }
}
