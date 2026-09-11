import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { FileEntry } from '../types/vfs';
import { useEditorStore } from './editorStore';
import { useMediaViewerStore } from './mediaViewerStore';

export interface ToastMessage {
  id: string;
  type: 'success' | 'error' | 'info' | 'warning';
  message: string;
}

export const useUiStore = defineStore('ui', () => {
  const isCreateOpen = ref<boolean>(false);
  const createType = ref<'file' | 'directory'>('file');

  const isRenameOpen = ref<boolean>(false);
  const renameTarget = ref<FileEntry | null>(null);

  const isDeleteOpen = ref<boolean>(false);
  const deleteTargets = ref<string[]>([]);
  const deletePermanent = ref<boolean>(false);

  const isUploadOpen = ref<boolean>(false);
  const isSearchOpen = ref<boolean>(false);
  const isSyncOpen = ref<boolean>(false);
  const syncSourceConnection = ref<string>('local');
  const syncSourcePath = ref<string>('/');
  const syncDestConnection = ref<string>('local');
  const syncDestPath = ref<string>('/');

  // Code Editor (Delegated to editorStore)
  const editorStore = useEditorStore();
  const isEditorOpen = computed<boolean>({
    get: () => editorStore.isOpen,
    set: (val) => { editorStore.isOpen = val; },
  });
  const editorFile = computed<FileEntry | null>({
    get: () => editorStore.activeFile,
    set: (val) => { editorStore.activeFile = val; },
  });
  const editorContent = computed<string>({
    get: () => editorStore.content,
    set: (val) => { editorStore.content = val; },
  });
  const editorEtag = computed<string>({
    get: () => editorStore.etag,
    set: (val) => { editorStore.etag = val; },
  });
  const editorConnectionId = computed<string>({
    get: () => editorStore.connectionId,
    set: (val) => { editorStore.connectionId = val; },
  });

  // Media Viewer & Player (Delegated to mediaViewerStore)
  const mediaViewerStore = useMediaViewerStore();
  const isMediaViewerOpen = computed<boolean>({
    get: () => mediaViewerStore.isOpen,
    set: (val) => { mediaViewerStore.isOpen = val; },
  });
  const mediaViewerUrl = computed<string>({
    get: () => mediaViewerStore.activeUrl,
    set: (val) => { mediaViewerStore.activeUrl = val; },
  });
  const mediaViewerTitle = computed<string>({
    get: () => mediaViewerStore.activeTitle,
    set: (val) => { mediaViewerStore.activeTitle = val; },
  });
  const mediaViewerFile = computed<FileEntry | null>({
    get: () => mediaViewerStore.activeEntry,
    set: (val) => { mediaViewerStore.activeEntry = val; },
  });
  const mediaViewerList = computed<FileEntry[]>({
    get: () => mediaViewerStore.playlist,
    set: (val) => { mediaViewerStore.playlist = val; },
  });
  const mediaViewerConnectionId = computed<string>({
    get: () => mediaViewerStore.connectionId,
    set: (val) => { mediaViewerStore.connectionId = val; },
  });

  const contextMenu = ref<{
    visible: boolean;
    x: number;
    y: number;
    item: FileEntry | null;
    connectionId: string;
    panelId: 'left' | 'right';
  }>({
    visible: false,
    x: 0,
    y: 0,
    item: null,
    connectionId: 'local',
    panelId: 'left',
  });

  const toasts = ref<ToastMessage[]>([]);
  const maxEditableSize = ref<number>(
    (typeof localStorage !== 'undefined' && Number(localStorage.getItem('fb:limits:max_editable_size'))) || 10 * 1024 * 1024
  );

  function setMaxEditableSize(bytes: number) {
    if (bytes > 0) {
      maxEditableSize.value = bytes;
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('fb:limits:max_editable_size', String(bytes));
      }
    }
  }

  function showToast(message: string, type: 'success' | 'error' | 'info' | 'warning' = 'info') {
    const id = Math.random().toString(36).substring(2, 9);
    toasts.value.push({ id, message, type });
    setTimeout(() => {
      toasts.value = toasts.value.filter((t) => t.id !== id);
    }, 4000);
  }

  function openCreate(type: 'file' | 'directory') {
    createType.value = type;
    isCreateOpen.value = true;
  }

  function openRename(entry: FileEntry) {
    renameTarget.value = entry;
    isRenameOpen.value = true;
  }

  function openDelete(paths: string[], permanent: boolean = false) {
    deleteTargets.value = paths;
    deletePermanent.value = permanent;
    isDeleteOpen.value = true;
  }

  function openUpload() {
    isUploadOpen.value = true;
  }

  function openSync(
    sourceConn: string = 'local',
    sourcePath: string = '/',
    destConn: string = 'local',
    destPath: string = '/'
  ) {
    syncSourceConnection.value = sourceConn;
    syncSourcePath.value = sourcePath;
    syncDestConnection.value = destConn;
    syncDestPath.value = destPath;
    isSyncOpen.value = true;
  }

  function openEditor(
    entry: FileEntry,
    content: string,
    etag: string = '',
    connectionId: string = 'local'
  ) {
    editorStore.openFile(entry, content, etag, connectionId);
  }

  function openMediaViewer(
    title: string,
    url: string,
    currentFile: FileEntry | null = null,
    list: FileEntry[] = [],
    connectionId: string = 'local'
  ) {
    mediaViewerStore.openMedia(title, url, currentFile, list, connectionId);
  }

  function navigateMedia(direction: 'next' | 'prev') {
    mediaViewerStore.navigate(direction);
  }

  function openContextMenu(
    e: MouseEvent,
    item: FileEntry | null = null,
    connectionId: string = 'local',
    panelId: 'left' | 'right' = 'left'
  ) {
    e.preventDefault();
    contextMenu.value = {
      visible: true,
      x: e.clientX,
      y: e.clientY,
      item,
      connectionId,
      panelId,
    };
  }

  function checkIsMobile(): boolean {
    if (typeof window === 'undefined') return false;
    return window.matchMedia('(max-width: 767px)').matches || window.innerWidth < 768;
  }

  const isMobile = ref<boolean>(checkIsMobile());

  if (typeof window !== 'undefined') {
    const updateMobile = () => {
      const newVal = checkIsMobile();
      if (isMobile.value !== newVal) {
        isMobile.value = newVal;
      }
    };
    window.addEventListener('resize', updateMobile, { passive: true });
    window.addEventListener('orientationchange', updateMobile, { passive: true });
    try {
      const mql = window.matchMedia('(max-width: 767px)');
      if (mql.addEventListener) {
        mql.addEventListener('change', updateMobile);
      } else if ((mql as any).addListener) {
        (mql as any).addListener(updateMobile);
      }
    } catch {}
  }
  const isMobileSidebarOpen = ref<boolean>(false);
  const isCommandPaletteOpen = ref<boolean>(false);
  const listDensity = ref<'comfortable' | 'compact' | 'dense'>(
    (typeof localStorage !== 'undefined' && (localStorage.getItem('fb:ui:density') as any)) || 'compact'
  );

  function setListDensity(density: 'comfortable' | 'compact' | 'dense') {
    listDensity.value = density;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('fb:ui:density', density);
    }
  }

  function toggleCommandPalette() {
    isCommandPaletteOpen.value = !isCommandPaletteOpen.value;
  }

  function openContextMenuForTouch(
    item: FileEntry | null = null,
    connectionId: string = 'local',
    panelId: 'left' | 'right' = 'left'
  ) {
    contextMenu.value = {
      visible: true,
      x: typeof window !== 'undefined' ? window.innerWidth / 2 : 0,
      y: typeof window !== 'undefined' ? window.innerHeight / 2 : 0,
      item,
      connectionId,
      panelId,
    };
  }

  function closeContextMenu() {
    contextMenu.value.visible = false;
  }

  return {
    isMobile,
    isMobileSidebarOpen,
    isCommandPaletteOpen,
    listDensity,
    setListDensity,
    toggleCommandPalette,
    isCreateOpen,
    createType,
    isRenameOpen,
    renameTarget,
    isDeleteOpen,
    deleteTargets,
    deletePermanent,
    isUploadOpen,
    isSearchOpen,
    isSyncOpen,
    syncSourceConnection,
    syncSourcePath,
    syncDestConnection,
    syncDestPath,
    openSync,
    isEditorOpen,
    editorFile,
    editorContent,
    editorEtag,
    editorConnectionId,
    isMediaViewerOpen,
    mediaViewerUrl,
    mediaViewerTitle,
    mediaViewerFile,
    mediaViewerList,
    mediaViewerConnectionId,
    maxEditableSize,
    setMaxEditableSize,
    contextMenu,
    toasts,
    showToast,
    openCreate,
    openRename,
    openDelete,
    openUpload,
    openEditor,
    openMediaViewer,
    navigateMedia,
    openContextMenu,
    openContextMenuForTouch,
    closeContextMenu,
  };
});
