import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { apiClient } from '../api/client';
import type { TransferJob, TransferType } from '../types/transfer';

export const useTransferStore = defineStore('transfer', () => {
  const jobs = ref<TransferJob[]>([]);
  const isDrawerOpen = ref<boolean>(false);
  const isConnected = ref<boolean>(false);
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
            const updatedJob: TransferJob = payload.data;
            const idx = jobs.value.findIndex((j) => j.id === updatedJob.id);
            if (idx >= 0) {
              jobs.value[idx] = updatedJob;
            } else {
              jobs.value.unshift(updatedJob);
            }
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

  function clearFinished() {
    jobs.value = jobs.value.filter(
      (j) => j.status === 'running' || j.status === 'queued'
    );
  }

  return {
    jobs,
    activeJobs,
    activeCount,
    isDrawerOpen,
    isConnected,
    fetchJobs,
    connectWs,
    submitTransfer,
    cancelTransfer,
    clearFinished,
  };
});
