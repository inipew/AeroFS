/**
 * AeroFS Code Editor Session & Preferences Persistence Service
 * Handles localStorage serialization, legacy prefix migration (fb:editor -> aerofs:editor),
 * and session state snapshots for crash and browser reload recovery.
 */

export const EDITOR_SESSION_STORAGE_KEY = 'aerofs:editor:session';

export interface EditorCursorPosition {
  row: number;
  column: number;
}

export interface PersistedEditorSession {
  version: 1;
  open: boolean;
  connectionId: string;
  path: string;
  name: string;
  etag: string;
  draft?: string;
  dirty: boolean;
  cursor?: EditorCursorPosition;
  scrollTop?: number;
  languageMode?: string;
  showMarkdownPreview?: boolean;
  savedAt: number;
}

export interface EditorPreferences {
  theme: string;
  fontFamily: string;
  fontSize: number;
  tabSize: number;
  wordWrap: boolean;
  showGutter: boolean;
  highlightActiveLine: boolean;
}

const PREFERENCE_KEY_MAP: Record<keyof EditorPreferences, { modern: string; legacy: string }> = {
  theme: { modern: 'aerofs:editor:theme', legacy: 'fb:editor:theme' },
  fontFamily: { modern: 'aerofs:editor:fontFamily', legacy: 'fb:editor:fontFamily' },
  fontSize: { modern: 'aerofs:editor:fontSize', legacy: 'fb:editor:fontSize' },
  tabSize: { modern: 'aerofs:editor:tabSize', legacy: 'fb:editor:tabSize' },
  wordWrap: { modern: 'aerofs:editor:wordWrap', legacy: 'fb:editor:wordWrap' },
  showGutter: { modern: 'aerofs:editor:showGutter', legacy: 'fb:editor:showGutter' },
  highlightActiveLine: { modern: 'aerofs:editor:highlightActiveLine', legacy: 'fb:editor:highlightActiveLine' },
};

/**
 * Load user editor preferences with transparent fallback from legacy fb:editor keys.
 */
export function loadEditorPreferences(isDark: boolean): EditorPreferences {
  const getRaw = (key: keyof EditorPreferences): string | null => {
    try {
      const modernVal = localStorage.getItem(PREFERENCE_KEY_MAP[key].modern);
      if (modernVal !== null) return modernVal;
      return localStorage.getItem(PREFERENCE_KEY_MAP[key].legacy);
    } catch {
      return null;
    }
  };

  const themeRaw = getRaw('theme');
  const defaultTheme = isDark ? 'ace/theme/tomorrow_night' : 'ace/theme/chrome';
  const theme = themeRaw || defaultTheme;

  const fontFamily = getRaw('fontFamily') || "'JetBrains Mono', 'Fira Code', monospace";
  const fontSize = Number(getRaw('fontSize')) || 13;
  const tabSize = Number(getRaw('tabSize')) || 2;
  const wordWrap = getRaw('wordWrap') !== 'false';
  const showGutter = getRaw('showGutter') !== 'false';
  const highlightActiveLine = getRaw('highlightActiveLine') !== 'false';

  return {
    theme,
    fontFamily,
    fontSize,
    tabSize,
    wordWrap,
    showGutter,
    highlightActiveLine,
  };
}

/**
 * Persist an updated preference to modern aerofs:editor:* localStorage key.
 */
export function saveEditorPreference<K extends keyof EditorPreferences>(key: K, value: EditorPreferences[K]): void {
  try {
    const config = PREFERENCE_KEY_MAP[key];
    localStorage.setItem(config.modern, String(value));
  } catch {
    // Ignore storage quota or disabled storage errors
  }
}

/**
 * Save active session snapshot.
 * Invariant: Only persists `draft` when dirty === true to prevent localStorage QuotaExceededError.
 */
export function saveEditorSession(session: PersistedEditorSession): void {
  try {
    const payload: PersistedEditorSession = {
      ...session,
      draft: session.dirty ? session.draft : undefined,
    };
    localStorage.setItem(EDITOR_SESSION_STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // Ignore storage quota errors
  }
}

/**
 * Synchronously flush editor session now (e.g. on beforeunload).
 */
export function flushEditorSessionNow(session: PersistedEditorSession): void {
  saveEditorSession(session);
}

/**
 * Load persisted session, validating schema.
 */
export function loadEditorSession(): PersistedEditorSession | null {
  try {
    const raw = localStorage.getItem(EDITOR_SESSION_STORAGE_KEY);
    if (!raw) return null;

    const parsed = JSON.parse(raw);
    if (parsed && parsed.version === 1 && typeof parsed.path === 'string' && parsed.open) {
      return parsed as PersistedEditorSession;
    }
    return null;
  } catch {
    clearEditorSession();
    return null;
  }
}

/**
 * Remove persisted session (e.g. when file is saved and cleanly closed).
 */
export function clearEditorSession(): void {
  try {
    localStorage.removeItem(EDITOR_SESSION_STORAGE_KEY);
  } catch {
    // Ignore
  }
}
