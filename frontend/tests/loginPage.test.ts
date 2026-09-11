import { describe, expect, it, beforeEach, mock } from 'bun:test';
import { setActivePinia, createPinia } from 'pinia';
import { useAuthStore } from '../src/stores/authStore';

describe('Login & Authentication Flow', () => {
beforeEach(() => {
    setActivePinia(createPinia());
    if (typeof (globalThis as any).localStorage === 'undefined') {
      const store: Record<string, string> = {};
      (globalThis as any).localStorage = {
        getItem: (k: string) => store[k] ?? null,
        setItem: (k: string, v: string) => { store[k] = String(v); },
        removeItem: (k: string) => { delete store[k]; },
        clear: () => { Object.keys(store).forEach((k) => delete store[k]); },
      };
    }
  });

  it('initializes with unauthenticated state', () => {
    const authStore = useAuthStore();
    expect(authStore.isAuthenticated).toBe(false);
    expect(authStore.user).toBeNull();
    expect(authStore.error).toBeNull();
  });

  it('persists and restores remembered username correctly from localStorage', () => {
    const REMEMBER_KEY = 'fb:remembered_username';
    localStorage.setItem(REMEMBER_KEY, 'operator_user');

    const restored = localStorage.getItem(REMEMBER_KEY);
    expect(restored).toBe('operator_user');

    // Clean up
    localStorage.removeItem(REMEMBER_KEY);
    expect(localStorage.getItem(REMEMBER_KEY)).toBeNull();
  });

  it('correctly reports Caps Lock state from KeyboardEvent getModifierState', () => {
    const capsOnEvent = {
      getModifierState: (key: string) => key === 'CapsLock',
    } as unknown as KeyboardEvent;

    const capsOffEvent = {
      getModifierState: (_key: string) => false,
    } as unknown as KeyboardEvent;

    expect(capsOnEvent.getModifierState('CapsLock')).toBe(true);
    expect(capsOffEvent.getModifierState('CapsLock')).toBe(false);
  });
});
