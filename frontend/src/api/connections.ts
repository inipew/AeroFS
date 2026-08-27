import { apiClient } from './client';
import type { Connection } from '../types/connection';
import type { Capabilities } from '../types/vfs';

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
