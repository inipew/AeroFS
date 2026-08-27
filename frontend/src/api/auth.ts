import { apiClient } from './client';
import type { UserInfo } from '../types/auth';

export interface LoginPayload {
  username: string;
  password: string;
}

export interface AuthResponse {
  user: UserInfo;
  session_id: string;
}

export async function loginApi(payload: LoginPayload): Promise<AuthResponse> {
  const resp = await apiClient.post<AuthResponse>('/auth/login', payload);
  return resp.data;
}

export async function logoutApi(): Promise<void> {
  await apiClient.post('/auth/logout');
}

export async function meApi(): Promise<UserInfo> {
  const resp = await apiClient.get<UserInfo>('/auth/me');
  return resp.data;
}
