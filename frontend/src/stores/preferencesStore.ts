import { defineStore } from 'pinia';
import { ref } from 'vue';
import { getUserPreferences, updateUserPreferences, type UserPreferences } from '../api/preferences';
import { useThemeStore } from './themeStore';
import { useUiStore } from './uiStore';
import { useWorkspaceStore } from './workspaceStore';

const DEFAULT_PREFERENCES: UserPreferences = {
  theme: 'dark',
  language: 'en',
  default_view: 'grid',
  list_density: 'comfortable',
  default_layout: 'single',
  show_hidden: false,
  default_sort: 'name',
  sort_direction: 'asc',
  remember_last_directories: true,
  confirm_destructive: true,
  show_breadcrumbs: true,
  show_file_size: true,
  show_permissions: true,
};

export const usePreferencesStore = defineStore('preferences', () => {
  const preferences = ref<UserPreferences>({ ...DEFAULT_PREFERENCES });
  const isLoaded = ref(false);
  const isSaving = ref(false);

  // Load cached preferences from localStorage if available
  try {
    const cached = localStorage.getItem('fb:user_preferences');
    if (cached) {
      preferences.value = { ...DEFAULT_PREFERENCES, ...JSON.parse(cached) };
    }
  } catch {}

  function applyPreferencesToStores(prefs: UserPreferences) {
    // 1. Apply Theme
    const themeStore = useThemeStore();
    if (prefs.theme && ['light', 'dark', 'system'].includes(prefs.theme)) {
      themeStore.setTheme(prefs.theme as 'light' | 'dark' | 'system');
    }

    // 2. Apply UI Density
    const uiStore = useUiStore();
    if (prefs.list_density && ['comfortable', 'compact', 'dense'].includes(prefs.list_density)) {
      uiStore.listDensity = prefs.list_density as 'comfortable' | 'compact' | 'dense';
    }

    // 3. Apply to Workspace Panels if not already initialized
    const workspaceStore = useWorkspaceStore();
    if (prefs.default_view) {
      const mode = (prefs.default_view === 'list' ? 'list' : 'grid') as 'grid' | 'list';
      if (!workspaceStore.leftPanel.runtime.initialized) {
        workspaceStore.leftPanel.view.viewMode = mode;
      }
      if (!workspaceStore.rightPanel.runtime.initialized) {
        workspaceStore.rightPanel.view.viewMode = mode;
      }
    }
    if (prefs.show_hidden !== undefined) {
      if (!workspaceStore.leftPanel.runtime.initialized) {
        workspaceStore.leftPanel.view.showHidden = prefs.show_hidden;
      }
      if (!workspaceStore.rightPanel.runtime.initialized) {
        workspaceStore.rightPanel.view.showHidden = prefs.show_hidden;
      }
    }
  }

  async function fetchPreferences(): Promise<UserPreferences> {
    try {
      const data = await getUserPreferences();
      if (data) {
        preferences.value = { ...DEFAULT_PREFERENCES, ...data };
        localStorage.setItem('fb:user_preferences', JSON.stringify(preferences.value));
        applyPreferencesToStores(preferences.value);
        isLoaded.value = true;
      }
    } catch (err) {
      console.warn('Failed to fetch user preferences from server, using local defaults', err);
    }
    return preferences.value;
  }

  async function updatePreferences(partial: Partial<UserPreferences>): Promise<boolean> {
    isSaving.value = true;
    preferences.value = { ...preferences.value, ...partial };
    localStorage.setItem('fb:user_preferences', JSON.stringify(preferences.value));
    applyPreferencesToStores(preferences.value);

    try {
      await updateUserPreferences(preferences.value);
      return true;
    } catch (err) {
      console.error('Failed to persist user preferences to backend', err);
      return false;
    } finally {
      isSaving.value = false;
    }
  }

  return {
    preferences,
    isLoaded,
    isSaving,
    fetchPreferences,
    updatePreferences,
    applyPreferencesToStores,
  };
});
