import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import {
  createTransferApi,
  listTransfersApi,
  cancelTransferApi,
  retryTransferApi,
  dismissTransferApi,
  clearFinishedTransfersApi,
} from '../api/transfers';
import { realtimeClient } from '../transport/websocket';
import { publishFileChange } from '../services/fileChangeBus';
import type { TransferJob, TransferType } from '../types/transfer';

export type ConflictResolution = 'replace' | 'skip' | 'keep_both' | 'cancel';

export interface ConflictState {
  isOpen: boolean;
  fileName: string;
  sourcePath: string;
  destPath: string;
  resolve?: (resolution: ConflictResolution, applyToAll: boolean) => void;
}

export interface LiveSpeedMetrics {
  speedBytesPerSec: number;
  etaSeconds: number | null;
}

export const useTransferStore = defineStore('transfer', () => {
  const jobs = ref<TransferJob[]>([]);
  const isDrawerOpen = ref<boolean>(false);
  const isConnected = ref<boolean>(false);
  const isRefreshing = ref<boolean>(false);

  // Live speed tracking (jobId -> { speed, eta })
  const speedMetrics = ref<Record<string, LiveSpeedMetrics>>({});

  // Conflict Resolution State
  const conflictState = ref<ConflictState | null>(null);
  let batchResolution: ConflictResolution | null = null;
  let isRealtimeSubscribed = false;
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  let watchdogInterval: ReturnType<typeof setInterval> | null = null;
  let lastProgressTimestamp = Date.now();

  const activeJobs = computed(() => {
    return jobs.value.filter(
      (j) =>
        j.status === 'running' ||
        j.status === 'queued' ||
        j.status === 'cancellation_requested'
    );
  });

  const activeCount = computed(() => activeJobs.value.length);

  // Background fallback poll while offline or recovering (5,000ms safety interval)
  function startPollingIfNeeded() {
    if (pollInterval || isConnected.value) return;
    pollInterval = setInterval(async () => {
      if (activeCount.value > 0 && !isConnected.value) {
        await fetchJobs();
      } else {
        if (pollInterval) clearInterval(pollInterval);
        pollInterval = null;
      }
    }, 5000);
  }

  // Safety watchdog: if active transfers exist but no progress received for > 5s while connected, reconcile
  function startWatchdogIfNeeded() {
    if (watchdogInterval) return;
    watchdogInterval = setInterval(async () => {
      if (activeCount.value > 0) {
        if (Date.now() - lastProgressTimestamp > 5000) {
          lastProgressTimestamp = Date.now();
          await fetchJobs();
        }
      } else {
        if (watchdogInterval) {
          clearInterval(watchdogInterval);
          watchdogInterval = null;
        }
      }
    }, 2500);
  }

  async function fetchJobs() {
    try {
      jobs.value = await listTransfersApi();
      if (!isConnected.value && activeCount.value > 0) {
        startPollingIfNeeded();
      } else if (activeCount.value > 0) {
        startWatchdogIfNeeded();
      }
    } catch (err) {
      console.error('Failed to fetch transfers', err);
    }
  }

  function updateJobProgress(job: TransferJob) {
    lastProgressTimestamp = Date.now();
    speedMetrics.value[job.id] = {
      speedBytesPerSec: job.speed_bytes_per_sec || 0,
      etaSeconds: job.eta_seconds ?? null,
    };

    const idx = jobs.value.findIndex((j) => j.id === job.id);
    if (idx >= 0) {
      jobs.value[idx] = { ...jobs.value[idx], ...job };
      jobs.value = [...jobs.value];
    } else {
      jobs.value = [job, ...jobs.value];
    }

    if (activeCount.value > 0) {
      startWatchdogIfNeeded();
    }
  }

  function setupRealtimeListeners() {
    if (isRealtimeSubscribed) return;
    isRealtimeSubscribed = true;

    realtimeClient.onProgress((job) => {
      updateJobProgress(job);
    });

    realtimeClient.onCompleted((job) => {
      updateJobProgress(job);
      publishFileChange({
        connectionId: job.destination_connection_id,
        path: job.destination_path,
        action: 'write',
      });
      if (job.transfer_type === 'move') {
        publishFileChange({
          connectionId: job.source_connection_id,
          path: job.source_path,
          action: 'delete',
        });
      }
    });

    realtimeClient.onFailed((job) => {
      updateJobProgress(job);
    });

    realtimeClient.onResyncRequired(() => {
      fetchJobs();
    });

    realtimeClient.onStatusChange((connected) => {
      isConnected.value = connected;
      if (connected) {
        if (pollInterval) {
          clearInterval(pollInterval);
          pollInterval = null;
        }
        fetchJobs();
      } else {
        startPollingIfNeeded();
      }
    });
  }

  function connectWs() {
    setupRealtimeListeners();
    realtimeClient.connect();
  }

  async function submitTransfer(
    name: string,
    transferType: TransferType,
    sourceConnectionId: string,
    sourcePath: string,
    destConnectionId: string,
    destPath: string
  ) {
    const idempotencyKey = crypto.randomUUID();
    const data = await createTransferApi(
      {
        name,
        transfer_type: transferType,
        source_connection_id: sourceConnectionId,
        source_path: sourcePath,
        destination_connection_id: destConnectionId,
        destination_path: destPath,
      },
      idempotencyKey
    );
    isDrawerOpen.value = true;
    await fetchJobs();
    return data;
  }

  async function cancelTransfer(jobId: string) {
    // Optimistically transition to cancellation_requested for instant UI feedback
    const idx = jobs.value.findIndex((j) => j.id === jobId);
    if (idx >= 0) {
      jobs.value[idx] = {
        ...jobs.value[idx],
        status: 'cancellation_requested',
        speed_bytes_per_sec: 0,
        eta_seconds: undefined,
      };
      jobs.value = [...jobs.value];
    }
    try {
      await cancelTransferApi(jobId);
    } catch (err) {
      console.error('Failed to cancel transfer', err);
    }
  }

  const totalSpeedBytesPerSec = computed(() => {
    let total = 0;
    for (const job of activeJobs.value) {
      if (job.status === 'running') {
        const metric = speedMetrics.value[job.id];
        total += metric?.speedBytesPerSec || job.speed_bytes_per_sec || 0;
      }
    }
    return total;
  });

  async function retryTransfer(jobId: string) {
    try {
      await retryTransferApi(jobId);
      const job = jobs.value.find((j) => j.id === jobId);
      if (job) {
        job.status = 'queued';
        job.phase = 'preparing';
        job.error_message = undefined;
        jobs.value = [...jobs.value];
      }
    } catch (err) {
      console.error('retryTransferApi failed', err);
      throw err;
    }
  }

  async function removeJob(jobId: string) {
    jobs.value = jobs.value.filter((j) => j.id !== jobId);
    delete speedMetrics.value[jobId];
    try {
      await dismissTransferApi(jobId);
    } catch (err) {
      console.error('Failed to dismiss transfer on server', err);
    }
  }

  async function clearFinished() {
    jobs.value = jobs.value.filter(
      (j) =>
        j.status === 'running' ||
        j.status === 'queued' ||
        j.status === 'cancellation_requested'
    );
    try {
      await clearFinishedTransfersApi();
    } catch (err) {
      console.error('Failed to clear finished transfers on server', err);
    }
  }

  async function refreshJobs() {
    isRefreshing.value = true;
    try {
      await fetchJobs();
    } finally {
      setTimeout(() => {
        isRefreshing.value = false;
      }, 300);
    }
  }

  // --- CONFLICT RESOLUTION ---

  function requestConflict(fileName: string, sourcePath: string, destPath: string): Promise<ConflictResolution> {
    if (batchResolution !== null) {
      return Promise.resolve(batchResolution);
    }

    return new Promise((resolve) => {
      conflictState.value = {
        isOpen: true,
        fileName,
        sourcePath,
        destPath,
        resolve: (resolution, applyToAll) => {
          if (applyToAll) {
            batchResolution = resolution;
          }
          conflictState.value = null;
          resolve(resolution);
        },
      };
    });
  }

  function resolveConflict(resolution: ConflictResolution, applyToAll: boolean) {
    if (conflictState.value?.resolve) {
      conflictState.value.resolve(resolution, applyToAll);
    }
    conflictState.value = null;
  }

  function resetBatchConflict() {
    batchResolution = null;
    conflictState.value = null;
  }

  return {
    jobs,
    activeJobs,
    activeCount,
    isDrawerOpen,
    isConnected,
    isRefreshing,
    totalSpeedBytesPerSec,
    speedMetrics,
    conflictState,
    fetchJobs,
    refreshJobs,
    connectWs,
    submitTransfer,
    cancelTransfer,
    retryTransfer,
    removeJob,
    clearFinished,
    requestConflict,
    resolveConflict,
    resetBatchConflict,
  };
});
