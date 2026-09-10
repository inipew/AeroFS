import { apiClient } from './client';
import type { components } from './generated/openapi';

export type UserInfo = components['schemas']['UserInfo'];
export type LoginPayload = components['schemas']['LoginRequest'];
export type AuthResponse = components['schemas']['AuthResponse'];
export type LogoutResponse = components['schemas']['LogoutResponse'];

export async function loginApi(payload: LoginPayload): Promise<AuthResponse> {
  const resp = await apiClient.post<AuthResponse>('/auth/login', payload);
  return resp.data;
}

export async function logoutApi(): Promise<LogoutResponse> {
  const resp = await apiClient.post<LogoutResponse>('/auth/logout');
  return resp.data;
}

export async function meApi(): Promise<UserInfo> {
  const resp = await apiClient.get<UserInfo>('/auth/me');
  return resp.data;
}
