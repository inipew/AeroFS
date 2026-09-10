import { computed } from 'vue';
import { useQuery } from '@tanstack/vue-query';
import { listSyncJobsApi } from '../api/sync';
import { queryKeys } from '../api/queryKeys';
import { queryClient } from '../queryClient';
import type { SyncJob } from '../api/sync';

export function syncJobsQueryOptions() {
  return {
    queryKey: queryKeys.syncJobs(),
    queryFn: () => listSyncJobsApi(),
    staleTime: 5_000,
  };
}

export function useSyncJobsQuery() {
  const query = useQuery({
    queryKey: queryKeys.syncJobs(),
    queryFn: () => listSyncJobsApi(),
    staleTime: 5_000,
    refetchInterval: (q) => {
      const data = q.state.data as SyncJob[] | undefined;
      const hasActive = data?.some(
        (j) =>
          j.status === 'scanning' ||
          j.status === 'planning' ||
          j.status === 'reconciling' ||
          j.status === 'executing' ||
          j.status === 'verifying'
      );
      return hasActive ? 3_000 : false;
    },
  });

  const syncJobs = computed(() => query.data.value ?? []);
  const isLoading = computed(() => query.isLoading.value);
  const isFetching = computed(() => query.isFetching.value);
  const error = computed(() => query.error.value as Error | null);

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: queryKeys.syncJobs() });
  }

  return {
    syncJobs,
    isLoading,
    isFetching,
    error,
    invalidate,
    query,
  };
}
