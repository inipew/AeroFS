import { describe, expect, it, beforeEach, spyOn } from 'bun:test';
import { setActivePinia, createPinia } from 'pinia';
import {
  loadEditorPreferences,
  saveEditorPreference,
  saveEditorSession,
  loadEditorSession,
  clearEditorSession,
  flushEditorSessionNow,
  EDITOR_SESSION_STORAGE_KEY,
} from '../src/services/editorSession';
import { useEditorStore } from '../src/stores/editorStore';
import { useUiStore } from '../src/stores/uiStore';
import { useOverlayStore } from '../src/overlays/overlayStore';
import * as fileApi from '../src/api/files';
import type { FileEntry } from '../src/types/vfs';

describe('Editor Session & Preferences Persistence', () => {
  beforeEach(() => {
    if (typeof (globalThis as any).localStorage === 'undefined') {
      const store: Record<string, string> = {};
      (globalThis as any).localStorage = {
        getItem: (k: string) => store[k] ?? null,
        setItem: (k: string, v: string) => { store[k] = String(v); },
        removeItem: (k: string) => { delete store[k]; },
        clear: () => { Object.keys(store).forEach((k) => delete store[k]); },
      };
    }
    localStorage.clear();
    setActivePinia(createPinia());
  });

  it('loads default preferences based on theme', () => {
    const darkPrefs = loadEditorPreferences(true);
    expect(darkPrefs.theme).toBe('ace/theme/tomorrow_night');
    expect(darkPrefs.fontSize).toBe(13);
    expect(darkPrefs.tabSize).toBe(2);

    const lightPrefs = loadEditorPreferences(false);
    expect(lightPrefs.theme).toBe('ace/theme/chrome');
  });

  it('migrates legacy fb:editor preferences when modern aerofs:editor keys are absent', () => {
    localStorage.setItem('fb:editor:theme', 'ace/theme/dracula');
    localStorage.setItem('fb:editor:fontSize', '16');
    localStorage.setItem('fb:editor:tabSize', '4');

    const prefs = loadEditorPreferences(true);
    expect(prefs.theme).toBe('ace/theme/dracula');
    expect(prefs.fontSize).toBe(16);
    expect(prefs.tabSize).toBe(4);
  });

  it('prefers modern aerofs:editor keys over legacy keys', () => {
    localStorage.setItem('fb:editor:theme', 'ace/theme/monokai');
    localStorage.setItem('aerofs:editor:theme', 'ace/theme/nord_dark');

    const prefs = loadEditorPreferences(true);
    expect(prefs.theme).toBe('ace/theme/nord_dark');
  });

  it('saves preference to modern aerofs:editor namespace', () => {
    saveEditorPreference('fontSize', 14);
    expect(localStorage.getItem('aerofs:editor:fontSize')).toBe('14');

    saveEditorPreference('theme', 'ace/theme/one_dark');
    expect(localStorage.getItem('aerofs:editor:theme')).toBe('ace/theme/one_dark');
  });

  it('serializes and restores active session snapshot', () => {
    saveEditorSession({
      version: 1,
      open: true,
      connectionId: 'conn-1',
      path: '/etc/nginx.conf',
      name: 'nginx.conf',
      etag: 'etag-123',
      draft: 'server { listen 80; }',
      dirty: true,
      cursor: { row: 4, column: 12 },
      scrollTop: 150,
      languageMode: 'ace/mode/nginx',
      showMarkdownPreview: false,
      savedAt: Date.now(),
    });

    const session = loadEditorSession();
    expect(session).not.toBeNull();
    expect(session?.name).toBe('nginx.conf');
    expect(session?.path).toBe('/etc/nginx.conf');
    expect(session?.dirty).toBe(true);
    expect(session?.draft).toBe('server { listen 80; }');
    expect(session?.cursor).toEqual({ row: 4, column: 12 });
    expect(session?.scrollTop).toBe(150);

    clearEditorSession();
    expect(loadEditorSession()).toBeNull();
  });

  it('omits draft when dirty is false to avoid localStorage quota exhaustion', () => {
    saveEditorSession({
      version: 1,
      open: true,
      connectionId: 'conn-1',
      path: '/etc/nginx.conf',
      name: 'nginx.conf',
      etag: 'etag-123',
      draft: 'clean server content',
      dirty: false,
      cursor: { row: 0, column: 0 },
      scrollTop: 0,
      languageMode: 'ace/mode/nginx',
      showMarkdownPreview: false,
      savedAt: Date.now(),
    });

    const raw = localStorage.getItem(EDITOR_SESSION_STORAGE_KEY);
    expect(raw).not.toBeNull();
    const parsed = JSON.parse(raw!);
    expect(parsed.draft).toBeUndefined();
    expect(parsed.dirty).toBe(false);
  });

  it('flushes session immediately via flushEditorSessionNow', () => {
    const sessionData = {
      version: 1 as const,
      open: true,
      connectionId: 'conn-1',
      path: '/app.ts',
      name: 'app.ts',
      etag: 'etag-1',
      draft: 'immediate flush content',
      dirty: true,
      cursor: { row: 1, column: 2 },
      scrollTop: 10,
      languageMode: 'ace/mode/typescript',
      showMarkdownPreview: false,
      savedAt: Date.now(),
    };

    flushEditorSessionNow(sessionData);
    const loaded = loadEditorSession();
    expect(loaded).not.toBeNull();
    expect(loaded?.draft).toBe('immediate flush content');
  });

  it('gracefully handles and clears corrupted session JSON', () => {
    localStorage.setItem(EDITOR_SESSION_STORAGE_KEY, '{invalid json');
    const session = loadEditorSession();
    expect(session).toBeNull();
    expect(localStorage.getItem(EDITOR_SESSION_STORAGE_KEY)).toBeNull();
  });
});

describe('Editor Store & UI Store Delegation', () => {
  beforeEach(() => {
    if (typeof (globalThis as any).localStorage === 'undefined') {
      const store: Record<string, string> = {};
      (globalThis as any).localStorage = {
        getItem: (k: string) => store[k] ?? null,
        setItem: (k: string, v: string) => { store[k] = String(v); },
        removeItem: (k: string) => { delete store[k]; },
        clear: () => { Object.keys(store).forEach((k) => delete store[k]); },
      };
    }
    localStorage.clear();
    setActivePinia(createPinia());
  });

  const mockFile: FileEntry = {
    name: 'app.ts',
    path: '/src/app.ts',
    kind: 'file',
    size: 1024,
    modified_at: new Date().toISOString(),
    is_hidden: false,
  };

  it('opens file and initializes clean state in editorStore and overlayStore', () => {
    const editorStore = useEditorStore();
    const overlayStore = useOverlayStore();

    editorStore.openFile(mockFile, 'console.log("hello");', 'etag-abc', 'local');

    expect(editorStore.isOpen).toBe(true);
    expect(editorStore.activeFile).toEqual(mockFile);
    expect(editorStore.content).toBe('console.log("hello");');
    expect(editorStore.etag).toBe('etag-abc');
    expect(editorStore.isDirty).toBe(false);
    expect(overlayStore.isOpen('editor')).toBe(true);
  });

  it('updates content and marks dirty state reactively', () => {
    const editorStore = useEditorStore();
    editorStore.openFile(mockFile, 'const a = 1;', 'etag-1', 'local');

    expect(editorStore.isDirty).toBe(false);

    editorStore.updateContent('const a = 2;');
    expect(editorStore.isDirty).toBe(true);

    editorStore.updateContent('const a = 1;');
    expect(editorStore.isDirty).toBe(false);
  });

  it('delegates uiStore.openEditor to editorStore cleanly', () => {
    const uiStore = useUiStore();
    const editorStore = useEditorStore();

    uiStore.openEditor(mockFile, 'export default {};', 'etag-99', 'conn-xyz');

    expect(editorStore.isOpen).toBe(true);
    expect(uiStore.isEditorOpen).toBe(true);
    expect(uiStore.editorFile?.name).toBe('app.ts');
    expect(uiStore.editorContent).toBe('export default {};');
    expect(uiStore.editorEtag).toBe('etag-99');
    expect(uiStore.editorConnectionId).toBe('conn-xyz');
  });

  it('requestClose prompts confirmation modal when dirty, and closes immediately when clean', () => {
    const editorStore = useEditorStore();
    editorStore.openFile(mockFile, 'foo', 'etag-1', 'local');

    // Clean close
    editorStore.requestClose();
    expect(editorStore.isUnsavedConfirmOpen).toBe(false);
    expect(editorStore.isOpen).toBe(false);

    // Dirty close
    editorStore.openFile(mockFile, 'foo', 'etag-1', 'local');
    editorStore.updateContent('foo modified');
    editorStore.requestClose();
    expect(editorStore.isUnsavedConfirmOpen).toBe(true);
    expect(editorStore.isOpen).toBe(true);

    // Discard & close
    editorStore.discardAndClose();
    expect(editorStore.isUnsavedConfirmOpen).toBe(false);
    expect(editorStore.isOpen).toBe(false);
    expect(editorStore.isDirty).toBe(false);
  });

  it('syncs editor state immediately to storage on flushSession', () => {
    const editorStore = useEditorStore();
    editorStore.openFile(mockFile, 'initial', 'etag-1', 'conn-1');
    editorStore.updateContent('modified immediately');
    editorStore.flushSession();

    const stored = loadEditorSession();
    expect(stored).not.toBeNull();
    expect(stored?.draft).toBe('modified immediately');
    expect(stored?.dirty).toBe(true);
  });

  it('reloadFromDisk updates content, resets dirty, and closes conflict dialog', async () => {
    const readSpy = spyOn(fileApi, 'readFileApi').mockResolvedValue({
      content: 'updated server content',
      etag: 'etag-updated',
    });

    const editorStore = useEditorStore();
    editorStore.openFile(mockFile, 'initial content', 'etag-old', 'local');
    editorStore.updateContent('draft edits');
    editorStore.isConflictModalOpen = true;

    expect(editorStore.isDirty).toBe(true);
    expect(editorStore.isConflictModalOpen).toBe(true);

    const ok = await editorStore.reloadFromDisk();

    expect(ok).toBe(true);
    expect(editorStore.content).toBe('updated server content');
    expect(editorStore.savedContent).toBe('updated server content');
    expect(editorStore.etag).toBe('etag-updated');
    expect(editorStore.isDirty).toBe(false);
    expect(editorStore.isConflictModalOpen).toBe(false);
    expect(readSpy).toHaveBeenCalled();
    readSpy.mockRestore();
  });

  it('restoreSession closes editor and cleans ghost session when server returns 404', async () => {
    const readSpy = spyOn(fileApi, 'readFileApi').mockRejectedValue({
      response: { status: 404, data: { message: 'File not found' } },
    });

    saveEditorSession({
      version: 1,
      open: true,
      connectionId: 'conn-1',
      path: '/missing/file.ts',
      name: 'file.ts',
      etag: 'etag-missing',
      dirty: false,
      cursor: { row: 0, column: 0 },
      scrollTop: 0,
      languageMode: 'ace/mode/typescript',
      showMarkdownPreview: false,
      savedAt: Date.now(),
    });

    const editorStore = useEditorStore();
    const restored = await editorStore.restoreSession();

    expect(restored).toBe(false);
    expect(editorStore.isOpen).toBe(false);
    expect(editorStore.activeFile).toBeNull();
    expect(loadEditorSession()).toBeNull();
    readSpy.mockRestore();
  });
});
