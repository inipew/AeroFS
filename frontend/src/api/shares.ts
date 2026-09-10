import { apiClient } from './client';
import type { components } from './generated/openapi';

export type ShareItem = components['schemas']['ShareItem'];
export type CreateShareRequest = components['schemas']['CreateShareRequest'];
export type ShareActionResponse = components['schemas']['ShareActionResponse'];

export async function listShares(): Promise<ShareItem[]> {
  const res = await apiClient.get<ShareItem[]>('/shares');
  return res.data;
}

export async function createShare(payload: CreateShareRequest): Promise<ShareItem> {
  const res = await apiClient.post<ShareItem>('/shares', payload);
  return res.data;
}

export async function deleteShare(shareId: string): Promise<ShareActionResponse> {
  const res = await apiClient.delete<ShareActionResponse>(`/shares/${encodeURIComponent(shareId)}`);
  return res.data;
}
