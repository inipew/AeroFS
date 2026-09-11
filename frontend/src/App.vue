<template>
  <div class="h-[100dvh] w-screen overflow-hidden flex flex-col bg-white dark:bg-[#0b0f19] text-gray-800 dark:text-slate-100 font-sans select-none antialiased fixed inset-0">
    <!-- Unauthenticated Login View -->
    <LoginModal v-if="!authStore.isAuthenticated && !authStore.isChecking" />

    <!-- Authenticated Main Application Workspace Surface -->
    <template v-else-if="authStore.isAuthenticated">
      <AppShell />
      <OverlayRoot />
      <ToastHost />
      <TransferDrawerHost />
    </template>

    <!-- Initial App Booting Screen -->
    <div v-else class="h-full w-full flex items-center justify-center bg-white dark:bg-slate-950 text-gray-400 text-xs">
      <div class="flex flex-col items-center space-y-2">
        <div class="animate-spin rounded-full h-8 w-8 border-2 border-blue-600 border-t-transparent"></div>
        <span class="font-medium">Loading FileBrowser...</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, defineAsyncComponent } from 'vue';
import { useAuthStore } from './stores/authStore';
import { useConnectionStore } from './stores/connectionStore';
import { useWorkspaceStore } from './stores/workspaceStore';
import { useTransferStore } from './stores/transferStore';
import { useFileStore } from './stores/fileStore';
import { useUiStore } from './stores/uiStore';
import { usePreferencesStore } from './stores/preferencesStore';
import { useOverlayStore } from './overlays/overlayStore';
import { initializeCommandRegistry, commandRegistry } from './services/commandRegistry';
import { PreviewResolver } from './services/previewResolver';
import type { FileEntry } from './types/vfs';

// Core layout & host components
import AppShell from './app/AppShell.vue';
import OverlayRoot from './overlays/OverlayRoot.vue';
import ToastHost from './feedback/ToastHost.vue';
import TransferDrawerHost from './feedback/TransferDrawerHost.vue';

const LoginModal = defineAsyncComponent(() => import('./components/auth/LoginModal.vue'));

const authStore = useAuthStore();
const connStore = useConnectionStore();
const workspaceStore = useWorkspaceStore();
const transferStore = useTransferStore();
const fileStore = useFileStore();
const uiStore = useUiStore();
const preferencesStore = usePreferencesStore();
const overlayStore = useOverlayStore();

function handleGlobalKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

  // If typing in input or any modal is open, don't trigger file manager workspace shortcuts
  const isAnyModalOpen =
    overlayStore.current !== null ||
    uiStore.isEditorOpen ||
    uiStore.isMediaViewerOpen;

  if (isInput || isAnyModalOpen) return;

  // 1. Ctrl+K / Cmd+K: Universal Command Palette
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault();
    uiStore.toggleCommandPalette();
    return;
  }

  // 1b. Ctrl+F / Cmd+F: Search Dialog
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    if (overlayStore.isOpen('search')) {
      overlayStore.close();
    } else {
      overlayStore.open({ type: 'search' });
    }
    return;
  }

  // 1c. Ctrl+H / Cmd+H: Toggle Hidden files
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'h') {
    e.preventDefault();
    workspaceStore.toggleShowHidden(workspaceStore.activePanelId);
    return;
  }

  const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);

  // 2. Tab: Switch active panel in Dual Pane mode
  if (e.key === 'Tab' && workspaceStore.isDualPane) {
    e.preventDefault();
    workspaceStore.activePanelId = workspaceStore.activePanelId === 'left' ? 'right' : 'left';
    return;
  }

  // 3. Swap Panels (Alt+S or Ctrl+Shift+Tab)
  if ((e.altKey && e.key.toLowerCase() === 's') || ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'Tab')) {
    e.preventDefault();
    if (workspaceStore.isDualPane) {
      workspaceStore.swapPanels();
      uiStore.showToast('Panels swapped', 'info');
    }
    return;
  }

  // 4. F2: Rename selected item
  if (e.key === 'F2') {
    e.preventDefault();
    if (activeP.selectedEntries.length === 1) {
      const selectedEntry = activeP.entries.find((entry: FileEntry) => entry.path === activeP.selectedEntries[0]);
      if (selectedEntry) {
        fileStore.currentConnectionId = activeP.connectionId;
        fileStore.currentPath = activeP.path;
        uiStore.openRename(selectedEntry);
      }
    }
    return;
  }

  // 5. Delete: Delete selected items
  if (e.key === 'Delete') {
    e.preventDefault();
    if (activeP.selectedEntries.length > 0) {
      fileStore.currentConnectionId = activeP.connectionId;
      fileStore.currentPath = activeP.path;
      uiStore.openDelete(activeP.selectedEntries);
    }
    return;
  }

  // 6. Ctrl+A: Select All
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
    e.preventDefault();
    activeP.selectedEntries = activeP.entries.map((entry: FileEntry) => entry.path);
    return;
  }

  // 7. Ctrl+Z / Cmd+Z: Reversible Undo
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z' && !e.shiftKey) {
    e.preventDefault();
    commandRegistry.execute('edit.undo');
    return;
  }

  // 8. Ctrl+Y / Cmd+Y / Ctrl+Shift+Z: Reversible Redo
  if (
    ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') ||
    ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'z')
  ) {
    e.preventDefault();
    commandRegistry.execute('edit.redo');
    return;
  }

  // 9. Escape: Deselect all items & close context menu
  if (e.key === 'Escape') {
    if (activeP.selectedEntries.length > 0) {
      e.preventDefault();
      activeP.selectedEntries = [];
      uiStore.closeContextMenu();
      return;
    }
  }

  // 10. Desktop-Grade Keyboard Navigation (Arrows, Shift+Arrows, Enter, Space, Home, End)
  if (['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' '].includes(e.key)) {
    const visibleEntries = activeP.entries.filter(
      (ent: FileEntry) => activeP.showHidden || (!ent.is_hidden && !ent.name.startsWith('.'))
    );
    if (visibleEntries.length === 0) return;

    const lastSelectedPath = activeP.selectedEntries[activeP.selectedEntries.length - 1];
    const currentIndex = visibleEntries.findIndex((ent: FileEntry) => ent.path === lastSelectedPath);

    if (e.key === 'Home') {
      e.preventDefault();
      activeP.selectedEntries = [visibleEntries[0].path];
      return;
    }
    if (e.key === 'End') {
      e.preventDefault();
      activeP.selectedEntries = [visibleEntries[visibleEntries.length - 1].path];
      return;
    }
    if (e.key === ' ') {
      e.preventDefault();
      if (currentIndex !== -1) {
        const path = visibleEntries[currentIndex].path;
        if (activeP.selectedEntries.includes(path)) {
          activeP.selectedEntries = activeP.selectedEntries.filter((p: string) => p !== path);
        } else {
          activeP.selectedEntries.push(path);
        }
      }
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      if (currentIndex !== -1) {
        const entry = visibleEntries[currentIndex];
        if (entry.kind === 'directory') {
          workspaceStore.navigatePanel(workspaceStore.activePanelId, entry.path);
        } else {
          const resolution = PreviewResolver.resolve(
            entry,
            activeP.connectionId,
            visibleEntries.filter((ent: FileEntry) => ent.kind === 'file')
          );
          resolution.open();
        }
      }
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      const nextIndex = currentIndex < visibleEntries.length - 1 ? currentIndex + 1 : 0;
      const nextEntry = visibleEntries[nextIndex];
      if (e.shiftKey) {
        activeP.selectedEntries = Array.from(new Set([...activeP.selectedEntries, nextEntry.path]));
      } else {
        activeP.selectedEntries = [nextEntry.path];
      }
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      const prevIndex = currentIndex > 0 ? currentIndex - 1 : visibleEntries.length - 1;
      const prevEntry = visibleEntries[prevIndex];
      if (e.shiftKey) {
        activeP.selectedEntries = Array.from(new Set([...activeP.selectedEntries, prevEntry.path]));
      } else {
        activeP.selectedEntries = [prevEntry.path];
      }
      return;
    }
  }
}

onMounted(async () => {
  initializeCommandRegistry();
  window.addEventListener('keydown', handleGlobalKeydown);
  await authStore.checkAuth();
  if (authStore.isAuthenticated) {
    await preferencesStore.fetchPreferences();
    await connStore.fetchConnections().catch(() => undefined);
    await workspaceStore.refreshPanel('left');
    if (workspaceStore.isDualPane) {
      await workspaceStore.refreshPanel('right');
    }
    transferStore.connectWs();
    await transferStore.fetchJobs();
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
});
</script>
