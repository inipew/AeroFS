import {
  realtimeClient,
  type FileChangeEvent,
  type ResyncRequiredEvent,
} from '../transport/websocket';
import { queryClient } from '../queryClient';
import { queryKeys, isDirectoryQueryFor } from '../api/queryKeys';
import { parentPath, normalizePath } from '../utils/path';
import type { TransferJob } from '../types/transfer';

export class RealtimeSyncCoordinator {
  private pendingDirectories = new Map<string, Set<string>>(); // connectionId -> Set<dirPath>
  private debounceTimer: ReturnType<typeof setTimeout> | null = null;
  public readonly debounceMs = 150;
  private unsubs: Array<() => void> = [];
  private isRunning = false;

  public start(): void {
    if (this.isRunning) return;
    this.isRunning = true;

    this.unsubs.push(
      realtimeClient.onFileChange((event) => this.handleFileChange(event)),
      realtimeClient.onCompleted((job) => this.handleTransferCompleted(job)),
      realtimeClient.onFailed((job) => this.handleTransferFailed(job)),
      realtimeClient.onCancelled((job) => this.handleTransferCancelled(job)),
      realtimeClient.onPermissionChanged((event) => this.handlePermissionChange(event)),
      realtimeClient.onResyncRequired((event) => this.handleResync(event))
    );
  }

  public stop(): void {
    this.unsubs.forEach((unsub) => unsub());
    this.unsubs = [];
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }
    this.pendingDirectories.clear();
    this.isRunning = false;
  }

  public invalidateDirectory(connectionId: string, dirPath: string): void {
    this.queueDirectoryInvalidation(connectionId, dirPath);
  }

  private handleFileChange(event: FileChangeEvent): void {
    const connId = event.connection_id;
    const targetDir = event.parent_path ? normalizePath(event.parent_path) : parentPath(event.path);
    this.queueDirectoryInvalidation(connId, targetDir);

    // Invalidate metadata for mutated path
    queryClient.invalidateQueries({ queryKey: queryKeys.metadata(connId, event.path) });

    if (event.old_path || event.old_parent_path) {
      const oldDir = event.old_parent_path
        ? normalizePath(event.old_parent_path)
        : parentPath(event.old_path!);
      if (oldDir !== targetDir) {
        this.queueDirectoryInvalidation(connId, oldDir);
      }
      if (event.old_path) {
        queryClient.invalidateQueries({ queryKey: queryKeys.metadata(connId, event.old_path) });
      }
    }
  }

  private handleTransferCompleted(job?: TransferJob): void {
    queryClient.invalidateQueries({ queryKey: queryKeys.transfers(), refetchType: 'active' });
    if (job) {
      if (job.destination_connection_id && job.destination_path) {
        this.queueDirectoryInvalidation(job.destination_connection_id, parentPath(job.destination_path));
      }
      if (job.transfer_type === 'move' && job.source_connection_id && job.source_path) {
        this.queueDirectoryInvalidation(job.source_connection_id, parentPath(job.source_path));
      }
    }
  }

  private handleTransferFailed(_job?: TransferJob): void {
    queryClient.invalidateQueries({ queryKey: queryKeys.transfers(), refetchType: 'active' });
  }

  private handleTransferCancelled(_job?: TransferJob): void {
    queryClient.invalidateQueries({ queryKey: queryKeys.transfers(), refetchType: 'active' });
  }

  private handlePermissionChange(event: { user_id: string; connection_id: string }): void {
    queryClient.invalidateQueries({ queryKey: queryKeys.connections() });
    queryClient.invalidateQueries({ queryKey: queryKeys.connection(event.connection_id) });
    queryClient.invalidateQueries({ queryKey: queryKeys.capabilities(event.connection_id) });
    queryClient.invalidateQueries({ queryKey: queryKeys.directoryConnection(event.connection_id) });
  }

  private handleResync(event: ResyncRequiredEvent): void {
    console.warn('[RealtimeSync] Targeted resync triggered:', event.reason);
    queryClient.invalidateQueries({ queryKey: queryKeys.directories(), refetchType: 'active' });
    queryClient.invalidateQueries({ queryKey: queryKeys.metadataPrefix(), refetchType: 'active' });
    queryClient.invalidateQueries({ queryKey: queryKeys.capabilitiesPrefix(), refetchType: 'active' });
    queryClient.invalidateQueries({ queryKey: queryKeys.connections(), refetchType: 'active' });
    queryClient.invalidateQueries({ queryKey: queryKeys.transfers(), refetchType: 'active' });
    queryClient.invalidateQueries({ queryKey: queryKeys.syncJobs(), refetchType: 'active' });
  }

  public queueDirectoryInvalidation(connectionId: string, dirPath: string): void {
    if (!this.pendingDirectories.has(connectionId)) {
      this.pendingDirectories.set(connectionId, new Set());
    }
    this.pendingDirectories.get(connectionId)!.add(normalizePath(dirPath));

    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }
    this.debounceTimer = setTimeout(() => {
      this.flush();
    }, this.debounceMs);
  }

  public flush(): void {
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }

    for (const [connectionId, dirPaths] of this.pendingDirectories.entries()) {
      for (const dirPath of dirPaths) {
        queryClient.invalidateQueries({
          predicate: (query) => {
            return isDirectoryQueryFor(
              query.queryKey,
              connectionId,
              dirPath === '/' ? undefined : dirPath
            );
          },
        });
      }
    }
    this.pendingDirectories.clear();
  }

  public getPendingCount(): number {
    let count = 0;
    for (const set of this.pendingDirectories.values()) {
      count += set.size;
    }
    return count;
  }
}

export const realtimeSync = new RealtimeSyncCoordinator();
