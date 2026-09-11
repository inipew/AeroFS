import { describe, it, expect, beforeEach, spyOn } from 'bun:test';
import { setActivePinia, createPinia } from 'pinia';
import { useTransferStore } from '../src/stores/transferStore';
import { queryClient } from '../src/queryClient';
import { queryKeys } from '../src/api/queryKeys';
import * as transferApi from '../src/api/transfers';
import type { TransferJob } from '../src/types/transfer';

describe('Transfer capabilities and lifecycle invariants', () => {
  let cancelSpy: any;
  let retrySpy: any;

  beforeEach(() => {
    setActivePinia(createPinia());
    queryClient.clear();
    if (!cancelSpy) {
      cancelSpy = spyOn(transferApi, 'cancelTransferApi');
    }
    cancelSpy.mockReset();
    cancelSpy.mockResolvedValue(true);

    if (!retrySpy) {
      retrySpy = spyOn(transferApi, 'retryTransferApi');
    }
    retrySpy.mockReset();
    retrySpy.mockResolvedValue(true);
  });

  it('cancelTransfer calls server API exactly once and transitions to cancellation_requested', async () => {
    const store = useTransferStore();

    const serverJob: TransferJob = {
      id: 'job-server-123',
      name: 'file.txt',
      transfer_type: 'upload',
      source_connection_id: 'upload',
      source_path: 'upload://job-server-123',
      destination_connection_id: 'local',
      destination_path: '/dst/file.txt',
      status: 'running',
      phase: 'transferring',
      execution_mode: 'inline',
      staging: 'none',
      transferred_bytes: 50,
      total_bytes: 100,
      speed_bytes_per_sec: 10,
      capabilities: {
        can_cancel: true,
        can_pause: false,
        can_resume: false,
        can_retry: false,
      },
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    queryClient.setQueryData(queryKeys.transfers(), [serverJob]);

    await store.cancelTransfer('job-server-123');

    // API must be called exactly once for server transfer
    expect(cancelSpy).toHaveBeenCalledTimes(1);
    expect(cancelSpy).toHaveBeenCalledWith('job-server-123');

    // Active progress must be cancellation_requested, NOT prematurely marked as terminal
    const live = store.liveProgress['job-server-123'];
    expect(live).toBeDefined();
    expect(live?.status).toBe('cancellation_requested');
    expect(live?.terminalObserved).not.toBe(true);
  });

  it('synthetic queued upload cancellation does NOT call server API', async () => {
    const store = useTransferStore();

    const tempId = 'queued-upload-abc';
    store.registerLiveTransfer({
      id: tempId,
      name: 'pending.txt',
      totalBytes: 500,
      transferType: 'upload',
      sourceConnectionId: 'upload',
      sourcePath: `upload://${tempId}`,
      destinationConnectionId: 'local',
      destinationPath: '/dst/pending.txt',
      phase: 'preparing',
    });

    expect(store.liveProgress[tempId]).toBeDefined();

    await store.cancelTransfer(tempId);

    // Synthetic queue item cancellation must NOT hit the server API
    expect(cancelSpy).toHaveBeenCalledTimes(0);
    expect(store.liveProgress[tempId]).toBeUndefined();
  });

  it('cancellation 409 conflict or error triggers reconciliation and query invalidation', async () => {
    cancelSpy.mockRejectedValue(new Error('409 Conflict'));
    const invalidateSpy = spyOn(queryClient, 'invalidateQueries');
    const consoleSpy = spyOn(console, 'error').mockImplementation(() => {});
    const store = useTransferStore();

    const finalizingJob: TransferJob = {
      id: 'job-finalizing-999',
      name: 'data.bin',
      transfer_type: 'upload',
      source_connection_id: 'upload',
      source_path: 'upload://job-finalizing-999',
      destination_connection_id: 'local',
      destination_path: '/dst/data.bin',
      status: 'running',
      phase: 'finalizing',
      execution_mode: 'inline',
      staging: 'none',
      transferred_bytes: 100,
      total_bytes: 100,
      speed_bytes_per_sec: 0,
      capabilities: {
        can_cancel: false,
        can_pause: false,
        can_resume: false,
        can_retry: false,
      },
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    queryClient.setQueryData(queryKeys.transfers(), [finalizingJob]);

    await expect(store.cancelTransfer('job-finalizing-999')).rejects.toThrow('409 Conflict');

    expect(cancelSpy).toHaveBeenCalledTimes(1);
    // On failure, cache must be invalidated to reconcile true server state
    expect(invalidateSpy).toHaveBeenCalled();

    invalidateSpy.mockRestore();
    consoleSpy.mockRestore();
  });

  it('retryTransfer strictly obeys can_retry capability and rejects non-retryable transfers', async () => {
    const store = useTransferStore();

    const inlineFailedJob: TransferJob = {
      id: 'job-upload-failed',
      name: 'failed_upload.txt',
      transfer_type: 'upload',
      source_connection_id: 'upload',
      source_path: 'upload://job-upload-failed',
      destination_connection_id: 'local',
      destination_path: '/dst/failed_upload.txt',
      status: 'failed',
      phase: 'completed',
      execution_mode: 'inline',
      staging: 'none',
      transferred_bytes: 50,
      total_bytes: 100,
      speed_bytes_per_sec: 0,
      capabilities: {
        can_cancel: false,
        can_pause: false,
        can_resume: false,
        can_retry: false, // Upload + Inline cannot be retried
      },
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    const backgroundFailedJob: TransferJob = {
      id: 'job-copy-failed',
      name: 'failed_copy.txt',
      transfer_type: 'copy',
      source_connection_id: 'local',
      source_path: '/src/failed_copy.txt',
      destination_connection_id: 'remote',
      destination_path: '/dst/failed_copy.txt',
      status: 'failed',
      phase: 'transferring',
      execution_mode: 'background',
      staging: 'none',
      transferred_bytes: 10,
      total_bytes: 100,
      speed_bytes_per_sec: 0,
      capabilities: {
        can_cancel: false,
        can_pause: false,
        can_resume: false,
        can_retry: true, // Copy Background can be retried
      },
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    queryClient.setQueryData(queryKeys.transfers(), [inlineFailedJob, backgroundFailedJob]);

    // 1. Non-existent job ID: must not call API
    await store.retryTransfer('non-existent-id');
    expect(retrySpy).toHaveBeenCalledTimes(0);

    // 2. Inline upload with can_retry: false: must not call API
    await store.retryTransfer('job-upload-failed');
    expect(retrySpy).toHaveBeenCalledTimes(0);

    // 3. Background copy with can_retry: true: must call API once
    await store.retryTransfer('job-copy-failed');
    expect(retrySpy).toHaveBeenCalledTimes(1);
    expect(retrySpy).toHaveBeenCalledWith('job-copy-failed');
  });
});
