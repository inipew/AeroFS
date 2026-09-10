import { describe, expect, it, beforeEach } from 'bun:test';
import { queryClient } from '../src/queryClient';
import { queryKeys } from '../src/api/queryKeys';
import type { FileEntry } from '../src/types/vfs';

describe('Directory cache optimistic update and rollback', () => {
  const connId = 'test-conn';
  const dirPath = '/test-dir';
  const queryKey = queryKeys.directory(connId, dirPath);

  beforeEach(() => {
    queryClient.clear();
  });

  it('updates cache optimistically and rolls back cleanly on failure', () => {
    const initialEntry: FileEntry = {
      name: 'file1.txt',
      path: '/test-dir/file1.txt',
      kind: 'file',
      size: 100,
      modified_at: '2026-01-01T00:00:00Z',
      is_hidden: false,
    };

    // Populate initial cache
    queryClient.setQueryData(queryKey, {
      pages: [{ entries: [initialEntry], path: dirPath, has_more: false }],
      pageParams: [undefined],
    });

    // Verify initial state
    const cachedBefore = queryClient.getQueryData<{ pages: Array<{ entries: FileEntry[] }> }>(queryKey);
    expect(cachedBefore?.pages[0].entries.length).toBe(1);

    // Save snapshot
    const matchingQueries = queryClient.getQueriesData<{ pages: Array<{ entries: FileEntry[] }> }>({
      queryKey: ['directory', connId, dirPath],
      exact: false,
    });
    const snapshots: Array<[readonly unknown[], unknown]> = [];
    for (const [key, oldData] of matchingQueries) {
      if (oldData) snapshots.push([key, oldData]);
    }

    // Apply optimistic addition of file2.txt
    const optimisticEntry: FileEntry = {
      name: 'file2.txt',
      path: '/test-dir/file2.txt',
      kind: 'file',
      size: 0,
      modified_at: new Date().toISOString(),
      is_hidden: false,
    };

    for (const [key, oldData] of matchingQueries) {
      if (!oldData?.pages) continue;
      queryClient.setQueryData(key, {
        ...oldData,
        pages: oldData.pages.map((p, idx) =>
          idx === 0 ? { ...p, entries: [optimisticEntry, ...p.entries] } : p
        ),
      });
    }

    // Check optimistic update is immediately visible
    const cachedOptimistic = queryClient.getQueryData<{ pages: Array<{ entries: FileEntry[] }> }>(queryKey);
    expect(cachedOptimistic?.pages[0].entries.length).toBe(2);
    expect(cachedOptimistic?.pages[0].entries[0].name).toBe('file2.txt');

    // Simulate API rejection and rollback
    for (const [key, oldData] of snapshots) {
      queryClient.setQueryData(key, oldData);
    }

    // Verify rollback restored initial entries
    const cachedAfterRollback = queryClient.getQueryData<{ pages: Array<{ entries: FileEntry[] }> }>(queryKey);
    expect(cachedAfterRollback?.pages[0].entries.length).toBe(1);
    expect(cachedAfterRollback?.pages[0].entries[0].name).toBe('file1.txt');
  });
});
