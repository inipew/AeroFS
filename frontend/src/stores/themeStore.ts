import { defineStore } from 'pinia';
import { ref } from 'vue';

export const useThemeStore = defineStore('theme', () => {
  const savedTheme = typeof localStorage !== 'undefined' ? (localStorage.getItem('theme') as any) : null;
  const currentTheme = ref<'light' | 'dark' | 'system'>(savedTheme || 'system');
  const prefersDark =
    typeof window !== 'undefined' && typeof window.matchMedia === 'function'
      ? window.matchMedia('(prefers-color-scheme: dark)').matches
      : false;
  const isDark = ref<boolean>(
    currentTheme.value === 'dark' || (currentTheme.value === 'system' && prefersDark)
  );

  function applyTheme(dark: boolean) {
    isDark.value = dark;
    if (typeof document !== 'undefined') {
      if (dark) {
        document.documentElement.classList.add('dark');
        document.body.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
        document.body.classList.remove('dark');
      }
    }
  }

  function setTheme(theme: 'light' | 'dark' | 'system') {
    currentTheme.value = theme;
    try {
      localStorage.setItem('theme', theme);
    } catch {
      // Ignore
    }
    if (theme === 'system') {
      const isSysDark =
        typeof window !== 'undefined' && typeof window.matchMedia === 'function'
          ? window.matchMedia('(prefers-color-scheme: dark)').matches
          : false;
      applyTheme(isSysDark);
    } else {
      applyTheme(theme === 'dark');
    }
  }

  function toggleTheme() {
    setTheme(isDark.value ? 'light' : 'dark');
  }

  // Initialize theme on store creation
  setTheme(currentTheme.value);

  return {
    theme: currentTheme,
    isDark,
    toggleTheme,
    applyTheme,
    setTheme,
  };
});
