import { computed } from 'vue';
import { useQuery } from '@tanstack/vue-query';
import { listTransfersApi } from '../api/transfers';
import { queryKeys } from '../api/queryKeys';
import { queryClient } from '../queryClient';
import type { TransferJob } from '../types/transfer';
import { useTransferStore } from '../stores/transferStore';

export function transfersQueryOptions() {
  return {
    queryKey: queryKeys.transfers(),
    queryFn: () => listTransfersApi(),
    staleTime: 5_000,
  };
}

export function useTransfersQuery() {
  const transferStore = useTransferStore();
  const query = useQuery({
    queryKey: queryKeys.transfers(),
    queryFn: () => listTransfersApi(),
    staleTime: 5_000,
    refetchInterval: (q) => {
      const data = q.state.data as TransferJob[] | undefined;
      const hasActiveInRest = data?.some(
        (j) => j.status === 'queued' || j.status === 'running'
      );
      const hasActive = hasActiveInRest || transferStore.activeJobs.length > 0;
      if (!hasActive) return false;
      // When the socket is unavailable REST is the fallback transport. Keep
      // the last visible value, but reconcile it more aggressively.
      return transferStore.isConnected ? 2_000 : 1_000;
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
