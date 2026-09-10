/**
 * useDirectoryQuery — TanStack infinite query composable for directory listing.
 * Each unique (connectionId, path, params) tuple gets its own cache entry.
 * Pagination is handled via `useInfiniteQuery` with cursor-based `next_cursor`.
 */
import { computed } from 'vue';
import type { Ref } from 'vue';
import { useInfiniteQuery } from '@tanstack/vue-query';
import { listFilesApi } from '../api/files';
import { queryKeys, type DirectoryQueryKeyParams } from '../api/queryKeys';
import { queryClient as singletonQueryClient } from '../queryClient';
import type { DirectoryListing } from '../types/vfs';

export type DirectoryQueryParams = DirectoryQueryKeyParams;

export function directoryQueryOptions(
  connectionId: string,
  path: string,
  params: DirectoryQueryParams = {}
) {
  return {
    queryKey: queryKeys.directory(connectionId, path, params),
    queryFn: async ({ pageParam }: { pageParam?: unknown }) => {
      return listFilesApi(connectionId, {
        path,
        show_hidden: params.show_hidden,
        sort: params.sort,
        order: params.order,
        limit: params.limit ?? 100,
        cursor: pageParam as string | undefined,
      });
    },
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage: DirectoryListing) =>
      lastPage.has_more ? lastPage.next_cursor : undefined,
    staleTime: 30_000,
    gcTime: 120_000,
  };
}

export function useDirectoryQuery(
  connectionId: Ref<string>,
  path: Ref<string>,
  params: Ref<DirectoryQueryParams> = { value: {} } as Ref<DirectoryQueryParams>
) {
  const queryKey = computed(() =>
    queryKeys.directory(connectionId.value, path.value, params.value)
  );

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
    singletonQueryClient.invalidateQueries({ queryKey: queryKey.value });
  }

  /** Invalidate all directories under a given connection. */
  function invalidateConnection(connId: string) {
    singletonQueryClient.invalidateQueries({
      queryKey: queryKeys.directoryConnection(connId),
    });
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
    query,
  };
}
