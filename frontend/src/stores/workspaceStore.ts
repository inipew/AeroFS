import { defineStore } from 'pinia';
import { ref, computed, reactive, watch } from 'vue';
import { queryClient } from '../queryClient';
import { queryKeys } from '../api/queryKeys';
import { directoryQueryOptions } from '../composables/useDirectoryQuery';
import { ensureDirectoryData, getCachedDirectoryEntries } from '../composables/usePanelDirectory';
import { realtimeSync } from '../services/realtimeSync';
import { useTransferStore } from './transferStore';
import { useUiStore } from './uiStore';
import { normalizePath, parentPath } from '../utils/path';
import { isAbortError, normalizeApiError } from '../utils/errorNormalizer';
import { PanelSession } from '../workspace/panelSession';
import { useConnectionStore } from './connectionStore';
import type { FileEntry } from '../types/vfs';
import type {
  PanelId,
  WorkspaceLayout,
  Panel,
  PanelLocation,
  NavigationState,
  PanelViewState,
  SelectionState,
  PanelRuntimeState,
  WorkspaceClipboard,
  PersistedWorkspace,
} from '../types/workspace';

export type { PanelState } from '../types/workspace';

function createPanel(id: PanelId, initialConnection: string = 'local', initialPath: string = '/'): Panel {
  const location = reactive<PanelLocation>({
    connectionId: initialConnection,
    path: initialPath,
  });

  const navigation = reactive<NavigationState>({
    history: [initialPath],
    historyIndex: 0,
  });

  const view = reactive<PanelViewState>({
    viewMode: 'grid',
    showHidden: false,
    sortField: 'name',
    sortOrder: 'asc',
    filterType: 'all',
    searchQuery: '',
  });

  const selection = reactive<SelectionState>({
    paths: [] as string[],
    focusedPath: undefined,
  });

  const runtime = reactive<PanelRuntimeState>({
    status: 'idle',
    get loading() { return this.status === 'loading' || this.status === 'refreshing'; },
    set loading(_v: boolean) {},
    get refreshing() { return this.status === 'refreshing'; },
    set refreshing(_v: boolean) {},
    get loadingMore() { return this.status === 'loading_more'; },
    set loadingMore(_v: boolean) {},
    error: null,
    initialized: false,
    hasMore: false,
    nextCursor: undefined,
    totalCount: undefined,
    get entries() {
      return getCachedDirectoryEntries(location.connectionId, location.path);
    },
    set entries(_v: FileEntry[] | undefined) {},
  });

  const panel: Panel = {
    id,
    location,
    navigation,
    view,
    selection,
    runtime,

    get connectionId() { return location.connectionId; },
    set connectionId(val: string) { location.connectionId = val; },

    get path() { return location.path; },
    set path(val: string) { location.path = val; },

    get entries(): FileEntry[] {
      return getCachedDirectoryEntries(location.connectionId, location.path);
    },
    set entries(_val: FileEntry[]) {
      // Invariant: TanStack Query cache is single source of truth
    },

    get selectedEntries(): string[] { return selection.paths; },
    set selectedEntries(val: string[]) { selection.paths = val; },

    get viewMode() { return view.viewMode; },
    set viewMode(val: 'grid' | 'list') { view.viewMode = val; },

    get showHidden() { return view.showHidden; },
    set showHidden(val: boolean) { view.showHidden = val; },

    get sortField() { return view.sortField; },
    set sortField(val: string) { view.sortField = val as any; },

    get sortOrder() { return view.sortOrder; },
    set sortOrder(val: 'asc' | 'desc') { view.sortOrder = val; },

    get filterType() { return view.filterType; },
    set filterType(val: string) { view.filterType = val; },

    get searchQuery() { return view.searchQuery; },
    set searchQuery(val: string) { view.searchQuery = val; },

    get loading() { return runtime.status === 'loading' || runtime.status === 'refreshing'; },
    set loading(_val: boolean) {},

    get loadingMore() { return runtime.status === 'loading_more'; },
    set loadingMore(_val: boolean) {},

    get error(): string | null { return runtime.error ?? null; },
    set error(val: string | null) { runtime.error = val; },

    get stale() { return !!runtime.error && getCachedDirectoryEntries(location.connectionId, location.path).length > 0; },

    get history() { return navigation.history; },
    set history(val: string[]) { navigation.history = val; },

    get historyIndex() { return navigation.historyIndex; },
    set historyIndex(val: number) { navigation.historyIndex = val; },

    get initialized() { return runtime.initialized; },
    set initialized(val: boolean) { runtime.initialized = val; },

    get hasMore(): boolean { return runtime.hasMore ?? false; },
    set hasMore(val: boolean) { runtime.hasMore = val; },

    get nextCursor() { return runtime.nextCursor; },
    set nextCursor(val: string | undefined) { runtime.nextCursor = val; },

    get totalCount() { return runtime.totalCount; },
    set totalCount(val: number | undefined) { runtime.totalCount = val; },

    get lastLoadedAt() { return runtime.lastLoadedAt; },
    set lastLoadedAt(val: number | undefined) { runtime.lastLoadedAt = val; },

    get lastError() { return runtime.lastError; },
    set lastError(val: string | undefined) { runtime.lastError = val; },

    get status() { return runtime.status; },
    set status(val: any) { runtime.status = val; },
  };

  return reactive(panel) as Panel;
}

export const useWorkspaceStore = defineStore('workspace', () => {
  // Load persistent session from localStorage (Pure versioned session) + history per panel (Phase6 P2)
  let initialLayout: WorkspaceLayout = 'single';
  let initialSplitRatio = 0.5;
  let initialLeftConn = 'local';
  let initialLeftPath = '/';
  let initialLeftView: 'grid' | 'list' = 'grid';
  let initialLeftHidden = false;
  let initialLeftHistory: string[] | undefined;
  let initialLeftHistoryIndex: number | undefined;

  let initialRightConn = 'local';
  let initialRightPath = '/';
  let initialRightView: 'grid' | 'list' = 'grid';
  let initialRightHidden = false;
  let initialRightHistory: string[] | undefined;
  let initialRightHistoryIndex: number | undefined;

  try {
    const rawV1 = localStorage.getItem('fb:workspace_v1');
    if (rawV1) {
      const parsed: PersistedWorkspace = JSON.parse(rawV1);
      if (parsed.version === 1) {
        initialLayout = parsed.layout || 'single';
        initialSplitRatio = parsed.splitRatio || 0.5;
        if (parsed.left) {
          initialLeftConn = parsed.left.connectionId || 'local';
          initialLeftPath = parsed.left.path || '/';
          initialLeftView = parsed.left.viewMode || 'grid';
          initialLeftHidden = !!parsed.left.showHidden;
          initialLeftHistory = parsed.left.history;
          initialLeftHistoryIndex = parsed.left.historyIndex;
        }
        if (parsed.right) {
          initialRightConn = parsed.right.connectionId || 'local';
          initialRightPath = parsed.right.path || '/';
          initialRightView = parsed.right.viewMode || 'grid';
          initialRightHidden = !!parsed.right.showHidden;
          initialRightHistory = parsed.right.history;
          initialRightHistoryIndex = parsed.right.historyIndex;
        }
      }
    } else {
      // Fallback for legacy keys and cleanup
      if (localStorage.getItem('fb:isDualPane') === 'true') initialLayout = 'split';
      initialLeftConn = localStorage.getItem('fb:left:connectionId') || 'local';
      initialLeftPath = localStorage.getItem('fb:left:path') || '/';
      initialRightConn = localStorage.getItem('fb:right:connectionId') || 'local';
      initialRightPath = localStorage.getItem('fb:right:path') || '/';
    }
  } catch {
    // ignore
  }

  const leftPanel = ref<Panel>(createPanel('left', initialLeftConn, initialLeftPath));
  leftPanel.value.view.viewMode = initialLeftView;
  leftPanel.value.view.showHidden = initialLeftHidden;
  if (initialLeftHistory && initialLeftHistory.length > 0) {
    leftPanel.value.navigation.history = initialLeftHistory;
    leftPanel.value.navigation.historyIndex = Math.min(initialLeftHistoryIndex ?? initialLeftHistory.length - 1, initialLeftHistory.length - 1);
  }

  const rightPanel = ref<Panel>(createPanel('right', initialRightConn, initialRightPath));
  rightPanel.value.view.viewMode = initialRightView;
  rightPanel.value.view.showHidden = initialRightHidden;
  if (initialRightHistory && initialRightHistory.length > 0) {
    rightPanel.value.navigation.history = initialRightHistory;
    rightPanel.value.navigation.historyIndex = Math.min(initialRightHistoryIndex ?? initialRightHistory.length - 1, initialRightHistory.length - 1);
  }

  const layout = ref<WorkspaceLayout>(initialLayout);
  const activePanelId = ref<PanelId>('left');
  const activePanel = computed<Panel>(() => {
    return activePanelId.value === 'left' ? leftPanel.value : rightPanel.value;
  });
  const splitRatio = ref<number>(initialSplitRatio);
  const clipboard = ref<WorkspaceClipboard | null>(null);

  // PanelSession formalized lifecycle (66.md §2, §5) — per-panel generation + abort
  const leftSession = new PanelSession('left', initialLeftConn, initialLeftPath);
  const rightSession = new PanelSession('right', initialRightConn, initialRightPath);
  function sessionFor(panelId: PanelId): PanelSession { return panelId === 'left' ? leftSession : rightSession; }

  // Legacy globals kept for backward compat but now delegated to PanelSession
  let leftRequestGen = 0;
  let rightRequestGen = 0;
  function syncSessionWithPanel(panelId: PanelId): void {
    const p = getPanel(panelId);
    const s = sessionFor(panelId);
    s.connectionId = p.location.connectionId;
    s.path = p.location.path;
    s.entries = getCachedDirectoryEntries(p.location.connectionId, p.location.path);
    s.status = p.runtime.status;
    s.error = p.runtime.error ?? null;
  }

  // Offline snapshot cache (66.md §16) — preserve last successful listing per dir
  function snapshotKey(connId: string, path: string): string {
    return `fb:snapshot:${connId}::${normalizePath(path)}`;
  }
  function saveSnapshot(connId: string, path: string, entries: FileEntry[]): void {
    try {
      const key = snapshotKey(connId, path);
      const payload = JSON.stringify({ t: Date.now(), entries: entries.slice(0, 200) });
      localStorage.setItem(key, payload);
    } catch {}
  }
  function loadSnapshot(connId: string, path: string): FileEntry[] | null {
    try {
      const raw = localStorage.getItem(snapshotKey(connId, path));
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      if (!parsed.entries || !Array.isArray(parsed.entries)) return null;
      // Expire after 7 days
      if (Date.now() - (parsed.t || 0) > 7 * 24 * 3600 * 1000) return null;
      return parsed.entries as FileEntry[];
    } catch { return null; }
  }

  const isDualPane = computed<boolean>({
    get: () => layout.value === 'split',
    set: (val: boolean) => {
      layout.value = val ? 'split' : 'single';
      if (!val) {
        activePanelId.value = 'left';
      }
    },
  });

  function setSplitRatio(ratio: number) {
    splitRatio.value = Math.min(Math.max(ratio, 0.2), 0.8);
    saveState();
  }

  function saveState() {
    const prefsJson = localStorage.getItem('fb:user_preferences');
    let rememberLastDir = true;
    if (prefsJson) {
      try {
        const parsed = JSON.parse(prefsJson);
        if (parsed.remember_last_directories !== undefined) {
          rememberLastDir = Boolean(parsed.remember_last_directories);
        } else if (parsed.remember_last_dir !== undefined) {
          rememberLastDir = Boolean(parsed.remember_last_dir);
        }
      } catch {}
    }

    const persisted: PersistedWorkspace = {
      version: 1,
      layout: layout.value,
      activePanel: activePanelId.value,
      splitRatio: splitRatio.value,
      left: {
        connectionId: leftPanel.value.location.connectionId,
        path: rememberLastDir ? leftPanel.value.location.path : '/',
        viewMode: leftPanel.value.view.viewMode,
        showHidden: leftPanel.value.view.showHidden,
        sortField: leftPanel.value.view.sortField,
        sortOrder: leftPanel.value.view.sortOrder,
        history: leftPanel.value.navigation.history.slice(0, 50),
        historyIndex: leftPanel.value.navigation.historyIndex,
      },
      right: {
        connectionId: rightPanel.value.location.connectionId,
        path: rememberLastDir ? rightPanel.value.location.path : '/',
        viewMode: rightPanel.value.view.viewMode,
        showHidden: rightPanel.value.view.showHidden,
        sortField: rightPanel.value.view.sortField,
        sortOrder: rightPanel.value.view.sortOrder,
        history: rightPanel.value.navigation.history.slice(0, 50),
        historyIndex: rightPanel.value.navigation.historyIndex,
      },
    };
    localStorage.setItem('fb:workspace_v1', JSON.stringify(persisted));
    localStorage.setItem('fb:isDualPane', isDualPane.value ? 'true' : 'false');
  }

  function getPanel(id: PanelId): Panel {
    return id === 'left' ? leftPanel.value : rightPanel.value;
  }

  function setActivePanel(id: PanelId) {
    if (layout.value === 'single') {
      activePanelId.value = 'left';
    } else {
      activePanelId.value = id;
    }
  }

  function setDualPane(enable: boolean) {
    layout.value = enable ? 'split' : 'single';
    if (!enable) {
      activePanelId.value = 'left';
    }
    saveState();
    if (enable && !rightPanel.value.runtime.initialized) {
      refreshPanel('right');
    }
  }

  function abortPanel(panelId: PanelId) {
    const sess = sessionFor(panelId);
    sess.bumpGeneration();
    if (panelId === 'left') {
      leftRequestGen = sess.generation;
    } else {
      rightRequestGen = sess.generation;
    }
  }

  function cancelPendingInvalidations() {
    // legacy no-op: RealtimeSyncCoordinator now handles 150ms debounced invalidations
  }

  function closePanel(panelId: PanelId) {
    // Formalized lifecycle: mark closed, bump generation, abort, clear selection, release memory (66.md §4)
    const sess = sessionFor(panelId);
    sess.markClosed();
    const panel = getPanel(panelId);
    panel.selection.paths = [];
    panel.selection.focusedPath = undefined;
    cancelPendingInvalidations();
    abortPanel(panelId);
    if (panelId === 'left') {
      // Promote right to left — also promote session state for continuity
      const rightSess = sessionFor('right');
      // Reset left session to reflect promoted right's location (new view)
      leftSession.connectionId = rightPanel.value.location.connectionId;
      leftSession.path = rightPanel.value.location.path;
      leftSession.closed = false;
      leftSession.status = rightSess.status;
      leftSession.entries = rightSess.entries.slice();
      abortPanel('right');
      const oldRight = rightPanel.value;
      oldRight.id = 'left';
      leftPanel.value = oldRight;
      rightPanel.value = createPanel('right', 'local', '/');
      // Reset right session to fresh
      rightSession.closed = false;
      rightSession.generation = 0;
      rightSession.entries = [];
      rightSession.status = 'idle';
    } else {
      // closing right — just clear its session
      sess.dispose();
    }
    layout.value = 'single';
    activePanelId.value = 'left';
    saveState();
  }

  function swapPanels() {
    const uiStore = useUiStore();
    if (uiStore.isMobile) {
      setActivePanel(activePanelId.value === 'left' ? 'right' : 'left');
      return;
    }
    cancelPendingInvalidations();
    abortPanel('left');
    abortPanel('right');
    const oldLeft = leftPanel.value;
    const oldRight = rightPanel.value;
    oldLeft.id = 'right';
    oldRight.id = 'left';
    leftPanel.value = oldRight;
    rightPanel.value = oldLeft;
    // Swap session state to keep generation tied to slot (66.md §29 isolation)
    const tmpConn = leftSession.connectionId;
    const tmpPath = leftSession.path;
    leftSession.connectionId = rightSession.connectionId;
    leftSession.path = rightSession.path;
    rightSession.connectionId = tmpConn;
    rightSession.path = tmpPath;
    // Swap entries/status for continuity
    const tmpEntries = leftSession.entries.slice();
    leftSession.entries = rightSession.entries.slice();
    rightSession.entries = tmpEntries;
    activePanelId.value = activePanelId.value === 'left' ? 'right' : 'left';
    saveState();
  }

  async function openInOtherPanel(sourcePanelId: PanelId, targetPath: string) {
    const source = getPanel(sourcePanelId);
    const targetId: PanelId = sourcePanelId === 'left' ? 'right' : 'left';
    const target = getPanel(targetId);

    if (!isDualPane.value) {
      setDualPane(true);
    }

    target.location.connectionId = source.location.connectionId;
    await navigateTo(targetId, targetPath);
    setActivePanel(targetId);
    saveState();
  }

  // --- TRANSACTIONAL NAVIGATION ENGINE ---

  async function navigateTo(
    panelId: PanelId,
    targetPath: string,
    addToHistory: boolean = true
  ): Promise<{ ok: boolean; path?: string; error?: string }> {
    const p = getPanel(panelId);
    const normalizedTarget = normalizePath(targetPath);
    const previousPath = p.location.path;
    const currentPath = p.location.path;
    let direction: 'forward' | 'back' | 'replace' = 'replace';
    if (normalizedTarget.startsWith(currentPath) && normalizedTarget.length > currentPath.length) {
      direction = 'forward';
    } else if (currentPath.startsWith(normalizedTarget) && currentPath.length > normalizedTarget.length) {
      direction = 'back';
    }
    p.navigation.direction = direction;
    p.navigationDirection = direction;

    const sess = sessionFor(panelId);
    p.runtime.status = p.runtime.initialized ? 'refreshing' : 'loading';
    sess.status = p.runtime.status;
    p.runtime.error = null;

    const { generation: currentGen } = sess.newRequest();
    if (panelId === 'left') {
      leftRequestGen = currentGen;
    } else {
      rightRequestGen = currentGen;
    }
    syncSessionWithPanel(panelId);

    try {
      const data = await queryClient.fetchInfiniteQuery(
        directoryQueryOptions(p.location.connectionId, normalizedTarget, {
          show_hidden: p.view.showHidden,
          sort: p.view.sortField,
          order: p.view.sortOrder,
        })
      );

      // Discard stale out-of-order response (generation guard + closed panel guard 66.md §5, §21)
      if (panelId === 'left' ? currentGen !== leftRequestGen : currentGen !== rightRequestGen) {
        return { ok: false, error: 'Stale response discarded' };
      }
      if (sess.isStale(currentGen)) {
        return { ok: false, error: 'Stale response discarded' };
      }

      const allEntries: FileEntry[] = data.pages.flatMap((pg: any) => pg.entries);
      const lastPage = data.pages[data.pages.length - 1];

      // TRANSACTIONAL COMMIT: commit path only upon verified success!
      p.location.path = lastPage?.path || normalizedTarget;
      p.runtime.hasMore = lastPage?.has_more ?? false;
      p.runtime.nextCursor = lastPage?.next_cursor ?? undefined;
      p.runtime.totalCount = lastPage?.total_count ?? undefined;

      // Reconcile selection: preserve items that still exist
      const previousSelection = new Set(p.selection.paths);
      p.selection.paths = allEntries
        .map((e: FileEntry) => e.path)
        .filter((entryPath: string) => previousSelection.has(entryPath));

      p.runtime.status = 'idle';
      p.runtime.error = null;
      p.runtime.lastError = undefined;
      p.runtime.lastLoadedAt = Date.now();
      p.runtime.initialized = true;
      try { saveSnapshot(p.location.connectionId, p.location.path, allEntries); } catch {}

      if (addToHistory && p.location.path !== p.navigation.history[p.navigation.historyIndex]) {
        p.navigation.history = p.navigation.history.slice(0, p.navigation.historyIndex + 1);
        p.navigation.history.push(p.location.path);
        p.navigation.historyIndex = p.navigation.history.length - 1;
      }
      saveState();
      return { ok: true, path: p.location.path };
    } catch (err: unknown) {
      if (isAbortError(err)) {
        p.runtime.status = p.runtime.initialized ? 'idle' : 'error';
        return { ok: false, error: 'Aborted', aborted: true } as any;
      }
      if (panelId === 'left' ? currentGen !== leftRequestGen : currentGen !== rightRequestGen) {
        return { ok: false, error: 'Stale response discarded', aborted: true } as any;
      }
      if (sess.isStale(currentGen)) {
        return { ok: false, error: 'Stale response discarded', aborted: true } as any;
      }
      const norm = normalizeApiError(err);
      // Offline cached snapshot fallback (66.md §22) — show last cached if available
      if (p.entries.length === 0) {
        const cached = loadSnapshot(p.location.connectionId, normalizedTarget);
        if (cached && cached.length > 0) {
          queryClient.setQueryData(
            queryKeys.directory(p.location.connectionId, normalizedTarget, {
              show_hidden: p.view.showHidden,
              sort: p.view.sortField,
              order: p.view.sortOrder,
            }),
            {
              pages: [{ entries: cached, path: normalizedTarget, has_more: false, total_count: cached.length }],
              pageParams: [undefined],
            }
          );
          p.runtime.status = 'offline';
          p.runtime.error = norm.message;
          p.runtime.lastError = norm.message;
          return { ok: false, error: norm.message };
        }
      }
      p.runtime.status = p.entries.length > 0 ? 'degraded' : 'error';
      p.runtime.error = norm.message;
      p.runtime.lastError = norm.message;

      // Rollback path
      p.location.path = previousPath;
      if (norm.message) {
        const uiStore = useUiStore();
        uiStore.showToast(norm.message || 'Failed to open directory', 'error');
      }
      return { ok: false, error: norm.message };
    }
  }

  async function fetchNextPage(
    panelId: PanelId
  ): Promise<{ ok: boolean; count?: number; error?: string }> {
    const p = getPanel(panelId);
    if (
      !p.runtime.hasMore ||
      !p.runtime.nextCursor ||
      p.runtime.status === 'loading_more' ||
      p.runtime.status === 'loading'
    ) {
      return { ok: true, count: 0 };
    }

    const currentGen = panelId === 'left' ? leftRequestGen : rightRequestGen;
    const currentPath = p.location.path;
    const currentConn = p.location.connectionId;

    p.runtime.status = 'loading_more';
    try {
      const data = await queryClient.fetchInfiniteQuery(
        directoryQueryOptions(p.location.connectionId, p.location.path, {
          show_hidden: p.view.showHidden,
          sort: p.view.sortField,
          order: p.view.sortOrder,
        })
      );

      // Discard stale out-of-order pagination if directory changed mid-flight
      if (
        p.location.path !== currentPath ||
        p.location.connectionId !== currentConn ||
        (panelId === 'left' ? currentGen !== leftRequestGen : currentGen !== rightRequestGen)
      ) {
        p.runtime.status = 'idle';
        return { ok: false, error: 'Stale pagination response discarded' };
      }

      const allEntries: FileEntry[] = data.pages.flatMap((pg: any) => pg.entries);
      const lastPage = data.pages[data.pages.length - 1];

      p.runtime.hasMore = lastPage?.has_more ?? false;
      p.runtime.nextCursor = lastPage?.next_cursor ?? undefined;
      p.runtime.totalCount = lastPage?.total_count ?? undefined;
      p.runtime.status = 'idle';
      return { ok: true, count: allEntries.length };
    } catch (err: unknown) {
      p.runtime.status = 'idle';
      const norm = normalizeApiError(err);
      return { ok: false, error: norm.message };
    }
  }

  async function navigatePanel(panelId: PanelId, targetPath: string, addToHistory: boolean = true) {
    await navigateTo(panelId, targetPath, addToHistory);
  }

  async function switchPanelConnection(panelId: PanelId, connectionId: string, basePath: string = '/') {
    abortPanel(panelId);
    const p = getPanel(panelId);
    p.navigation.direction = 'replace';
    p.navigationDirection = 'replace';
    p.location.connectionId = connectionId;
    p.navigation.history = [basePath];
    p.navigation.historyIndex = 0;
    p.selection.paths = [];
    p.selection.focusedPath = undefined;
    p.runtime.initialized = false;
    const res = await navigateTo(panelId, basePath, false);
    if (!res.ok) {
      p.location.path = basePath;
    }
    saveState();
  }

  async function goBack(panelId: PanelId) {
    const p = getPanel(panelId);
    if (p.navigation.historyIndex > 0) {
      p.navigation.direction = 'back';
      p.navigationDirection = 'back';
      const targetIdx = p.navigation.historyIndex - 1;
      const targetPath = p.navigation.history[targetIdx];
      const res = await navigateTo(panelId, targetPath, false);
      if (res.ok) {
        p.navigation.historyIndex = targetIdx;
        saveState();
      }
    }
  }

  async function goForward(panelId: PanelId) {
    const p = getPanel(panelId);
    if (p.navigation.historyIndex < p.navigation.history.length - 1) {
      p.navigation.direction = 'forward';
      p.navigationDirection = 'forward';
      const targetIdx = p.navigation.historyIndex + 1;
      const targetPath = p.navigation.history[targetIdx];
      const res = await navigateTo(panelId, targetPath, false);
      if (res.ok) {
        p.navigation.historyIndex = targetIdx;
        saveState();
      }
    }
  }

  async function navigateUp(panelId: PanelId) {
    const p = getPanel(panelId);
    if (p.location.path === '/' || p.location.path === '') return;
    p.navigation.direction = 'back';
    p.navigationDirection = 'back';
    const parent = parentPath(p.location.path);
    await navigateTo(panelId, parent);
  }

  async function goBackPanel(panelId: PanelId) { await goBack(panelId); }
  async function goForwardPanel(panelId: PanelId) { await goForward(panelId); }
  async function navigateUpPanel(panelId: PanelId) { await navigateUp(panelId); }

  function invalidatePanel(panelId: PanelId) {
    const p = getPanel(panelId);
    try {
      queryClient.invalidateQueries({
        queryKey: queryKeys.directoryPrefix(p.location.connectionId, p.location.path),
      });
    } catch {}
  }

  async function refresh(panelId: PanelId): Promise<{ ok: boolean; path?: string; error?: string }> {
    const p = getPanel(panelId);
    const res = await navigateTo(panelId, p.location.path, false);
    invalidatePanel(panelId);
    return res;
  }

  async function refreshPanel(panelId: PanelId): Promise<{ ok: boolean; path?: string; error?: string }> {
    return await refresh(panelId);
  }

  async function refreshActive() {
    await refresh(activePanelId.value);
  }

  async function refreshAll() {
    await Promise.all([
      refresh('left'),
      ...(isDualPane.value ? [refresh('right')] : []),
    ]);
  }

  async function toggleShowHidden(panelId?: PanelId) {
    const id = panelId || activePanelId.value;
    const p = getPanel(id);
    p.view.showHidden = !p.view.showHidden;
    saveState();
    await refresh(id);
  }

  // --- WORKSPACE CLIPBOARD ---

  function setClipboard(operation: 'copy' | 'cut', sourcePanelId: PanelId, paths: string[], items?: FileEntry[]) {
    const p = getPanel(sourcePanelId);
    clipboard.value = {
      operation,
      sourceConnectionId: p.location.connectionId,
      sourcePanelId,
      paths,
      items,
    };
  }

  function clearClipboard() {
    clipboard.value = null;
  }

  function copySelection(panelId?: PanelId) {
    const id = panelId || activePanelId.value;
    const p = getPanel(id);
    if (p.selection.paths.length === 0) return;
    setClipboard('copy', id, p.selection.paths);
  }

  function cutSelection(panelId?: PanelId) {
    const id = panelId || activePanelId.value;
    const p = getPanel(id);
    if (p.selection.paths.length === 0) return;
    setClipboard('cut', id, p.selection.paths);
  }

  function isCutItem(connectionId: string, path: string): boolean {
    if (!clipboard.value || clipboard.value.operation !== 'cut') return false;
    return clipboard.value.sourceConnectionId === connectionId && clipboard.value.paths.includes(path);
  }

  async function paste(targetPanelId?: PanelId) {
    const id = targetPanelId || activePanelId.value;
    const targetPanel = getPanel(id);
    const transferStore = useTransferStore();
    const uiStore = useUiStore();

    if (!clipboard.value || clipboard.value.paths.length === 0) {
      uiStore.showToast('Clipboard is empty', 'info');
      return;
    }

    const { operation, sourceConnectionId, paths } = clipboard.value;
    const isCut = operation === 'cut';
    transferStore.resetBatchConflict();

    try {
      const targetEntries = await ensureDirectoryData(targetPanel.location.connectionId, targetPanel.location.path);

      for (const filePath of paths) {
        let fileName = filePath.split('/').pop() || 'file';
        let destPath = targetPanel.location.path === '/'
          ? `/${fileName}`
          : `${targetPanel.location.path}/${fileName}`;

        // Skip pasting into exact same path on same connection
        if (sourceConnectionId === targetPanel.location.connectionId && filePath === destPath) {
          continue;
        }

        // Check if destination directory already has an entry with the same name
        const alreadyExists = targetEntries.some((e) => e.name === fileName);
        if (alreadyExists) {
          const resolution = await transferStore.requestConflict(fileName, filePath, destPath);
          if (resolution === 'cancel') {
            break;
          }
          if (resolution === 'skip') {
            continue;
          }
          if (resolution === 'keep_both') {
            const dotIdx = fileName.lastIndexOf('.');
            let count = 1;
            let candidateName = dotIdx > 0
              ? `${fileName.substring(0, dotIdx)} (${count})${fileName.substring(dotIdx)}`
              : `${fileName} (${count})`;

            while (targetEntries.some((e) => e.name === candidateName)) {
              count++;
              candidateName = dotIdx > 0
                ? `${fileName.substring(0, dotIdx)} (${count})${fileName.substring(dotIdx)}`
                : `${fileName} (${count})`;
            }

            fileName = candidateName;
            destPath = targetPanel.location.path === '/'
              ? `/${fileName}`
              : `${targetPanel.location.path}/${fileName}`;
          }
        }

        await transferStore.submitTransfer(
          `${isCut ? 'Move' : 'Copy'} ${fileName} to ${targetPanel.location.path}`,
          isCut ? 'move' : 'copy',
          sourceConnectionId,
          filePath,
          targetPanel.location.connectionId,
          destPath
        );
      }

      uiStore.showToast(`Queued ${paths.length} ${isCut ? 'move' : 'copy'} transfer(s)`, 'info');

      if (isCut) {
        clearClipboard();
      }
    } catch (err: any) {
      uiStore.showToast(normalizeApiError(err).message || 'Paste transfer failed', 'error');
    }
  }

  async function transferBetweenPanels(
    sourcePanelId: PanelId,
    destPanelId: PanelId,
    filePaths: string[],
    isMove: boolean = false
  ) {
    const destPanel = getPanel(destPanelId);
    const sourcePanel = getPanel(sourcePanelId);
    const transferStore = useTransferStore();
    transferStore.resetBatchConflict();

    const destEntries = await ensureDirectoryData(destPanel.location.connectionId, destPanel.location.path);

    for (const filePath of filePaths) {
      let fileName = filePath.split('/').pop() || 'file';
      let destPath = destPanel.location.path === '/'
        ? `/${fileName}`
        : `${destPanel.location.path}/${fileName}`;

      // Check if destination directory already has an entry with the same name
      const alreadyExists = destEntries.some((e) => e.name === fileName);
      if (alreadyExists) {
        const resolution = await transferStore.requestConflict(fileName, filePath, destPath);
        if (resolution === 'cancel') {
          break;
        }
        if (resolution === 'skip') {
          continue;
        }
        if (resolution === 'keep_both') {
          const dotIdx = fileName.lastIndexOf('.');
          let count = 1;
          let candidateName = dotIdx > 0
            ? `${fileName.substring(0, dotIdx)} (${count})${fileName.substring(dotIdx)}`
            : `${fileName} (${count})`;

          while (destEntries.some((e) => e.name === candidateName)) {
            count++;
            candidateName = dotIdx > 0
              ? `${fileName.substring(0, dotIdx)} (${count})${fileName.substring(dotIdx)}`
              : `${fileName} (${count})`;
          }

          fileName = candidateName;
          destPath = destPanel.location.path === '/'
            ? `/${fileName}`
            : `${destPanel.location.path}/${fileName}`;
        }
      }

      await transferStore.submitTransfer(
        `${isMove ? 'Move' : 'Copy'} ${fileName} to ${destPanel.location.path}`,
        isMove ? 'move' : 'copy',
        sourcePanel.location.connectionId,
        filePath,
        destPanel.location.connectionId,
        destPath
      );
    }
  }

  // --- WORKSPACE PRESETS ---

  const presets = ref([
    {
      id: 'default-single',
      name: 'Single Local Workspace',
      description: 'Single full-width local storage panel',
      layout: 'single' as const,
      leftConn: 'local',
      leftPath: '/',
    },
    {
      id: 'dual-local',
      name: 'Dual Local Split',
      description: 'Side-by-side local panels for fast local organization',
      layout: 'split' as const,
      leftConn: 'local',
      leftPath: '/',
      rightConn: 'local',
      rightPath: '/',
    },
  ]);

  async function applyPreset(presetId: string) {
    const preset = presets.value.find((p) => p.id === presetId);
    if (!preset) return;
    layout.value = preset.layout;
    leftPanel.value.location.connectionId = preset.leftConn;
    leftPanel.value.location.path = preset.leftPath;
    await refreshPanel('left');

    if (preset.layout === 'split' && preset.rightConn) {
      rightPanel.value.location.connectionId = preset.rightConn;
      rightPanel.value.location.path = preset.rightPath || '/';
      await refreshPanel('right');
    }
    saveState();
  }

  function queueDirectoryInvalidation(connectionId: string, dirPath: string) {
    realtimeSync.queueDirectoryInvalidation(connectionId, dirPath);
  }

  function notifyFileChange(connectionId: string, filePath: string) {
    const parentDir = parentPath(normalizePath(filePath));
    realtimeSync.queueDirectoryInvalidation(connectionId, parentDir);
  }

  // Connection orphan detection (66.md §24-25) — panel becomes orphaned if connection removed/disabled
  const connStoreRef = useConnectionStore();
  watch(
    () => connStoreRef.connections.map((c: any) => c.id),
    (ids: string[]) => {
      (['left', 'right'] as PanelId[]).forEach((pid) => {
        const p = getPanel(pid);
        const exists = p.connectionId === 'local' || ids.includes(p.connectionId);
        if (!exists && p.runtime.status !== 'orphaned') {
          p.runtime.status = 'orphaned';
          p.runtime.error = 'Connection unavailable';
        } else if (exists && p.runtime.status === 'orphaned') {
          p.runtime.status = 'idle';
          refreshPanel(pid);
        }
      });
    },
    { deep: false }
  );

  // Global keyboard shortcuts (66.md §30) — file-manager like
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', (e: KeyboardEvent) => {
      // F5 or Ctrl+R → refresh active panel (prevent browser reload on F5)
      if (e.key === 'F5' || (e.ctrlKey && e.key.toLowerCase() === 'r' && !e.shiftKey)) {
        e.preventDefault();
        refreshActive();
        return;
      }
      if (e.altKey && e.key === 'ArrowLeft') {
        e.preventDefault();
        goBack(activePanelId.value);
        return;
      }
      if (e.altKey && e.key === 'ArrowRight') {
        e.preventDefault();
        goForward(activePanelId.value);
        return;
      }
      if (e.ctrlKey && e.key.toLowerCase() === 'l') {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('aerofs:open-address-bar', { detail: { panelId: activePanelId.value } }));
        return;
      }
      if (e.key === 'Escape') {
        const p = activePanel.value;
        if (p.selection.paths.length > 0) {
          p.selection.paths = [];
        } else if (p.runtime.status === 'loading' || p.runtime.status === 'refreshing') {
          abortPanel(activePanelId.value);
        }
      }
    });
  }

  function disposeWorkspace() {
    cancelPendingInvalidations();
    leftSession.dispose();
    rightSession.dispose();
    abortPanel('left');
    abortPanel('right');
  }

  return {
    layout,
    isDualPane,
    activePanelId,
    activePanel,
    splitRatio,
    leftPanel,
    rightPanel,
    clipboard,
    presets,
    applyPreset,
    getPanel,
    setActivePanel,
    setDualPane,
    setSplitRatio,
    closePanel,
    swapPanels,
    openInOtherPanel,
    fetchNextPage,
    navigateTo,
    navigatePanel,
    switchPanelConnection,
    goBack,
    goForward,
    navigateUp,
    goBackPanel,
    goForwardPanel,
    navigateUpPanel,
    refresh,
    refreshPanel,
    refreshActive,
    refreshAll,
    toggleShowHidden,
    setClipboard,
    clearClipboard,
    copySelection,
    cutSelection,
    paste,
    isCutItem,
    transferBetweenPanels,
    queueDirectoryInvalidation,
    notifyFileChange,
    saveState,
    disposeWorkspace,
  };
});
