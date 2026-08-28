import { apiClient } from './client';
import type { DirectoryListing, FileMetadata } from '../types/vfs';

export interface ListFilesParams {
  path?: string;
  show_hidden?: boolean;
  sort?: string;
  order?: 'asc' | 'desc';
  signal?: AbortSignal;
}

export async function listFilesApi(
  connectionId: string,
  params: ListFilesParams = {}
): Promise<DirectoryListing> {
  const { signal, ...queryParams } = params;
  const resp = await apiClient.get<DirectoryListing>(`/connections/${connectionId}/files`, {
    params: queryParams,
    signal,
  });
  return resp.data;
}

export async function getMetadataApi(connectionId: string, path: string): Promise<FileMetadata> {
  const resp = await apiClient.get<FileMetadata>(`/connections/${connectionId}/files/metadata`, {
    params: { path },
  });
  return resp.data;
}

export function getDownloadUrl(connectionId: string, path: string): string {
  const base = apiClient.defaults.baseURL?.startsWith('http')
    ? apiClient.defaults.baseURL
    : `${window.location.origin}${apiClient.defaults.baseURL || '/api/v1'}`;
  return `${base}/connections/${connectionId}/files/content?path=${encodeURIComponent(
    path
  )}&download=true`;
}

export async function readFileApi(
  connectionId: string,
  path: string
): Promise<{ content: string; etag: string }> {
  const resp = await apiClient.get<string>(`/connections/${connectionId}/files/content`, {
    params: { path, _t: Date.now() },
    responseType: 'text',
    headers: {
      'Cache-Control': 'no-cache',
    },
  });
  return {
    content: resp.data,
    etag: (resp.headers['etag'] as string) || '',
  };
}

export async function createFileApi(connectionId: string, path: string): Promise<void> {
  await apiClient.post(`/connections/${connectionId}/files`, { path });
}

export async function createDirectoryApi(connectionId: string, path: string): Promise<void> {
  await apiClient.post(`/connections/${connectionId}/directories`, { path });
}

export async function deleteFilesApi(connectionId: string, paths: string[]): Promise<void> {
  await apiClient.delete(`/connections/${connectionId}/files`, {
    data: { paths },
  });
}

export async function renameEntryApi(
  connectionId: string,
  from: string,
  to: string
): Promise<void> {
  await apiClient.post(`/connections/${connectionId}/files/rename`, { from, to });
}

export async function copyEntryApi(
  connectionId: string,
  from: string,
  to: string
): Promise<void> {
  await apiClient.post(`/connections/${connectionId}/files/copy`, { from, to });
}

export async function moveEntryApi(
  connectionId: string,
  from: string,
  to: string
): Promise<void> {
  await apiClient.post(`/connections/${connectionId}/files/move`, { from, to });
}

export async function uploadFileApi(
  connectionId: string,
  targetDir: string,
  file: File,
  onProgress?: (percent: number) => void
): Promise<void> {
  const formData = new FormData();
  formData.append('path', targetDir);
  formData.append('file', file);

  await apiClient.post(`/connections/${connectionId}/upload`, formData, {
    headers: {
      'Content-Type': 'multipart/form-data',
    },
    onUploadProgress: (progressEvent) => {
      if (progressEvent.total && onProgress) {
        const percent = Math.round((progressEvent.loaded * 100) / progressEvent.total);
        onProgress(percent);
      }
    },
  });
}
