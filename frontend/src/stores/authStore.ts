import { defineStore } from 'pinia';
import { ref } from 'vue';
import { loginApi, logoutApi, meApi, type LoginPayload } from '../api/auth';
import type { UserInfo } from '../types/auth';

export const useAuthStore = defineStore('auth', () => {
  const user = ref<UserInfo | null>(null);
  const isAuthenticated = ref<boolean>(false);
  const isChecking = ref<boolean>(true);
  const error = ref<string | null>(null);

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
      localStorage.removeItem('session_id');
    } finally {
      isChecking.value = false;
    }
  }

  async function login(payload: LoginPayload) {
    error.value = null;
    try {
      const resp = await loginApi(payload);
      if (resp.session_id) {
        localStorage.setItem('session_id', resp.session_id);
      }
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
      localStorage.removeItem('session_id');
      user.value = null;
      isAuthenticated.value = false;
    }
  }

  return {
    user,
    isAuthenticated,
    isChecking,
    error,
    checkAuth,
    login,
    logout,
  };
});
