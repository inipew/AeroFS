/**
 * useDirectoryQuery — TanStack infinite query composable for directory listing.
 * Each unique (connectionId, path, params) tuple gets its own cache entry.
 * Pagination is handled via `useInfiniteQuery` with cursor-based `next_cursor`.
 */
import { computed } from 'vue';
import type { Ref } from 'vue';
import { useInfiniteQuery, type QueryClient } from '@tanstack/vue-query';
import { listFilesApi } from '../api/files';
import { queryKeys, type DirectoryQueryKeyParams } from '../api/queryKeys';
import { queryClient as singletonQueryClient } from '../queryClient';
import type { DirectoryListing } from '../types/vfs';
import { flattenDirectoryPages, getDirectoryTotalCount } from '../domain/directoryPagination';

export type DirectoryQueryParams = DirectoryQueryKeyParams;

/** Mark cached variants of a directory stale without starting a background fetch. */
export function invalidateDirectoryForRefresh(
  client: QueryClient,
  connectionId: string,
  path: string
) {
  return client.invalidateQueries({
    queryKey: queryKeys.directoryPrefix(connectionId, path),
    refetchType: 'none',
  });
}

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

  /** Flat array of all loaded entries across pages, preserving backend order. */
  const entries = computed(() => flattenDirectoryPages(query.data.value?.pages));

  /** Total count belongs to the directory query, so page 1 is authoritative. */
  const totalCount = computed(() => getDirectoryTotalCount(query.data.value?.pages));

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
