import { describe, expect, it } from 'bun:test';
import { queryKeys, isDirectoryQueryFor } from '../src/api/queryKeys';

describe('queryKeys factory', () => {
  it('generates consistent keys for directory domain', () => {
    const rootKey = queryKeys.directories();
    expect(rootKey).toEqual(['directory']);

    const connKey = queryKeys.directoryConnection('local');
    expect(connKey).toEqual(['directory', 'local']);

    const dirKey = queryKeys.directory('local', '/documents', { show_hidden: true, sort: 'name' });
    expect(dirKey).toEqual(['directory', 'local', '/documents', { show_hidden: true, sort: 'name' }]);
  });

  it('generates consistent keys for all entity domains', () => {
    expect(queryKeys.transfers()).toEqual(['transfers']);
    expect(queryKeys.connections()).toEqual(['connections']);
    expect(queryKeys.connection('conn-1')).toEqual(['connections', 'conn-1']);
    expect(queryKeys.shares()).toEqual(['shares']);
    expect(queryKeys.trash()).toEqual(['trash']);
    expect(queryKeys.settings()).toEqual(['settings']);
    expect(queryKeys.preferences()).toEqual(['preferences']);
    expect(queryKeys.auditLogs({ limit: 50 })).toEqual(['auditLogs', { limit: 50 }]);
  });
});

describe('isDirectoryQueryFor predicate', () => {
  it('returns true when query matches connection and exact path', () => {
    const key = ['directory', 'conn-1', '/photos', {}];
    expect(isDirectoryQueryFor(key, 'conn-1', '/photos')).toBe(true);
  });

  it('returns true when query matches connection and descendant path', () => {
    const key = ['directory', 'conn-1', '/photos/vacation', {}];
    expect(isDirectoryQueryFor(key, 'conn-1', '/photos')).toBe(true);
  });

  it('returns true when query matches connection and pathPrefix is omitted', () => {
    const key = ['directory', 'conn-1', '/any/deep/path', {}];
    expect(isDirectoryQueryFor(key, 'conn-1')).toBe(true);
  });

  it('returns false when connection does not match', () => {
    const key = ['directory', 'conn-2', '/photos', {}];
    expect(isDirectoryQueryFor(key, 'conn-1', '/photos')).toBe(false);
  });

  it('returns false when path does not match prefix', () => {
    const key = ['directory', 'conn-1', '/documents', {}];
    expect(isDirectoryQueryFor(key, 'conn-1', '/photos')).toBe(false);
  });

  it('returns false for non-directory query keys', () => {
    expect(isDirectoryQueryFor(['transfers'], 'conn-1')).toBe(false);
    expect(isDirectoryQueryFor(['connections', 'conn-1'], 'conn-1')).toBe(false);
  });
});
