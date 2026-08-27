import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { FileEntry } from '../types/vfs';
import { getDownloadUrl } from '../api/files';

export interface ToastMessage {
  id: string;
  type: 'success' | 'error' | 'info';
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

  // Code Editor
  const isEditorOpen = ref<boolean>(false);
  const editorFile = ref<FileEntry | null>(null);
  const editorContent = ref<string>('');
  const editorEtag = ref<string>('');

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
  }>({
    visible: false,
    x: 0,
    y: 0,
    item: null,
  });

  const toasts = ref<ToastMessage[]>([]);

  function showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
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

  function openEditor(entry: FileEntry, content: string, etag: string = '') {
    editorFile.value = entry;
    editorContent.value = content;
    editorEtag.value = etag;
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

  function openContextMenu(e: MouseEvent, item: FileEntry | null = null) {
    e.preventDefault();
    contextMenu.value = {
      visible: true,
      x: e.clientX,
      y: e.clientY,
      item,
    };
  }

  function closeContextMenu() {
    contextMenu.value.visible = false;
  }

  return {
    isCreateOpen,
    createType,
    isRenameOpen,
    renameTarget,
    isDeleteOpen,
    deleteTargets,
    isUploadOpen,
    isEditorOpen,
    editorFile,
    editorContent,
    editorEtag,
    isMediaViewerOpen,
    mediaViewerUrl,
    mediaViewerTitle,
    mediaViewerFile,
    mediaViewerList,
    mediaViewerConnectionId,
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
    closeContextMenu,
  };
});
