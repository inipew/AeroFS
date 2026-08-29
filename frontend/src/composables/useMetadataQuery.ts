/**
 * useMetadataQuery — TanStack query composable for file/directory metadata.
 * Uses ETag-aware caching: staleTime 60 s, refetches after mutations.
 */
import { computed } from 'vue';
import type { Ref } from 'vue';
import { useQuery, useQueryClient } from '@tanstack/vue-query';
import { getMetadataApi } from '../api/files';

export function useMetadataQuery(
  connectionId: Ref<string>,
  path: Ref<string>,
  enabled?: Ref<boolean>
) {
  const queryClient = useQueryClient();

  const queryKey = computed(() => ['metadata', connectionId.value, path.value]);

  const query = useQuery({
    queryKey,
    queryFn: () => getMetadataApi(connectionId.value, path.value),
    staleTime: 60_000,
    gcTime: 180_000,
    enabled: computed(() => {
      const e = enabled?.value ?? true;
      return e && !!connectionId.value && !!path.value;
    }),
  });

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: queryKey.value });
  }

  return {
    metadata: query.data,
    isLoading: query.isLoading,
    isFetching: query.isFetching,
    error: query.error,
    invalidate,
    query,
  };
}
