import { defineStore } from 'pinia';
import { ref } from 'vue';
import { apiClient } from '../api/client';
import { useThemeStore } from './themeStore';
import { useUiStore } from './uiStore';
import { useWorkspaceStore } from './workspaceStore';

export interface UserPreferences {
  theme: string;
  language: string;
  default_view: 'grid' | 'list';
  list_density: 'comfortable' | 'compact' | 'dense';
  default_layout: 'single' | 'split';
  show_hidden: boolean;
  sort_field: string;
  sort_order: 'asc' | 'desc';
  remember_last_dir: boolean;
}

const DEFAULT_PREFERENCES: UserPreferences = {
  theme: 'system',
  language: 'en',
  default_view: 'grid',
  list_density: 'comfortable',
  default_layout: 'single',
  show_hidden: false,
  sort_field: 'name',
  sort_order: 'asc',
  remember_last_dir: true,
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
      uiStore.listDensity = prefs.list_density;
    }

    // 3. Apply to Workspace Panels if not already initialized
    const workspaceStore = useWorkspaceStore();
    if (prefs.default_view) {
      if (!workspaceStore.leftPanel.runtime.initialized) {
        workspaceStore.leftPanel.view.viewMode = prefs.default_view;
      }
      if (!workspaceStore.rightPanel.runtime.initialized) {
        workspaceStore.rightPanel.view.viewMode = prefs.default_view;
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
      const resp = await apiClient.get<UserPreferences>('/user/preferences');
      if (resp.data) {
        preferences.value = { ...DEFAULT_PREFERENCES, ...resp.data };
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
      await apiClient.put('/user/preferences', preferences.value);
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
