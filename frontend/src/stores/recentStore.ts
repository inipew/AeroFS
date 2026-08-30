import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { FileEntry } from '../types/vfs';

export interface RecentItem {
  connectionId: string;
  entry: FileEntry;
  accessedAt: string;
}

export const useRecentStore = defineStore('recent', () => {
  const saved = localStorage.getItem('fb:recent');
  const recentItems = ref<RecentItem[]>(saved ? JSON.parse(saved) : []);

  function save() {
    localStorage.setItem('fb:recent', JSON.stringify(recentItems.value));
  }

  function addRecent(connectionId: string, entry: FileEntry) {
    if (!entry || !entry.path) return;
    const idx = recentItems.value.findIndex(
      (item) => item.connectionId === connectionId && item.entry.path === entry.path
    );
    if (idx >= 0) {
      recentItems.value.splice(idx, 1);
    }

    recentItems.value.unshift({
      connectionId,
      entry,
      accessedAt: new Date().toISOString(),
    });

    if (recentItems.value.length > 50) {
      recentItems.value.length = 50;
    }

    save();
  }

  function removeRecent(connectionId: string, path: string) {
    const idx = recentItems.value.findIndex(
      (item) => item.connectionId === connectionId && item.entry.path === path
    );
    if (idx >= 0) {
      recentItems.value.splice(idx, 1);
      save();
    }
  }

  function clearRecent() {
    recentItems.value = [];
    save();
  }

  return {
    recentItems,
    addRecent,
    removeRecent,
    clearRecent,
  };
});
