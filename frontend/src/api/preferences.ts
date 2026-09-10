import { apiClient } from './client';
import type { components } from './generated/openapi';

export type UserPreferences = components['schemas']['UserPreferences'];
export type UpdatePreferencesResponse = components['schemas']['UpdatePreferencesResponse'];

export async function getUserPreferences(): Promise<UserPreferences> {
  const res = await apiClient.get<UserPreferences>('/user/preferences');
  return res.data;
}

export async function updateUserPreferences(payload: UserPreferences): Promise<UpdatePreferencesResponse> {
  const res = await apiClient.put<UpdatePreferencesResponse>('/user/preferences', payload);
  return res.data;
}
