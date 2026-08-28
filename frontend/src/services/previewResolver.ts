import type { FileEntry } from '../types/vfs';
import { useUiStore } from '../stores/uiStore';
import { readFileApi, getDownloadUrl } from '../api/files';
import { isArchiveFile } from '../utils/archive';

export type PreviewKind =
  | 'image'
  | 'video'
  | 'audio'
  | 'pdf'
  | 'archive'
  | 'code'
  | 'markdown'
  | 'json'
  | 'binary';

export interface PreviewResolution {
  kind: PreviewKind;
  canPreview: boolean;
  open: () => Promise<void> | void;
}

export class PreviewResolver {
  private static readonly IMAGE_EXTS = new Set([
    'png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico', 'avif', 'tiff', 'heic'
  ]);

  private static readonly VIDEO_EXTS = new Set([
    'mp4', 'webm', 'mov', 'mkv', 'avi', 'ogg', 'm4v', 'flv'
  ]);

  private static readonly AUDIO_EXTS = new Set([
    'mp3', 'wav', 'flac', 'aac', 'm4a', 'opus', 'ogg', 'wma'
  ]);

  private static readonly CODE_EXTS = new Set([
    'txt', 'md', 'json', 'toml', 'yaml', 'yml', 'xml', 'html', 'css', 'scss', 'less',
    'js', 'ts', 'jsx', 'tsx', 'vue', 'svelte', 'rs', 'py', 'go', 'c', 'cpp', 'h',
    'hpp', 'java', 'kt', 'rb', 'php', 'sh', 'bash', 'zsh', 'sql', 'dockerfile',
    'env', 'gitignore', 'conf', 'ini', 'log', 'lock', 'lua', 'dart', 'swift', 'scala'
  ]);

  public static getKind(entry: FileEntry): PreviewKind {
    if (entry.kind === 'directory') return 'binary';
    if (isArchiveFile(entry.name)) return 'archive';

    const ext = entry.name.split('.').pop()?.toLowerCase() || '';
    if (this.IMAGE_EXTS.has(ext)) return 'image';
    if (this.VIDEO_EXTS.has(ext)) return 'video';
    if (this.AUDIO_EXTS.has(ext)) return 'audio';
    if (ext === 'pdf') return 'pdf';
    if (ext === 'md') return 'markdown';
    if (ext === 'json') return 'json';
    if (this.CODE_EXTS.has(ext)) return 'code';

    return 'binary';
  }

  public static resolve(
    entry: FileEntry,
    connectionId: string,
    allEntries: FileEntry[] = [],
    onOpenArchive?: (payload: { connectionId: string; path: string }) => void
  ): PreviewResolution {
    const kind = this.getKind(entry);
    const uiStore = useUiStore();

    if (kind === 'archive') {
      return {
        kind: 'archive',
        canPreview: true,
        open: () => {
          if (onOpenArchive) {
            onOpenArchive({ connectionId, path: entry.path });
          } else {
            // Trigger archive modal through event
            window.dispatchEvent(
              new CustomEvent('open-archive-viewer', {
                detail: { connectionId, path: entry.path },
              })
            );
          }
        },
      };
    }

    if (kind === 'image' || kind === 'video' || kind === 'audio') {
      return {
        kind,
        canPreview: true,
        open: () => {
          const downloadUrl = getDownloadUrl(connectionId, entry.path);
          uiStore.openMediaViewer(
            entry.name,
            downloadUrl,
            entry,
            allEntries.filter((e) => e.kind === 'file'),
            connectionId
          );
        },
      };
    }

    if (kind === 'pdf') {
      return {
        kind: 'pdf',
        canPreview: true,
        open: () => {
          const downloadUrl = getDownloadUrl(connectionId, entry.path);
          window.open(downloadUrl, '_blank');
        },
      };
    }

    if (kind === 'code' || kind === 'markdown' || kind === 'json') {
      return {
        kind,
        canPreview: true,
        open: async () => {
          // Dynamic safety guard: Maximum editable file size guard (P1 #18)
          const size = entry.size || 0;
          const maxEditable = uiStore.maxEditableSize || 10 * 1024 * 1024;

          if (size > maxEditable) {
            uiStore.showToast(
              `File is too large to edit in browser (${(size / (1024 * 1024)).toFixed(1)} MB > ${(maxEditable / (1024 * 1024)).toFixed(1)} MB limit). Downloading directly...`,
              'warning'
            );
            const downloadUrl = getDownloadUrl(connectionId, entry.path);
            window.open(downloadUrl, '_blank');
            return;
          }

          if (size > 5 * 1024 * 1024) {
            const proceed = window.confirm(
              `This file is large (${(size / (1024 * 1024)).toFixed(1)} MB). Opening it in the browser code editor may cause temporary lag. Do you want to continue?`
            );
            if (!proceed) return;
          }

          try {
            const resp = await readFileApi(connectionId, entry.path);
            uiStore.openEditor(entry, resp.content, resp.etag, connectionId);
          } catch (err: any) {
            uiStore.showToast(err.response?.data?.error?.message || 'Failed to read file', 'error');
          }
        },
      };
    }

    // Binary / Unknown
    return {
      kind: 'binary',
      canPreview: false,
      open: () => {
        const downloadUrl = getDownloadUrl(connectionId, entry.path);
        window.open(downloadUrl, '_blank');
      },
    };
  }
}
