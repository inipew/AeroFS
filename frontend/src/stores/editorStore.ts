import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import type { FileEntry } from '../types/vfs';
import { readFileApi, updateFileContentApi } from '../api/files';
import {
  loadEditorPreferences,
  saveEditorPreference,
  saveEditorSession,
  loadEditorSession,
  clearEditorSession,
  type EditorPreferences,
  type EditorCursorPosition,
} from '../services/editorSession';
import { useThemeStore } from './themeStore';
import { useOverlayStore } from '../overlays/overlayStore';
import { useWorkspaceStore } from './workspaceStore';

export const useEditorStore = defineStore('editor', () => {
  const themeStore = useThemeStore();
  const overlayStore = useOverlayStore();
  const workspaceStore = useWorkspaceStore();

  // State
  const isOpen = ref(false);
  const activeFile = ref<FileEntry | null>(null);
  const connectionId = ref<string>('local');
  const content = ref<string>('');
  const savedContent = ref<string>('');
  const etag = ref<string>('');
  const isDirty = ref(false);

  const cursorPosition = ref<EditorCursorPosition>({ row: 0, column: 0 });
  const scrollTop = ref<number>(0);
  const languageMode = ref<string>('ace/mode/text');
  const showMarkdownPreview = ref(false);

  const saving = ref(false);
  const isLoading = ref(false);
  const isConflictModalOpen = ref(false);
  const isUnsavedConfirmOpen = ref(false);
  const remoteConflictDetected = ref(false);

  // Preferences
  const preferences = ref<EditorPreferences>(loadEditorPreferences(themeStore.isDark));

  // Sync theme changes if user hasn't chosen an explicit theme
  watch(
    () => themeStore.isDark,
    (isDark) => {
      const stored = localStorage.getItem('aerofs:editor:theme') || localStorage.getItem('fb:editor:theme');
      if (!stored) {
        preferences.value.theme = isDark ? 'ace/theme/tomorrow_night' : 'ace/theme/chrome';
      }
    }
  );

  function updatePreference<K extends keyof EditorPreferences>(key: K, value: EditorPreferences[K]) {
    preferences.value[key] = value;
    saveEditorPreference(key, value);
  }

  // Session persistence with debounced autosave and synchronous flush
  let sessionSaveTimer: ReturnType<typeof setTimeout> | null = null;

  function flushSession() {
    if (sessionSaveTimer) {
      clearTimeout(sessionSaveTimer);
      sessionSaveTimer = null;
    }
    if (!isOpen.value || !activeFile.value) return;
    saveEditorSession({
      version: 1,
      open: true,
      connectionId: connectionId.value,
      path: activeFile.value.path,
      name: activeFile.value.name,
      etag: etag.value,
      draft: isDirty.value ? content.value : undefined,
      dirty: isDirty.value,
      cursor: cursorPosition.value,
      scrollTop: scrollTop.value,
      languageMode: languageMode.value,
      showMarkdownPreview: showMarkdownPreview.value,
      savedAt: Date.now(),
    });
  }

  function scheduleSessionPersist() {
    if (sessionSaveTimer) clearTimeout(sessionSaveTimer);
    sessionSaveTimer = setTimeout(() => {
      flushSession();
    }, 250);
  }

  function openFile(
    entry: FileEntry,
    initialContent: string,
    initialEtag: string = '',
    connId: string = 'local',
    initialCursor?: EditorCursorPosition,
    initialScrollTop?: number
  ) {
    activeFile.value = entry;
    content.value = initialContent;
    savedContent.value = initialContent;
    etag.value = initialEtag;
    connectionId.value = connId;
    isDirty.value = false;
    remoteConflictDetected.value = false;

    if (initialCursor) cursorPosition.value = initialCursor;
    if (initialScrollTop !== undefined) scrollTop.value = initialScrollTop;

    isConflictModalOpen.value = false;
    isUnsavedConfirmOpen.value = false;
    isOpen.value = true;

    // Synchronize with overlayStore if not already open
    if (overlayStore.current?.type !== 'editor' || overlayStore.current.path !== entry.path) {
      overlayStore.open({ type: 'editor', connectionId: connId, path: entry.path });
    }

    scheduleSessionPersist();
  }

  function updateContent(newContent: string) {
    content.value = newContent;
    isDirty.value = newContent !== savedContent.value;
    scheduleSessionPersist();
  }

  function updateCursor(cursor: EditorCursorPosition) {
    cursorPosition.value = cursor;
    scheduleSessionPersist();
  }

  function updateScroll(top: number) {
    scrollTop.value = top;
    scheduleSessionPersist();
  }

  function setLanguageMode(mode: string) {
    languageMode.value = mode;
    scheduleSessionPersist();
  }

  function toggleMarkdownPreview(forceVal?: boolean) {
    showMarkdownPreview.value = forceVal !== undefined ? forceVal : !showMarkdownPreview.value;
    scheduleSessionPersist();
  }

  async function saveFile(): Promise<boolean> {
    if (!activeFile.value) return false;
    saving.value = true;

    const currentText = content.value;
    try {
      const resp = await updateFileContentApi(
        connectionId.value,
        activeFile.value.path,
        currentText,
        {
          ifMatch: etag.value || undefined,
        }
      );

      if (resp.etag) {
        etag.value = resp.etag;
      }
      savedContent.value = currentText;
      isDirty.value = false;
      remoteConflictDetected.value = false;

      scheduleSessionPersist();
      await workspaceStore.refreshAll();
      return true;
    } catch (err: any) {
      if (err.response?.status === 409 || err.response?.status === 412) {
        isConflictModalOpen.value = true;
      }
      throw err;
    } finally {
      saving.value = false;
    }
  }

  async function forceSaveFile(): Promise<boolean> {
    if (!activeFile.value) return false;
    saving.value = true;

    const currentText = content.value;
    try {
      const resp = await updateFileContentApi(
        connectionId.value,
        activeFile.value.path,
        currentText,
        {
          forceOverwrite: true,
        }
      );

      if (resp.etag) {
        etag.value = resp.etag;
      }
      savedContent.value = currentText;
      isDirty.value = false;
      remoteConflictDetected.value = false;
      isConflictModalOpen.value = false;

      scheduleSessionPersist();
      await workspaceStore.refreshAll();
      return true;
    } catch (err: any) {
      throw err;
    } finally {
      saving.value = false;
    }
  }

  /**
   * Restore session on boot / reload.
   * Reconciliation:
   * - If clean (not dirty): re-fetch fresh content from server.
   * - If dirty: keep local draft, fetch server ETag in background, and alert if remote modified.
   */
  async function restoreSession(): Promise<boolean> {
    const session = loadEditorSession();
    if (!session || !session.open || !session.path) return false;

    // Create synthetic FileEntry from session metadata
    const entry: FileEntry = {
      name: session.name,
      path: session.path,
      kind: 'file',
      size: session.draft?.length ?? 0,
      modified_at: new Date(session.savedAt).toISOString(),
      is_hidden: false,
    };

    activeFile.value = entry;
    connectionId.value = session.connectionId || 'local';
    etag.value = session.etag || '';
    cursorPosition.value = session.cursor || { row: 0, column: 0 };
    scrollTop.value = session.scrollTop || 0;
    if (session.languageMode) languageMode.value = session.languageMode;
    showMarkdownPreview.value = !!session.showMarkdownPreview;

    if (session.dirty && session.draft !== undefined) {
      content.value = session.draft;
      savedContent.value = ''; // Will be checked against server
      isDirty.value = true;
      isOpen.value = true;

      if (overlayStore.current?.type !== 'editor') {
        overlayStore.open({ type: 'editor', connectionId: connectionId.value, path: entry.path });
      }

      // Check server in background for changes
      try {
        const resp = await readFileApi(connectionId.value, entry.path);
        savedContent.value = resp.content;
        if (resp.etag && session.etag && resp.etag !== session.etag) {
          remoteConflictDetected.value = true;
        }
      } catch {
        // File may have been deleted or network temporarily unavailable
      }
      return true;
    } else {
      // Clean reload: fetch fresh content from server
      isLoading.value = true;
      isOpen.value = true;
      if (overlayStore.current?.type !== 'editor') {
        overlayStore.open({ type: 'editor', connectionId: connectionId.value, path: entry.path });
      }

      try {
        const resp = await readFileApi(connectionId.value, entry.path);
        content.value = resp.content;
        savedContent.value = resp.content;
        etag.value = resp.etag;
        isDirty.value = false;
        flushSession();
      } catch (err: any) {
        if (err.response?.status === 404) {
          closeEditor();
          return false;
        }
        // If server fetch fails, fallback to session draft if available
        if (session.draft !== undefined) {
          content.value = session.draft;
          savedContent.value = session.draft;
        }
      } finally {
        isLoading.value = false;
      }
      return true;
    }
  }

  async function reloadFromDisk(): Promise<boolean> {
    if (!activeFile.value) return false;
    isLoading.value = true;
    try {
      const resp = await readFileApi(connectionId.value, activeFile.value.path);
      content.value = resp.content;
      savedContent.value = resp.content;
      etag.value = resp.etag;
      isDirty.value = false;
      remoteConflictDetected.value = false;
      isConflictModalOpen.value = false;
      flushSession();
      return true;
    } catch (err) {
      throw err;
    } finally {
      isLoading.value = false;
    }
  }

  async function openByPath(connId: string = 'local', path: string): Promise<boolean> {
    isLoading.value = true;
    try {
      const resp = await readFileApi(connId, path);
      const filename = path.split('/').pop() || 'file';
      const entry: FileEntry = {
        name: filename,
        path,
        kind: 'file',
        size: resp.content.length,
        modified_at: new Date().toISOString(),
        is_hidden: filename.startsWith('.'),
      };
      openFile(entry, resp.content, resp.etag, connId);
      return true;
    } catch (err) {
      throw err;
    } finally {
      isLoading.value = false;
    }
  }

  function requestClose() {
    if (isDirty.value) {
      isUnsavedConfirmOpen.value = true;
    } else {
      closeEditor();
    }
  }

  function discardAndClose() {
    if (sessionSaveTimer) {
      clearTimeout(sessionSaveTimer);
      sessionSaveTimer = null;
    }
    isDirty.value = false;
    isUnsavedConfirmOpen.value = false;
    closeEditor();
  }

  function closeEditor() {
    if (sessionSaveTimer) {
      clearTimeout(sessionSaveTimer);
      sessionSaveTimer = null;
    }
    isOpen.value = false;
    activeFile.value = null;
    content.value = '';
    savedContent.value = '';
    isDirty.value = false;
    remoteConflictDetected.value = false;
    isConflictModalOpen.value = false;
    isUnsavedConfirmOpen.value = false;
    clearEditorSession();

    if (overlayStore.current?.type === 'editor') {
      overlayStore.close();
    }
  }

  return {
    isOpen,
    activeFile,
    connectionId,
    content,
    savedContent,
    etag,
    isDirty,
    cursorPosition,
    scrollTop,
    languageMode,
    showMarkdownPreview,
    saving,
    isLoading,
    isConflictModalOpen,
    isUnsavedConfirmOpen,
    remoteConflictDetected,
    preferences,
    updatePreference,
    openFile,
    openByPath,
    updateContent,
    updateCursor,
    updateScroll,
    setLanguageMode,
    toggleMarkdownPreview,
    saveFile,
    forceSaveFile,
    restoreSession,
    reloadFromDisk,
    flushSession,
    requestClose,
    discardAndClose,
    closeEditor,
  };
});
