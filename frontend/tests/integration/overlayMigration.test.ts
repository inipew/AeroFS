import { describe, expect, it, beforeEach } from 'bun:test';
import { setActivePinia, createPinia } from 'pinia';
import { useOverlayStore } from '../../src/overlays/overlayStore';
import { useUiStore } from '../../src/stores/uiStore';

describe('Integration: Overlay Store & Migration Compatibility', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('initializes overlayStore with null current state', () => {
    const store = useOverlayStore();
    expect(store.current).toBeNull();
    expect(store.isOpen('search')).toBe(false);
  });

  it('opens and closes modal overlay cleanly', () => {
    const store = useOverlayStore();
    store.open({ type: 'settings' });
    expect(store.current).toEqual({ type: 'settings' });
    expect(store.isOpen('settings')).toBe(true);
    expect(store.isOpen('search')).toBe(false);

    store.close();
    expect(store.current).toBeNull();
    expect(store.isOpen('settings')).toBe(false);
  });

  it('replaces active overlay with next overlay', () => {
    const store = useOverlayStore();
    store.open({ type: 'settings' });
    expect(store.isOpen('settings')).toBe(true);

    store.replace({ type: 'search' });
    expect(store.isOpen('settings')).toBe(false);
    expect(store.isOpen('search')).toBe(true);
  });

  it('delegates legacy uiStore actions to overlayStore', () => {
    const overlayStore = useOverlayStore();
    const uiStore = useUiStore();

    // Legacy openCreate
    uiStore.openCreate('file');
    expect(uiStore.isCreateOpen).toBe(true);
    expect(uiStore.createType).toBe('file');

    // Canonical open
    overlayStore.open({ type: 'rename', panelId: 'left', path: '/foo.txt' });
    expect(overlayStore.isOpen('rename')).toBe(true);
    expect(overlayStore.current).toEqual({ type: 'rename', panelId: 'left', path: '/foo.txt' });
  });
});
