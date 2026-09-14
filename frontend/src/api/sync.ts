import apiClient from './client';
import type { components } from './generated/openapi';

export type CreateSyncRequest = components['schemas']['CreateSyncRequest'];
export type CreateSyncResponse = components['schemas']['CreateSyncResponse'];
export type SyncJob = components['schemas']['SyncJob'];
export type SyncOperation = components['schemas']['SyncOperation'];
export type ResolveConflictRequest = components['schemas']['ResolveConflictRequest'];
export type ResolveConflictResponse = components['schemas']['ResolveConflictResponse'];

export interface SyncHistoryPage<T> {
  items: T[];
  nextCursor?: string;
}

export interface SyncHistoryPageOptions {
  cursor?: string;
  limit?: number;
}

function pageFromResponse<T>(items: T[], headers: Record<string, unknown>): SyncHistoryPage<T> {
  const nextCursor = headers['x-next-cursor'];
  return {
    items,
    nextCursor: typeof nextCursor === 'string' && nextCursor.length > 0 ? nextCursor : undefined,
  };
}

export async function createSyncJobApi(payload: CreateSyncRequest): Promise<CreateSyncResponse> {
  const resp = await apiClient.post<CreateSyncResponse>('/sync', payload);
  return resp.data;
}

export async function listSyncJobsPageApi(
  options: SyncHistoryPageOptions = {}
): Promise<SyncHistoryPage<SyncJob>> {
  const resp = await apiClient.get<SyncJob[]>('/sync', {
    params: {
      cursor: options.cursor,
      limit: options.limit,
    },
  });
  return pageFromResponse(resp.data, resp.headers as Record<string, unknown>);
}

export async function listSyncJobsApi(): Promise<SyncJob[]> {
  return (await listSyncJobsPageApi()).items;
}

export async function getSyncOperationsPageApi(
  id: string,
  options: SyncHistoryPageOptions = {}
): Promise<SyncHistoryPage<SyncOperation>> {
  const resp = await apiClient.get<SyncOperation[]>(`/sync/${encodeURIComponent(id)}/operations`, {
    params: {
      cursor: options.cursor,
      limit: options.limit,
    },
  });
  return pageFromResponse(resp.data, resp.headers as Record<string, unknown>);
}

export async function getSyncOperationsApi(id: string): Promise<SyncOperation[]> {
  return (await getSyncOperationsPageApi(id)).items;
}

export async function resolveSyncConflictApi(
  id: string,
  payload: ResolveConflictRequest
): Promise<ResolveConflictResponse> {
  const resp = await apiClient.post<ResolveConflictResponse>(`/sync/${encodeURIComponent(id)}/resolve`, payload);
  return resp.data;
}
