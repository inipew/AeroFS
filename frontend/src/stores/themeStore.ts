import { defineStore } from 'pinia';
import { ref } from 'vue';

export const useThemeStore = defineStore('theme', () => {
  const currentTheme = ref<'light' | 'dark' | 'system'>(
    (localStorage.getItem('theme') as any) || 'system'
  );
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const isDark = ref<boolean>(
    currentTheme.value === 'dark' || (currentTheme.value === 'system' && prefersDark)
  );

  function applyTheme(dark: boolean) {
    isDark.value = dark;
    if (dark) {
      document.documentElement.classList.add('dark');
      document.body.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
      document.body.classList.remove('dark');
    }
  }

  function setTheme(theme: 'light' | 'dark' | 'system') {
    currentTheme.value = theme;
    localStorage.setItem('theme', theme);
    if (theme === 'system') {
      const isSysDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
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
