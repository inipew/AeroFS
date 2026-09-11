import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { FileEntry } from '../types/vfs';
import { getContentUrl, getDownloadUrl } from '../api/files';
import { useOverlayStore } from '../overlays/overlayStore';

export type MediaKind = 'image' | 'video' | 'audio' | 'other';

export const IMAGE_EXTENSIONS = new Set([
  'png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico', 'avif', 'tiff', 'heic'
]);

export const VIDEO_EXTENSIONS = new Set([
  'mp4', 'webm', 'mov', 'mkv', 'avi', 'ogg', 'm4v', 'flv'
]);

export const AUDIO_EXTENSIONS = new Set([
  'mp3', 'wav', 'flac', 'aac', 'm4a', 'opus', 'ogg', 'wma'
]);

export function detectMediaKind(entry?: FileEntry | null, title: string = ''): MediaKind {
  const mime = entry?.mime_type?.toLowerCase();
  if (mime) {
    if (mime.startsWith('image/')) return 'image';
    if (mime.startsWith('video/')) return 'video';
    if (mime.startsWith('audio/')) return 'audio';
  }

  const filename = entry?.name || title;
  const ext = filename.split('.').pop()?.toLowerCase() || '';

  if (IMAGE_EXTENSIONS.has(ext)) return 'image';
  if (VIDEO_EXTENSIONS.has(ext)) return 'video';
  if (AUDIO_EXTENSIONS.has(ext)) return 'audio';

  return 'other';
}

export function isMediaEntry(entry: FileEntry): boolean {
  if (entry.kind !== 'file') return false;
  return detectMediaKind(entry) !== 'other';
}

export const useMediaViewerStore = defineStore('mediaViewer', () => {
  const overlayStore = useOverlayStore();

  const isOpen = ref(false);
  const activeEntry = ref<FileEntry | null>(null);
  const activeUrl = ref<string>('');
  const activeTitle = ref<string>('');
  const connectionId = ref<string>('local');
  const playlist = ref<FileEntry[]>([]);

  // UI state
  const isInfoOpen = ref(false);
  const isFilmstripOpen = ref(false);
  const isMobileMenuOpen = ref(false);
  const playbackError = ref(false);

  // Dynamic media metadata
  const mediaDimensions = ref<{ width: number; height: number } | null>(null);
  const mediaDuration = ref<number | null>(null);

  // Shared & Persisted Audio/Video Volume & Mute Preferences
  const storedVol = typeof localStorage !== 'undefined' ? localStorage.getItem('aerofs:media:volume') : null;
  const volume = ref<number>(storedVol !== null ? Number(storedVol) : 1.0);
  const storedMute = typeof localStorage !== 'undefined' ? localStorage.getItem('aerofs:media:muted') : null;
  const isMuted = ref<boolean>(storedMute === 'true');

  function setVolume(vol: number) {
    volume.value = Math.max(0, Math.min(1, vol));
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('aerofs:media:volume', String(volume.value));
    }
    if (volume.value > 0 && isMuted.value) {
      setMuted(false);
    }
  }

  function setMuted(muted: boolean) {
    isMuted.value = muted;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('aerofs:media:muted', String(muted));
    }
  }

  function toggleMute() {
    setMuted(!isMuted.value);
  }

  const activeMediaKind = computed<MediaKind>(() => {
    return detectMediaKind(activeEntry.value, activeTitle.value);
  });

  const currentIndex = computed(() => {
    if (!activeEntry.value || playlist.value.length === 0) return 0;
    return playlist.value.findIndex((item) => item.path === activeEntry.value?.path);
  });

  const hasMultiple = computed(() => playlist.value.length > 1);

  const downloadUrl = computed(() => {
    if (activeEntry.value && connectionId.value) {
      return getDownloadUrl(connectionId.value, activeEntry.value.path);
    }
    return activeUrl.value;
  });

  function resetDynamicMetadata() {
    mediaDimensions.value = null;
    mediaDuration.value = null;
    playbackError.value = false;
  }

  function openMedia(
    title: string,
    url: string,
    currentFile: FileEntry | null = null,
    list: FileEntry[] = [],
    connId: string = 'local'
  ) {
    activeTitle.value = title;
    activeUrl.value = url;
    activeEntry.value = currentFile;
    connectionId.value = connId;

    const mediaFiles = list.filter((item) => isMediaEntry(item));
    playlist.value = mediaFiles.length > 0 ? mediaFiles : (currentFile ? [currentFile] : []);

    resetDynamicMetadata();
    isOpen.value = true;

    // Synchronize with overlayStore (Section 6 Invariant)
    overlayStore.open({
      type: 'media-viewer',
      connectionId: connId,
      path: currentFile?.path || '',
    });
  }

  function close() {
    isOpen.value = false;
    isInfoOpen.value = false;
    isFilmstripOpen.value = false;
    isMobileMenuOpen.value = false;
    resetDynamicMetadata();

    // Clean up overlayStore current state
    if (overlayStore.current?.type === 'media-viewer') {
      overlayStore.close();
    }
  }

  function selectEntry(entry: FileEntry) {
    activeEntry.value = entry;
    activeTitle.value = entry.name;
    activeUrl.value = getContentUrl(connectionId.value, entry.path);
    resetDynamicMetadata();

    if (overlayStore.current?.type === 'media-viewer') {
      overlayStore.current.path = entry.path;
    }
  }

  function navigate(direction: 'prev' | 'next') {
    if (playlist.value.length <= 1 || !activeEntry.value) return;

    const idx = playlist.value.findIndex((item) => item.path === activeEntry.value?.path);
    if (idx === -1) return;

    let nextIdx = direction === 'next' ? idx + 1 : idx - 1;
    if (nextIdx >= playlist.value.length) nextIdx = 0;
    if (nextIdx < 0) nextIdx = playlist.value.length - 1;

    const nextItem = playlist.value[nextIdx];
    selectEntry(nextItem);
  }

  function setMediaDimensions(width: number, height: number) {
    if (width > 0 && height > 0) {
      mediaDimensions.value = { width, height };
    }
  }

  function setMediaDuration(duration: number) {
    if (duration > 0) {
      mediaDuration.value = duration;
    }
  }

  function toggleInfo(force?: boolean) {
    isInfoOpen.value = typeof force === 'boolean' ? force : !isInfoOpen.value;
  }

  function toggleFilmstrip(force?: boolean) {
    isFilmstripOpen.value = typeof force === 'boolean' ? force : !isFilmstripOpen.value;
  }

  function toggleMobileMenu(force?: boolean) {
    isMobileMenuOpen.value = typeof force === 'boolean' ? force : !isMobileMenuOpen.value;
  }

  function setPlaybackError(err: boolean) {
    playbackError.value = err;
  }

  return {
    isOpen,
    activeEntry,
    activeUrl,
    activeTitle,
    connectionId,
    playlist,
    isInfoOpen,
    isFilmstripOpen,
    isMobileMenuOpen,
    playbackError,
    mediaDimensions,
    mediaDuration,
    volume,
    isMuted,
    setVolume,
    setMuted,
    toggleMute,
    activeMediaKind,
    currentIndex,
    hasMultiple,
    downloadUrl,
    openMedia,
    close,
    selectEntry,
    navigate,
    setMediaDimensions,
    setMediaDuration,
    toggleInfo,
    toggleFilmstrip,
    toggleMobileMenu,
    setPlaybackError,
  };
});
