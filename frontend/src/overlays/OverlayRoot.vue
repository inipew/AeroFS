<template>
  <div class="overlay-root">
    <!-- Context Menu Host -->
    <ContextMenu
      @openArchiveDialog="handleContextMenuArchive"
      @openCreateShareDialog="handleContextMenuCreateShare"
      @openPropertiesDialog="handleContextMenuProperties"
      @openArchiveViewer="handleContextMenuArchiveViewer"
    />

    <!-- User Overlay Host: Canonical overlayStore Dialogs -->
    <CreateDialog v-if="overlayStore.current?.type === 'create' || uiStore.isCreateOpen" />
    <RenameDialog v-if="overlayStore.current?.type === 'rename' || uiStore.isRenameOpen" />
    <DeleteDialog v-if="overlayStore.current?.type === 'delete' || uiStore.isDeleteOpen" />
    <UploadDialog v-if="overlayStore.current?.type === 'upload' || uiStore.isUploadOpen" />

    <ConnectionDialog
      v-if="isConnectionOpen"
      :model-value="isConnectionOpen"
      :connection-to-edit="connectionDialogProps"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <DeleteConnectionDialog
      v-if="isDeleteConnectionOpen"
      :model-value="isDeleteConnectionOpen"
      :connection="deleteConnectionProps"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <ArchiveDialog
      v-if="isArchiveOpen"
      :model-value="isArchiveOpen"
      :connection-id="archiveProps.connectionId"
      :base-path="archiveProps.basePath"
      :selected-paths="archiveProps.selectedPaths"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <ArchiveViewerModal
      v-if="isArchiveViewerOpen"
      :model-value="isArchiveViewerOpen"
      :connection-id="archiveViewerProps.connectionId"
      :archive-path="archiveViewerProps.archivePath"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <SearchModal
      v-if="isSearchOpen"
      :model-value="isSearchOpen"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <SettingsModal
      v-if="isSettingsOpen"
      :model-value="isSettingsOpen"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <SharesModal
      v-if="isSharesOpen"
      :model-value="isSharesOpen"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <TrashModal
      v-if="isTrashOpen"
      :model-value="isTrashOpen"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <StarredModal
      v-if="isStarredOpen"
      :model-value="isStarredOpen"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <RecentModal
      v-if="isRecentOpen"
      :model-value="isRecentOpen"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <PropertiesModal
      v-if="isPropertiesOpen"
      :model-value="isPropertiesOpen"
      :connection-id="propertiesProps.connectionId"
      :path="propertiesProps.path"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <CreateShareModal
      v-if="isCreateShareOpen"
      :model-value="isCreateShareOpen"
      :connection-id="createShareProps.connectionId"
      :path="createShareProps.path"
      @update:model-value="val => { if (!val) overlayStore.close(); }"
    />

    <SyncDialog v-if="overlayStore.current?.type === 'sync' || uiStore.isSyncOpen" />
    <CodeEditorModal v-if="overlayStore.current?.type === 'editor' || uiStore.isEditorOpen" />
    <MediaViewerModal v-if="overlayStore.current?.type === 'media-viewer' || uiStore.isMediaViewerOpen" />

    <CommandPaletteModal
      @open-settings="overlayStore.open({ type: 'settings' })"
      @open-connection-dialog="overlayStore.open({ type: 'connection' })"
      @open-search-dialog="overlayStore.open({ type: 'search' })"
    />

    <!-- Workflow Overlay Host: ConflictDialog is domain-owned by transferStore -->
    <ConflictDialog v-if="transferStore.conflictState?.isOpen" />
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent } from 'vue';
import { useOverlayStore } from './overlayStore';
import { useUiStore } from '../stores/uiStore';
import { useTransferStore } from '../stores/transferStore';
import { useWorkspaceStore } from '../stores/workspaceStore';
import type { Connection } from '../types/connection';

// ContextMenu is loaded synchronously for immediate pointer response
import ContextMenu from '../components/browser/ContextMenu.vue';

// Lazy-loaded dialogs and modals
const CreateDialog = defineAsyncComponent(() => import('../components/dialogs/CreateDialog.vue'));
const RenameDialog = defineAsyncComponent(() => import('../components/dialogs/RenameDialog.vue'));
const DeleteDialog = defineAsyncComponent(() => import('../components/dialogs/DeleteDialog.vue'));
const UploadDialog = defineAsyncComponent(() => import('../components/dialogs/UploadDialog.vue'));
const ConnectionDialog = defineAsyncComponent(() => import('../components/dialogs/ConnectionDialog.vue'));
const DeleteConnectionDialog = defineAsyncComponent(() => import('../components/dialogs/DeleteConnectionDialog.vue'));
const ArchiveDialog = defineAsyncComponent(() => import('../components/dialogs/ArchiveDialog.vue'));
const ArchiveViewerModal = defineAsyncComponent(() => import('../components/dialogs/ArchiveViewerModal.vue'));
const SearchModal = defineAsyncComponent(() => import('../components/dialogs/SearchModal.vue'));
const SettingsModal = defineAsyncComponent(() => import('../components/dialogs/SettingsModal.vue'));
const SharesModal = defineAsyncComponent(() => import('../components/dialogs/SharesModal.vue'));
const TrashModal = defineAsyncComponent(() => import('../components/dialogs/TrashModal.vue'));
const StarredModal = defineAsyncComponent(() => import('../components/dialogs/StarredModal.vue'));
const RecentModal = defineAsyncComponent(() => import('../components/dialogs/RecentModal.vue'));
const PropertiesModal = defineAsyncComponent(() => import('../components/dialogs/PropertiesModal.vue'));
const CreateShareModal = defineAsyncComponent(() => import('../components/dialogs/CreateShareModal.vue'));
const SyncDialog = defineAsyncComponent(() => import('../components/dialogs/SyncDialog.vue'));
const CodeEditorModal = defineAsyncComponent(() => import('../components/editor/CodeEditorModal.vue'));
const MediaViewerModal = defineAsyncComponent(() => import('../components/viewer/MediaViewerModal.vue'));
const CommandPaletteModal = defineAsyncComponent(() => import('../components/dialogs/CommandPaletteModal.vue'));
const ConflictDialog = defineAsyncComponent(() => import('../components/dialogs/ConflictDialog.vue'));

const overlayStore = useOverlayStore();
const uiStore = useUiStore();
const transferStore = useTransferStore();
const workspaceStore = useWorkspaceStore();

// Computed states for v-model bindings
const isConnectionOpen = computed(() => overlayStore.current?.type === 'connection');
const connectionDialogProps = computed<Connection | null | undefined>(() => {
  if (overlayStore.current?.type === 'connection') {
    return overlayStore.current.connectionToEdit;
  }
  return null;
});

const isDeleteConnectionOpen = computed(() => overlayStore.current?.type === 'delete-connection');
const deleteConnectionProps = computed<Connection | null>(() => {
  if (overlayStore.current?.type === 'delete-connection') {
    return overlayStore.current.connection;
  }
  return null;
});

const isArchiveOpen = computed(() => overlayStore.current?.type === 'archive');
const archiveProps = computed(() => {
  if (overlayStore.current?.type === 'archive') {
    return {
      connectionId: overlayStore.current.connectionId,
      basePath: overlayStore.current.basePath,
      selectedPaths: overlayStore.current.selectedPaths,
    };
  }
  const activePanel = workspaceStore.activePanel;
  return {
    connectionId: activePanel.connectionId,
    basePath: activePanel.path,
    selectedPaths: activePanel.selectedEntries,
  };
});

const isArchiveViewerOpen = computed(() => overlayStore.current?.type === 'archive-viewer');
const archiveViewerProps = computed(() => {
  if (overlayStore.current?.type === 'archive-viewer') {
    return {
      connectionId: overlayStore.current.connectionId,
      archivePath: overlayStore.current.archivePath,
    };
  }
  return { connectionId: 'local', archivePath: '' };
});

const isSearchOpen = computed(() => overlayStore.current?.type === 'search');
const isSettingsOpen = computed(() => overlayStore.current?.type === 'settings');
const isSharesOpen = computed(() => overlayStore.current?.type === 'shares');
const isTrashOpen = computed(() => overlayStore.current?.type === 'trash');
const isStarredOpen = computed(() => overlayStore.current?.type === 'starred');
const isRecentOpen = computed(() => overlayStore.current?.type === 'recent');

const isPropertiesOpen = computed(() => overlayStore.current?.type === 'properties');
const propertiesProps = computed(() => {
  if (overlayStore.current?.type === 'properties') {
    return {
      connectionId: overlayStore.current.connectionId,
      path: overlayStore.current.path,
    };
  }
  return { connectionId: 'local', path: '/' };
});

const isCreateShareOpen = computed(() => overlayStore.current?.type === 'create-share');
const createShareProps = computed(() => {
  if (overlayStore.current?.type === 'create-share') {
    return {
      connectionId: overlayStore.current.connectionId,
      path: overlayStore.current.path,
    };
  }
  return { connectionId: 'local', path: '/' };
});

// Handlers from ContextMenu
function handleContextMenuArchive(selectedPaths?: string[]) {
  const activePanel = workspaceStore.activePanel;
  overlayStore.open({
    type: 'archive',
    connectionId: activePanel.connectionId,
    basePath: activePanel.path,
    selectedPaths: selectedPaths && selectedPaths.length > 0 ? selectedPaths : activePanel.selectedEntries,
  });
}

function handleContextMenuCreateShare(payload: { connectionId: string; path: string }) {
  overlayStore.open({ type: 'create-share', connectionId: payload.connectionId, path: payload.path });
}

function handleContextMenuProperties(payload: { connectionId: string; path: string }) {
  overlayStore.open({ type: 'properties', connectionId: payload.connectionId, path: payload.path });
}

function handleContextMenuArchiveViewer(payload: { connectionId: string; path: string }) {
  overlayStore.open({ type: 'archive-viewer', connectionId: payload.connectionId, archivePath: payload.path });
}
</script>
