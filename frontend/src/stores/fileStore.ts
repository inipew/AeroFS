import { defineStore } from 'pinia';
import { computed } from 'vue';
import { useWorkspaceStore } from './workspaceStore';
import {
  createFileApi,
  createDirectoryApi,
  deleteFilesApi,
  renameEntryApi,
  copyEntryApi,
  uploadFileApi,
} from '../api/files';
import { joinPath, parentPath } from '../utils/path';
import type { FileEntry } from '../types/vfs';

/**
 * FileStore acts as an ergonomic facade directly bound to the canonical
 * `workspaceStore.activePanel`, ensuring single source of truth across all components.
 */
export const useFileStore = defineStore('file', () => {
  const workspaceStore = useWorkspaceStore();

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

  const entries = computed<FileEntry[]>({
    get: () => workspaceStore.activePanel.runtime.entries,
    set: (val: FileEntry[]) => {
      workspaceStore.activePanel.runtime.entries = val;
    },
  });

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

  const loading = computed(() => workspaceStore.activePanel.runtime.loading);
  const loadingMore = computed(() => workspaceStore.activePanel.runtime.loadingMore);
  const hasMore = computed(() => workspaceStore.activePanel.runtime.hasMore);
  const totalCount = computed(() => workspaceStore.activePanel.runtime.totalCount);
  const error = computed(() => workspaceStore.activePanel.runtime.error);

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
    await workspaceStore.fetchPanelEntries(workspaceStore.activePanelId, path);
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

  // --- OPTIMISTIC MUTATION ENGINE (Plan 54 P0.16, P1.41) ---

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

    // Optimistic addition
    entries.value = [...entries.value, optimisticEntry];

    try {
      await createFileApi(currentConnectionId.value, fullPath);
    } catch (err) {
      // Revert on failure
      entries.value = entries.value.filter((e) => e.path !== fullPath);
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

    // Optimistic addition
    entries.value = [...entries.value, optimisticEntry];

    try {
      await createDirectoryApi(currentConnectionId.value, fullPath);
    } catch (err) {
      // Revert on failure
      entries.value = entries.value.filter((e) => e.path !== fullPath);
      throw err;
    }
  }

  async function deleteSelected() {
    const targets = [...selectedEntries.value];
    if (targets.length === 0) return;

    const previousEntries = [...entries.value];
    const previousSelection = [...selectedEntries.value];
    const targetSet = new Set(targets);

    // Optimistic removal
    entries.value = entries.value.filter((e) => !targetSet.has(e.path));
    selectedEntries.value = [];

    try {
      await deleteFilesApi(currentConnectionId.value, targets);
    } catch (err) {
      // Revert on failure
      entries.value = previousEntries;
      selectedEntries.value = previousSelection;
      throw err;
    }
  }

  async function renameEntry(from: string, newName: string) {
    const parent = parentPath(from);
    const to = joinPath(parent, newName);

    const previousEntries = [...entries.value];
    // Optimistic rename
    entries.value = entries.value.map((e) => {
      if (e.path === from) {
        return {
          ...e,
          name: newName,
          path: to,
          is_hidden: newName.startsWith('.'),
        };
      }
      return e;
    });

    try {
      await renameEntryApi(currentConnectionId.value, from, to);
    } catch (err) {
      // Revert on failure
      entries.value = previousEntries;
      throw err;
    }
  }

  async function copyEntry(from: string, destDir: string) {
    const fileName = from.split('/').pop() || 'file';
    const to = joinPath(destDir, fileName);
    await copyEntryApi(currentConnectionId.value, from, to);
    await workspaceStore.refreshActive();
  }

  /**
   * Bounded Concurrent Upload Queue (concurrency: 3) with Per-File & Aggregated Progress Tracking
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
      if (totalBytes === 0) {
        onProgress(100);
        return;
      }
      const loaded = loadedBytesMap.reduce((acc, v) => acc + v, 0);
      const percent = Math.min(100, Math.round((loaded * 100) / totalBytes));
      onProgress(percent);
    };

    const CONCURRENCY = 3;
    let nextIndex = 0;

    async function worker() {
      while (nextIndex < fileArray.length) {
        if (signal?.aborted) throw new Error('Upload aborted');
        const idx = nextIndex++;
        const file = fileArray[idx];

        await uploadFileApi(
          currentConnectionId.value,
          currentPath.value,
          file,
          (filePercent) => {
            loadedBytesMap[idx] = Math.round((filePercent / 100) * file.size);
            reportProgress();
          },
          signal
        );

        loadedBytesMap[idx] = file.size;
        reportProgress();
      }
    }

    const workerCount = Math.min(CONCURRENCY, fileArray.length);
    const workers = Array.from({ length: workerCount }, () => worker());
    await Promise.all(workers);

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
