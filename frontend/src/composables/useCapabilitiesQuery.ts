/**
 * useCapabilitiesQuery — TanStack query composable for connection capabilities.
 *
 * Capabilities rarely change (only when provider config changes), so staleTime
 * is set to 5 minutes. This replaces direct provider.capabilities() calls
 * scattered across FileService/TransferManager/etc.
 */
import { computed } from 'vue';
import type { Ref } from 'vue';
import { useQuery, useQueryClient } from '@tanstack/vue-query';
import { getConnectionApi } from '../api/connections';

export function useCapabilitiesQuery(connectionId: Ref<string>) {
  const queryClient = useQueryClient();

  const queryKey = computed(() => ['capabilities', connectionId.value]);

  const query = useQuery({
    queryKey,
    queryFn: () => getConnectionApi(connectionId.value),
    staleTime: 5 * 60_000,   // 5 minutes
    gcTime: 10 * 60_000,     // 10 minutes
    select: (data) => data.capabilities,
    enabled: computed(() => !!connectionId.value),
  });

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: queryKey.value });
  }

  return {
    capabilities: query.data,
    isLoading: query.isLoading,
    error: query.error,
    invalidate,
    query,
  };
}
