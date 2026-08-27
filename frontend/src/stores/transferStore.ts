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

  const activeJobs = computed(() => {
    return jobs.value.filter(
      (j) => j.status === 'running' || j.status === 'queued'
    );
  });

  const activeCount = computed(() => activeJobs.value.length);

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

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.port === '5173' ? '127.0.0.1:8080' : window.location.host;
    const url = `${protocol}//${host}/api/v1/ws`;

    socket = new WebSocket(url);

    socket.onopen = () => {
      isConnected.value = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
    };

    socket.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data);
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
      reconnectTimer = setTimeout(() => {
        connectWs();
      }, 3000);
    };

    socket.onerror = () => {
      socket?.close();
    };
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
