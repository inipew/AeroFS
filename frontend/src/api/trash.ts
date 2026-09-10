import { apiClient } from './client';
import type { components } from './generated/openapi';

export type TrashItem = components['schemas']['TrashItem'];
export type MoveToTrashRequest = components['schemas']['MoveToTrashRequest'];
export type MovedTrashItem = components['schemas']['MovedTrashItem'];
export type MoveToTrashResponse = components['schemas']['MoveToTrashResponse'];
export type TrashActionResponse = components['schemas']['TrashActionResponse'];
export type EmptyTrashResponse = components['schemas']['EmptyTrashResponse'];

export async function listTrash(): Promise<TrashItem[]> {
  const res = await apiClient.get<TrashItem[]>('/trash');
  return res.data;
}

export async function moveToTrash(connectionId: string, paths: string[]): Promise<MoveToTrashResponse> {
  const res = await apiClient.post<MoveToTrashResponse>('/trash/move', {
    connection_id: connectionId,
    paths,
  });
  return res.data;
}

export async function restoreTrashItem(trashId: string): Promise<TrashActionResponse> {
  const res = await apiClient.post<TrashActionResponse>(`/trash/restore/${encodeURIComponent(trashId)}`);
  return res.data;
}

export async function deleteTrashItem(trashId: string): Promise<TrashActionResponse> {
  const res = await apiClient.delete<TrashActionResponse>(`/trash/${encodeURIComponent(trashId)}`);
  return res.data;
}

export async function emptyTrash(): Promise<EmptyTrashResponse> {
  const res = await apiClient.delete<EmptyTrashResponse>('/trash/empty');
  return res.data;
}
