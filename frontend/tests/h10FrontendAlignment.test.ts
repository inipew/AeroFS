import { describe, expect, it } from 'bun:test';
import {
  directoryCountLabel,
  flattenDirectoryPages,
  getDirectoryTotalCount,
} from '../src/domain/directoryPagination';
import { supportsRecursiveChmod } from '../src/domain/capabilities';
import { queryKeys } from '../src/api/queryKeys';
import { RealtimeSyncCoordinator } from '../src/services/realtimeSync';
import { queryClient } from '../src/queryClient';

describe('H10 frontend alignment', () => {
  it('preserves backend global ordering across loaded pages', () => {
    const pages = [
      { entries: [{ name: 'a' }, { name: 'b' }], total_count: 4 },
      { entries: [{ name: 'm' }, { name: 'z' }], total_count: 4 },
    ];
    expect(flattenDirectoryPages(pages).map((entry) => entry.name)).toEqual(['a', 'b', 'm', 'z']);
  });

  it('keeps exact directory total stable as more pages are loaded', () => {
    const first = [{ entries: new Array(100).fill(null), total_count: 2_000 }];
    const second = [...first, { entries: new Array(100).fill(null), total_count: 2_000 }];
    expect(getDirectoryTotalCount(first)).toBe(2_000);
    expect(getDirectoryTotalCount(second)).toBe(2_000);
    expect(directoryCountLabel(100, 2_000)).toBe('100 loaded · 2,000 items');
    expect(directoryCountLabel(2_000, 2_000)).toBe('2,000 items');
  });

  it('changes the infinite-query identity when sort changes', () => {
    const byName = queryKeys.directory('local', '/', { sort: 'name', order: 'asc', limit: 100 });
    const bySize = queryKeys.directory('local', '/', { sort: 'size', order: 'asc', limit: 100 });
    expect(byName).not.toEqual(bySize);
    expect(byName.slice(0, 3)).toEqual(bySize.slice(0, 3));
  });

  it('exposes recursive chmod only for local storage', () => {
    expect(supportsRecursiveChmod('local')).toBe(true);
    expect(supportsRecursiveChmod('sftp-prod')).toBe(false);
    expect(supportsRecursiveChmod('webdav')).toBe(false);
  });

  it('realtime directory invalidation covers every paginated/sorted variant of the directory', () => {
    const coordinator = new RealtimeSyncCoordinator();
    let predicate: ((query: { queryKey: readonly unknown[] }) => boolean) | undefined;
    const original = queryClient.invalidateQueries.bind(queryClient);
    queryClient.invalidateQueries = ((filters?: any) => {
      if (filters?.predicate) predicate = filters.predicate;
      return Promise.resolve();
    }) as any;

    try {
      coordinator.queueDirectoryInvalidation('conn-1', '/docs');
      coordinator.flush();
      expect(predicate).toBeDefined();
      expect(predicate!({ queryKey: queryKeys.directory('conn-1', '/docs', { sort: 'name', limit: 100 }) })).toBe(true);
      expect(predicate!({ queryKey: queryKeys.directory('conn-1', '/docs', { sort: 'modified', order: 'desc', limit: 50 }) })).toBe(true);
      expect(predicate!({ queryKey: queryKeys.directory('conn-1', '/other', { sort: 'name', limit: 100 }) })).toBe(false);
    } finally {
      queryClient.invalidateQueries = original;
      coordinator.stop();
    }
  });
});
