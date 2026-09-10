import { describe, expect, it } from 'bun:test';
import { RealtimeSyncCoordinator } from '../src/services/realtimeSync';
import { realtimeClient } from '../src/transport/websocket';
import { queryClient } from '../src/queryClient';
import type { TransferJob } from '../src/types/transfer';

describe('RealtimeSyncCoordinator', () => {
  it('coalesces multiple invalidations on the same directory', () => {
    const coordinator = new RealtimeSyncCoordinator();

    coordinator.queueDirectoryInvalidation('conn-1', '/documents');
    coordinator.queueDirectoryInvalidation('conn-1', '/documents');
    coordinator.queueDirectoryInvalidation('conn-1', '/documents/');

    expect(coordinator.getPendingCount()).toBe(1);
    coordinator.stop();
  });

  it('keeps distinct directories and connections separated in queue', () => {
    const coordinator = new RealtimeSyncCoordinator();

    coordinator.queueDirectoryInvalidation('conn-1', '/documents');
    coordinator.queueDirectoryInvalidation('conn-1', '/photos');
    coordinator.queueDirectoryInvalidation('conn-2', '/documents');

    expect(coordinator.getPendingCount()).toBe(3);
    coordinator.stop();
  });

  it('flushes pending directories cleanly', () => {
    const coordinator = new RealtimeSyncCoordinator();

    coordinator.queueDirectoryInvalidation('conn-1', '/downloads');
    expect(coordinator.getPendingCount()).toBe(1);

    coordinator.flush();
    expect(coordinator.getPendingCount()).toBe(0);
    coordinator.stop();
  });

  it('automatically flushes after debounce interval', async () => {
    const coordinator = new RealtimeSyncCoordinator();

    coordinator.queueDirectoryInvalidation('conn-1', '/music');
    expect(coordinator.getPendingCount()).toBe(1);

    // Wait slightly longer than 150ms debounce
    await new Promise((resolve) => setTimeout(resolve, 200));

    expect(coordinator.getPendingCount()).toBe(0);
    coordinator.stop();
  });

  it('handles transfer move by queueing both source and destination parent dirs', () => {
    const coordinator = new RealtimeSyncCoordinator();
    coordinator.start();

    const moveJob: TransferJob = {
      id: 'job-move-1',
      transfer_type: 'move',
      source_connection_id: 'conn-source',
      source_path: '/folderA/file.txt',
      destination_connection_id: 'conn-dest',
      destination_path: '/folderB/file.txt',
      status: 'completed',
      bytes_transferred: 100,
      total_bytes: 100,
      created_at: new Date().toISOString(),
    };

    // Trigger onCompleted via transport listeners
    (realtimeClient as any).completedListeners.forEach((l: any) => l(moveJob));

    // Should queue both source parent and destination parent directories
    expect(coordinator.getPendingCount()).toBe(2);

    coordinator.flush();
    coordinator.stop();
  });

  it('handles file_change events and invalidates metadata query', () => {
    const coordinator = new RealtimeSyncCoordinator();
    coordinator.start();

    let invalidatedMetadata = false;
    const origInvalidate = queryClient.invalidateQueries.bind(queryClient);
    queryClient.invalidateQueries = ((filters?: any) => {
      if (filters?.queryKey && filters.queryKey[0] === 'metadata') {
        invalidatedMetadata = true;
      }
      return origInvalidate(filters);
    }) as any;

    try {
      (realtimeClient as any).fileChangeListeners.forEach((l: any) =>
        l({
          type: 'created',
          connection_id: 'conn-1',
          path: '/docs/newfile.txt',
          parent_path: '/docs',
        })
      );

      expect(coordinator.getPendingCount()).toBe(1);
      expect(invalidatedMetadata).toBe(true);
    } finally {
      queryClient.invalidateQueries = origInvalidate;
      coordinator.stop();
    }
  });

  it('handles permission_changed events by invalidating connection and capabilities', () => {
    const coordinator = new RealtimeSyncCoordinator();
    coordinator.start();

    const invalidatedKeys: any[] = [];
    const origInvalidate = queryClient.invalidateQueries.bind(queryClient);
    queryClient.invalidateQueries = ((filters?: any) => {
      if (filters?.queryKey) {
        invalidatedKeys.push(filters.queryKey);
      }
      return origInvalidate(filters);
    }) as any;

    try {
      (realtimeClient as any).permissionListeners.forEach((l: any) =>
        l({
          user_id: 'user-1',
          connection_id: 'conn-sftp',
        })
      );

      // Should invalidate connections, connection('conn-sftp'), capabilities('conn-sftp'), directoryConnection('conn-sftp')
      const hasCap = invalidatedKeys.some(
        (k) => Array.isArray(k) && k[0] === 'capabilities' && k[1] === 'conn-sftp'
      );
      expect(hasCap).toBe(true);
    } finally {
      queryClient.invalidateQueries = origInvalidate;
      coordinator.stop();
    }
  });
});
