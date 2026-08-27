use crate::domain::Capabilities;

/// Map OpenDAL Capability into AeroFS Capabilities domain struct with provider-specific policy
pub fn map_opendal_capabilities(cap: opendal::Capability) -> Capabilities {
    map_opendal_capabilities_for_scheme(cap, "generic")
}

/// Map OpenDAL Capability with provider scheme awareness
pub fn map_opendal_capabilities_for_scheme(cap: opendal::Capability, scheme: &str) -> Capabilities {
    let (permissions, symlink, server_side_copy, checksum) = match scheme {
        "fs" => {
            #[cfg(unix)]
            let perms = true;
            #[cfg(not(unix))]
            let perms = false;
            (perms, cfg!(unix), false, false)
        }
        "s3" => {
            // S3 natively supports CopyObject and ETag/MD5/CRC32 checksums
            (false, false, cap.copy, true)
        }
        "sftp" => {
            // SFTP supports remote POSIX file permissions
            (true, false, false, false)
        }
        "ftp" => {
            (false, false, false, false)
        }
        _ => (false, false, false, false),
    };

    Capabilities {
        list: cap.list,
        stat: cap.stat,
        read: cap.read,
        write: cap.write,
        create_file: cap.write,
        create_dir: cap.create_dir,
        delete: cap.delete,
        rename: cap.rename,
        copy: cap.copy,
        move_: cap.rename,
        upload: cap.write,
        download: cap.read,
        resume_upload: false,
        resume_download: false,
        atomic_write: false,
        atomic_rename: false,
        server_side_copy,
        symlink,
        permissions,
        watch: false,
        checksum,
    }
}
