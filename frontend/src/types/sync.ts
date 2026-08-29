import type { FileKind } from './vfs';

export type SyncStrategy =
  | 'source_wins'
  | 'dest_wins'
  | 'newest_wins'
  | 'keep_both'
  | 'manual';

export type SyncStatus =
  | 'created'
  | 'scanning'
  | 'planning'
  | 'reconciling'
  | 'executing'
  | 'verifying'
  | 'completed'
  | 'paused'
  | 'failed'
  | 'conflict';

export interface FileManifest {
  path: string;
  kind: FileKind;
  size: number;
  modified_at?: string;
  content_hash?: string;
  etag?: string;
}

export interface SyncJob {
  id: string;
  user_id: string;
  source_connection_id: string;
  source_path: string;
  destination_connection_id: string;
  destination_path: string;
  status: SyncStatus;
  strategy: SyncStrategy;
  total_files: number;
  synced_files: number;
  conflict_files: number;
  created_at: string;
  updated_at: string;
}

export interface CreateSyncRequest {
  source_connection_id: string;
  source_path: string;
  destination_connection_id: string;
  destination_path: string;
  strategy?: SyncStrategy;
  source_manifest?: FileManifest[];
  destination_manifest?: FileManifest[];
}

export interface CreateSyncResponse {
  success: boolean;
  job: SyncJob;
  transfers_submitted: number;
  message: string;
}
