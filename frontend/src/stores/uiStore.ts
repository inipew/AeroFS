import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { FileEntry } from '../types/vfs';
import { getDownloadUrl } from '../api/files';

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

  const isUploadOpen = ref<boolean>(false);
  const isSearchOpen = ref<boolean>(false);
  const isSyncOpen = ref<boolean>(false);
  const syncSourceConnection = ref<string>('local');
  const syncSourcePath = ref<string>('/');
  const syncDestConnection = ref<string>('local');
  const syncDestPath = ref<string>('/');

  // Code Editor
  const isEditorOpen = ref<boolean>(false);
  const editorFile = ref<FileEntry | null>(null);
  const editorContent = ref<string>('');
  const editorEtag = ref<string>('');
  const editorConnectionId = ref<string>('local');

  // Media Viewer & Player
  const isMediaViewerOpen = ref<boolean>(false);
  const mediaViewerUrl = ref<string>('');
  const mediaViewerTitle = ref<string>('');
  const mediaViewerFile = ref<FileEntry | null>(null);
  const mediaViewerList = ref<FileEntry[]>([]);
  const mediaViewerConnectionId = ref<string>('local');

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

  function openDelete(paths: string[]) {
    deleteTargets.value = paths;
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
    editorFile.value = entry;
    editorContent.value = content;
    editorEtag.value = etag;
    editorConnectionId.value = connectionId;
    isEditorOpen.value = true;
  }

  function isMediaEntry(entry: FileEntry): boolean {
    const ext = entry.name.split('.').pop()?.toLowerCase() || '';
    return [
      'png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico', 'avif',
      'mp4', 'webm', 'mov', 'mkv', 'avi', 'ogg',
      'mp3', 'wav', 'flac', 'aac', 'm4a', 'opus'
    ].includes(ext);
  }

  function openMediaViewer(
    title: string,
    url: string,
    currentFile: FileEntry | null = null,
    list: FileEntry[] = [],
    connectionId: string = 'local'
  ) {
    mediaViewerTitle.value = title;
    mediaViewerUrl.value = url;
    mediaViewerFile.value = currentFile;
    mediaViewerConnectionId.value = connectionId;

    const mediaFiles = list.filter((item) => item.kind === 'file' && isMediaEntry(item));
    mediaViewerList.value = mediaFiles.length > 0 ? mediaFiles : (currentFile ? [currentFile] : []);
    isMediaViewerOpen.value = true;
  }

  function navigateMedia(direction: 'next' | 'prev') {
    if (mediaViewerList.value.length <= 1 || !mediaViewerFile.value) return;

    const currentIndex = mediaViewerList.value.findIndex(
      (item) => item.path === mediaViewerFile.value?.path
    );
    if (currentIndex === -1) return;

    let nextIndex = direction === 'next' ? currentIndex + 1 : currentIndex - 1;
    if (nextIndex >= mediaViewerList.value.length) nextIndex = 0;
    if (nextIndex < 0) nextIndex = mediaViewerList.value.length - 1;

    const nextItem = mediaViewerList.value[nextIndex];
    mediaViewerFile.value = nextItem;
    mediaViewerTitle.value = nextItem.name;
    mediaViewerUrl.value = getDownloadUrl(mediaViewerConnectionId.value, nextItem.path);
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
