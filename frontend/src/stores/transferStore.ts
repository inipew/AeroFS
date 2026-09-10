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
import { queryClient } from '../queryClient';
import { queryKeys } from '../api/queryKeys';
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

/** Ephemeral WebSocket state.  REST remains the source of truth for job history. */
export interface LiveTransferProgress {
  transferredBytes: number;
  totalBytes: number;
  speedBytesPerSec: number;
  etaSeconds: number | null;
  phase?: TransferJob['phase'];
  status: TransferJob['status'];
  updatedAt?: string;
  receivedAt: number;
  terminalObserved?: boolean;
  stale?: boolean;
}

const TERMINAL_STATUSES = new Set<TransferJob['status']>([
  'completed', 'failed', 'cancelled', 'interrupted',
]);

export const useTransferStore = defineStore('transfer', () => {
  const isDrawerOpen = ref<boolean>(false);
  const isConnected = ref<boolean>(false);
  const isRefreshing = ref<boolean>(false);

  // Live speed tracking (jobId -> { speed, eta })
  const speedMetrics = ref<Record<string, LiveSpeedMetrics>>({});
  const liveProgress = ref<Record<string, LiveTransferProgress>>({});

  // Conflict Resolution State
  const conflictState = ref<ConflictState | null>(null);
  let batchResolution: ConflictResolution | null = null;
  let isRealtimeSubscribed = false;

  // `QueryClient#getQueryData` is not a Vue reactive source by itself.  Keep
  // only a revision signal here; transfer data remains owned by TanStack Query.
  const queryRevision = ref(0);
  queryClient.getQueryCache().subscribe(() => {
    queryRevision.value++;
    // A terminal WebSocket event is only discarded after the REST source has
    // confirmed it. This prevents a late progress frame from reviving a job.
    const restJobs = queryClient.getQueryData<TransferJob[]>(queryKeys.transfers());
    if (restJobs) {
      for (const job of restJobs) {
        if (TERMINAL_STATUSES.has(job.status) && liveProgress.value[job.id]) {
          delete liveProgress.value[job.id];
          delete speedMetrics.value[job.id];
        }
      }
    }
  });

  const jobs = computed<TransferJob[]>(() => {
    queryRevision.value;
    return queryClient.getQueryData<TransferJob[]>(queryKeys.transfers()) ?? [];
  });

  const activeJobs = computed(() => {
    return displayJobs.value.filter(
      (j) =>
        j.status === 'running' ||
        j.status === 'queued' ||
        j.status === 'cancellation_requested'
    );
  });

  const displayJobs = computed<TransferJob[]>(() => jobs.value.map((job) => {
    const live = liveProgress.value[job.id];
    if (!live || TERMINAL_STATUSES.has(job.status)) return job;
    return {
      ...job,
      transferred_bytes: Math.max(job.transferred_bytes, live.transferredBytes),
      total_bytes: Math.max(job.total_bytes, live.totalBytes),
      speed_bytes_per_sec: live.speedBytesPerSec,
      eta_seconds: live.etaSeconds ?? undefined,
      phase: live.phase ?? job.phase,
      status: live.status,
    };
  }));

  const activeCount = computed(() => activeJobs.value.length);

  async function fetchJobs() {
    isRefreshing.value = true;
    try {
      return await queryClient.fetchQuery({
        queryKey: queryKeys.transfers(),
        queryFn: () => listTransfersApi(),
      });
    } catch (err) {
      console.error('Failed to fetch transfers', err);
      return [];
    } finally {
      isRefreshing.value = false;
    }
  }

  function updateJobProgress(job: TransferJob) {
    const restJob = jobs.value.find((item) => item.id === job.id);
    if (restJob && TERMINAL_STATUSES.has(restJob.status)) return;
    const previous = liveProgress.value[job.id];
    if (previous?.terminalObserved) return;
    const incomingTime = job.updated_at ? Date.parse(job.updated_at) : NaN;
    const previousTime = previous?.updatedAt ? Date.parse(previous.updatedAt) : NaN;
    if (Number.isFinite(incomingTime) && Number.isFinite(previousTime) && incomingTime < previousTime) return;
    // Progress transport is not guaranteed to arrive in order. Never make an
    // active transfer bar move backwards because of an older frame.
    if (previous && job.transferred_bytes < previous.transferredBytes) return;
    const next: LiveTransferProgress = {
      transferredBytes: Math.max(restJob?.transferred_bytes ?? 0, previous?.transferredBytes ?? 0, job.transferred_bytes),
      totalBytes: Math.max(restJob?.total_bytes ?? 0, previous?.totalBytes ?? 0, job.total_bytes),
      speedBytesPerSec: job.speed_bytes_per_sec ?? 0,
      etaSeconds: job.eta_seconds ?? null,
      phase: job.phase,
      status: job.status,
      updatedAt: job.updated_at,
      receivedAt: Date.now(),
      stale: false,
    };
    liveProgress.value[job.id] = next;
    speedMetrics.value[job.id] = {
      speedBytesPerSec: next.speedBytesPerSec,
      etaSeconds: next.etaSeconds,
    };
  }

  function observeTerminal(job?: TransferJob) {
    if (!job) return;
    liveProgress.value[job.id] = {
      ...(liveProgress.value[job.id] ?? {
        transferredBytes: job.transferred_bytes,
        totalBytes: job.total_bytes,
        speedBytesPerSec: 0,
        etaSeconds: null,
        receivedAt: Date.now(),
      }),
      status: job.status,
      phase: job.phase,
      terminalObserved: true,
      stale: false,
      receivedAt: Date.now(),
    };
  }

  function setupRealtimeListeners() {
    if (isRealtimeSubscribed) return;
    isRealtimeSubscribed = true;

    realtimeClient.onProgress((job) => {
      updateJobProgress(job);
    });
    // RealtimeSync owns invalidation/refetching. These listeners only protect
    // the local overlay while that authoritative REST reconciliation happens.
    realtimeClient.onCompleted(observeTerminal);
    realtimeClient.onFailed(observeTerminal);
    realtimeClient.onCancelled(observeTerminal);

    realtimeClient.onStatusChange((connected) => {
      isConnected.value = connected;
      if (connected) {
        fetchJobs().then(() => {
          Object.values(liveProgress.value).forEach((progress) => { progress.stale = false; });
        });
      } else {
        Object.values(liveProgress.value).forEach((progress) => { progress.stale = true; });
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
    queryClient.setQueryData<TransferJob[]>(queryKeys.transfers(), (old) => {
      return (old ?? []).map((j) =>
        j.id === jobId
          ? { ...j, status: 'cancellation_requested', speed_bytes_per_sec: 0, eta_seconds: undefined }
          : j
      );
    });
    try {
      await cancelTransferApi(jobId);
    } catch (err) {
      console.error('Failed to cancel transfer', err);
    } finally {
      await queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
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
      queryClient.setQueryData<TransferJob[]>(queryKeys.transfers(), (old) => {
        return (old ?? []).map((j) =>
          j.id === jobId
            ? { ...j, status: 'queued', phase: 'preparing', error_message: undefined }
            : j
        );
      });
      await queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
    } catch (err) {
      console.error('retryTransferApi failed', err);
      throw err;
    }
  }

  async function removeJob(jobId: string) {
    queryClient.setQueryData<TransferJob[]>(queryKeys.transfers(), (old) => {
      return (old ?? []).filter((j) => j.id !== jobId);
    });
    delete speedMetrics.value[jobId];
    delete liveProgress.value[jobId];
    try {
      await dismissTransferApi(jobId);
    } catch (err) {
      console.error('Failed to dismiss transfer on server', err);
    } finally {
      await queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
    }
  }

  async function clearFinished() {
    queryClient.setQueryData<TransferJob[]>(queryKeys.transfers(), (old) => {
      return (old ?? []).filter(
        (j) =>
          j.status === 'running' ||
          j.status === 'queued' ||
          j.status === 'cancellation_requested'
      );
    });
    try {
      await clearFinishedTransfersApi();
    } catch (err) {
      console.error('Failed to clear finished transfers on server', err);
    } finally {
      await queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
    }
  }

  async function refreshJobs() {
    isRefreshing.value = true;
    try {
      await queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
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
    displayJobs,
    activeJobs,
    activeCount,
    isDrawerOpen,
    isConnected,
    isRefreshing,
    totalSpeedBytesPerSec,
    speedMetrics,
    liveProgress,
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
