import apiClient from './client';
import type { CreateSyncRequest, CreateSyncResponse, SyncJob } from '../types/sync';

export async function createSyncJobApi(payload: CreateSyncRequest): Promise<CreateSyncResponse> {
  const resp = await apiClient.post<CreateSyncResponse>('/sync', payload);
  return resp.data;
}

export async function listSyncJobsApi(): Promise<SyncJob[]> {
  const resp = await apiClient.get<SyncJob[]>('/sync');
  return resp.data;
}
