import type { PersistedWorkspace } from '../types/workspace';
import type { FileEntry } from '../types/vfs';

const WORKSPACE_KEY = 'fb:workspace_v1';
const DUAL_PANE_KEY = 'fb:isDualPane';
const PREFERENCES_KEY = 'fb:user_preferences';

export function loadWorkspaceState(): PersistedWorkspace | null {
  try {
    const raw = localStorage.getItem(WORKSPACE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed.version === 1) {
        return parsed as PersistedWorkspace;
      }
    }

    // Fallback for legacy keys
    const isDual = localStorage.getItem(DUAL_PANE_KEY) === 'true';
    const leftConn = localStorage.getItem('fb:left:connectionId') || 'local';
    const leftPath = localStorage.getItem('fb:left:path') || '/';
    const rightConn = localStorage.getItem('fb:right:connectionId') || 'local';
    const rightPath = localStorage.getItem('fb:right:path') || '/';

    return {
      version: 1,
      layout: isDual ? 'split' : 'single',
      activePanel: 'left',
      splitRatio: 0.5,
      left: {
        connectionId: leftConn,
        path: leftPath,
        viewMode: 'grid',
        showHidden: false,
        sortField: 'name',
        sortOrder: 'asc',
      },
      right: {
        connectionId: rightConn,
        path: rightPath,
        viewMode: 'grid',
        showHidden: false,
        sortField: 'name',
        sortOrder: 'asc',
      },
    };
  } catch {
    return null;
  }
}

export function saveWorkspaceState(persisted: PersistedWorkspace, isDualPane: boolean): void {
  try {
    localStorage.setItem(WORKSPACE_KEY, JSON.stringify(persisted));
    localStorage.setItem(DUAL_PANE_KEY, isDualPane ? 'true' : 'false');
  } catch {
    // Ignore storage write quota errors
  }
}

export function loadUserPreferences(): { rememberLastDirectory?: boolean } | null {
  try {
    const raw = localStorage.getItem(PREFERENCES_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

function snapshotKey(connId: string, path: string): string {
  return `fb:snapshot:${connId}:${path}`;
}

export function saveDirectorySnapshot(connId: string, path: string, entries: FileEntry[]): void {
  try {
    const key = snapshotKey(connId, path);
    const payload = JSON.stringify(entries.slice(0, 100));
    localStorage.setItem(key, payload);
  } catch {
    // Ignore quota errors
  }
}

export function loadDirectorySnapshot(connId: string, path: string): FileEntry[] | null {
  try {
    const raw = localStorage.getItem(snapshotKey(connId, path));
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}
