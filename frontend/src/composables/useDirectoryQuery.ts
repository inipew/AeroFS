/**
 * useDirectoryQuery — TanStack infinite query composable for directory listing.
 *
 * Replaces the manual `fetchPanelEntries` + `fetchNextPage` logic in workspaceStore.
 * Each unique (connectionId, path, params) tuple gets its own cache entry.
 * Pagination is handled via `useInfiniteQuery` with cursor-based `next_cursor`.
 */
import { computed } from 'vue';
import type { Ref } from 'vue';
import { useInfiniteQuery, useQueryClient } from '@tanstack/vue-query';
import { listFilesApi } from '../api/files';
import type { DirectoryListing } from '../types/vfs';

export interface DirectoryQueryParams {
  show_hidden?: boolean;
  sort?: string;
  order?: 'asc' | 'desc';
  limit?: number;
}

export function useDirectoryQuery(
  connectionId: Ref<string>,
  path: Ref<string>,
  params: Ref<DirectoryQueryParams> = { value: {} } as Ref<DirectoryQueryParams>
) {
  const queryClient = useQueryClient();

  const queryKey = computed(() => [
    'directory',
    connectionId.value,
    path.value,
    params.value,
  ]);

  const query = useInfiniteQuery({
    queryKey,
    queryFn: async ({ pageParam }) => {
      return listFilesApi(connectionId.value, {
        path: path.value,
        show_hidden: params.value.show_hidden,
        sort: params.value.sort,
        order: params.value.order,
        limit: params.value.limit ?? 100,
        cursor: pageParam as string | undefined,
      });
    },
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage: DirectoryListing) =>
      lastPage.has_more ? lastPage.next_cursor : undefined,
    staleTime: 30_000,
    gcTime: 120_000,
    enabled: computed(() => !!connectionId.value && !!path.value),
  });

  /** Flat array of all loaded entries across pages */
  const entries = computed(() =>
    query.data.value?.pages.flatMap((p) => p.entries) ?? []
  );

  /** Total count from the most recent page (optional, backend may omit) */
  const totalCount = computed(
    () => query.data.value?.pages.at(-1)?.total_count
  );

  const hasMore = computed(() => query.hasNextPage.value);
  const isFetching = computed(() => query.isFetching.value);
  const isLoading = computed(() => query.isLoading.value);
  const isFetchingNextPage = computed(() => query.isFetchingNextPage.value);
  const error = computed(() => query.error.value as Error | null);

  function loadMore() {
    if (query.hasNextPage.value && !query.isFetchingNextPage.value) {
      query.fetchNextPage();
    }
  }

  /** Imperatively invalidate this directory so it refetches. */
  function invalidate() {
    queryClient.invalidateQueries({ queryKey: queryKey.value });
  }

  /** Invalidate all directories under a given connection. */
  function invalidateConnection(connId: string) {
    queryClient.invalidateQueries({ queryKey: ['directory', connId] });
  }

  return {
    entries,
    totalCount,
    hasMore,
    isFetching,
    isLoading,
    isFetchingNextPage,
    error,
    loadMore,
    invalidate,
    invalidateConnection,
    // Expose raw query for advanced usage
    query,
  };
}
