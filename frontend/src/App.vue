<template>
  <div class="h-screen w-screen flex flex-col bg-white dark:bg-[#0b0f19] text-gray-800 dark:text-slate-100 font-sans antialiased overflow-hidden select-none">
    <!-- Unauthenticated View: Fullscreen Auth Modal -->
    <LoginModal v-if="!authStore.isAuthenticated && !authStore.isChecking" />

    <!-- Authenticated View: AeroFS Core Interface -->
    <div v-else-if="authStore.isAuthenticated" class="flex h-full w-full overflow-hidden">
      <!-- Full-Height Sidebar Navigation Drawer -->
      <AppSidebar
        @openConnectionDialog="isConnDialogOpen = true"
        @openSharesDialog="isSharesDialogOpen = true"
        @openTrashDialog="isTrashDialogOpen = true"
        @openStarredDialog="isStarredDialogOpen = true"
        @openSettingsDialog="isSettingsDialogOpen = true"
        @showRecent="handleRecentView"
      />

      <!-- Right Area: Universal Header + Dynamic Workspace Panels + Mobile Bottom Bar -->
      <div class="flex-1 flex flex-col h-full min-w-0 overflow-hidden">
        <!-- Universal App Header (Breadcrumb, Search, Sort & Filter, New Button) -->
        <AppHeader
          @openSearchDialog="isSearchDialogOpen = true"
          @openConnectionDialog="isConnDialogOpen = true"
          @openAuditLogDialog="isSettingsDialogOpen = true"
        />

        <!-- Mobile Dual-Pane Tab Switcher (When Dual Pane is Enabled on Mobile) -->
        <div
          v-if="uiStore.isMobile && workspaceStore.isDualPane"
          class="flex items-center p-1.5 bg-gray-100 dark:bg-slate-900 border-b border-gray-200 dark:border-slate-800 text-xs font-semibold select-none shrink-0"
        >
          <button
            @click="workspaceStore.setActivePanel('left')"
            :class="[
              'flex-1 py-1.5 px-3 rounded-xl flex items-center justify-center space-x-1.5 transition truncate cursor-pointer',
              workspaceStore.activePanelId === 'left'
                ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-xs'
                : 'text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
            ]"
          >
            <span>📁 Panel 1</span>
            <span class="text-[10px] font-mono opacity-70 truncate max-w-[100px]">({{ workspaceStore.leftPanel.path }})</span>
          </button>

          <button
            @click="workspaceStore.setActivePanel('right')"
            :class="[
              'flex-1 py-1.5 px-3 rounded-xl flex items-center justify-center space-x-1.5 transition truncate cursor-pointer',
              workspaceStore.activePanelId === 'right'
                ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-xs'
                : 'text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
            ]"
          >
            <span>💾 Panel 2</span>
            <span class="text-[10px] font-mono opacity-70 truncate max-w-[100px]">({{ workspaceStore.rightPanel.path }})</span>
          </button>
        </div>

        <!-- Dynamic Workspace Shell (Continuous Surface) -->
        <main
          ref="mainContainerRef"
          class="flex-1 flex overflow-hidden min-w-0 bg-white dark:bg-[#0b0f19] p-0"
        >
          <!-- MOBILE VIEW: 100% Full-Width Active Panel -->
          <div v-if="uiStore.isMobile" class="w-full h-full flex flex-col min-w-0">
            <FilePanel :panelId="workspaceStore.activePanelId" @open-archive-viewer="handleOpenArchiveViewer" />
          </div>

          <!-- DESKTOP VIEW: Continuous Workspace Surface (Single or Split) -->
          <template v-else>
            <!-- Left Panel -->
            <div
              :style="{
                width: workspaceStore.isDualPane ? `calc(${workspaceStore.splitRatio * 100}% - 3px)` : '100%'
              }"
              class="h-full flex flex-col min-w-[200px]"
            >
              <FilePanel panelId="left" @open-archive-viewer="handleOpenArchiveViewer" />
            </div>

            <!-- Draggable Split Divider (Continuous 1px seam with subtle hover handle) -->
            <div
              v-if="workspaceStore.isDualPane"
              @mousedown="startSplitResize"
              class="w-1.5 relative flex items-center justify-center cursor-col-resize hover:bg-blue-500/20 active:bg-blue-500/30 group select-none transition shrink-0 bg-gray-200 dark:border-slate-800 bg-gray-100 dark:bg-[#070b14]"
              title="Drag to resize panels"
            >
              <div class="w-0.5 h-10 bg-gray-300 dark:bg-slate-700 group-hover:bg-blue-500 rounded-full transition-colors"></div>
            </div>

            <!-- Right Panel (when Dual Pane is enabled) -->
            <div
              v-if="workspaceStore.isDualPane"
              :style="{
                width: `calc(${(1 - workspaceStore.splitRatio) * 100}% - 3px)`
              }"
              class="h-full flex flex-col min-w-[200px]"
            >
              <FilePanel panelId="right" @open-archive-viewer="handleOpenArchiveViewer" />
            </div>
          </template>
        </main>

        <!-- Mobile Bottom Navigation Bar (Thumb Zone) -->
        <nav
          v-if="uiStore.isMobile"
          class="h-14 bg-white/95 dark:bg-[#090d16]/95 backdrop-blur-md border-t border-gray-200 dark:border-slate-800 px-4 flex items-center justify-around text-gray-500 dark:text-slate-400 text-[10px] font-semibold shrink-0 z-20 pb-safe select-none"
        >
          <button
            @click="workspaceStore.setActivePanel('left')"
            :class="[
              'flex flex-col items-center space-y-1 transition cursor-pointer',
              workspaceStore.activePanelId === 'left' ? 'text-blue-600 dark:text-blue-400 font-bold' : ''
            ]"
          >
            <FbIcon name="folder" size="18px" />
            <span>Files</span>
          </button>

          <button
            @click="uiStore.isMobileSidebarOpen = true"
            class="flex flex-col items-center space-y-1 hover:text-blue-600 dark:hover:text-blue-400 transition cursor-pointer"
          >
            <FbIcon name="share" size="18px" />
            <span>Storage</span>
          </button>

          <button
            @click="transferStore.isDrawerOpen = !transferStore.isDrawerOpen"
            class="flex flex-col items-center space-y-1 hover:text-blue-600 dark:hover:text-blue-400 transition cursor-pointer relative"
          >
            <FbIcon name="refresh" size="18px" />
            <span
              v-if="transferStore.activeCount > 0"
              class="absolute -top-1 right-2 w-2 h-2 rounded-full bg-blue-600 animate-pulse"
            ></span>
            <span>Transfers</span>
          </button>

          <button
            @click="isSettingsDialogOpen = true"
            class="flex flex-col items-center space-y-1 hover:text-blue-600 dark:hover:text-blue-400 transition cursor-pointer"
          >
            <FbIcon name="settings" size="18px" />
            <span>Settings</span>
          </button>
        </nav>
      </div>

      <!-- Context Menu -->
      <ContextMenu
        @openArchiveDialog="handleOpenArchive"
        @openCreateShareDialog="handleOpenCreateShare"
        @openPropertiesDialog="handleOpenProperties"
        @openArchiveViewer="handleOpenArchiveViewer"
      />

      <!-- Dialogs & Modals -->
      <CreateDialog />
      <RenameDialog />
      <DeleteDialog />
      <UploadDialog />
      <ConnectionDialog v-model="isConnDialogOpen" />
      <ArchiveDialog
        v-model="isArchiveDialogOpen"
        :connectionId="fileStore.currentConnectionId"
        :basePath="fileStore.currentPath"
        :selectedPaths="archiveSelectedPaths"
      />
      <ArchiveViewerModal
        v-model="isArchiveViewerOpen"
        :connectionId="archiveViewerConnectionId"
        :archivePath="archiveViewerPath"
      />
      <SearchModal v-model="isSearchDialogOpen" />
      <SettingsModal v-model="isSettingsDialogOpen" />
      <SharesModal v-model="isSharesDialogOpen" />
      <TrashModal v-model="isTrashDialogOpen" />
      <StarredModal v-model="isStarredDialogOpen" />
      <PropertiesModal
        v-model="isPropertiesDialogOpen"
        :connectionId="propsTargetConnection"
        :path="propsTargetPath"
      />
      <CreateShareModal
        v-model="isCreateShareDialogOpen"
        :connectionId="shareTargetConnection"
        :path="shareTargetPath"
      />
      <ConflictDialog />
      <CodeEditorModal />
      <MediaViewerModal />
      <CommandPaletteModal
        @open-settings="isSettingsDialogOpen = true"
        @open-connection-dialog="isConnDialogOpen = true"
        @open-search-dialog="isSearchDialogOpen = true"
      />

      <!-- Toast Notifications Container -->
      <div class="fixed bottom-5 right-5 z-[9999] flex flex-col space-y-2 pointer-events-none max-w-sm w-full">
        <TransitionGroup
          enter-active-class="transform ease-out duration-300 transition"
          enter-from-class="translate-y-2 opacity-0 sm:translate-y-0 sm:translate-x-2"
          enter-to-class="translate-y-0 opacity-100 sm:translate-x-0"
          leave-active-class="transition ease-in duration-100"
          leave-from-class="opacity-100"
          leave-to-class="opacity-0"
        >
          <div
            v-for="toast in uiStore.toasts"
            :key="toast.id"
            :class="[
              'pointer-events-auto p-3.5 rounded-2xl shadow-xl border flex items-center space-x-3 text-xs font-semibold backdrop-blur-md transition-all',
              toast.type === 'success' ? 'bg-emerald-950/90 border-emerald-700/60 text-emerald-200' : '',
              toast.type === 'error' ? 'bg-rose-950/90 border-rose-700/60 text-rose-200' : '',
              toast.type === 'warning' ? 'bg-amber-950/90 border-amber-700/60 text-amber-200' : '',
              toast.type === 'info' ? 'bg-slate-900/90 border-slate-700/60 text-slate-200' : ''
            ]"
          >
            <span class="text-base shrink-0">
              {{ toast.type === 'success' ? '✅' : (toast.type === 'error' ? '❌' : (toast.type === 'warning' ? '⚠️' : 'ℹ️')) }}
            </span>
            <span class="flex-1 leading-snug">{{ toast.message }}</span>
          </div>
        </TransitionGroup>
      </div>

      <!-- Floating Transfer Engine Manager -->
      <TransferDrawer />
    </div>

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
import { ref, onMounted, onUnmounted } from 'vue';
import FbIcon from './components/common/FbIcon.vue';
import { useAuthStore } from './stores/authStore';
import { useConnectionStore } from './stores/connectionStore';
import { useWorkspaceStore } from './stores/workspaceStore';
import { useTransferStore } from './stores/transferStore';
import { useFileStore } from './stores/fileStore';
import { useUiStore } from './stores/uiStore';
import { initializeCommandRegistry, commandRegistry } from './services/commandRegistry';
import { PreviewResolver } from './services/previewResolver';
import type { FileEntry } from './types/vfs';

import AppHeader from './components/layout/AppHeader.vue';
import AppSidebar from './components/layout/AppSidebar.vue';
import FilePanel from './components/browser/FilePanel.vue';
import ContextMenu from './components/browser/ContextMenu.vue';
import CommandPaletteModal from './components/dialogs/CommandPaletteModal.vue';
import CreateDialog from './components/dialogs/CreateDialog.vue';
import RenameDialog from './components/dialogs/RenameDialog.vue';
import DeleteDialog from './components/dialogs/DeleteDialog.vue';
import UploadDialog from './components/dialogs/UploadDialog.vue';
import ConnectionDialog from './components/dialogs/ConnectionDialog.vue';
import ArchiveDialog from './components/dialogs/ArchiveDialog.vue';
import ArchiveViewerModal from './components/dialogs/ArchiveViewerModal.vue';
import ConflictDialog from './components/dialogs/ConflictDialog.vue';
import SearchModal from './components/dialogs/SearchModal.vue';
import SettingsModal from './components/dialogs/SettingsModal.vue';
import SharesModal from './components/dialogs/SharesModal.vue';
import TrashModal from './components/dialogs/TrashModal.vue';
import StarredModal from './components/dialogs/StarredModal.vue';
import PropertiesModal from './components/dialogs/PropertiesModal.vue';
import CreateShareModal from './components/dialogs/CreateShareModal.vue';
import CodeEditorModal from './components/editor/CodeEditorModal.vue';
import MediaViewerModal from './components/viewer/MediaViewerModal.vue';
import TransferDrawer from './components/transfer/TransferDrawer.vue';
import LoginModal from './components/auth/LoginModal.vue';

const authStore = useAuthStore();
const connStore = useConnectionStore();
const workspaceStore = useWorkspaceStore();
const transferStore = useTransferStore();
const fileStore = useFileStore();
const uiStore = useUiStore();

const mainContainerRef = ref<HTMLElement | null>(null);
let isResizingSplit = false;

const isConnDialogOpen = ref(false);
const isSearchDialogOpen = ref(false);
const isSettingsDialogOpen = ref(false);
const isSharesDialogOpen = ref(false);
const isTrashDialogOpen = ref(false);
const isStarredDialogOpen = ref(false);
const isPropertiesDialogOpen = ref(false);
const isCreateShareDialogOpen = ref(false);
const isArchiveDialogOpen = ref(false);
const isArchiveViewerOpen = ref(false);
const archiveViewerConnectionId = ref('local');
const archiveViewerPath = ref('');

function handleOpenArchiveViewer(payload: { connectionId: string; path: string }) {
  archiveViewerConnectionId.value = payload.connectionId;
  archiveViewerPath.value = payload.path;
  isArchiveViewerOpen.value = true;
}

const propsTargetConnection = ref('local');
const propsTargetPath = ref('/');
const shareTargetConnection = ref('local');
const shareTargetPath = ref('/');
const archiveSelectedPaths = ref<string[]>([]);

function startSplitResize(e: MouseEvent) {
  e.preventDefault();
  isResizingSplit = true;
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
  window.addEventListener('mousemove', onSplitResizeMove);
  window.addEventListener('mouseup', stopSplitResize);
}

function onSplitResizeMove(e: MouseEvent) {
  if (!isResizingSplit || !mainContainerRef.value) return;
  const rect = mainContainerRef.value.getBoundingClientRect();
  const ratio = (e.clientX - rect.left) / rect.width;
  workspaceStore.setSplitRatio(ratio);
}

function stopSplitResize() {
  if (isResizingSplit) {
    isResizingSplit = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    window.removeEventListener('mousemove', onSplitResizeMove);
    window.removeEventListener('mouseup', stopSplitResize);
  }
}

function handleOpenArchive(paths: string[]) {
  archiveSelectedPaths.value = paths;
  isArchiveDialogOpen.value = true;
}

function handleOpenProperties(payload: { connectionId: string; path: string }) {
  propsTargetConnection.value = payload.connectionId;
  propsTargetPath.value = payload.path;
  isPropertiesDialogOpen.value = true;
}

function handleOpenCreateShare(payload: { connectionId: string; path: string }) {
  shareTargetConnection.value = payload.connectionId;
  shareTargetPath.value = payload.path;
  isCreateShareDialogOpen.value = true;
}

function handleRecentView() {
  const panel = workspaceStore.getPanel(workspaceStore.activePanelId);
  panel.entries.sort((a: FileEntry, b: FileEntry) => {
    const timeA = a.modified_at ? new Date(a.modified_at).getTime() : 0;
    const timeB = b.modified_at ? new Date(b.modified_at).getTime() : 0;
    return timeB - timeA;
  });
  uiStore.showToast('Sorted by most recently modified files', 'info');
}

function handleGlobalKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

  // 1. Ctrl+K / Cmd+K: Universal Command Palette
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault();
    uiStore.toggleCommandPalette();
    return;
  }

  // 1b. Ctrl+F / Cmd+F: Search Dialog
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    isSearchDialogOpen.value = !isSearchDialogOpen.value;
    return;
  }

  // 1c. Ctrl+H / Cmd+H: Toggle Hidden files
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'h') {
    e.preventDefault();
    workspaceStore.toggleShowHidden(workspaceStore.activePanelId);
    return;
  }

  // If typing in input or any modal is open, don't trigger file manager workspace shortcuts
  const isAnyModalOpen =
    uiStore.isEditorOpen ||
    uiStore.isMediaViewerOpen ||
    isConnDialogOpen.value ||
    isArchiveDialogOpen.value ||
    isArchiveViewerOpen.value ||
    isSearchDialogOpen.value ||
    isSettingsDialogOpen.value ||
    isSharesDialogOpen.value ||
    isTrashDialogOpen.value ||
    isStarredDialogOpen.value ||
    isPropertiesDialogOpen.value ||
    isCreateShareDialogOpen.value;

  if (isInput || isAnyModalOpen) return;

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

  // 8. Ctrl+Y / Cmd+Shift+Z: Redo
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
    await connStore.fetchConnections();
    await workspaceStore.fetchPanelEntries('left');
    if (workspaceStore.isDualPane) {
      await workspaceStore.fetchPanelEntries('right');
    }
    transferStore.connectWs();
    await transferStore.fetchJobs();
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
  stopSplitResize();
});
</script>
