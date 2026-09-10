import { QueryClient } from '@tanstack/vue-query';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000, // 30s before background refetch
      gcTime: 120_000, // 2min cache retention after unmount
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

export default queryClient;
