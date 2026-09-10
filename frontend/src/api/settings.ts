import { apiClient } from './client';
import type { components } from './generated/openapi';

export type SettingsResponse = components['schemas']['SettingsResponse'];
export type UpdateSettingsRequest = components['schemas']['UpdateSettingsRequest'];
export type UpdateSettingsResponse = components['schemas']['UpdateSettingsResponse'];
export type AppSettings = components['schemas']['AppSettings'];

export async function getSettings(): Promise<SettingsResponse> {
  const res = await apiClient.get<SettingsResponse>('/settings');
  return res.data;
}

export async function updateSettings(payload: UpdateSettingsRequest): Promise<UpdateSettingsResponse> {
  const res = await apiClient.put<UpdateSettingsResponse>('/settings', payload);
  return res.data;
}
