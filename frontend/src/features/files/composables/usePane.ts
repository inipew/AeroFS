import { computed } from 'vue';
import { useWorkspaceStore } from '../../../stores/workspaceStore';
import type { PanelId } from '../../../types/workspace';

export function usePane(panelId: PanelId) {
  const workspaceStore = useWorkspaceStore();

  const panel = computed(() => workspaceStore.getPanel(panelId));
  const isActive = computed(() => workspaceStore.activePanelId === panelId);

  function setActive() {
    workspaceStore.setActivePanel(panelId);
  }

  async function navigate(path: string) {
    await workspaceStore.navigateTo(panelId, path);
  }

  async function goBack() {
    workspaceStore.goBack(panelId);
  }

  async function goForward() {
    workspaceStore.goForward(panelId);
  }

  async function navigateUp() {
    workspaceStore.navigateUp(panelId);
  }

  async function refresh() {
    await workspaceStore.refreshPanel(panelId);
  }

  function clearSelection() {
    panel.value.selectedEntries = [];
  }

  function selectAll(paths: string[]) {
    panel.value.selectedEntries = [...paths];
  }

  function toggleSelect(path: string, multi: boolean = false) {
    if (!multi) {
      panel.value.selectedEntries = panel.value.selectedEntries.includes(path) ? [] : [path];
    } else {
      if (panel.value.selectedEntries.includes(path)) {
        panel.value.selectedEntries = panel.value.selectedEntries.filter((p) => p !== path);
      } else {
        panel.value.selectedEntries.push(path);
      }
    }
  }

  function selectEntries(paths: string[]) {
    panel.value.selectedEntries = [...paths];
  }

  function setViewMode(mode: 'list' | 'grid') {
    panel.value.viewMode = mode;
    workspaceStore.saveState();
  }

  function toggleHidden() {
    panel.value.showHidden = !panel.value.showHidden;
    workspaceStore.saveState();
  }

  function setSort(field: 'name' | 'size' | 'modified' | 'kind', order?: 'asc' | 'desc') {
    if (order) {
      panel.value.sortField = field;
      panel.value.sortOrder = order;
    } else {
      if (panel.value.sortField === field) {
        panel.value.sortOrder = panel.value.sortOrder === 'asc' ? 'desc' : 'asc';
      } else {
        panel.value.sortField = field;
        panel.value.sortOrder = 'asc';
      }
    }
    workspaceStore.saveState();
  }

  return {
    panel,
    isActive,
    setActive,
    navigate,
    goBack,
    goForward,
    navigateUp,
    refresh,
    clearSelection,
    selectAll,
    toggleSelect,
    selectEntries,
    setViewMode,
    toggleHidden,
    setSort,
  };
}
