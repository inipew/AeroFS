import { computed } from 'vue';
import { useQuery } from '@tanstack/vue-query';
import { listTransfersApi } from '../api/transfers';
import { queryKeys } from '../api/queryKeys';
import { queryClient } from '../queryClient';
import type { TransferJob } from '../types/transfer';

export function transfersQueryOptions() {
  return {
    queryKey: queryKeys.transfers(),
    queryFn: () => listTransfersApi(),
    staleTime: 5_000,
  };
}

export function useTransfersQuery() {
  const query = useQuery({
    queryKey: queryKeys.transfers(),
    queryFn: () => listTransfersApi(),
    staleTime: 5_000,
    refetchInterval: (q) => {
      const data = q.state.data as TransferJob[] | undefined;
      const hasActive = data?.some(
        (j) => j.status === 'queued' || j.status === 'running'
      );
      return hasActive ? 2_000 : false;
    },
  });

  const transfers = computed(() => query.data.value ?? []);
  const isLoading = computed(() => query.isLoading.value);
  const isFetching = computed(() => query.isFetching.value);
  const error = computed(() => query.error.value as Error | null);

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: queryKeys.transfers() });
  }

  return {
    transfers,
    isLoading,
    isFetching,
    error,
    invalidate,
    query,
  };
}
