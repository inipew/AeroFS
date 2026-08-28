import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { apiClient } from '../api/client';
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

  // Live speed tracking (jobId -> { lastBytes, lastTimestamp, speed, eta })
  const speedMetrics = ref<Record<string, LiveSpeedMetrics>>({});

  // Conflict Resolution State
  const conflictState = ref<ConflictState | null>(null);
  let batchResolution: ConflictResolution | null = null;

  let socket: WebSocket | null = null;
  let reconnectTimer: any = null;
  let pollInterval: any = null;
  let lastSequence = 0;

  const activeJobs = computed(() => {
    return jobs.value.filter(
      (j) => j.status === 'running' || j.status === 'queued'
    );
  });

  const activeCount = computed(() => activeJobs.value.length);

  // Background fallback poll while jobs are active
  function startPollingIfNeeded() {
    if (pollInterval) return;
    pollInterval = setInterval(async () => {
      if (activeCount.value > 0) {
        await fetchJobs();
      } else {
        clearInterval(pollInterval);
        pollInterval = null;
      }
    }, 1000);
  }

  async function fetchJobs() {
    try {
      const resp = await apiClient.get<TransferJob[]>('/transfers');
      jobs.value = resp.data;
    } catch (err) {
      console.error('Failed to fetch transfers', err);
    }
  }

  function updateJobProgress(job: TransferJob) {
    speedMetrics.value[job.id] = {
      speedBytesPerSec: job.speed_bytes_per_sec || 0,
      etaSeconds: job.eta_seconds ?? null,
    };

    const idx = jobs.value.findIndex((j) => j.id === job.id);
    if (idx >= 0) {
      jobs.value[idx] = job;
    } else {
      jobs.value.unshift(job);
    }
  }

  function connectWs() {
    if (socket && socket.readyState === WebSocket.OPEN) return;

    // Use current origin to preserve cookie & proxy across ports
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const url = `${protocol}//${window.location.host}/api/v1/ws`;

    try {
      socket = new WebSocket(url);

      socket.onopen = () => {
        isConnected.value = true;
        lastSequence = 0;
        if (reconnectTimer) {
          clearTimeout(reconnectTimer);
          reconnectTimer = null;
        }
      };

      socket.onmessage = (event) => {
        try {
          const payload = JSON.parse(event.data);

          // Sequence Gap Detection & Auto-Resync
          if (typeof payload.sequence === 'number') {
            if (lastSequence > 0 && payload.sequence > lastSequence + 1) {
              console.warn(`WS sequence gap detected (${lastSequence} -> ${payload.sequence}). Re-syncing transfer state.`);
              fetchJobs();
            }
            lastSequence = payload.sequence;
          }

          if (
            payload.type === 'transfer_progress' ||
            payload.type === 'transfer_completed' ||
            payload.type === 'transfer_failed'
          ) {
            updateJobProgress(payload.data);
          }
        } catch (e) {
          console.error('WS Parse Error', e);
        }
      };

      socket.onclose = () => {
        isConnected.value = false;
        socket = null;
        if (!reconnectTimer) {
          reconnectTimer = setTimeout(() => {
            reconnectTimer = null;
            connectWs();
          }, 3000);
        }
      };

      socket.onerror = () => {
        socket?.close();
      };
    } catch (err) {
      console.error('WebSocket connection error', err);
    }
  }

  async function submitTransfer(
    name: string,
    transferType: TransferType,
    sourceConnectionId: string,
    sourcePath: string,
    destConnectionId: string,
    destPath: string
  ) {
    const resp = await apiClient.post('/transfers', {
      name,
      transfer_type: transferType,
      source_connection_id: sourceConnectionId,
      source_path: sourcePath,
      destination_connection_id: destConnectionId,
      destination_path: destPath,
    });
    isDrawerOpen.value = true;
    startPollingIfNeeded();
    await fetchJobs();
    return resp.data;
  }

  async function cancelTransfer(jobId: string) {
    try {
      await apiClient.post(`/transfers/${jobId}/cancel`);
      const job = jobs.value.find((j) => j.id === jobId);
      if (job) job.status = 'cancelled';
    } catch (err) {
      console.error('Failed to cancel transfer', err);
    }
  }

  const isRefreshing = ref<boolean>(false);

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
    const job = jobs.value.find((j) => j.id === jobId);
    if (!job) return;
    await submitTransfer(
      job.name,
      job.transfer_type,
      job.source_connection_id,
      job.source_path,
      job.destination_connection_id,
      job.destination_path
    );
  }

  async function removeJob(jobId: string) {
    jobs.value = jobs.value.filter((j) => j.id !== jobId);
    delete speedMetrics.value[jobId];
    try {
      await apiClient.post(`/transfers/${jobId}/dismiss`);
    } catch (err) {
      console.error('Failed to dismiss transfer on server', err);
    }
  }

  async function clearFinished() {
    jobs.value = jobs.value.filter(
      (j) => j.status === 'running' || j.status === 'queued'
    );
    try {
      await apiClient.post('/transfers/clear-finished');
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
