import { describe, expect, it } from 'bun:test';
import { QueryClient } from '@tanstack/vue-query';
import {
  directoryQueryOptions,
  invalidateDirectoryForRefresh,
} from '../src/composables/useDirectoryQuery';

describe('manual directory refresh', () => {
  it('marks a fresh listing stale so the next fetch reaches the server', async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const options = directoryQueryOptions('local', '/documents');
    let requestCount = 0;
    const requestOptions = {
      ...options,
      queryFn: async () => {
        requestCount++;
        return {
          entries: [],
          path: '/documents',
          has_more: false,
          total_count: 0,
        };
      },
    };

    await client.fetchInfiniteQuery(requestOptions);
    await client.fetchInfiniteQuery(requestOptions);
    expect(requestCount).toBe(1);

    await invalidateDirectoryForRefresh(client, 'local', '/documents');
    await client.fetchInfiniteQuery(requestOptions);
    expect(requestCount).toBe(2);
  });

  it('does not invalidate a different panel connection', async () => {
    const client = new QueryClient();
    const localKey = directoryQueryOptions('local', '/documents').queryKey;
    const remoteKey = directoryQueryOptions('remote', '/documents').queryKey;
    client.setQueryData(localKey, { pages: [], pageParams: [] });
    client.setQueryData(remoteKey, { pages: [], pageParams: [] });

    await invalidateDirectoryForRefresh(client, 'local', '/documents');

    expect(client.getQueryState(localKey)?.isInvalidated).toBe(true);
    expect(client.getQueryState(remoteKey)?.isInvalidated).toBe(false);
  });
});
