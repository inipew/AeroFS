import { apiClient } from './client';
import type { components } from './generated/openapi';

export type AuditLogEntry = components['schemas']['AuditLogEntry'];

export interface ListAuditLogsParams {
  limit?: number;
  offset?: number;
}

export async function listAuditLogs(params?: ListAuditLogsParams): Promise<AuditLogEntry[]> {
  const res = await apiClient.get<AuditLogEntry[]>('/audit-logs', { params });
  return res.data;
}
