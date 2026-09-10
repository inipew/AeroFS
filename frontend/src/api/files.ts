import { apiClient } from './client';
import type { DirectoryListing, FileMetadata } from '../types/vfs';
import { streamUpload } from '../services/transfer/fetchStream';

export interface ListFilesParams {
  path?: string;
  show_hidden?: boolean;
  sort?: string;
  order?: 'asc' | 'desc';
  cursor?: string;
  limit?: number;
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

export interface PresignResponse {
  url: string;
  expires_in_seconds: number;
}

export async function getPresignedDownloadUrlApi(
  connectionId: string,
  path: string,
  expireSeconds: number = 3600
): Promise<PresignResponse> {
  const resp = await apiClient.post<PresignResponse>(
    `/connections/${connectionId}/files/presign/download`,
    {
      path,
      expire_seconds: expireSeconds,
    }
  );
  return resp.data;
}

export async function getPresignedUploadUrlApi(
  connectionId: string,
  path: string,
  expireSeconds: number = 3600
): Promise<PresignResponse> {
  const resp = await apiClient.post<PresignResponse>(
    `/connections/${connectionId}/files/presign/upload`,
    {
      path,
      expire_seconds: expireSeconds,
    }
  );
  return resp.data;
}

export async function completePresignedUploadApi(
  connectionId: string,
  path: string,
  expectedSize?: number,
  expectedChecksum?: string
): Promise<FileMetadata> {
  const resp = await apiClient.post<FileMetadata>(
    `/connections/${connectionId}/files/presign/complete`,
    {
      path,
      expected_size: expectedSize,
      expected_checksum: expectedChecksum,
    }
  );
  return resp.data;
}

export function getContentUrl(connectionId: string, path: string): string {
  const base = getApiBaseUrl();
  return `${base}/connections/${connectionId}/files/content?path=${encodeURIComponent(
    path
  )}`;
}

export function getDownloadUrl(connectionId: string, path: string): string {
  const base = getApiBaseUrl();
  return `${base}/connections/${connectionId}/files/content?path=${encodeURIComponent(
    path
  )}&download=true`;
}

export function getApiBaseUrl(): string {
  const configured = apiClient.defaults.baseURL || '/api/v1';
  return configured.startsWith('http') ? configured.replace(/\/$/, '') : `${window.location.origin}${configured}`;
}

export interface UploadSessionResponse {
  job_id: string;
  upload_url: string;
}

/** Upload through a job-bound session so the transfer drawer and cancellation
 * endpoint operate on the same server-side transfer. */
export async function uploadFileAsTransferApi(
  connectionId: string,
  targetDir: string,
  file: File,
  signal: AbortSignal,
  onProgress?: (percent: number) => void,
  onSession?: (session: UploadSessionResponse) => void
): Promise<UploadSessionResponse> {
  const path = targetDir === '/' ? `/${file.name}` : `${targetDir.replace(/\/$/, '')}/${file.name}`;
  const session = await apiClient.post<UploadSessionResponse>(
    `/connections/${connectionId}/uploads`,
    { path, file_name: file.name, total_bytes: file.size },
    { signal }
  );
  const response = session.data;
  onSession?.(response);
  const url = response.upload_url.startsWith('http') ? response.upload_url : `${getApiBaseUrl()}${response.upload_url}`;
  await streamUpload(url, file, signal, (loaded, total) => {
    if (total > 0) onProgress?.(Math.round((loaded * 100) / total));
  });
  return response;
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

export async function uploadFileApi(
  connectionId: string,
  targetDir: string,
  file: File,
  onProgress?: (percent: number) => void,
  signal?: AbortSignal
): Promise<void> {
  const formData = new FormData();
  formData.append('path', targetDir);
  formData.append('file', file);

  await apiClient.post(`/connections/${connectionId}/upload`, formData, {
    headers: {
      'Content-Type': 'multipart/form-data',
    },
    signal,
    onUploadProgress: (progressEvent) => {
      if (progressEvent.total && onProgress) {
        const percent = Math.round((progressEvent.loaded * 100) / progressEvent.total);
        onProgress(percent);
      }
    },
  });
}

export interface ChmodPayload {
  path: string;
  mode: number;
  recursive?: boolean;
}

export async function chmodFileApi(
  connectionId: string,
  payload: ChmodPayload
): Promise<any> {
  const resp = await apiClient.post(
    `/connections/${connectionId}/files/chmod`,
    payload
  );
  return resp.data;
}

export async function getStorageInfoApi(connectionId: string): Promise<any> {
  const resp = await apiClient.get(
    `/connections/${connectionId}/storage-info`
  );
  return resp.data;
}

export async function updateFileContentApi(
  connectionId: string,
  path: string,
  content: string,
  options?: {
    ifMatch?: string;
    forceOverwrite?: boolean;
  }
): Promise<{ success: boolean; message: string; etag: string }> {
  const headers: Record<string, string> = {};
  if (options?.forceOverwrite) {
    headers['X-Force-Overwrite'] = 'true';
  } else if (options?.ifMatch) {
    headers['If-Match'] = options.ifMatch;
  }

  const resp = await apiClient.put<{ success: boolean; message: string }>(
    `/connections/${connectionId}/files/content`,
    { path, content },
    { headers }
  );

  return {
    success: resp.data.success,
    message: resp.data.message,
    etag: (resp.headers['etag'] as string) || '',
  };
}

