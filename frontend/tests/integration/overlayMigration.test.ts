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

  it('supports properties overlay intent with connectionId and path', () => {
    const overlayStore = useOverlayStore();
    overlayStore.open({ type: 'properties', connectionId: 'local', path: '/var/log/app.log' });
    expect(overlayStore.isOpen('properties')).toBe(true);
    expect(overlayStore.current).toEqual({
      type: 'properties',
      connectionId: 'local',
      path: '/var/log/app.log',
    });
    overlayStore.close();
    expect(overlayStore.isOpen('properties')).toBe(false);
  });

  it('calculates smart filename selection range excluding extension', () => {
    function getSelectionRange(filename: string): [number, number] {
      const dotIdx = filename.lastIndexOf('.');
      if (dotIdx > 0) {
        return [0, dotIdx];
      }
      return [0, filename.length];
    }

    expect(getSelectionRange('report.final.pdf')).toEqual([0, 12]);
    expect(getSelectionRange('archive.tar.gz')).toEqual([0, 11]);
    expect(getSelectionRange('Makefile')).toEqual([0, 8]);
    expect(getSelectionRange('.env')).toEqual([0, 4]);
  });

  it('supports archive-viewer overlay intent with connectionId and archivePath', () => {
    const overlayStore = useOverlayStore();
    overlayStore.open({
      type: 'archive-viewer',
      connectionId: 'local',
      archivePath: '/backup/data.zip',
    });
    expect(overlayStore.isOpen('archive-viewer')).toBe(true);
    expect(overlayStore.current).toEqual({
      type: 'archive-viewer',
      connectionId: 'local',
      archivePath: '/backup/data.zip',
    });
    overlayStore.close();
    expect(overlayStore.isOpen('archive-viewer')).toBe(false);
  });
});
