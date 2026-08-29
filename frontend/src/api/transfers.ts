import apiClient from './client';
import type { TransferJob, TransferType } from '../types/transfer';

export interface CreateTransferPayload {
  name: string;
  transfer_type: TransferType;
  source_connection_id: string;
  source_path: string;
  destination_connection_id: string;
  destination_path: string;
}

export interface CreateTransferResponse {
  success: boolean;
  job_id: string;
  message: string;
}

export async function createTransferApi(
  payload: CreateTransferPayload,
  idempotencyKey?: string
): Promise<CreateTransferResponse> {
  const headers: Record<string, string> = {};
  if (idempotencyKey) {
    headers['idempotency-key'] = idempotencyKey;
  }
  const resp = await apiClient.post<CreateTransferResponse>('/transfers', payload, { headers });
  return resp.data;
}

export async function listTransfersApi(): Promise<TransferJob[]> {
  const resp = await apiClient.get<TransferJob[]>('/transfers');
  return resp.data;
}

export async function cancelTransferApi(jobId: string): Promise<{ success: boolean; message: string }> {
  const resp = await apiClient.post<{ success: boolean; message: string }>(`/transfers/${jobId}/cancel`);
  return resp.data;
}

export async function retryTransferApi(jobId: string): Promise<{ success: boolean; message: string }> {
  const resp = await apiClient.post<{ success: boolean; message: string }>(`/transfers/${jobId}/retry`);
  return resp.data;
}

export async function dismissTransferApi(jobId: string): Promise<{ success: boolean; message: string }> {
  const resp = await apiClient.post<{ success: boolean; message: string }>(`/transfers/${jobId}/dismiss`);
  return resp.data;
}

export async function clearFinishedTransfersApi(): Promise<{ success: boolean; cleared: number; message: string }> {
  const resp = await apiClient.post<{ success: boolean; cleared: number; message: string }>(
    '/transfers/clear-finished'
  );
  return resp.data;
}
