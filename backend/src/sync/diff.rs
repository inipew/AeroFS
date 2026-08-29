use crate::sync::models::{FileManifest, SyncOpKind, SyncOperation, SyncStrategy};
use std::collections::HashMap;

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
        for entry in dest_entries {
            dest_map.insert(&entry.path, entry);
        }

        let mut ops = Vec::new();

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

                if same_hash || (same_etag && same_size) || (same_size && src.modified_at == dst.modified_at) {
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
                // Source only -> Create on destination
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
            if !source_map.contains_key(path) {
                ops.push(SyncOperation {
                    relative_path: path.to_string(),
                    kind: SyncOpKind::Noop,
                    source_manifest: None,
                    dest_manifest: Some((*dst).clone()),
                });
            }
        }

        ops
    }
}
