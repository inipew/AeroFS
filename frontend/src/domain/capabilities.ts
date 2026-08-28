import type { FileEntry } from '../types/vfs';

export interface FileCapabilities {
  canRead: boolean;
  canWrite: boolean;
  canCreate: boolean;
  canRename: boolean;
  canDelete: boolean;
  canMove: boolean;
  canCopy: boolean;
  canUpload: boolean;
  canDownload: boolean;
  canShare: boolean;
  canArchive: boolean;
  canExtract: boolean;
  canChmod: boolean;
}

export function isArchiveFile(name: string): boolean {
  const lower = name.toLowerCase();
  return lower.endsWith('.zip') ||
    lower.endsWith('.tar.gz') ||
    lower.endsWith('.tgz') ||
    lower.endsWith('.tar') ||
    lower.endsWith('.tar.bz2') ||
    lower.endsWith('.tar.xz');
}

export function isEditableFile(name: string): boolean {
  const lower = name.toLowerCase();
  const editableExtensions = [
    '.txt', '.md', '.json', '.yaml', '.yml', '.toml', '.xml', '.html', '.htm',
    '.css', '.scss', '.sass', '.less', '.js', '.ts', '.jsx', '.tsx', '.vue',
    '.rs', '.py', '.go', '.c', '.cpp', '.h', '.hpp', '.java', '.kt', '.php',
    '.sh', '.bash', '.zsh', '.env', '.gitignore', '.dockerfile', '.conf', '.ini'
  ];
  return editableExtensions.some(ext => lower.endsWith(ext)) || !name.includes('.');
}

export function isMediaFile(name: string): boolean {
  const lower = name.toLowerCase();
  const mediaExtensions = [
    '.jpg', '.jpeg', '.png', '.gif', '.svg', '.webp', '.bmp', '.ico',
    '.mp4', '.webm', '.ogg', '.mp3', '.wav', '.flac', '.m4a', '.pdf'
  ];
  return mediaExtensions.some(ext => lower.endsWith(ext));
}

export function getEntryCapabilities(
  entry: FileEntry | null,
  contextCanWrite: boolean = true,
  isRemote: boolean = false
): FileCapabilities {
  if (!entry) {
    return {
      canRead: true,
      canWrite: contextCanWrite,
      canCreate: contextCanWrite,
      canRename: false,
      canDelete: false,
      canMove: false,
      canCopy: false,
      canUpload: contextCanWrite,
      canDownload: false,
      canShare: false,
      canArchive: false,
      canExtract: false,
      canChmod: false,
    };
  }

  const isDir = entry.kind === 'directory';
  const isArchive = isArchiveFile(entry.name);

  return {
    canRead: true,
    canWrite: contextCanWrite,
    canCreate: contextCanWrite && isDir,
    canRename: contextCanWrite,
    canDelete: contextCanWrite,
    canMove: contextCanWrite,
    canCopy: true,
    canUpload: contextCanWrite && isDir,
    canDownload: !isDir,
    canShare: true,
    canArchive: true,
    canExtract: isArchive && contextCanWrite,
    canChmod: !isRemote && contextCanWrite,
  };
}
