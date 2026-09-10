import { normalizePath } from '../utils/path';

export interface DirectoryQueryKeyParams {
  show_hidden?: boolean;
  sort?: string;
  order?: 'asc' | 'desc';
  limit?: number;
}

export function normalizeDirectoryParams(params?: DirectoryQueryKeyParams) {
  return {
    show_hidden: !!params?.show_hidden,
    sort: params?.sort || 'name',
    order: params?.order || 'asc',
    limit: params?.limit ?? 100,
  };
}

export const queryKeys = {
  all: ['aerofs'] as const,

  directories: () => ['directory'] as const,
  directoryConnection: (connectionId: string) => ['directory', connectionId] as const,
  directoryPrefix: (connectionId?: string, path?: string) => {
    if (!connectionId) return ['directory'] as const;
    if (!path) return ['directory', connectionId] as const;
    return ['directory', connectionId, normalizePath(path)] as const;
  },
  directory: (
    connectionId: string,
    path: string,
    params?: DirectoryQueryKeyParams
  ) => ['directory', connectionId, normalizePath(path), normalizeDirectoryParams(params)] as const,

  metadataPrefix: (connectionId?: string, path?: string) => {
    if (!connectionId) return ['metadata'] as const;
    if (!path) return ['metadata', connectionId] as const;
    return ['metadata', connectionId, normalizePath(path)] as const;
  },
  metadata: (connectionId: string, path: string) =>
    ['metadata', connectionId, normalizePath(path)] as const,

  capabilitiesPrefix: (connectionId?: string) => {
    if (!connectionId) return ['capabilities'] as const;
    return ['capabilities', connectionId] as const;
  },
  capabilities: (connectionId: string) =>
    ['capabilities', connectionId] as const,

  transfers: () => ['transfers'] as const,
  syncJobs: () => ['syncJobs'] as const,

  connections: () => ['connections'] as const,
  connection: (id: string) => ['connections', id] as const,

  shares: () => ['shares'] as const,
  trash: () => ['trash'] as const,
  settings: () => ['settings'] as const,
  preferences: () => ['preferences'] as const,
  auditLogs: (params?: { limit?: number; offset?: number }) =>
    ['auditLogs', params ?? {}] as const,
};

/**
 * Predicate to check if a queryKey matches a directory under connectionId and optional path prefix.
 */
export function isDirectoryQueryFor(
  key: readonly unknown[],
  connectionId: string,
  pathPrefix?: string
): boolean {
  if (!Array.isArray(key) || key[0] !== 'directory') return false;
  if (key[1] !== connectionId) return false;
  if (!pathPrefix) return true;

  const keyPath = typeof key[2] === 'string' ? key[2] : '';
  const normPrefix = normalizePath(pathPrefix);
  const normalizedKeyPath = normalizePath(keyPath);
  const prefixSlash = normPrefix.endsWith('/') ? normPrefix : `${normPrefix}/`;
  return normalizedKeyPath === normPrefix || normalizedKeyPath.startsWith(prefixSlash);
}

/**
 * Predicate to check if a queryKey matches metadata under connectionId and optional path.
 */
export function isMetadataQueryFor(
  key: readonly unknown[],
  connectionId: string,
  path?: string
): boolean {
  if (!Array.isArray(key) || key[0] !== 'metadata') return false;
  if (key[1] !== connectionId) return false;
  if (!path) return true;
  return key[2] === normalizePath(path);
}
