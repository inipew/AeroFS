/**
 * Unified query key factory for TanStack Query cache.
 * Ensures consistent key structures across queries, mutations, and WebSocket invalidations.
 */

export interface DirectoryQueryKeyParams {
  show_hidden?: boolean;
  sort?: string;
  order?: 'asc' | 'desc';
  limit?: number;
}

export const queryKeys = {
  all: ['aerofs'] as const,

  directories: () => ['directory'] as const,
  directoryConnection: (connectionId: string) => ['directory', connectionId] as const,
  directory: (
    connectionId: string,
    path: string,
    params?: DirectoryQueryKeyParams
  ) => ['directory', connectionId, path, params ?? {}] as const,

  transfers: () => ['transfers'] as const,

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
  const normalizedPrefix = pathPrefix.endsWith('/') ? pathPrefix : `${pathPrefix}/`;
  return keyPath === pathPrefix || keyPath.startsWith(normalizedPrefix);
}
