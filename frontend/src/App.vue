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
import { useUiStore } from './stores/uiStore';
import { usePreferencesStore } from './stores/preferencesStore';
import { useOverlayStore } from './overlays/overlayStore';
import { useEditorStore } from './stores/editorStore';
import { initializeCommandRegistry, commandRegistry } from './services/commandRegistry';

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
const uiStore = useUiStore();
const preferencesStore = usePreferencesStore();
const overlayStore = useOverlayStore();
const editorStore = useEditorStore();

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

  // 4. Ctrl+Z / Cmd+Z: Reversible Undo
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z' && !e.shiftKey) {
    e.preventDefault();
    commandRegistry.execute('edit.undo');
    return;
  }

  // 5. Ctrl+Y / Cmd+Y / Ctrl+Shift+Z: Reversible Redo
  if (
    ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') ||
    ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'z')
  ) {
    e.preventDefault();
    commandRegistry.execute('edit.redo');
    return;
  }
}

function handleGlobalDragOver(e: DragEvent) {
  e.preventDefault();
}

function handleGlobalDrop(e: DragEvent) {
  e.preventDefault();
}

function handleBeforeUnload() {
  editorStore.flushSession();
}

onMounted(async () => {
  initializeCommandRegistry();
  window.addEventListener('keydown', handleGlobalKeydown);
  window.addEventListener('dragover', handleGlobalDragOver);
  window.addEventListener('drop', handleGlobalDrop);
  window.addEventListener('beforeunload', handleBeforeUnload);
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
    await editorStore.restoreSession();
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
  window.removeEventListener('dragover', handleGlobalDragOver);
  window.removeEventListener('drop', handleGlobalDrop);
  window.removeEventListener('beforeunload', handleBeforeUnload);
});
</script>
