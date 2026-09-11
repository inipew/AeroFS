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
import { uploadFileAsTransferApi, type UploadSessionResponse } from '../api/files';
import type { TransferJob, TransferType } from '../types/transfer';
import { useUiStore } from './uiStore';

export type ConflictResolution = 'replace' | 'skip' | 'keep_both' | 'cancel';

export interface ConflictState {
  isOpen: boolean;
  fileName: string;
  sourcePath: string;
  destPath: string;
  resolve?: (resolution: ConflictResolution, applyToAll: boolean) => void;
}

export interface UploadBatchItem {
  file: File;
  targetDir?: string;
}

export interface UploadBatchOptions {
  connectionId: string;
  targetDir: string;
  files: (File | UploadBatchItem)[];
  existingNames?: string[];
  concurrency?: number;
  uploader?: (
    connectionId: string,
    targetDir: string,
    file: File,
    signal: AbortSignal,
    onProgress?: (percent: number, loaded: number, total: number) => void,
    onSession?: (session: UploadSessionResponse) => void
  ) => Promise<UploadSessionResponse>;
}

export function generateUniqueName(name: string, existingNames: Set<string>): string {
  const dotIdx = name.lastIndexOf('.');
  const base = dotIdx > 0 ? name.substring(0, dotIdx) : name;
  const ext = dotIdx > 0 ? name.substring(dotIdx) : '';
  let counter = 1;
  while (true) {
    const candidate = `${base} (${counter})${ext}`;
    if (!existingNames.has(candidate)) {
      return candidate;
    }
    counter++;
  }
}

export interface LiveSpeedMetrics {
  speedBytesPerSec: number;
  etaSeconds: number | null;
}

/** Ephemeral WebSocket state.  REST remains the source of truth for job history. */
export interface LiveTransferProgress {
  name?: string;
  transferType?: TransferType;
  sourceConnectionId?: string;
  sourcePath?: string;
  destinationConnectionId?: string;
  destinationPath?: string;
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

/** Materialize a transfer first discovered through realtime. Existing REST
 * snapshots are intentionally kept untouched; subsequent ticks belong only
 * in the volatile progress overlay. */
export function materializeTransferJob(
  current: TransferJob[] | undefined,
  incoming: TransferJob
): TransferJob[] {
  const jobs = current ?? [];
  if (jobs.some((job) => job.id === incoming.id)) return jobs;

  return [...jobs, incoming].sort((a, b) => {
    const aTime = Date.parse(a.created_at);
    const bTime = Date.parse(b.created_at);
    return (Number.isFinite(bTime) ? bTime : 0) - (Number.isFinite(aTime) ? aTime : 0);
  });
}

export const useTransferStore = defineStore('transfer', () => {
  const isDrawerOpen = ref<boolean>(false);
  const isConnected = ref<boolean>(false);
  const isRefreshing = ref<boolean>(false);

  // Live speed tracking (jobId -> { speed, eta })
  const speedMetrics = ref<Record<string, LiveSpeedMetrics>>({});
  const liveProgress = ref<Record<string, LiveTransferProgress>>({});

  // Active in-flight upload abort controllers (jobId -> AbortController)
  const activeUploadControllers = new Map<string, AbortController>();
  // Queued upload cancellation tokens (tempId -> cancel callback)
  const queuedUploadTokens = new Map<string, { cancel: () => void }>();

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

  const displayJobs = computed<TransferJob[]>(() => {
    const knownIds = new Set<string>();
    const mapped = jobs.value.map((job) => {
      knownIds.add(job.id);
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
    });

    for (const [id, live] of Object.entries(liveProgress.value)) {
      if (!knownIds.has(id) && !live.terminalObserved) {
        mapped.unshift({
          id,
          user_id: undefined,
          name: live.name || 'Transfer',
          transfer_type: live.transferType || 'upload',
          source_connection_id: live.sourceConnectionId || 'upload',
          source_path: live.sourcePath || `upload://${id}`,
          destination_connection_id: live.destinationConnectionId || '',
          destination_path: live.destinationPath || '',
          status: live.status,
          phase: live.phase ?? 'transferring',
          execution_mode: 'inline',
          staging: 'none',
          transferred_bytes: live.transferredBytes,
          total_bytes: live.totalBytes,
          speed_bytes_per_sec: live.speedBytesPerSec,
          eta_seconds: live.etaSeconds ?? undefined,
          created_at: new Date(live.receivedAt).toISOString(),
          updated_at: live.updatedAt || new Date(live.receivedAt).toISOString(),
        });
      }
    }

    return mapped;
  });

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

  function ensureRealtimeJob(job: TransferJob) {
    const current = queryClient.getQueryData<TransferJob[]>(queryKeys.transfers());
    if (current?.some((item) => item.id === job.id)) return;
    queryClient.setQueryData<TransferJob[]>(queryKeys.transfers(), (current) =>
      materializeTransferJob(current, job)
    );
  }

  function updateJobProgress(job: TransferJob) {
    ensureRealtimeJob(job);
    const restJob = jobs.value.find((item) => item.id === job.id);
    if (restJob && TERMINAL_STATUSES.has(restJob.status)) return;
    const previous = liveProgress.value[job.id];
    if (previous?.terminalObserved) return;
    const incomingTime = job.updated_at ? Date.parse(job.updated_at) : NaN;
    const previousTime = previous?.updatedAt ? Date.parse(previous.updatedAt) : NaN;
    if (Number.isFinite(incomingTime) && Number.isFinite(previousTime) && incomingTime < previousTime) return;
    // Client-side upload bytes can be ahead of server acknowledgement. Keep
    // bytes monotonic while still accepting newer server phase/speed updates.
    const next: LiveTransferProgress = {
      name: job.name,
      transferType: job.transfer_type,
      sourceConnectionId: job.source_connection_id,
      sourcePath: job.source_path,
      destinationConnectionId: job.destination_connection_id,
      destinationPath: job.destination_path,
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

  function registerLiveTransfer(meta: {
    id: string;
    name: string;
    transferType: TransferType;
    totalBytes: number;
    sourceConnectionId: string;
    sourcePath: string;
    destinationConnectionId: string;
    destinationPath: string;
  }) {
    const previous = liveProgress.value[meta.id];
    if (previous?.terminalObserved) return;
    liveProgress.value[meta.id] = {
      ...previous,
      name: meta.name,
      transferType: meta.transferType,
      sourceConnectionId: meta.sourceConnectionId,
      sourcePath: meta.sourcePath,
      destinationConnectionId: meta.destinationConnectionId,
      destinationPath: meta.destinationPath,
      transferredBytes: previous?.transferredBytes ?? 0,
      totalBytes: Math.max(previous?.totalBytes ?? 0, meta.totalBytes),
      speedBytesPerSec: previous?.speedBytesPerSec ?? 0,
      etaSeconds: previous?.etaSeconds ?? null,
      phase: previous?.phase ?? 'transferring',
      status: previous?.status ?? 'running',
      receivedAt: Date.now(),
      stale: false,
    };
    isDrawerOpen.value = true;
    queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
  }

  /** One upload lifecycle for dialogs, store actions, and external drag/drop. */
  async function uploadTrackedFile(
    connectionId: string,
    targetDir: string,
    file: File,
    signal: AbortSignal,
    onProgress?: (percent: number, loaded: number, total: number) => void,
    onSession?: (session: UploadSessionResponse) => void
  ): Promise<UploadSessionResponse> {
    let jobId: string | undefined;
    return uploadFileAsTransferApi(
      connectionId,
      targetDir,
      file,
      signal,
      (percent, loaded, total) => {
        if (jobId) updateClientProgress(jobId, loaded, total);
        onProgress?.(percent, loaded, total);
      },
      (session) => {
        jobId = session.job_id;
        const destinationPath = targetDir === '/'
          ? `/${file.name}`
          : `${targetDir.replace(/\/$/, '')}/${file.name}`;
        registerLiveTransfer({
          id: session.job_id,
          name: file.name,
          transferType: 'upload',
          totalBytes: file.size,
          sourceConnectionId: 'upload',
          sourcePath: `upload://${session.job_id}`,
          destinationConnectionId: connectionId,
          destinationPath,
        });
        onSession?.(session);
        // REST bootstrap is deliberately detached so streaming starts at once.
        void fetchJobs();
      }
    );
  }

  function updateClientProgress(jobId: string, loaded: number, total: number) {
    const prev = liveProgress.value[jobId];
    if (prev?.terminalObserved) return;
    const now = Date.now();
    const normalizedTotal = Math.max(total, prev?.totalBytes ?? 0);
    const normalizedLoaded = Math.min(
      normalizedTotal || Number.MAX_SAFE_INTEGER,
      Math.max(loaded, prev?.transferredBytes ?? 0)
    );
    let speed = 0;
    let eta: number | null = null;
    if (prev && prev.receivedAt) {
      const timeDelta = (now - prev.receivedAt) / 1000;
      const bytesDelta = normalizedLoaded - prev.transferredBytes;
      if (timeDelta > 0.1 && bytesDelta >= 0) {
        speed = Math.round(bytesDelta / timeDelta);
        if (speed > 0 && normalizedTotal > normalizedLoaded) {
          eta = Math.round((normalizedTotal - normalizedLoaded) / speed);
        }
      } else {
        speed = prev.speedBytesPerSec;
        eta = prev.etaSeconds;
      }
    }

    liveProgress.value[jobId] = {
      ...(prev ?? { status: 'running', phase: 'transferring' }),
      transferredBytes: normalizedLoaded,
      totalBytes: normalizedTotal,
      speedBytesPerSec: speed,
      etaSeconds: eta,
      receivedAt: now,
      stale: false,
    };
    speedMetrics.value[jobId] = {
      speedBytesPerSec: speed,
      etaSeconds: eta,
    };
  }

  function observeTerminal(job?: TransferJob) {
    if (!job) return;
    ensureRealtimeJob(job);
    liveProgress.value[job.id] = {
      ...(liveProgress.value[job.id] ?? {
        name: job.name,
        transferType: job.transfer_type,
        sourceConnectionId: job.source_connection_id,
        sourcePath: job.source_path,
        destinationConnectionId: job.destination_connection_id,
        destinationPath: job.destination_path,
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
    registerLiveTransfer({
      id: data.job_id,
      name,
      transferType,
      totalBytes: 0,
      sourceConnectionId,
      sourcePath,
      destinationConnectionId: destConnectionId,
      destinationPath: destPath,
    });
    isDrawerOpen.value = true;
    await fetchJobs();
    return data;
  }

  async function cancelTransfer(jobId: string) {
    const uploadController = activeUploadControllers.get(jobId);
    if (uploadController) {
      uploadController.abort();
      activeUploadControllers.delete(jobId);
    }

    const queued = queuedUploadTokens.get(jobId);
    if (queued) {
      queued.cancel();
      queuedUploadTokens.delete(jobId);
    }

    if (liveProgress.value[jobId]) {
      liveProgress.value[jobId] = {
        ...liveProgress.value[jobId],
        status: 'cancelled',
        speedBytesPerSec: 0,
        etaSeconds: null,
        terminalObserved: true,
      };
    }

    queryClient.setQueryData<TransferJob[]>(queryKeys.transfers(), (old) => {
      return (old ?? []).map((j) =>
        j.id === jobId
          ? { ...j, status: 'cancellation_requested', speed_bytes_per_sec: 0, eta_seconds: undefined }
          : j
      );
    });

    if (!jobId.startsWith('queued-upload-')) {
      try {
        await cancelTransferApi(jobId);
      } catch (err) {
        console.error('Failed to cancel transfer', err);
      }
    }

    await queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
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

  async function submitUploadBatch(options: UploadBatchOptions): Promise<{
    successfulCount: number;
    failedCount: number;
    cancelled: boolean;
  }> {
    const { connectionId, targetDir: defaultTargetDir, files, existingNames, concurrency = 2 } = options;
    if (!files || files.length === 0) {
      return { successfulCount: 0, failedCount: 0, cancelled: false };
    }

    isDrawerOpen.value = true;
    resetBatchConflict();

    const uploader = options.uploader || uploadTrackedFile;
    const existingFiles = new Set<string>(existingNames ?? []);
    const uiStore = useUiStore();

    const items = files.map((item, idx) => {
      const isBatchItem = typeof item === 'object' && 'file' in item && item.file instanceof File;
      const file = isBatchItem ? (item as UploadBatchItem).file : (item as File);
      const targetDir = (isBatchItem ? (item as UploadBatchItem).targetDir : undefined) || defaultTargetDir;
      return {
        tempId: `queued-upload-${crypto.randomUUID()}`,
        index: idx,
        file,
        targetDir,
        isCancelled: false,
      };
    });

    for (const item of items) {
      const destPath = item.targetDir === '/'
        ? `/${item.file.name}`
        : `${item.targetDir.replace(/\/$/, '')}/${item.file.name}`;

      liveProgress.value[item.tempId] = {
        name: item.file.name,
        transferType: 'upload',
        sourceConnectionId: 'upload',
        sourcePath: `upload://${item.tempId}`,
        destinationConnectionId: connectionId,
        destinationPath: destPath,
        transferredBytes: 0,
        totalBytes: item.file.size,
        speedBytesPerSec: 0,
        etaSeconds: null,
        phase: 'preparing',
        status: 'queued',
        receivedAt: Date.now(),
        stale: false,
      };

      queuedUploadTokens.set(item.tempId, {
        cancel: () => {
          item.isCancelled = true;
          delete liveProgress.value[item.tempId];
        },
      });
    }

    let nextIdx = 0;
    let isBatchCancelled = false;
    let successfulCount = 0;
    const errors: string[] = [];
    const affectedDirs = new Set<string>([defaultTargetDir]);

    async function uploadWorker() {
      while (nextIdx < items.length) {
        if (isBatchCancelled) return;
        const item = items[nextIdx++];
        if (item.isCancelled) {
          delete liveProgress.value[item.tempId];
          queuedUploadTokens.delete(item.tempId);
          continue;
        }

        affectedDirs.add(item.targetDir);
        let uploadFile = item.file;
        const destPath = item.targetDir === '/'
          ? `/${item.file.name}`
          : `${item.targetDir.replace(/\/$/, '')}/${item.file.name}`;

        if (existingFiles.has(uploadFile.name)) {
          const resolution = await requestConflict(uploadFile.name, uploadFile.name, destPath);
          if (resolution === 'cancel') {
            isBatchCancelled = true;
            for (const [id, ctrl] of activeUploadControllers.entries()) {
              ctrl.abort();
              activeUploadControllers.delete(id);
            }
            for (const rem of items) {
              delete liveProgress.value[rem.tempId];
              queuedUploadTokens.delete(rem.tempId);
            }
            return;
          }
          if (resolution === 'skip') {
            delete liveProgress.value[item.tempId];
            queuedUploadTokens.delete(item.tempId);
            continue;
          }
          if (resolution === 'keep_both') {
            const uniqueName = generateUniqueName(uploadFile.name, existingFiles);
            existingFiles.add(uniqueName);
            uploadFile = new File([uploadFile], uniqueName, { type: uploadFile.type });
          }
        }

        if (isBatchCancelled || item.isCancelled) {
          delete liveProgress.value[item.tempId];
          queuedUploadTokens.delete(item.tempId);
          continue;
        }

        delete liveProgress.value[item.tempId];
        queuedUploadTokens.delete(item.tempId);

        const controller = new AbortController();
        let activeJobId: string | undefined;

        try {
          await uploader(
            connectionId,
            item.targetDir,
            uploadFile,
            controller.signal,
            undefined,
            (session) => {
              activeJobId = session.job_id;
              activeUploadControllers.set(session.job_id, controller);
            }
          );
          successfulCount++;
          existingFiles.add(uploadFile.name);
        } catch (err: any) {
          if (isBatchCancelled || controller.signal.aborted || item.isCancelled) {
            // Cancelled cleanly
          } else {
            const msg = err.response?.data?.error?.message || err.message || 'Upload failed';
            errors.push(`${uploadFile.name}: ${msg}`);
          }
        } finally {
          if (activeJobId) {
            activeUploadControllers.delete(activeJobId);
          }
        }
      }
    }

    const workerCount = Math.min(concurrency, items.length);
    const workers = Array.from({ length: workerCount }, () => uploadWorker());
    await Promise.all(workers);

    for (const dir of affectedDirs) {
      await queryClient.invalidateQueries({
        queryKey: queryKeys.directoryPrefix(connectionId, dir),
      });
    }

    if (isBatchCancelled) {
      uiStore.showToast('Upload cancelled', 'warning');
      return { successfulCount, failedCount: errors.length, cancelled: true };
    }

    if (errors.length === 0) {
      uiStore.showToast(`Successfully uploaded ${successfulCount} file(s)`, 'success');
    } else if (successfulCount > 0) {
      uiStore.showToast(`Uploaded ${successfulCount} file(s), ${errors.length} failed`, 'warning');
    } else {
      uiStore.showToast(errors[0] || 'Failed to upload files', 'error');
    }

    return { successfulCount, failedCount: errors.length, cancelled: false };
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
    registerLiveTransfer,
    updateClientProgress,
    uploadTrackedFile,
    submitTransfer,
    submitUploadBatch,
    cancelTransfer,
    retryTransfer,
    removeJob,
    clearFinished,
    requestConflict,
    resolveConflict,
    resetBatchConflict,
  };
});
