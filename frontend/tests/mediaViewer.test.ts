import { describe, expect, it, beforeEach } from 'bun:test';
import { setActivePinia, createPinia } from 'pinia';
import {
  useMediaViewerStore,
  detectMediaKind,
  isMediaEntry,
} from '../src/stores/mediaViewerStore';
import { useImageTransform } from '../src/composables/useImageTransform';
import type { FileEntry } from '../src/types/vfs';

describe('Media Viewer Redesign & Invariants', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  describe('detectMediaKind and isMediaEntry', () => {
    it('detects image extensions and MIME types', () => {
      const entry1: FileEntry = { name: 'photo.jpg', path: '/photo.jpg', kind: 'file', is_hidden: false };
      expect(detectMediaKind(entry1)).toBe('image');
      expect(isMediaEntry(entry1)).toBe(true);

      const entry2: FileEntry = {
        name: 'custom_file',
        path: '/custom_file',
        kind: 'file',
        mime_type: 'image/webp',
        is_hidden: false,
      };
      expect(detectMediaKind(entry2)).toBe('image');
      expect(isMediaEntry(entry2)).toBe(true);
    });

    it('detects video extensions and MIME types', () => {
      const entry: FileEntry = { name: 'movie.mp4', path: '/movie.mp4', kind: 'file', is_hidden: false };
      expect(detectMediaKind(entry)).toBe('video');
      expect(isMediaEntry(entry)).toBe(true);
    });

    it('detects audio extensions and MIME types', () => {
      const entry: FileEntry = { name: 'song.flac', path: '/song.flac', kind: 'file', is_hidden: false };
      expect(detectMediaKind(entry)).toBe('audio');
      expect(isMediaEntry(entry)).toBe(true);
    });

    it('returns other for non-media files or directories', () => {
      const doc: FileEntry = { name: 'doc.pdf', path: '/doc.pdf', kind: 'file', is_hidden: false };
      expect(detectMediaKind(doc)).toBe('other');
      expect(isMediaEntry(doc)).toBe(false);

      const dir: FileEntry = { name: 'photos', path: '/photos', kind: 'directory', is_hidden: false };
      expect(isMediaEntry(dir)).toBe(false);
    });
  });

  describe('mediaViewerStore state & navigation', () => {
    const mockFiles: FileEntry[] = [
      { name: 'img1.png', path: '/img1.png', kind: 'file', size: 1000, is_hidden: false },
      { name: 'notes.txt', path: '/notes.txt', kind: 'file', size: 500, is_hidden: false },
      { name: 'video1.mp4', path: '/video1.mp4', kind: 'file', size: 5000, is_hidden: false },
      { name: 'audio1.mp3', path: '/audio1.mp3', kind: 'file', size: 2000, is_hidden: false },
    ];

    it('opens media and filters playlist to only valid media files', () => {
      const store = useMediaViewerStore();
      store.openMedia('img1.png', 'http://localhost/files/img1.png', mockFiles[0], mockFiles, 'conn-1');

      expect(store.isOpen).toBe(true);
      expect(store.activeTitle).toBe('img1.png');
      expect(store.activeMediaKind).toBe('image');
      expect(store.connectionId).toBe('conn-1');

      // Only img1.png, video1.mp4, audio1.mp3 should be in playlist (notes.txt filtered out)
      expect(store.playlist).toHaveLength(3);
      expect(store.currentIndex).toBe(0);
      expect(store.hasMultiple).toBe(true);
    });

    it('navigates through playlist in circular fashion', () => {
      const store = useMediaViewerStore();
      store.openMedia('img1.png', 'http://localhost/files/img1.png', mockFiles[0], mockFiles, 'conn-1');

      // Next -> video1.mp4
      store.navigate('next');
      expect(store.activeTitle).toBe('video1.mp4');
      expect(store.activeMediaKind).toBe('video');
      expect(store.currentIndex).toBe(1);

      // Next -> audio1.mp3
      store.navigate('next');
      expect(store.activeTitle).toBe('audio1.mp3');
      expect(store.activeMediaKind).toBe('audio');
      expect(store.currentIndex).toBe(2);

      // Next -> wraps back to img1.png
      store.navigate('next');
      expect(store.activeTitle).toBe('img1.png');
      expect(store.currentIndex).toBe(0);

      // Prev -> wraps to audio1.mp3
      store.navigate('prev');
      expect(store.activeTitle).toBe('audio1.mp3');
      expect(store.currentIndex).toBe(2);
    });

    it('allows direct selection of an entry from filmstrip', () => {
      const store = useMediaViewerStore();
      store.openMedia('img1.png', 'http://localhost/files/img1.png', mockFiles[0], mockFiles, 'conn-1');

      store.selectEntry(mockFiles[2]); // video1.mp4
      expect(store.activeTitle).toBe('video1.mp4');
      expect(store.activeMediaKind).toBe('video');
    });

    it('toggles info drawer and filmstrip state', () => {
      const store = useMediaViewerStore();
      expect(store.isInfoOpen).toBe(false);
      store.toggleInfo();
      expect(store.isInfoOpen).toBe(true);
      store.toggleInfo(false);
      expect(store.isInfoOpen).toBe(false);

      expect(store.isFilmstripOpen).toBe(false);
      store.toggleFilmstrip(true);
      expect(store.isFilmstripOpen).toBe(true);
    });
  });

  describe('useImageTransform composable', () => {
    it('handles zoom level clamping and steps', () => {
      const transform = useImageTransform({ minZoom: 0.5, maxZoom: 3.0, zoomStep: 0.5 });
      expect(transform.zoomLevel.value).toBe(1);

      transform.zoomIn();
      expect(transform.zoomLevel.value).toBe(1.5);

      transform.zoomIn(2.0);
      // Clamped to maxZoom
      expect(transform.zoomLevel.value).toBe(3.0);

      transform.zoomOut(5.0);
      // Clamped to minZoom
      expect(transform.zoomLevel.value).toBe(0.5);

      transform.resetZoom();
      expect(transform.zoomLevel.value).toBe(1);
    });

    it('handles rotation modulo 360', () => {
      const transform = useImageTransform();
      expect(transform.rotation.value).toBe(0);

      transform.rotateClockwise();
      expect(transform.rotation.value).toBe(90);

      transform.rotateClockwise();
      expect(transform.rotation.value).toBe(180);

      transform.rotateClockwise();
      expect(transform.rotation.value).toBe(270);

      transform.rotateClockwise();
      expect(transform.rotation.value).toBe(0);

      transform.rotateCounterClockwise();
      expect(transform.rotation.value).toBe(270);
    });

    it('resets transform completely', () => {
      const transform = useImageTransform();
      transform.setZoom(2.5);
      transform.rotateClockwise();
      transform.panX.value = 100;
      transform.panY.value = 50;

      transform.resetTransform();
      expect(transform.zoomLevel.value).toBe(1);
      expect(transform.rotation.value).toBe(0);
      expect(transform.panX.value).toBe(0);
      expect(transform.panY.value).toBe(0);
      expect(transform.isPanning.value).toBe(false);
    });

    it('generates screen-space CSS transform with translate3d preceding scale and rotate', () => {
      const transform = useImageTransform();
      transform.setZoom(2.0);
      transform.rotateClockwise(); // 90deg
      transform.panX.value = 50;
      transform.panY.value = -30;

      const style = transform.transformStyle.value;
      // Invariant: translate3d must come first in string
      expect(style.transform).toBe('translate3d(50px, -30px, 0px) scale(2) rotate(90deg)');
    });
  });

  describe('Audio/Video Volume & Mute Persistence', () => {
    it('persists volume changes to localStorage and restores them', () => {
      const store = useMediaViewerStore();
      store.setVolume(0.45);
      expect(store.volume).toBe(0.45);
      expect(localStorage.getItem('aerofs:media:volume')).toBe('0.45');

      store.setMuted(true);
      expect(store.isMuted).toBe(true);
      expect(localStorage.getItem('aerofs:media:muted')).toBe('true');

      store.toggleMute();
      expect(store.isMuted).toBe(false);
      expect(localStorage.getItem('aerofs:media:muted')).toBe('false');
    });
  });

  describe('OverlayStore Synchronization Invariant', () => {
    it('synchronizes opening and closing with overlayStore', () => {
      const store = useMediaViewerStore();
      const mockFile: FileEntry = { name: 'photo.jpg', path: '/gallery/photo.jpg', kind: 'file', is_hidden: false };

      store.openMedia('photo.jpg', 'http://localhost/photo.jpg', mockFile, [mockFile], 'local');
      expect(store.isOpen).toBe(true);

      store.close();
      expect(store.isOpen).toBe(false);
    });
  });
});

