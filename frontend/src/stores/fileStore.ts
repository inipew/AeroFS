import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import {
  listFilesApi,
  createFileApi,
  createDirectoryApi,
  deleteFilesApi,
  renameEntryApi,
  copyEntryApi,
  uploadFileApi,
} from '../api/files';
import type { FileEntry } from '../types/vfs';

export const useFileStore = defineStore('file', () => {
  const currentConnectionId = ref<string>('local');
  const currentPath = ref<string>('/');
  const entries = ref<FileEntry[]>([]);
  const selectedEntries = ref<string[]>([]);
  const viewMode = ref<'list' | 'grid'>('list');
  const showHidden = ref<boolean>(false);
  const sortField = ref<string>('name');
  const sortOrder = ref<'asc' | 'desc'>('asc');
  const searchQuery = ref<string>('');
  const loading = ref<boolean>(false);
  const error = ref<string | null>(null);

  // History for Back / Forward navigation
  const history = ref<string[]>(['/']);
  const historyIndex = ref<number>(0);

  const filteredEntries = computed(() => {
    let list = entries.value;
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLowerCase();
      list = list.filter((e) => e.name.toLowerCase().includes(q));
    }
    return list;
  });

  const selectedCount = computed(() => selectedEntries.value.length);

  async function fetchEntries(path: string = currentPath.value) {
    loading.value = true;
    error.value = null;
    try {
      const data = await listFilesApi(currentConnectionId.value, {
        path,
        show_hidden: showHidden.value,
        sort: sortField.value,
        order: sortOrder.value,
      });
      entries.value = data.entries;
      currentPath.value = data.path;
      selectedEntries.value = [];
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || 'Failed to list files';
    } finally {
      loading.value = false;
    }
  }

  async function navigateTo(path: string, addToHistory: boolean = true) {
    await fetchEntries(path);
    if (addToHistory && currentPath.value !== history.value[historyIndex.value]) {
      history.value = history.value.slice(0, historyIndex.value + 1);
      history.value.push(currentPath.value);
      historyIndex.value = history.value.length - 1;
    }
  }

  async function goBack() {
    if (historyIndex.value > 0) {
      historyIndex.value--;
      await navigateTo(history.value[historyIndex.value], false);
    }
  }

  async function goForward() {
    if (historyIndex.value < history.value.length - 1) {
      historyIndex.value++;
      await navigateTo(history.value[historyIndex.value], false);
    }
  }

  async function navigateUp() {
    if (currentPath.value === '/' || currentPath.value === '') return;
    const parts = currentPath.value.split('/').filter(Boolean);
    parts.pop();
    const parent = parts.length === 0 ? '/' : `/${parts.join('/')}`;
    await navigateTo(parent);
  }

  function toggleSelect(path: string, multi: boolean = false) {
    if (!multi) {
      selectedEntries.value = selectedEntries.value.includes(path) ? [] : [path];
    } else {
      if (selectedEntries.value.includes(path)) {
        selectedEntries.value = selectedEntries.value.filter((p) => p !== path);
      } else {
        selectedEntries.value.push(path);
      }
    }
  }

  function selectAll() {
    selectedEntries.value = filteredEntries.value.map((e) => e.path);
  }

  function clearSelection() {
    selectedEntries.value = [];
  }

  async function createFile(name: string) {
    const fullPath =
      currentPath.value === '/' ? `/${name}` : `${currentPath.value}/${name}`;
    await createFileApi(currentConnectionId.value, fullPath);
    await fetchEntries();
  }

  async function createDirectory(name: string) {
    const fullPath =
      currentPath.value === '/' ? `/${name}` : `${currentPath.value}/${name}`;
    await createDirectoryApi(currentConnectionId.value, fullPath);
    await fetchEntries();
  }

  async function deleteSelected() {
    if (selectedEntries.value.length === 0) return;
    await deleteFilesApi(currentConnectionId.value, selectedEntries.value);
    await fetchEntries();
  }

  async function renameEntry(from: string, newName: string) {
    const parent = from.substring(0, from.lastIndexOf('/')) || '/';
    const to = parent === '/' ? `/${newName}` : `${parent}/${newName}`;
    await renameEntryApi(currentConnectionId.value, from, to);
    await fetchEntries();
  }

  async function copyEntry(from: string, destDir: string) {
    const fileName = from.split('/').pop() || 'file';
    const to = destDir === '/' ? `/${fileName}` : `${destDir}/${fileName}`;
    await copyEntryApi(currentConnectionId.value, from, to);
    await fetchEntries();
  }

  async function uploadFiles(files: FileList | File[], onProgress?: (p: number) => void) {
    for (let i = 0; i < files.length; i++) {
      await uploadFileApi(currentConnectionId.value, currentPath.value, files[i], onProgress);
    }
    await fetchEntries();
  }

  return {
    currentConnectionId,
    currentPath,
    entries,
    filteredEntries,
    selectedEntries,
    selectedCount,
    viewMode,
    showHidden,
    sortField,
    sortOrder,
    searchQuery,
    loading,
    error,
    history,
    historyIndex,
    fetchEntries,
    navigateTo,
    navigateUp,
    goBack,
    goForward,
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
