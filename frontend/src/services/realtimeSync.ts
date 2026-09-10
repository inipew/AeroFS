import {
  realtimeClient,
  type FileChangeEvent,
  type ResyncRequiredEvent,
} from '../transport/websocket';
import { queryClient } from '../queryClient';
import { queryKeys } from '../api/queryKeys';
import { parentPath, normalizePath } from '../utils/path';

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
      realtimeClient.onCompleted(() => this.handleTransferChange()),
      realtimeClient.onFailed(() => this.handleTransferChange()),
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

    if (event.old_path || event.old_parent_path) {
      const oldDir = event.old_parent_path
        ? normalizePath(event.old_parent_path)
        : parentPath(event.old_path!);
      if (oldDir !== targetDir) {
        this.queueDirectoryInvalidation(connId, oldDir);
      }
    }
  }

  private handleTransferChange(): void {
    queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
  }

  private handlePermissionChange(_event: { user_id: string; connection_id: string }): void {
    queryClient.invalidateQueries({ queryKey: queryKeys.connections() });
    queryClient.invalidateQueries({ queryKey: queryKeys.directories() });
  }

  private handleResync(event: ResyncRequiredEvent): void {
    console.warn('[RealtimeSync] Resync required:', event.reason);
    queryClient.invalidateQueries();
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
            const key = query.queryKey;
            return (
              Array.isArray(key) &&
              key[0] === 'directory' &&
              key[1] === connectionId &&
              (key[2] === dirPath || dirPath === '/')
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
