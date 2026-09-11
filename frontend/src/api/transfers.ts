import apiClient from './client';
import type { components } from './generated/openapi';

export type CreateTransferPayload = components['schemas']['CreateTransferRequest'];
export type CreateTransferResponse = components['schemas']['CreateTransferResponse'];
export type TransferJob = components['schemas']['TransferJobResponse'];
export type TransferActionResponse = components['schemas']['TransferActionResponse'];
export type ClearFinishedTransfersResponse = components['schemas']['ClearFinishedTransfersResponse'];

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

export async function cancelTransferApi(jobId: string): Promise<TransferActionResponse> {
  const resp = await apiClient.post<TransferActionResponse>(`/transfers/${jobId}/cancel`);
  return resp.data;
}

export async function retryTransferApi(jobId: string): Promise<TransferActionResponse> {
  const resp = await apiClient.post<TransferActionResponse>(`/transfers/${jobId}/retry`);
  return resp.data;
}

export async function dismissTransferApi(jobId: string): Promise<TransferActionResponse> {
  const resp = await apiClient.post<TransferActionResponse>(`/transfers/${jobId}/dismiss`);
  return resp.data;
}

export async function clearFinishedTransfersApi(): Promise<ClearFinishedTransfersResponse> {
  const resp = await apiClient.post<ClearFinishedTransfersResponse>(
    '/transfers/clear-finished'
  );
  return resp.data;
}
