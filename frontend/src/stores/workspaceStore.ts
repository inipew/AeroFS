import { defineStore } from 'pinia';
import { ref, computed, reactive } from 'vue';
import { listFilesApi } from '../api/files';
import { subscribeFileChanges } from '../services/fileChangeBus';
import { useTransferStore } from './transferStore';
import { useUiStore } from './uiStore';
import { normalizePath, parentPath } from '../utils/path';
import { isAbortError, normalizeApiError } from '../utils/errorNormalizer';
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
    entries: [] as FileEntry[],
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

    get entries(): FileEntry[] { return runtime.entries; },
    set entries(val: FileEntry[]) { runtime.entries = val; },

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

    get error() { return runtime.error; },
    set error(val: string | null) { runtime.error = val; },

    get stale() { return !!runtime.error && runtime.entries.length > 0; },

    get history() { return navigation.history; },
    set history(val: string[]) { navigation.history = val; },

    get historyIndex() { return navigation.historyIndex; },
    set historyIndex(val: number) { navigation.historyIndex = val; },

    get initialized() { return runtime.initialized; },
    set initialized(val: boolean) { runtime.initialized = val; },

    get hasMore() { return runtime.hasMore; },
    set hasMore(val: boolean) { runtime.hasMore = val; },

    get nextCursor() { return runtime.nextCursor; },
    set nextCursor(val: string | undefined) { runtime.nextCursor = val; },

    get totalCount() { return runtime.totalCount; },
    set totalCount(val: number | undefined) { runtime.totalCount = val; },
  };

  return reactive(panel) as Panel;
}

export const useWorkspaceStore = defineStore('workspace', () => {
  // Load persistent session from localStorage (Pure versioned session)
  let initialLayout: WorkspaceLayout = 'single';
  let initialSplitRatio = 0.5;
  let initialLeftConn = 'local';
  let initialLeftPath = '/';
  let initialLeftView: 'grid' | 'list' = 'grid';
  let initialLeftHidden = false;

  let initialRightConn = 'local';
  let initialRightPath = '/';
  let initialRightView: 'grid' | 'list' = 'grid';
  let initialRightHidden = false;

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
        }
        if (parsed.right) {
          initialRightConn = parsed.right.connectionId || 'local';
          initialRightPath = parsed.right.path || '/';
          initialRightView = parsed.right.viewMode || 'grid';
          initialRightHidden = !!parsed.right.showHidden;
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

  const rightPanel = ref<Panel>(createPanel('right', initialRightConn, initialRightPath));
  rightPanel.value.view.viewMode = initialRightView;
  rightPanel.value.view.showHidden = initialRightHidden;

  const layout = ref<WorkspaceLayout>(initialLayout);
  const activePanelId = ref<PanelId>('left');
  const activePanel = computed<Panel>(() => {
    return activePanelId.value === 'left' ? leftPanel.value : rightPanel.value;
  });
  const splitRatio = ref<number>(initialSplitRatio);
  const clipboard = ref<WorkspaceClipboard | null>(null);

  // Request sequencing generation counters and AbortControllers to avoid race conditions
  let leftRequestGen = 0;
  let rightRequestGen = 0;
  let leftAbortController: AbortController | null = null;
  let rightAbortController: AbortController | null = null;

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
      },
      right: {
        connectionId: rightPanel.value.location.connectionId,
        path: rememberLastDir ? rightPanel.value.location.path : '/',
        viewMode: rightPanel.value.view.viewMode,
        showHidden: rightPanel.value.view.showHidden,
        sortField: rightPanel.value.view.sortField,
        sortOrder: rightPanel.value.view.sortOrder,
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
      fetchPanelEntries('right');
    }
  }

  function abortPanel(panelId: PanelId) {
    if (panelId === 'left') {
      if (leftAbortController) {
        leftAbortController.abort();
        leftAbortController = null;
      }
      leftRequestGen++;
    } else {
      if (rightAbortController) {
        rightAbortController.abort();
        rightAbortController = null;
      }
      rightRequestGen++;
    }
  }

  function closePanel(panelId: PanelId) {
    abortPanel(panelId);
    if (panelId === 'left') {
      abortPanel('right');
      // Promote right panel to single left panel without deep cloning
      const oldRight = rightPanel.value;
      oldRight.id = 'left';
      leftPanel.value = oldRight;
      rightPanel.value = createPanel('right', 'local', '/');
    }
    layout.value = 'single';
    activePanelId.value = 'left';
    saveState();
  }

  function swapPanels() {
    const uiStore = useUiStore();
    if (uiStore.isMobile) {
      // On mobile, swapping means switching active panel viewport focus via GPU translate3d
      setActivePanel(activePanelId.value === 'left' ? 'right' : 'left');
      return;
    }

    abortPanel('left');
    abortPanel('right');
    const oldLeft = leftPanel.value;
    const oldRight = rightPanel.value;
    oldLeft.id = 'right';
    oldRight.id = 'left';
    leftPanel.value = oldRight;
    rightPanel.value = oldLeft;
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

  async function fetchPanelEntries(
    panelId: PanelId,
    targetPath?: string
  ): Promise<{ ok: boolean; path?: string; error?: string }> {
    const p = getPanel(panelId);
    const queryPath = targetPath !== undefined ? targetPath : p.location.path;

    p.runtime.status = p.runtime.initialized ? 'refreshing' : 'loading';
    p.runtime.error = null;

    const currentGen = panelId === 'left' ? ++leftRequestGen : ++rightRequestGen;

    // Abort previous in-flight request for this panel
    if (panelId === 'left') {
      if (leftAbortController) leftAbortController.abort();
      leftAbortController = new AbortController();
    } else {
      if (rightAbortController) rightAbortController.abort();
      rightAbortController = new AbortController();
    }
    const signal = (panelId === 'left' ? leftAbortController! : rightAbortController!).signal;

    try {
      const data = await listFilesApi(p.location.connectionId, {
        path: queryPath,
        show_hidden: p.view.showHidden,
        sort: p.view.sortField,
        order: p.view.sortOrder,
        signal,
      });

      // Discard stale out-of-order response
      if (panelId === 'left' ? currentGen !== leftRequestGen : currentGen !== rightRequestGen) {
        return { ok: false, error: 'Stale response discarded' };
      }

      // TRANSACTIONAL COMMIT: commit path and entries only upon verified success!
      p.location.path = data.path;
      p.runtime.entries = data.entries;
      p.runtime.hasMore = data.has_more ?? false;
      p.runtime.nextCursor = data.next_cursor;
      p.runtime.totalCount = data.total_count;

      // Reconcile selection: preserve items that still exist
      const previousSelection = new Set(p.selection.paths);
      p.selection.paths = data.entries
        .map((e) => e.path)
        .filter((entryPath) => previousSelection.has(entryPath));

      p.runtime.status = 'idle';
      p.runtime.error = null;
      p.runtime.initialized = true;
      saveState();
      return { ok: true, path: data.path };
    } catch (err: unknown) {
      if (isAbortError(err)) {
        return { ok: false, error: 'Aborted', aborted: true } as any;
      }
      if (panelId === 'left' ? currentGen !== leftRequestGen : currentGen !== rightRequestGen) {
        return { ok: false, error: 'Stale response discarded', aborted: true } as any;
      }
      const norm = normalizeApiError(err);
      p.runtime.status = p.runtime.entries.length > 0 ? 'stale' : 'error';
      p.runtime.error = norm.message;
      return { ok: false, error: p.runtime.error || undefined };
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
      const data = await listFilesApi(p.location.connectionId, {
        path: p.location.path,
        show_hidden: p.view.showHidden,
        sort: p.view.sortField,
        order: p.view.sortOrder,
        cursor: p.runtime.nextCursor,
        limit: 50,
      });

      // Discard stale out-of-order pagination if directory changed mid-flight
      if (
        p.location.path !== currentPath ||
        p.location.connectionId !== currentConn ||
        (panelId === 'left' ? currentGen !== leftRequestGen : currentGen !== rightRequestGen)
      ) {
        p.runtime.status = 'idle';
        return { ok: false, error: 'Stale pagination response discarded' };
      }

      p.runtime.entries.push(...data.entries);
      p.runtime.hasMore = data.has_more ?? false;
      p.runtime.nextCursor = data.next_cursor;
      p.runtime.totalCount = data.total_count;
      p.runtime.status = 'idle';
      return { ok: true, count: data.entries.length };
    } catch (err: unknown) {
      p.runtime.status = 'idle';
      const norm = normalizeApiError(err);
      return { ok: false, error: norm.message };
    }
  }

  async function navigateTo(
    panelId: PanelId,
    targetPath: string,
    addToHistory: boolean = true
  ): Promise<{ ok: boolean; path?: string; error?: string }> {
    const p = getPanel(panelId);
    const currentPath = p.location.path;
    let direction: 'forward' | 'back' | 'replace' = 'replace';
    if (targetPath.startsWith(currentPath) && targetPath.length > currentPath.length) {
      direction = 'forward';
    } else if (currentPath.startsWith(targetPath) && currentPath.length > targetPath.length) {
      direction = 'back';
    }
    p.navigation.direction = direction;
    p.navigationDirection = direction;

    const res = await fetchPanelEntries(panelId, targetPath);

    if (!res.ok) {
      if (res.error !== 'Aborted' && res.error !== 'Stale response discarded' && !(res as any).aborted) {
        const uiStore = useUiStore();
        uiStore.showToast(res.error || 'Failed to open directory', 'error');
      }
      return res;
    }

    if (addToHistory && p.location.path !== p.navigation.history[p.navigation.historyIndex]) {
      // Truncate forward history on new verified navigation
      p.navigation.history = p.navigation.history.slice(0, p.navigation.historyIndex + 1);
      p.navigation.history.push(p.location.path);
      p.navigation.historyIndex = p.navigation.history.length - 1;
    }
    saveState();
    return res;
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
    const res = await fetchPanelEntries(panelId, basePath);
    if (!res.ok) {
      // Still set base path fallback if fetch failed
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
      const res = await fetchPanelEntries(panelId, targetPath);
      if (res.ok) {
        p.navigation.historyIndex = targetIdx;
        saveState();
      } else if (res.error !== 'Aborted' && res.error !== 'Stale response discarded' && !(res as any).aborted) {
        const uiStore = useUiStore();
        uiStore.showToast(res.error || `Failed to navigate back to ${targetPath}`, 'error');
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
      const res = await fetchPanelEntries(panelId, targetPath);
      if (res.ok) {
        p.navigation.historyIndex = targetIdx;
        saveState();
      } else if (res.error !== 'Aborted' && res.error !== 'Stale response discarded' && !(res as any).aborted) {
        const uiStore = useUiStore();
        uiStore.showToast(res.error || `Failed to navigate forward to ${targetPath}`, 'error');
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

  async function refresh(panelId: PanelId) {
    await fetchPanelEntries(panelId);
  }

  async function refreshPanel(panelId: PanelId) {
    await refresh(panelId);
  }

  async function refreshActive() {
    await refresh(activePanelId.value);
  }

  async function refreshAll() {
    await refresh('left');
    if (isDualPane.value) {
      await refresh('right');
    }
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
        const alreadyExists = targetPanel.runtime.entries.some((e) => e.name === fileName);
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

            while (targetPanel.runtime.entries.some((e) => e.name === candidateName)) {
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
      uiStore.showToast(err.response?.data?.error?.message || 'Paste transfer failed', 'error');
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

    for (const filePath of filePaths) {
      let fileName = filePath.split('/').pop() || 'file';
      let destPath = destPanel.location.path === '/'
        ? `/${fileName}`
        : `${destPanel.location.path}/${fileName}`;

      // Check if destination directory already has an entry with the same name
      const alreadyExists = destPanel.runtime.entries.some((e) => e.name === fileName);
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

          while (destPanel.runtime.entries.some((e) => e.name === candidateName)) {
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
    await fetchPanelEntries('left');

    if (preset.layout === 'split' && preset.rightConn) {
      rightPanel.value.location.connectionId = preset.rightConn;
      rightPanel.value.location.path = preset.rightPath || '/';
      await fetchPanelEntries('right');
    }
    saveState();
  }

  function notifyFileChange(connectionId: string, filePath: string) {
    const normFile = normalizePath(filePath);
    const parentDir = parentPath(normFile);

    function isPanelAffected(loc: PanelLocation): boolean {
      if (loc.connectionId !== connectionId) return false;
      const normPanelPath = normalizePath(loc.path);
      return normPanelPath === parentDir || normPanelPath === normFile;
    }

    // Check left panel
    if (isPanelAffected(leftPanel.value.location)) {
      fetchPanelEntries('left');
    }
    // Check right panel
    if (isDualPane.value && isPanelAffected(rightPanel.value.location)) {
      fetchPanelEntries('right');
    }
  }

  // Realtime Filesystem Event Invalidation (Plan 41 P0 #5, #6, #7, #9, #10)
  subscribeFileChanges((event) => {
    notifyFileChange(event.connectionId, event.path);
  });

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
    fetchPanelEntries,
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
    notifyFileChange,
    saveState,
  };
});
