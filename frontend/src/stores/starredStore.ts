import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { FileEntry } from '../types/vfs';

export interface StarredItem {
  connectionId: string;
  entry: FileEntry;
  starredAt: string;
}

export const useStarredStore = defineStore('starred', () => {
  const saved = localStorage.getItem('fb:starred');
  const starredItems = ref<StarredItem[]>(saved ? JSON.parse(saved) : []);

  function save() {
    localStorage.setItem('fb:starred', JSON.stringify(starredItems.value));
  }

  function isStarred(connectionId: string, path: string): boolean {
    return starredItems.value.some(
      (item) => item.connectionId === connectionId && item.entry.path === path
    );
  }

  function toggleStar(connectionId: string, entry: FileEntry) {
    const idx = starredItems.value.findIndex(
      (item) => item.connectionId === connectionId && item.entry.path === entry.path
    );

    if (idx >= 0) {
      starredItems.value.splice(idx, 1);
    } else {
      starredItems.value.unshift({
        connectionId,
        entry,
        starredAt: new Date().toISOString(),
      });
    }
    save();
  }

  return {
    starredItems,
    isStarred,
    toggleStar,
  };
});
