import apiClient from './client';
import type { components } from './generated/openapi';

export type CreateSyncRequest = components['schemas']['CreateSyncRequest'];
export type CreateSyncResponse = components['schemas']['CreateSyncResponse'];
export type SyncJob = components['schemas']['SyncJob'];
export type SyncOperation = components['schemas']['SyncOperation'];
export type ResolveConflictRequest = components['schemas']['ResolveConflictRequest'];
export type ResolveConflictResponse = components['schemas']['ResolveConflictResponse'];

export async function createSyncJobApi(payload: CreateSyncRequest): Promise<CreateSyncResponse> {
  const resp = await apiClient.post<CreateSyncResponse>('/sync', payload);
  return resp.data;
}

export async function listSyncJobsApi(): Promise<SyncJob[]> {
  const resp = await apiClient.get<SyncJob[]>('/sync');
  return resp.data;
}

export async function getSyncOperationsApi(id: string): Promise<SyncOperation[]> {
  const resp = await apiClient.get<SyncOperation[]>(`/sync/${encodeURIComponent(id)}/operations`);
  return resp.data;
}

export async function resolveSyncConflictApi(
  id: string,
  payload: ResolveConflictRequest
): Promise<ResolveConflictResponse> {
  const resp = await apiClient.post<ResolveConflictResponse>(`/sync/${encodeURIComponent(id)}/resolve`, payload);
  return resp.data;
}
