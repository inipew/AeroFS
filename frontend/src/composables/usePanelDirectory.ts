import { computed, type ComputedRef } from 'vue';
import { useWorkspaceStore } from '../stores/workspaceStore';
import { useDirectoryQuery } from './useDirectoryQuery';
import type { PanelId } from '../types/workspace';
import type { FileEntry } from '../types/vfs';
import { queryClient } from '../queryClient';
import { directoryQueryOptions } from './useDirectoryQuery';

export function getCachedDirectoryEntries(
  connectionId: string,
  path: string
): FileEntry[] {
  const matches = queryClient.getQueriesData<{ pages?: Array<{ entries: FileEntry[] }> }>({
    queryKey: ['directory', connectionId, path],
    exact: false,
  });
  for (const [_, data] of matches) {
    if (data?.pages?.length) {
      return data.pages.flatMap((p) => p.entries);
    }
  }
  return [];
}

export async function ensureDirectoryData(
  connectionId: string,
  path: string,
  params?: import('./useDirectoryQuery').DirectoryQueryParams
): Promise<FileEntry[]> {
  const cached = getCachedDirectoryEntries(connectionId, path);
  if (cached.length > 0) {
    return cached;
  }
  const result = await queryClient.fetchInfiniteQuery(
    directoryQueryOptions(connectionId, path, params)
  );
  return result.pages.flatMap((p) => p.entries);
}

export function usePanelDirectory(panelId: PanelId) {
  const workspaceStore = useWorkspaceStore();
  const panel = computed(() => workspaceStore.getPanel(panelId));

  const connectionId = computed(() => panel.value.location.connectionId);
  const path = computed(() => panel.value.location.path);
  const queryParams = computed(() => ({
    show_hidden: panel.value.view.showHidden,
    sort: panel.value.view.sortField,
    order: panel.value.view.sortOrder,
    limit: 100,
  }));

  const dirQuery = useDirectoryQuery(connectionId, path, queryParams);

  const rawEntries = computed(() => dirQuery.entries.value);

  const filteredEntries: ComputedRef<FileEntry[]> = computed(() => {
    let list = rawEntries.value;
    const q = panel.value.view.searchQuery.trim().toLowerCase();
    if (q) {
      list = list.filter((e) => e.name.toLowerCase().includes(q));
    }
    const filterType = panel.value.view.filterType;
    if (filterType && filterType !== 'all') {
      list = list.filter((e) => {
        if (filterType === 'directories' || filterType === 'folders') {
          return e.kind === 'directory';
        }
        if (filterType === 'files') {
          return e.kind === 'file';
        }
        return true;
      });
    }
    return list;
  });

  const selectedPaths = computed({
    get: () => panel.value.selection.paths,
    set: (paths: string[]) => {
      panel.value.selection.paths = paths;
    },
  });

  function selectAll() {
    selectedPaths.value = filteredEntries.value.map((e) => e.path);
  }

  function clearSelection() {
    selectedPaths.value = [];
  }

  function toggleSelect(entryPath: string, multi = false) {
    if (!multi) {
      selectedPaths.value = selectedPaths.value.includes(entryPath) ? [] : [entryPath];
    } else {
      if (selectedPaths.value.includes(entryPath)) {
        selectedPaths.value = selectedPaths.value.filter((p) => p !== entryPath);
      } else {
        selectedPaths.value = [...selectedPaths.value, entryPath];
      }
    }
  }

  return {
    panel,
    dirQuery,
    rawEntries,
    entries: rawEntries,
    filteredEntries,
    selectedPaths,
    totalCount: dirQuery.totalCount,
    hasMore: dirQuery.hasMore,
    isFetching: dirQuery.isFetching,
    isLoading: dirQuery.isLoading,
    isFetchingNextPage: dirQuery.isFetchingNextPage,
    error: dirQuery.error,
    loadMore: dirQuery.loadMore,
    invalidate: dirQuery.invalidate,
    invalidateConnection: dirQuery.invalidateConnection,
    selectAll,
    clearSelection,
    toggleSelect,
  };
}
