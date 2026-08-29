import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { loginApi, logoutApi, meApi, type LoginPayload } from '../api/auth';
import type { UserInfo } from '../types/auth';

import { realtimeClient } from '../transport/websocket';

export const useAuthStore = defineStore('auth', () => {
  const user = ref<UserInfo | null>(null);
  const isAuthenticated = ref<boolean>(false);
  const isChecking = ref<boolean>(true);
  const error = ref<string | null>(null);

  const isAdmin = computed<boolean>(() => {
    return user.value?.is_admin ?? false;
  });

  async function checkAuth() {
    isChecking.value = true;
    try {
      const u = await meApi();
      user.value = u;
      isAuthenticated.value = true;
      error.value = null;
    } catch {
      user.value = null;
      isAuthenticated.value = false;
    } finally {
      isChecking.value = false;
    }
  }

  async function login(payload: LoginPayload) {
    error.value = null;
    try {
      const resp = await loginApi(payload);
      user.value = resp.user;
      isAuthenticated.value = true;
      return true;
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || 'Login failed';
      return false;
    }
  }

  async function logout() {
    try {
      await logoutApi();
    } finally {
      user.value = null;
      isAuthenticated.value = false;
      realtimeClient.disconnect();
    }
  }

  return {
    user,
    isAuthenticated,
    isAdmin,
    isChecking,
    error,
    checkAuth,
    login,
    logout,
  };
});
