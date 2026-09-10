import { apiClient } from './client';
import type { components } from './generated/openapi';
import type { Connection } from '../types/connection';
import type { Capabilities } from '../types/vfs';

export type CreateConnectionRequest = components['schemas']['CreateConnectionRequest'];
export type UpdateConnectionRequest = components['schemas']['UpdateConnectionRequest'];
export type CreateConnectionResponse = components['schemas']['CreateConnectionResponse'];
export type ConnectionActionResponse = components['schemas']['ConnectionActionResponse'];
export type TestConnectionResponse = components['schemas']['TestConnectionResponse'];

export interface ConnectionDetail {
  connection: Connection;
  capabilities: Capabilities;
}

export async function listConnectionsApi(): Promise<Connection[]> {
  const resp = await apiClient.get<Connection[]>('/connections');
  return resp.data;
}

export async function getConnectionApi(id: string): Promise<ConnectionDetail> {
  const resp = await apiClient.get<ConnectionDetail>(`/connections/${id}`);
  return resp.data;
}

export async function createConnectionApi(payload: any): Promise<CreateConnectionResponse> {
  const resp = await apiClient.post<CreateConnectionResponse>('/connections', payload);
  return resp.data;
}

export async function updateConnectionApi(id: string, payload: any): Promise<ConnectionActionResponse> {
  const resp = await apiClient.put<ConnectionActionResponse>(`/connections/${encodeURIComponent(id)}`, payload);
  return resp.data;
}

export async function deleteConnectionApi(id: string): Promise<ConnectionActionResponse> {
  const resp = await apiClient.delete<ConnectionActionResponse>(`/connections/${encodeURIComponent(id)}`);
  return resp.data;
}

export async function testConnectionApi(id: string): Promise<TestConnectionResponse> {
  const resp = await apiClient.post<TestConnectionResponse>(`/connections/${encodeURIComponent(id)}/test`);
  return resp.data;
}
