<template>
  <div class="h-screen w-screen flex bg-white dark:bg-[#0b0f19] text-gray-900 dark:text-slate-100 font-sans overflow-hidden">
    <!-- Login Screen -->
    <LoginModal v-if="!authStore.isChecking && !authStore.isAuthenticated" />

    <!-- Main File Manager Workspace (Matching the-filebrowser layout) -->
    <template v-else-if="authStore.isAuthenticated">
      <!-- 1. Left Sidebar (Full Height) -->
      <AppSidebar
        @openConnectionDialog="isConnDialogOpen = true"
        @openSettingsDialog="isSettingsDialogOpen = true"
        @openSharesDialog="isSharesDialogOpen = true"
        @openTrashDialog="isTrashDialogOpen = true"
        @openStarredView="isStarredDialogOpen = true"
        @openRecentView="handleRecentView"
      />

      <!-- 2. Right Main Column (Header Toolbar + File Panels) -->
      <div class="flex-1 flex flex-col h-screen overflow-hidden min-w-0 bg-white dark:bg-[#0b0f19]">
        <!-- Top Toolbar with Breadcrumbs & Actions -->
        <AppHeader
          @openConnectionDialog="isConnDialogOpen = true"
          @openSearchDialog="isSearchDialogOpen = true"
          @openAuditLogDialog="isSettingsDialogOpen = true"
        />

        <!-- Main File Explorer View Area -->
        <main
          :class="[
            'flex-1 flex overflow-hidden min-w-0 bg-white dark:bg-[#0b0f19]',
            workspaceStore.isDualPane ? 'p-3 gap-3 bg-gray-100/60 dark:bg-[#060a12]' : 'p-0'
          ]"
        >
          <!-- Left Panel -->
          <FilePanel panelId="left" />

          <!-- Right Panel (when Dual Pane is enabled) -->
          <FilePanel v-if="workspaceStore.isDualPane" panelId="right" />
        </main>
      </div>

      <!-- Context Menu -->
      <ContextMenu
        @openArchiveDialog="handleOpenArchive"
        @openCreateShareDialog="handleOpenCreateShare"
        @openPropertiesDialog="handleOpenProperties"
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
      <CodeEditorModal />
      <MediaViewerModal />

      <!-- Floating Transfer Engine Manager -->
      <TransferDrawer />

      <!-- Toast Notifications -->
      <div class="fixed bottom-4 right-4 z-50 flex flex-col space-y-2 pointer-events-none">
        <div
          v-for="toast in uiStore.toasts"
          :key="toast.id"
          :class="[
            'px-4 py-2.5 rounded-xl shadow-xl text-xs font-medium border pointer-events-auto transition-all animate-in slide-in-from-bottom-2',
            toast.type === 'success' ? 'bg-emerald-50 dark:bg-emerald-950 border-emerald-300 dark:border-emerald-700 text-emerald-800 dark:text-emerald-200' : '',
            toast.type === 'error' ? 'bg-red-50 dark:bg-red-950 border-red-300 dark:border-red-700 text-red-800 dark:text-red-200' : '',
            toast.type === 'info' ? 'bg-gray-900 text-white border-gray-800' : '',
          ]"
        >
          {{ toast.message }}
        </div>
      </div>
    </template>

    <!-- Initial Loading Screen -->
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
import { useAuthStore } from './stores/authStore';
import { useConnectionStore } from './stores/connectionStore';
import { useWorkspaceStore } from './stores/workspaceStore';
import { useTransferStore } from './stores/transferStore';
import { useFileStore } from './stores/fileStore';
import { useUiStore } from './stores/uiStore';

import AppHeader from './components/layout/AppHeader.vue';
import AppSidebar from './components/layout/AppSidebar.vue';
import FilePanel from './components/browser/FilePanel.vue';
import ContextMenu from './components/browser/ContextMenu.vue';
import CreateDialog from './components/dialogs/CreateDialog.vue';
import RenameDialog from './components/dialogs/RenameDialog.vue';
import DeleteDialog from './components/dialogs/DeleteDialog.vue';
import UploadDialog from './components/dialogs/UploadDialog.vue';
import ConnectionDialog from './components/dialogs/ConnectionDialog.vue';
import ArchiveDialog from './components/dialogs/ArchiveDialog.vue';
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

const isConnDialogOpen = ref(false);
const isSearchDialogOpen = ref(false);
const isSettingsDialogOpen = ref(false);
const isSharesDialogOpen = ref(false);
const isTrashDialogOpen = ref(false);
const isStarredDialogOpen = ref(false);
const isPropertiesDialogOpen = ref(false);
const isCreateShareDialogOpen = ref(false);
const isArchiveDialogOpen = ref(false);

const propsTargetConnection = ref('local');
const propsTargetPath = ref('/');
const shareTargetConnection = ref('local');
const shareTargetPath = ref('/');
const archiveSelectedPaths = ref<string[]>([]);

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
  panel.entries.sort((a, b) => {
    const timeA = a.modified_at ? new Date(a.modified_at).getTime() : 0;
    const timeB = b.modified_at ? new Date(b.modified_at).getTime() : 0;
    return timeB - timeA;
  });
  uiStore.showToast('Sorted by most recently modified files', 'info');
}

function handleGlobalKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

  // 1. Ctrl+K / Cmd+K: Search
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault();
    isSearchDialogOpen.value = !isSearchDialogOpen.value;
    return;
  }

  // 1b. Ctrl+H / Cmd+H: Toggle Hidden files
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'h') {
    e.preventDefault();
    workspaceStore.toggleShowHidden(workspaceStore.activePanelId);
    return;
  }

  // If inside editor modal or typing in input, don't trigger file manager shortcuts
  if (isInput || uiStore.isEditorOpen) return;

  const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);

  // 2. Tab: Switch active panel in Dual Pane mode
  if (e.key === 'Tab' && workspaceStore.isDualPane) {
    e.preventDefault();
    workspaceStore.activePanelId = workspaceStore.activePanelId === 'left' ? 'right' : 'left';
    return;
  }

  // 3. F2: Rename selected item
  if (e.key === 'F2') {
    e.preventDefault();
    if (activeP.selectedEntries.length === 1) {
      const selectedEntry = activeP.entries.find((entry) => entry.path === activeP.selectedEntries[0]);
      if (selectedEntry) {
        fileStore.currentConnectionId = activeP.connectionId;
        fileStore.currentPath = activeP.path;
        uiStore.openRename(selectedEntry);
      }
    }
    return;
  }

  // 4. Delete: Delete selected items
  if (e.key === 'Delete') {
    e.preventDefault();
    if (activeP.selectedEntries.length > 0) {
      fileStore.currentConnectionId = activeP.connectionId;
      fileStore.currentPath = activeP.path;
      uiStore.openDelete(activeP.selectedEntries);
    }
    return;
  }

  // 5. Ctrl+A: Select All
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
    e.preventDefault();
    activeP.selectedEntries = activeP.entries.map((entry) => entry.path);
    return;
  }
}

onMounted(async () => {
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
});
</script>
