import { defineStore } from 'pinia';
import { ref } from 'vue';
import { listFilesApi, copyEntryApi, renameEntryApi } from '../api/files';
import type { FileEntry } from '../types/vfs';

export interface PanelState {
  id: 'left' | 'right';
  connectionId: string;
  path: string;
  entries: FileEntry[];
  selectedEntries: string[];
  viewMode: 'list' | 'grid';
  showHidden: boolean;
  sortField: string;
  sortOrder: 'asc' | 'desc';
  filterType: string;
  searchQuery: string;
  loading: boolean;
  error: string | null;
  history: string[];
  historyIndex: number;
}

export const useWorkspaceStore = defineStore('workspace', () => {
  // Load persistent state from localStorage
  const savedDualPane = localStorage.getItem('fb:isDualPane') === 'true';
  const isDualPane = ref<boolean>(savedDualPane);
  const activePanelId = ref<'left' | 'right'>('left');

  const leftConn = localStorage.getItem('fb:left:connectionId') || 'local';
  const leftPath = localStorage.getItem('fb:left:path') || '/';
  const leftView = (localStorage.getItem('fb:left:viewMode') as 'list' | 'grid') || 'grid';
  const leftHidden = localStorage.getItem('fb:left:showHidden') === 'true';

  const leftPanel = ref<PanelState>({
    id: 'left',
    connectionId: leftConn,
    path: leftPath,
    entries: [],
    selectedEntries: [],
    viewMode: leftView,
    showHidden: leftHidden,
    sortField: 'name',
    sortOrder: 'asc',
    filterType: 'all',
    searchQuery: '',
    loading: false,
    error: null,
    history: [leftPath],
    historyIndex: 0,
  });

  const rightConn = localStorage.getItem('fb:right:connectionId') || 'local';
  const rightPath = localStorage.getItem('fb:right:path') || '/';
  const rightView = (localStorage.getItem('fb:right:viewMode') as 'list' | 'grid') || 'grid';
  const rightHidden = localStorage.getItem('fb:right:showHidden') === 'true';

  const rightPanel = ref<PanelState>({
    id: 'right',
    connectionId: rightConn,
    path: rightPath,
    entries: [],
    selectedEntries: [],
    viewMode: rightView,
    showHidden: rightHidden,
    sortField: 'name',
    sortOrder: 'asc',
    filterType: 'all',
    searchQuery: '',
    loading: false,
    error: null,
    history: [rightPath],
    historyIndex: 0,
  });

  function saveState() {
    localStorage.setItem('fb:isDualPane', isDualPane.value ? 'true' : 'false');
    localStorage.setItem('fb:left:connectionId', leftPanel.value.connectionId);
    localStorage.setItem('fb:left:path', leftPanel.value.path);
    localStorage.setItem('fb:left:viewMode', leftPanel.value.viewMode);
    localStorage.setItem('fb:left:showHidden', leftPanel.value.showHidden ? 'true' : 'false');

    localStorage.setItem('fb:right:connectionId', rightPanel.value.connectionId);
    localStorage.setItem('fb:right:path', rightPanel.value.path);
    localStorage.setItem('fb:right:viewMode', rightPanel.value.viewMode);
    localStorage.setItem('fb:right:showHidden', rightPanel.value.showHidden ? 'true' : 'false');
  }

  function getPanel(id: 'left' | 'right'): PanelState {
    return id === 'left' ? leftPanel.value : rightPanel.value;
  }

  function setActivePanel(id: 'left' | 'right') {
    activePanelId.value = id;
  }

  function setDualPane(enable: boolean) {
    isDualPane.value = enable;
    saveState();
    if (enable && rightPanel.value.entries.length === 0) {
      fetchPanelEntries('right');
    }
  }

  function closePanel(panelId: 'left' | 'right') {
    if (panelId === 'right') {
      isDualPane.value = false;
      activePanelId.value = 'left';
    } else {
      // If closing left, transfer right panel's location to left panel
      leftPanel.value.connectionId = rightPanel.value.connectionId;
      leftPanel.value.path = rightPanel.value.path;
      leftPanel.value.entries = [...rightPanel.value.entries];
      isDualPane.value = false;
      activePanelId.value = 'left';
    }
    saveState();
  }

  async function fetchPanelEntries(panelId: 'left' | 'right', path?: string) {
    const p = getPanel(panelId);
    if (path) p.path = path;
    p.loading = true;
    p.error = null;

    try {
      const data = await listFilesApi(p.connectionId, {
        path: p.path,
        show_hidden: p.showHidden,
        sort: p.sortField,
        order: p.sortOrder,
      });
      p.entries = data.entries;
      p.path = data.path;
      p.selectedEntries = [];
      saveState();
    } catch (err: any) {
      p.error = err.response?.data?.error?.message || 'Failed to list files';
    } finally {
      p.loading = false;
    }
  }

  async function toggleShowHidden(panelId?: 'left' | 'right') {
    const id = panelId || activePanelId.value;
    const p = getPanel(id);
    p.showHidden = !p.showHidden;
    saveState();
    await fetchPanelEntries(id);
  }

  async function refreshActive() {
    await fetchPanelEntries(activePanelId.value);
  }

  async function refreshAll() {
    await fetchPanelEntries('left');
    if (isDualPane.value) {
      await fetchPanelEntries('right');
    }
  }

  async function navigatePanel(panelId: 'left' | 'right', targetPath: string, addToHistory: boolean = true) {
    const p = getPanel(panelId);
    await fetchPanelEntries(panelId, targetPath);
    if (addToHistory && p.path !== p.history[p.historyIndex]) {
      p.history = p.history.slice(0, p.historyIndex + 1);
      p.history.push(p.path);
      p.historyIndex = p.history.length - 1;
    }
    saveState();
  }

  async function switchPanelConnection(panelId: 'left' | 'right', connectionId: string, basePath: string = '/') {
    const p = getPanel(panelId);
    p.connectionId = connectionId;
    p.path = basePath;
    p.history = [basePath];
    p.historyIndex = 0;
    await fetchPanelEntries(panelId, basePath);
    saveState();
  }

  async function goBackPanel(panelId: 'left' | 'right') {
    const p = getPanel(panelId);
    if (p.historyIndex > 0) {
      p.historyIndex--;
      await navigatePanel(panelId, p.history[p.historyIndex], false);
    }
  }

  async function goForwardPanel(panelId: 'left' | 'right') {
    const p = getPanel(panelId);
    if (p.historyIndex < p.history.length - 1) {
      p.historyIndex++;
      await navigatePanel(panelId, p.history[p.historyIndex], false);
    }
  }

  async function navigateUpPanel(panelId: 'left' | 'right') {
    const p = getPanel(panelId);
    if (p.path === '/' || p.path === '') return;
    const parts = p.path.split('/').filter(Boolean);
    parts.pop();
    const parent = parts.length === 0 ? '/' : `/${parts.join('/')}`;
    await navigatePanel(panelId, parent);
  }

  async function transferBetweenPanels(
    _sourcePanelId: 'left' | 'right',
    destPanelId: 'left' | 'right',
    filePaths: string[],
    isMove: boolean = false
  ) {
    const destPanel = getPanel(destPanelId);

    for (const filePath of filePaths) {
      if (isMove) {
        await renameEntryApi(destPanel.connectionId, filePath, destPanel.path);
      } else {
        await copyEntryApi(destPanel.connectionId, filePath, destPanel.path);
      }
    }

    await refreshAll();
  }

  return {
    isDualPane,
    activePanelId,
    leftPanel,
    rightPanel,
    getPanel,
    setActivePanel,
    setDualPane,
    closePanel,
    fetchPanelEntries,
    toggleShowHidden,
    refreshActive,
    refreshAll,
    navigatePanel,
    switchPanelConnection,
    goBackPanel,
    goForwardPanel,
    navigateUpPanel,
    transferBetweenPanels,
    saveState,
  };
});
