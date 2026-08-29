export type FileKind = 'file' | 'directory' | 'symlink';

export interface VfsPath {
  connection_id: string;
  path: string;
}

export interface FileEntry {
  name: string;
  path: string;
  kind: FileKind;
  size?: number;
  modified_at?: string;
  permissions?: string;
  mime_type?: string;
  is_hidden: boolean;
  symlink_target?: string;
}

export interface FileMetadata {
  name: string;
  path: string;
  kind: FileKind;
  size: number;
  modified_at?: string;
  created_at?: string;
  permissions?: string;
  mime_type?: string;
  etag: string;
  is_readonly: boolean;
  is_hidden: boolean;
  symlink_target?: string;
}

export interface DirectoryListing {
  path: string;
  connection_id: string;
  entries: FileEntry[];
  total_count?: number;
  has_more?: boolean;
  next_cursor?: string;
}

export interface ChecksumCapabilities {
  md5: boolean;
  sha1: boolean;
  sha256: boolean;
}

export interface Capabilities {
  list: boolean;
  stat: boolean;
  read: boolean;
  write: boolean;
  create_file: boolean;
  create_dir: boolean;
  delete: boolean;
  rename: boolean;
  copy: boolean;
  move_: boolean;
  upload: boolean;
  download: boolean;
  resume_upload: boolean;
  resume_download: boolean;
  atomic_write: boolean;
  atomic_rename: boolean;
  server_side_copy: boolean;
  native_copy: boolean;
  symlink: boolean;
  permissions: boolean;
  watch: boolean;
  checksum: boolean;
  native_checksum: boolean;
  computed_checksums: ChecksumCapabilities;
  write_can_append: boolean;
  write_can_empty: boolean;
  write_can_multi: boolean;
  presign_read: boolean;
  presign_write: boolean;
  list_with_limit: boolean;
  list_with_start_after: boolean;
  list_with_recursive: boolean;
}
