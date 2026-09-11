import { defineStore } from 'pinia';
import { shallowRef } from 'vue';
import type { Overlay } from './overlayTypes';

export const useOverlayStore = defineStore('overlay', () => {
  const current = shallowRef<Overlay | null>(null);

  function open(next: Overlay) {
    current.value = next;
  }

  function replace(next: Overlay) {
    current.value = next;
  }

  function close() {
    current.value = null;
  }

  function isOpen(type: Overlay['type']): boolean {
    return current.value?.type === type;
  }

  return {
    current,
    open,
    replace,
    close,
    isOpen,
  };
});
