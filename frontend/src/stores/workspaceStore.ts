import { defineStore } from 'pinia';
import { ref, computed, reactive } from 'vue';
import { listFilesApi } from '../api/files';
import { useTransferStore } from './transferStore';
import { useUiStore } from './uiStore';
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
    loading: false,
    refreshing: false,
    error: null,
    initialized: false,
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

    get loading() { return runtime.loading; },
    set loading(val: boolean) { runtime.loading = val; },

    get error() { return runtime.error; },
    set error(val: string | null) { runtime.error = val; },

    get history() { return navigation.history; },
    set history(val: string[]) { navigation.history = val; },

    get historyIndex() { return navigation.historyIndex; },
    set historyIndex(val: number) { navigation.historyIndex = val; },

    get initialized() { return runtime.initialized; },
    set initialized(val: boolean) { runtime.initialized = val; },
  };

  return reactive(panel) as Panel;
}

export const useWorkspaceStore = defineStore('workspace', () => {
  // Load persistent session from localStorage
  const leftConn = localStorage.getItem('fb:left:connectionId') || 'local';
  const leftPath = localStorage.getItem('fb:left:path') || '/';
  const leftView = (localStorage.getItem('fb:left:viewMode') as 'list' | 'grid') || 'grid';
  const leftHidden = localStorage.getItem('fb:left:showHidden') === 'true';

  const rightConn = localStorage.getItem('fb:right:connectionId') || 'local';
  const rightPath = localStorage.getItem('fb:right:path') || '/';
  const rightView = (localStorage.getItem('fb:right:viewMode') as 'list' | 'grid') || 'grid';
  const rightHidden = localStorage.getItem('fb:right:showHidden') === 'true';

  const leftPanel = ref<Panel>(createPanel('left', leftConn, leftPath));
  leftPanel.value.view.viewMode = leftView;
  leftPanel.value.view.showHidden = leftHidden;

  const rightPanel = ref<Panel>(createPanel('right', rightConn, rightPath));
  rightPanel.value.view.viewMode = rightView;
  rightPanel.value.view.showHidden = rightHidden;

  const layout = ref<WorkspaceLayout>(localStorage.getItem('fb:isDualPane') === 'true' ? 'split' : 'single');
  const activePanelId = ref<PanelId>('left');
  const clipboard = ref<WorkspaceClipboard | null>(null);

  // Request sequencing generation counters to avoid race conditions
  let leftRequestGen = 0;
  let rightRequestGen = 0;

  const isDualPane = computed<boolean>({
    get: () => layout.value === 'split',
    set: (val: boolean) => {
      layout.value = val ? 'split' : 'single';
    },
  });

  function saveState() {
    localStorage.setItem('fb:isDualPane', isDualPane.value ? 'true' : 'false');
    localStorage.setItem('fb:left:connectionId', leftPanel.value.location.connectionId);
    localStorage.setItem('fb:left:path', leftPanel.value.location.path);
    localStorage.setItem('fb:left:viewMode', leftPanel.value.view.viewMode);
    localStorage.setItem('fb:left:showHidden', leftPanel.value.view.showHidden ? 'true' : 'false');

    localStorage.setItem('fb:right:connectionId', rightPanel.value.location.connectionId);
    localStorage.setItem('fb:right:path', rightPanel.value.location.path);
    localStorage.setItem('fb:right:viewMode', rightPanel.value.view.viewMode);
    localStorage.setItem('fb:right:showHidden', rightPanel.value.view.showHidden ? 'true' : 'false');

    const persisted: PersistedWorkspace = {
      version: 1,
      layout: layout.value,
      activePanel: activePanelId.value,
      left: {
        connectionId: leftPanel.value.location.connectionId,
        path: leftPanel.value.location.path,
        viewMode: leftPanel.value.view.viewMode,
        showHidden: leftPanel.value.view.showHidden,
        sortField: leftPanel.value.view.sortField,
        sortOrder: leftPanel.value.view.sortOrder,
      },
      right: {
        connectionId: rightPanel.value.location.connectionId,
        path: rightPanel.value.location.path,
        viewMode: rightPanel.value.view.viewMode,
        showHidden: rightPanel.value.view.showHidden,
        sortField: rightPanel.value.view.sortField,
        sortOrder: rightPanel.value.view.sortOrder,
      },
    };
    localStorage.setItem('fb:workspace_v1', JSON.stringify(persisted));
  }

  function getPanel(id: PanelId): Panel {
    return id === 'left' ? leftPanel.value : rightPanel.value;
  }

  function setActivePanel(id: PanelId) {
    activePanelId.value = id;
  }

  function setDualPane(enable: boolean) {
    layout.value = enable ? 'split' : 'single';
    saveState();
    if (enable && !rightPanel.value.runtime.initialized) {
      fetchPanelEntries('right');
    }
  }

  function clonePanel(source: Panel, targetId: PanelId): Panel {
    const target = createPanel(targetId, source.location.connectionId, source.location.path);
    target.navigation.history = [...source.navigation.history];
    target.navigation.historyIndex = source.navigation.historyIndex;
    target.view.viewMode = source.view.viewMode;
    target.view.showHidden = source.view.showHidden;
    target.view.sortField = source.view.sortField;
    target.view.sortOrder = source.view.sortOrder;
    target.view.filterType = source.view.filterType;
    target.view.searchQuery = source.view.searchQuery;
    target.selection.paths = [...source.selection.paths];
    target.selection.focusedPath = source.selection.focusedPath;
    target.runtime.entries = [...source.runtime.entries];
    target.runtime.initialized = source.runtime.initialized;
    return target;
  }

  function closePanel(panelId: PanelId) {
    if (panelId === 'left') {
      // Promote right panel to single left panel
      leftPanel.value = clonePanel(rightPanel.value, 'left');
    }
    layout.value = 'single';
    activePanelId.value = 'left';
    saveState();
  }

  // --- NAVIGATION ENGINE ---

  async function fetchPanelEntries(panelId: PanelId, path?: string) {
    const p = getPanel(panelId);
    if (path) p.location.path = path;
    p.runtime.loading = true;
    p.runtime.status = p.runtime.initialized ? 'refreshing' : 'loading';
    p.runtime.error = null;

    const currentGen = panelId === 'left' ? ++leftRequestGen : ++rightRequestGen;

    try {
      const data = await listFilesApi(p.location.connectionId, {
        path: p.location.path,
        show_hidden: p.view.showHidden,
        sort: p.view.sortField,
        order: p.view.sortOrder,
      });

      // Discard stale out-of-order response
      if (panelId === 'left' ? currentGen !== leftRequestGen : currentGen !== rightRequestGen) {
        return;
      }

      // Reconcile selection: preserve items that still exist
      const previousSelection = new Set(p.selection.paths);
      p.runtime.entries = data.entries;
      p.location.path = data.path;
      p.selection.paths = data.entries
        .map((e) => e.path)
        .filter((entryPath) => previousSelection.has(entryPath));

      p.runtime.status = 'idle';
      p.runtime.initialized = true;
      saveState();
    } catch (err: any) {
      if (panelId === 'left' ? currentGen !== leftRequestGen : currentGen !== rightRequestGen) {
        return;
      }
      p.runtime.status = 'error';
      p.runtime.error = err.response?.data?.error?.message || 'Failed to list files';
    } finally {
      if (panelId === 'left' ? currentGen === leftRequestGen : currentGen === rightRequestGen) {
        p.runtime.loading = false;
        p.runtime.refreshing = false;
      }
    }
  }

  async function navigateTo(panelId: PanelId, targetPath: string, addToHistory: boolean = true) {
    const p = getPanel(panelId);
    await fetchPanelEntries(panelId, targetPath);

    if (addToHistory && p.location.path !== p.navigation.history[p.navigation.historyIndex]) {
      // Truncate forward history on new navigation
      p.navigation.history = p.navigation.history.slice(0, p.navigation.historyIndex + 1);
      p.navigation.history.push(p.location.path);
      p.navigation.historyIndex = p.navigation.history.length - 1;
    }
    saveState();
  }

  async function navigatePanel(panelId: PanelId, targetPath: string, addToHistory: boolean = true) {
    await navigateTo(panelId, targetPath, addToHistory);
  }

  async function switchPanelConnection(panelId: PanelId, connectionId: string, basePath: string = '/') {
    const p = getPanel(panelId);
    p.location.connectionId = connectionId;
    p.location.path = basePath;
    p.navigation.history = [basePath];
    p.navigation.historyIndex = 0;
    p.runtime.initialized = false;
    await fetchPanelEntries(panelId, basePath);
    saveState();
  }

  async function goBack(panelId: PanelId) {
    const p = getPanel(panelId);
    if (p.navigation.historyIndex > 0) {
      p.navigation.historyIndex--;
      await navigateTo(panelId, p.navigation.history[p.navigation.historyIndex], false);
    }
  }

  async function goForward(panelId: PanelId) {
    const p = getPanel(panelId);
    if (p.navigation.historyIndex < p.navigation.history.length - 1) {
      p.navigation.historyIndex++;
      await navigateTo(panelId, p.navigation.history[p.navigation.historyIndex], false);
    }
  }

  async function navigateUp(panelId: PanelId) {
    const p = getPanel(panelId);
    if (p.location.path === '/' || p.location.path === '') return;
    const parts = p.location.path.split('/').filter(Boolean);
    parts.pop();
    const parent = parts.length === 0 ? '/' : `/${parts.join('/')}`;
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

    try {
      for (const filePath of paths) {
        const fileName = filePath.split('/').pop() || 'file';
        const destPath = targetPanel.location.path === '/'
          ? `/${fileName}`
          : `${targetPanel.location.path}/${fileName}`;

        // Skip pasting into exact same path on same connection
        if (sourceConnectionId === targetPanel.location.connectionId && filePath === destPath) {
          continue;
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

      setTimeout(() => {
        refresh(id);
      }, 1000);
    } catch (err: any) {
      uiStore.showToast(err.response?.data?.error?.message || 'Paste transfer failed', 'error');
    }
  }

  async function transferBetweenPanels(
    _sourcePanelId: PanelId,
    destPanelId: PanelId,
    filePaths: string[],
    isMove: boolean = false
  ) {
    const destPanel = getPanel(destPanelId);
    const transferStore = useTransferStore();

    for (const filePath of filePaths) {
      const fileName = filePath.split('/').pop() || 'file';
      const destPath = destPanel.location.path === '/'
        ? `/${fileName}`
        : `${destPanel.location.path}/${fileName}`;

      await transferStore.submitTransfer(
        `${isMove ? 'Move' : 'Copy'} ${fileName} to ${destPanel.location.path}`,
        isMove ? 'move' : 'copy',
        _sourcePanelId === 'left' ? leftPanel.value.location.connectionId : rightPanel.value.location.connectionId,
        filePath,
        destPanel.location.connectionId,
        destPath
      );
    }

    await refreshAll();
  }

  return {
    layout,
    isDualPane,
    activePanelId,
    leftPanel,
    rightPanel,
    clipboard,
    getPanel,
    setActivePanel,
    setDualPane,
    closePanel,
    fetchPanelEntries,
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
    saveState,
  };
});
