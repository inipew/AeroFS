use crate::sync::models::{FileManifest, SyncOpKind, SyncOperation, SyncStrategy};
use std::collections::{HashMap, HashSet};

pub struct ManifestDiffer;

impl ManifestDiffer {
    /// Compare source manifests and destination manifests, applying the sync strategy.
    pub fn diff(
        source_entries: &[FileManifest],
        dest_entries: &[FileManifest],
        strategy: SyncStrategy,
    ) -> Vec<SyncOperation> {
        let mut source_map: HashMap<&str, &FileManifest> = HashMap::new();
        for entry in source_entries {
            source_map.insert(&entry.path, entry);
        }

        let mut dest_map: HashMap<&str, &FileManifest> = HashMap::new();
        let mut dest_fingerprints: HashMap<String, &FileManifest> = HashMap::new();

        for entry in dest_entries {
            dest_map.insert(&entry.path, entry);
            let fingerprint = Self::fingerprint(entry);
            dest_fingerprints.insert(fingerprint, entry);
        }

        let mut ops = Vec::new();
        let mut renamed_dest_paths = HashSet::new();

        // 1. Process all source entries
        for (path, src) in &source_map {
            if let Some(dst) = dest_map.get(path) {
                // Exists in both source and destination
                let same_size = src.size == dst.size;
                let same_hash = match (&src.content_hash, &dst.content_hash) {
                    (Some(h1), Some(h2)) => h1 == h2,
                    _ => false,
                };
                let same_etag = match (&src.etag, &dst.etag) {
                    (Some(e1), Some(e2)) => e1 == e2,
                    _ => false,
                };

                if same_hash
                    || (same_etag && same_size)
                    || (same_size && src.modified_at == dst.modified_at)
                {
                    // Unchanged
                    ops.push(SyncOperation {
                        relative_path: path.to_string(),
                        kind: SyncOpKind::Noop,
                        source_manifest: Some((*src).clone()),
                        dest_manifest: Some((*dst).clone()),
                    });
                } else {
                    // Conflict or Update based on strategy
                    let op_kind = match strategy {
                        SyncStrategy::SourceWins => SyncOpKind::Update,
                        SyncStrategy::DestWins => SyncOpKind::Noop,
                        SyncStrategy::NewestWins => {
                            if src.modified_at >= dst.modified_at {
                                SyncOpKind::Update
                            } else {
                                SyncOpKind::Noop
                            }
                        }
                        SyncStrategy::KeepBoth => SyncOpKind::Conflict,
                        SyncStrategy::Manual => SyncOpKind::Conflict,
                    };
                    ops.push(SyncOperation {
                        relative_path: path.to_string(),
                        kind: op_kind,
                        source_manifest: Some((*src).clone()),
                        dest_manifest: Some((*dst).clone()),
                    });
                }
            } else {
                // Source only -> Check for rename or create
                let src_fingerprint = Self::fingerprint(src);
                if let Some(renamed_dst) = dest_fingerprints.get(&src_fingerprint) {
                    if !source_map.contains_key(renamed_dst.path.as_str()) {
                        ops.push(SyncOperation {
                            relative_path: path.to_string(),
                            kind: SyncOpKind::Rename {
                                old_path: renamed_dst.path.clone(),
                            },
                            source_manifest: Some((*src).clone()),
                            dest_manifest: Some((*renamed_dst).clone()),
                        });
                        renamed_dest_paths.insert(renamed_dst.path.clone());
                        continue;
                    }
                }

                ops.push(SyncOperation {
                    relative_path: path.to_string(),
                    kind: SyncOpKind::Create,
                    source_manifest: Some((*src).clone()),
                    dest_manifest: None,
                });
            }
        }

        // 2. Process entries only in destination
        for (path, dst) in &dest_map {
            if !source_map.contains_key(path) && !renamed_dest_paths.contains(*path) {
                let op_kind = match strategy {
                    SyncStrategy::SourceWins => SyncOpKind::Delete,
                    SyncStrategy::NewestWins => SyncOpKind::Delete,
                    SyncStrategy::DestWins => SyncOpKind::Noop,
                    SyncStrategy::KeepBoth => SyncOpKind::Noop,
                    SyncStrategy::Manual => SyncOpKind::Conflict,
                };
                ops.push(SyncOperation {
                    relative_path: path.to_string(),
                    kind: op_kind,
                    source_manifest: None,
                    dest_manifest: Some((*dst).clone()),
                });
            }
        }

        ops
    }

    fn fingerprint(manifest: &FileManifest) -> String {
        if let Some(etag) = &manifest.etag {
            format!("etag:{}", etag)
        } else if let Some(hash) = &manifest.content_hash {
            format!("hash:{}", hash)
        } else if let Some(mod_time) = manifest.modified_at {
            format!("size_time:{}_{}", manifest.size, mod_time.timestamp())
        } else {
            format!("size:{}", manifest.size)
        }
    }
}
