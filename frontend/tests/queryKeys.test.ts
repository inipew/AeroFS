import { describe, expect, it } from 'bun:test';
import { queryKeys, isDirectoryQueryFor, isMetadataQueryFor } from '../src/api/queryKeys';

describe('queryKeys factory', () => {
  it('generates consistent keys for directory domain with normalization', () => {
    const rootKey = queryKeys.directories();
    expect(rootKey).toEqual(['directory']);

    const connKey = queryKeys.directoryConnection('local');
    expect(connKey).toEqual(['directory', 'local']);

    // Path normalization: removes duplicate slashes and trailing slashes
    const dirKey = queryKeys.directory('local', '//documents//folder/', { show_hidden: true, sort: 'name' });
    expect(dirKey).toEqual(['directory', 'local', '/documents/folder', { show_hidden: true, sort: 'name', order: 'asc', limit: 100 }]);

    // Parameter normalization: fills defaults
    const dirKeyDefaultParams = queryKeys.directory('local', '/documents', {
      show_hidden: false,
      limit: 100,
    });
    expect(dirKeyDefaultParams).toEqual(['directory', 'local', '/documents', { show_hidden: false, sort: 'name', order: 'asc', limit: 100 }]);
  });

  it('generates consistent keys for all entity domains', () => {
    expect(queryKeys.transfers()).toEqual(['transfers']);
    expect(queryKeys.syncJobs()).toEqual(['syncJobs']);
    expect(queryKeys.connections()).toEqual(['connections']);
    expect(queryKeys.connection('conn-1')).toEqual(['connections', 'conn-1']);
    expect(queryKeys.capabilities('conn-1')).toEqual(['capabilities', 'conn-1']);
    expect(queryKeys.metadata('conn-1', '//docs//file.txt/')).toEqual(['metadata', 'conn-1', '/docs/file.txt']);
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

describe('isMetadataQueryFor predicate', () => {
  it('returns true when query matches connection and exact path', () => {
    const key = ['metadata', 'conn-1', '/docs/file.txt'];
    expect(isMetadataQueryFor(key, 'conn-1', '/docs/file.txt')).toBe(true);
  });

  it('returns true when query matches connection and pathPrefix is omitted', () => {
    const key = ['metadata', 'conn-1', '/docs/file.txt'];
    expect(isMetadataQueryFor(key, 'conn-1')).toBe(true);
  });

  it('returns false when connection does not match', () => {
    const key = ['metadata', 'conn-2', '/docs/file.txt'];
    expect(isMetadataQueryFor(key, 'conn-1', '/docs/file.txt')).toBe(false);
  });

  it('returns false for non-metadata query keys', () => {
    expect(isMetadataQueryFor(['directory', 'conn-1', '/docs'], 'conn-1')).toBe(false);
  });
});
