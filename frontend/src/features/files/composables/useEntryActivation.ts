import { type Ref } from 'vue';
import type { FileEntry } from '../../../types/vfs';
import { PreviewResolver } from '../../../services/previewResolver';
import { useOverlayStore } from '../../../overlays/overlayStore';

export interface UseEntryActivationOptions {
  connectionId: Ref<string>;
  displayedFiles: Ref<FileEntry[]>;
  onNavigate: (path: string) => Promise<void> | void;
}

export function useEntryActivation(options: UseEntryActivationOptions) {
  const overlayStore = useOverlayStore();
  let lastOpenTime = 0;
  let lastOpenPath = '';

  async function activateEntry(entry: FileEntry) {
    const now = Date.now();
    if (now - lastOpenTime < 350 && lastOpenPath === entry.path) {
      return;
    }
    lastOpenTime = now;
    lastOpenPath = entry.path;

    if (entry.kind === 'directory') {
      await options.onNavigate(entry.path);
      return;
    }

    const resolution = PreviewResolver.resolve(
      entry as any,
      options.connectionId.value,
      options.displayedFiles.value as any,
      (payload) => {
        overlayStore.open({
          type: 'archive-viewer',
          connectionId: payload.connectionId,
          archivePath: payload.path,
        });
      }
    );

    await resolution.open();
  }

  return {
    activateEntry,
  };
}
