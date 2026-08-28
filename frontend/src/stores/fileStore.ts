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

  async function createFile(name: string) {
    const fullPath = joinPath(currentPath.value, name);
    await createFileApi(currentConnectionId.value, fullPath);
    await workspaceStore.refreshActive();
  }

  async function createDirectory(name: string) {
    const fullPath = joinPath(currentPath.value, name);
    await createDirectoryApi(currentConnectionId.value, fullPath);
    await workspaceStore.refreshActive();
  }

  async function deleteSelected() {
    if (selectedEntries.value.length === 0) return;
    await deleteFilesApi(currentConnectionId.value, selectedEntries.value);
    await workspaceStore.refreshActive();
  }

  async function renameEntry(from: string, newName: string) {
    const parent = parentPath(from);
    const to = joinPath(parent, newName);
    await renameEntryApi(currentConnectionId.value, from, to);
    await workspaceStore.refreshActive();
  }

  async function copyEntry(from: string, destDir: string) {
    const fileName = from.split('/').pop() || 'file';
    const to = joinPath(destDir, fileName);
    await copyEntryApi(currentConnectionId.value, from, to);
    await workspaceStore.refreshActive();
  }

  async function uploadFiles(files: FileList | File[], onProgress?: (p: number) => void) {
    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      await uploadFileApi(
        currentConnectionId.value,
        currentPath.value,
        file,
        (progress) => {
          if (onProgress) {
            const overall = ((i + progress) / files.length) * 100;
            onProgress(Math.round(overall));
          }
        }
      );
    }
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
    error,
    history,
    historyIndex,
    filteredEntries,
    selectedCount,
    fetchEntries,
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
