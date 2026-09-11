import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { queryClient } from '../queryClient';
import { useWorkspaceStore } from './workspaceStore';
import {
  createFileApi,
  createDirectoryApi,
  deleteFilesApi,
  renameEntryApi,
  copyEntryApi,
} from '../api/files';
import type { DirectoryListing } from '../api/files';
import { joinPath, parentPath, normalizePath } from '../utils/path';
import { queryKeys } from '../api/queryKeys';
import type { FileEntry } from '../types/vfs';
import { useTransferStore } from './transferStore';

/**
 * FileStore acts as an ergonomic mutation facade directly bound to the canonical
 * `workspaceStore.activePanel`, ensuring single source of truth across all components.
 */
export const useFileStore = defineStore('file', () => {
  const workspaceStore = useWorkspaceStore();
  // Vue does not track QueryClient reads. This revision makes computed accessors
  // reactive while keeping directory data exclusively in TanStack Query.
  const cacheRevision = ref(0);
  queryClient.getQueryCache().subscribe(() => {
    cacheRevision.value++;
  });

  /** Invalidate TanStack Query directory cache for a given path after mutations */
  function invalidateDirectory(connectionId: string, path: string) {
    queryClient.invalidateQueries({ queryKey: queryKeys.directoryPrefix(connectionId, path) });
  }

  const currentConnectionId = computed({
    get: () => workspaceStore.activePanel.location.connectionId,
    set: (val: string) => {
      workspaceStore.activePanel.location.connectionId = val;
    },
  });

  const currentPath = computed({
    get: () => workspaceStore.activePanel.location.path,
    set: (val: string) => {
      workspaceStore.activePanel.location.path = val;
    },
  });

  const activeDirectoryQuery = computed(() => {
    // Read the revision so cache writes/refetches invalidate this derived view.
    cacheRevision.value;
    const panel = workspaceStore.activePanel;
    return queryClient.getQueryCache().find({
      queryKey: queryKeys.directory(panel.connectionId, panel.path, {
        show_hidden: panel.showHidden,
        sort: panel.sortField,
        order: panel.sortOrder,
      }),
      exact: true,
    });
  });

  const activeDirectoryData = computed(() =>
    activeDirectoryQuery.value?.state.data as
      | { pages: DirectoryListing[]; pageParams: unknown[] }
      | undefined
  );

  const entries = computed<FileEntry[]>(() =>
    activeDirectoryData.value?.pages.flatMap((page) => page.entries) ?? []
  );

  const selectedEntries = computed<string[]>({
    get: () => workspaceStore.activePanel.selection.paths,
    set: (val: string[]) => {
      workspaceStore.activePanel.selection.paths = val;
    },
  });

  const viewMode = computed({
    get: () => workspaceStore.activePanel.view.viewMode,
    set: (val: 'list' | 'grid') => {
      workspaceStore.activePanel.view.viewMode = val;
    },
  });

  const showHidden = computed({
    get: () => workspaceStore.activePanel.view.showHidden,
    set: (val: boolean) => {
      workspaceStore.activePanel.view.showHidden = val;
    },
  });

  const sortField = computed({
    get: () => workspaceStore.activePanel.view.sortField,
    set: (val: string) => {
      workspaceStore.activePanel.view.sortField = val as any;
    },
  });

  const sortOrder = computed({
    get: () => workspaceStore.activePanel.view.sortOrder,
    set: (val: 'asc' | 'desc') => {
      workspaceStore.activePanel.view.sortOrder = val;
    },
  });

  const searchQuery = computed({
    get: () => workspaceStore.activePanel.view.searchQuery,
    set: (val: string) => {
      workspaceStore.activePanel.view.searchQuery = val;
    },
  });

  const loading = computed(() =>
    activeDirectoryQuery.value?.state.status === 'pending'
  );
  const loadingMore = computed(() =>
    activeDirectoryQuery.value?.state.fetchStatus === 'fetching' &&
    (activeDirectoryData.value?.pages.length ?? 0) > 0
  );
  const hasMore = computed(() =>
    activeDirectoryData.value?.pages.at(-1)?.has_more ?? false
  );
  const totalCount = computed(() =>
    activeDirectoryData.value?.pages.at(-1)?.total_count
  );
  const error = computed(() => {
    const value = activeDirectoryQuery.value?.state.error;
    return value instanceof Error ? value.message : value ? String(value) : null;
  });

  const history = computed(() => workspaceStore.activePanel.navigation.history);
  const historyIndex = computed(() => workspaceStore.activePanel.navigation.historyIndex);

  const filteredEntries = computed(() => {
    let list = entries.value;
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLowerCase();
      list = list.filter((e) => e.name.toLowerCase().includes(q));
    }
    return list;
  });

  const selectedCount = computed(() => selectedEntries.value.length);

  async function fetchEntries(path?: string) {
    if (path && path !== workspaceStore.activePanel.location.path) {
      await workspaceStore.navigateTo(workspaceStore.activePanelId, path);
    } else {
      await workspaceStore.refreshPanel(workspaceStore.activePanelId);
    }
  }

  async function fetchNextPage() {
    return await workspaceStore.fetchNextPage(workspaceStore.activePanelId);
  }

  async function navigateTo(path: string, addToHistory: boolean = true) {
    await workspaceStore.navigateTo(workspaceStore.activePanelId, path, addToHistory);
  }

  async function goBack() {
    await workspaceStore.goBack(workspaceStore.activePanelId);
  }

  async function goForward() {
    await workspaceStore.goForward(workspaceStore.activePanelId);
  }

  async function navigateUp() {
    await workspaceStore.navigateUp(workspaceStore.activePanelId);
  }

  function toggleSelect(path: string, multi: boolean = false) {
    const p = workspaceStore.activePanel;
    if (!multi) {
      p.selection.paths = p.selection.paths.includes(path) ? [] : [path];
    } else {
      if (p.selection.paths.includes(path)) {
        p.selection.paths = p.selection.paths.filter((item) => item !== path);
      } else {
        p.selection.paths.push(path);
      }
    }
  }

  function selectAll() {
    workspaceStore.activePanel.selection.paths = filteredEntries.value.map((e) => e.path);
  }

  function clearSelection() {
    workspaceStore.activePanel.selection.paths = [];
  }

  // --- OPTIMISTIC MUTATION ENGINE ON QUERY CACHE ---

  function updateQueryDirectoryCache(
    connectionId: string,
    dirPath: string,
    updater: (entries: FileEntry[]) => FileEntry[]
  ): Array<[readonly unknown[], unknown]> {
    const matchingQueries = queryClient.getQueriesData<{ pages: Array<{ entries: FileEntry[] }> }>({
      queryKey: queryKeys.directoryPrefix(connectionId, dirPath),
      exact: false,
    });
    const snapshots: Array<[readonly unknown[], unknown]> = [];

    for (const [key, oldData] of matchingQueries) {
      if (!oldData?.pages) continue;
      snapshots.push([key, oldData]);

      queryClient.setQueryData(key, {
        ...oldData,
        pages: oldData.pages.map((page, idx) => {
          if (idx === 0) {
            return {
              ...page,
              entries: updater(page.entries),
            };
          }
          return page;
        }),
      });
    }

    return snapshots;
  }

  function restoreQueryDirectoryCache(snapshots: Array<[readonly unknown[], unknown]>) {
    for (const [key, oldData] of snapshots) {
      queryClient.setQueryData(key, oldData);
    }
  }

  async function createFile(name: string) {
    const fullPath = joinPath(currentPath.value, name);
    const optimisticEntry: FileEntry = {
      name,
      path: fullPath,
      kind: 'file',
      size: 0,
      modified_at: new Date().toISOString(),
      is_hidden: name.startsWith('.'),
    };

    const snapshots = updateQueryDirectoryCache(
      currentConnectionId.value,
      currentPath.value,
      (current) => [optimisticEntry, ...current.filter((e) => e.path !== fullPath)]
    );

    try {
      await createFileApi(currentConnectionId.value, fullPath);
      queryClient.invalidateQueries({ queryKey: queryKeys.directoryPrefix(currentConnectionId.value, currentPath.value) });
      queryClient.invalidateQueries({ queryKey: queryKeys.metadata(currentConnectionId.value, fullPath) });
    } catch (err) {
      restoreQueryDirectoryCache(snapshots);
      throw err;
    }
  }

  async function createDirectory(name: string) {
    const fullPath = joinPath(currentPath.value, name);
    const optimisticEntry: FileEntry = {
      name,
      path: fullPath,
      kind: 'directory',
      modified_at: new Date().toISOString(),
      is_hidden: name.startsWith('.'),
    };

    const snapshots = updateQueryDirectoryCache(
      currentConnectionId.value,
      currentPath.value,
      (current) => [optimisticEntry, ...current.filter((e) => e.path !== fullPath)]
    );

    try {
      await createDirectoryApi(currentConnectionId.value, fullPath);
      queryClient.invalidateQueries({ queryKey: queryKeys.directoryPrefix(currentConnectionId.value, currentPath.value) });
    } catch (err) {
      restoreQueryDirectoryCache(snapshots);
      throw err;
    }
  }

  async function deleteSelected() {
    const targets = [...selectedEntries.value];
    if (targets.length === 0) return;

    const previousSelection = [...selectedEntries.value];
    const targetSet = new Set(targets);

    const snapshots = updateQueryDirectoryCache(
      currentConnectionId.value,
      currentPath.value,
      (current) => current.filter((e) => !targetSet.has(e.path))
    );
    selectedEntries.value = [];

    try {
      await deleteFilesApi(currentConnectionId.value, targets);
      queryClient.invalidateQueries({ queryKey: queryKeys.directoryPrefix(currentConnectionId.value, currentPath.value) });
      for (const target of targets) {
        queryClient.invalidateQueries({ queryKey: queryKeys.metadata(currentConnectionId.value, target) });
      }
    } catch (err) {
      restoreQueryDirectoryCache(snapshots);
      selectedEntries.value = previousSelection;
      throw err;
    }
  }

  async function renameEntry(from: string, newName: string) {
    const parent = parentPath(from);
    const to = joinPath(parent, newName);

    const snapshots = updateQueryDirectoryCache(
      currentConnectionId.value,
      parent,
      (current) =>
        current.map((e) => {
          if (e.path === from) {
            return {
              ...e,
              name: newName,
              path: to,
              is_hidden: newName.startsWith('.'),
            };
          }
          return e;
        })
    );

    try {
      await renameEntryApi(currentConnectionId.value, from, to);
      const fromParent = parentPath(normalizePath(from));
      const toParent = parentPath(normalizePath(to));
      queryClient.invalidateQueries({ queryKey: queryKeys.directoryPrefix(currentConnectionId.value, fromParent) });
      if (toParent !== fromParent) {
        queryClient.invalidateQueries({ queryKey: queryKeys.directoryPrefix(currentConnectionId.value, toParent) });
      }
      queryClient.invalidateQueries({ queryKey: queryKeys.metadata(currentConnectionId.value, from) });
      queryClient.invalidateQueries({ queryKey: queryKeys.metadata(currentConnectionId.value, to) });
    } catch (err) {
      restoreQueryDirectoryCache(snapshots);
      throw err;
    }
  }

  async function copyEntry(from: string, destDir: string) {
    const fileName = from.split('/').pop() || 'file';
    const to = joinPath(destDir, fileName);
    await copyEntryApi(currentConnectionId.value, from, to);
    const fromParent = parentPath(normalizePath(from));
    const normalizedDest = normalizePath(destDir);
    queryClient.invalidateQueries({ queryKey: queryKeys.directoryPrefix(currentConnectionId.value, normalizedDest) });
    if (fromParent !== normalizedDest) {
      queryClient.invalidateQueries({ queryKey: queryKeys.directoryPrefix(currentConnectionId.value, fromParent) });
    }
    queryClient.invalidateQueries({ queryKey: queryKeys.metadata(currentConnectionId.value, to) });
    await workspaceStore.refreshActive();
  }

  /**
   * Bounded Concurrent Upload Queue (concurrency: 3) with byte-weighted progress.
   *
   * Every file uses a job-bound upload session so progress, cancellation, and
   * terminal reconciliation share one TransferJob lifecycle.
   */
  async function uploadFiles(
    files: FileList | File[],
    onProgress?: (p: number) => void,
    signal?: AbortSignal
  ) {
    const fileArray = Array.from(files);
    if (fileArray.length === 0) return;

    const totalBytes = fileArray.reduce((acc, f) => acc + f.size, 0);
    const loadedBytesMap: number[] = new Array(fileArray.length).fill(0);

    const reportProgress = () => {
      if (!onProgress) return;
      if (totalBytes === 0) { onProgress(100); return; }
      const loaded = loadedBytesMap.reduce((acc, v) => acc + v, 0);
      onProgress(Math.min(100, Math.round((loaded * 100) / totalBytes)));
    };

    const CONCURRENCY = 3;
    const transferStore = useTransferStore();
    let nextIndex = 0;

    async function worker() {
      while (nextIndex < fileArray.length) {
        if (signal?.aborted) throw new Error('Upload aborted');
        const idx = nextIndex++;
        const file = fileArray[idx];
        await transferStore.uploadTrackedFile(
          currentConnectionId.value,
          currentPath.value,
          file,
          signal ?? new AbortController().signal,
          (_percent, loaded) => {
            loadedBytesMap[idx] = loaded;
            reportProgress();
          }
        );

        loadedBytesMap[idx] = file.size;
        reportProgress();
      }
    }

    const workerCount = Math.min(CONCURRENCY, fileArray.length);
    await Promise.all(Array.from({ length: workerCount }, () => worker()));

    // Invalidate directory cache + legacy refresh
    invalidateDirectory(currentConnectionId.value, currentPath.value);
    await workspaceStore.refreshActive();
  }


  return {
    currentConnectionId,
    currentPath,
    entries,
    selectedEntries,
    viewMode,
    showHidden,
    sortField,
    sortOrder,
    searchQuery,
    loading,
    loadingMore,
    hasMore,
    totalCount,
    error,
    history,
    historyIndex,
    filteredEntries,
    selectedCount,
    fetchEntries,
    fetchNextPage,
    navigateTo,
    goBack,
    goForward,
    navigateUp,
    toggleSelect,
    selectAll,
    clearSelection,
    createFile,
    createDirectory,
    deleteSelected,
    renameEntry,
    copyEntry,
    uploadFiles,
  };
});
