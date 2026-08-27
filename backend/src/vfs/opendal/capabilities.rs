use crate::domain::Capabilities;

/// Map OpenDAL Capability into AeroFS Capabilities domain struct
pub fn map_opendal_capabilities(cap: opendal::Capability) -> Capabilities {
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
        atomic_rename: cap.rename,
        server_side_copy: cap.copy,
        symlink: false,
        permissions: false,
        watch: false,
        checksum: false,
    }
}
