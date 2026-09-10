import { apiClient } from './client';
import type { components } from './generated/openapi';

export type SearchOutput = components['schemas']['SearchOutput'];
export type SearchQuery = components['schemas']['SearchQuery'];

export async function searchFiles(
  connectionId: string,
  query: SearchQuery,
  signal?: AbortSignal
): Promise<SearchOutput> {
  const res = await apiClient.get<SearchOutput>(`/connections/${encodeURIComponent(connectionId)}/search`, {
    params: query,
    signal,
  });
  return res.data;
}
