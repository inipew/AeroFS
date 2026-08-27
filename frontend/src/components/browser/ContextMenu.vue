<template>
  <div v-if="uiStore.contextMenu.visible">
    <!-- Mobile Backdrop Overlay -->
    <div
      v-if="uiStore.isMobile"
      @click="uiStore.closeContextMenu"
      class="fixed inset-0 bg-black/60 backdrop-blur-xs z-50 animate-in fade-in duration-150"
    ></div>

    <!-- Menu Container (Desktop Floating Menu vs Mobile Bottom Sheet) -->
    <div
      ref="menuRef"
      :style="uiStore.isMobile ? {} : computedStyle"
      :class="[
        'z-50 bg-white dark:bg-[#0f1422] text-gray-700 dark:text-slate-200 select-none font-sans',
        uiStore.isMobile
          ? 'fixed inset-x-0 bottom-0 rounded-t-3xl border-t border-gray-200 dark:border-slate-800 shadow-2xl p-4 max-h-[85vh] overflow-y-auto pb-safe animate-in slide-in-from-bottom duration-200'
          : 'fixed border border-gray-200 dark:border-slate-700 shadow-2xl rounded-2xl py-1.5 w-56 text-xs animate-in fade-in zoom-in-95 duration-100'
      ]"
      @click.stop
    >
      <!-- Mobile Drag Indicator -->
      <div v-if="uiStore.isMobile" class="w-12 h-1.5 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto mb-3"></div>

      <!-- Mobile Target Item Header Preview -->
      <div
        v-if="uiStore.isMobile && uiStore.contextMenu.item"
        class="flex items-center space-x-3 pb-3 mb-2 border-b border-gray-100 dark:border-slate-800"
      >
        <span class="text-2xl">{{ uiStore.contextMenu.item.kind === 'directory' ? '📁' : '📄' }}</span>
        <div class="truncate flex-1">
          <p class="font-bold text-sm text-gray-900 dark:text-white truncate">
            {{ uiStore.contextMenu.item.name }}
          </p>
          <p class="text-xs text-gray-400 dark:text-slate-500 font-mono truncate">
            {{ uiStore.contextMenu.item.path }}
          </p>
        </div>
        <button
          @click="uiStore.closeContextMenu"
          class="p-1 text-gray-400 hover:text-gray-700 dark:hover:text-white text-base"
        >
          ✕
        </button>
      </div>

      <div v-if="uiStore.contextMenu.item" :class="uiStore.isMobile ? 'space-y-1' : ''">
      <!-- 1. Open / View / Edit in Code Editor -->
      <template v-if="uiStore.contextMenu.item.kind === 'directory'">
        <button
          @click="handleOpen"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
        >
          <span>📁 Open Folder</span>
        </button>

        <button
          @click="handleOpenInOtherPanel"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
        >
          <span>📂 Open in Other Panel</span>
          <span class="text-[10px] opacity-75 font-mono">Ctrl+Enter</span>
        </button>
      </template>

      <template v-else>
        <!-- Edit in Code Editor (Available for all files & dotfiles) -->
        <button
          @click="handleEditInEditor"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
        >
          <span>📝 Edit in Code Editor</span>
        </button>

        <button
          @click="handleOpen"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
        >
          <span>📄 Open / Download</span>
        </button>
      </template>

      <!-- Toggle Star Bookmark -->
      <button
        @click="handleToggleStar"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>{{ isItemStarred ? '⭐ Remove from Starred' : '⭐ Add to Starred' }}</span>
      </button>

      <!-- Share Link -->
      <button
        @click="handleShare"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
      >
        <span>🔗 Share Link...</span>
      </button>

      <!-- Properties / Info -->
      <button
        @click="handleProperties"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>ℹ️ Properties / Permissions</span>
      </button>

      <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

      <!-- Read-Only Badge -->
      <div v-if="!canWrite" class="px-3.5 py-1.5 mb-1 bg-amber-50 dark:bg-amber-950/40 border-b border-amber-200 dark:border-amber-800/50 text-[11px] font-semibold text-amber-700 dark:text-amber-400 flex items-center space-x-1.5 rounded-xl">
        <span>🔒</span>
        <span>Read-Only Storage</span>
      </div>

      <!-- Copy & Cut Operations -->
      <button
        @click="handleCopy"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
      >
        <span>📋 Copy</span>
        <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+C</span>
      </button>

      <button
        v-if="canWrite"
        @click="handleCut"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
      >
        <span>✂️ Cut</span>
        <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+X</span>
      </button>

      <button
        v-if="canWrite && workspaceStore.clipboard"
        @click="handlePaste"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
      >
        <span>📄 Paste</span>
        <span class="text-[10px] opacity-75 font-mono">Ctrl+V</span>
      </button>

      <div v-if="canWrite" class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

      <!-- Extract Archive if .zip or .tar.gz -->
      <button
        v-if="canWrite && isArchive(uiStore.contextMenu.item.name)"
        @click="handleExtract"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl text-amber-500 cursor-pointer"
      >
        <span>📦 Extract Archive Here</span>
      </button>

      <!-- Compress Selected -->
      <button
        v-if="canWrite"
        @click="handleCompress"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>📦 Compress...</span>
      </button>

      <!-- Rename -->
      <button
        v-if="canWrite"
        @click="handleRename"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>✏️ Rename</span>
      </button>

      <!-- Delete -->
      <button
        v-if="canWrite"
        @click="handleDelete"
        class="w-full text-left px-3.5 py-2 hover:bg-red-600 hover:text-white text-red-500 flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>🗑️ Delete</span>
      </button>
    </div>

    <!-- Blank Area Context Menu -->
    <div v-else>
      <div v-if="!canWrite" class="px-3.5 py-1.5 mb-1 bg-amber-50 dark:bg-amber-950/40 border-b border-amber-200 dark:border-amber-800/50 text-[11px] font-semibold text-amber-700 dark:text-amber-400 flex items-center space-x-1.5 rounded-xl">
        <span>🔒</span>
        <span>Read-Only Storage</span>
      </div>

      <button
        v-if="canWrite && workspaceStore.clipboard"
        @click="handlePaste"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
      >
        <span>📄 Paste Here</span>
        <span class="text-[10px] opacity-75 font-mono">Ctrl+V</span>
      </button>
      <div v-if="canWrite && workspaceStore.clipboard" class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
      <button
        v-if="canWrite"
        @click="uiStore.openCreate('file'); uiStore.closeContextMenu()"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>+ New File</span>
      </button>
      <button
        v-if="canWrite"
        @click="uiStore.openCreate('directory'); uiStore.closeContextMenu()"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>+ New Folder</span>
      </button>
      <div v-if="canWrite" class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
      <button
        v-if="canWrite"
        @click="uiStore.openUpload(); uiStore.closeContextMenu()"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>Upload Files</span>
      </button>
      <button
        @click="workspaceStore.refreshAll(); uiStore.closeContextMenu()"
        class="w-full text-left px-3.5 py-2 hover:bg-gray-100 dark:hover:bg-slate-800 text-gray-500 dark:text-slate-400 flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>↻ Refresh</span>
      </button>
    </div>
  </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { apiClient } from '../../api/client';
import { useFileStore } from '../../stores/fileStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useStarredStore } from '../../stores/starredStore';
import { useUiStore } from '../../stores/uiStore';
import { getDownloadUrl } from '../../api/files';

const emit = defineEmits<{
  (e: 'openArchiveDialog', paths: string[]): void;
  (e: 'openCreateShareDialog', payload: { connectionId: string; path: string }): void;
  (e: 'openPropertiesDialog', payload: { connectionId: string; path: string }): void;
}>();

const fileStore = useFileStore();
const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();
const starredStore = useStarredStore();
const uiStore = useUiStore();

const menuRef = ref<HTMLElement | null>(null);
const posTop = ref(0);
const posLeft = ref(0);

const activeConnectionId = computed(() => {
  return uiStore.contextMenu.connectionId || fileStore.currentConnectionId;
});

const canWrite = computed(() => {
  return connStore.canWrite(activeConnectionId.value);
});

const isItemStarred = computed(() => {
  if (!uiStore.contextMenu.item) return false;
  return starredStore.isStarred(activeConnectionId.value, uiStore.contextMenu.item.path);
});

// Smart Viewport Clamping: Prevents menu from overflowing off-screen
watch(
  () => [uiStore.contextMenu.visible, uiStore.contextMenu.x, uiStore.contextMenu.y],
  async ([visible]) => {
    if (visible) {
      posTop.value = uiStore.contextMenu.y;
      posLeft.value = uiStore.contextMenu.x;

      await nextTick();
      if (!menuRef.value) return;

      const menuWidth = menuRef.value.offsetWidth || 224;
      const menuHeight = menuRef.value.offsetHeight || 380;
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;

      let x = uiStore.contextMenu.x;
      let y = uiStore.contextMenu.y;

      // Adjust horizontal overflow
      if (x + menuWidth > viewportWidth - 8) {
        x = Math.max(8, viewportWidth - menuWidth - 8);
      }

      // Adjust vertical overflow (flip upwards or clamp if near bottom)
      if (y + menuHeight > viewportHeight - 8) {
        y = Math.max(8, viewportHeight - menuHeight - 8);
      }

      posTop.value = y;
      posLeft.value = x;
    }
  }
);

const computedStyle = computed(() => ({
  top: `${posTop.value}px`,
  left: `${posLeft.value}px`,
}));

async function handleEditInEditor() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const connId = activeConnectionId.value;
  uiStore.closeContextMenu();

  try {
    const resp = await apiClient.get(`/connections/${connId}/files/content`, {
      params: { path: item.path },
      responseType: 'text',
    });
    uiStore.openEditor(item, resp.data, resp.headers['etag'] || '');
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to open file in editor', 'error');
  }
}

function handleToggleStar() {
  if (uiStore.contextMenu.item) {
    starredStore.toggleStar(activeConnectionId.value, uiStore.contextMenu.item);
    uiStore.showToast(isItemStarred.value ? 'Added to Starred' : 'Removed from Starred', 'success');
  }
  uiStore.closeContextMenu();
}

async function handleShare() {
  const item = uiStore.contextMenu.item;
  if (!item) return;

  if (uiStore.isMobile && typeof navigator !== 'undefined' && 'share' in navigator) {
    uiStore.closeContextMenu();
    try {
      const downloadUrl = window.location.origin + getDownloadUrl(activeConnectionId.value, item.path);
      await navigator.share({
        title: item.name,
        text: `Sharing ${item.name} from AeroFS`,
        url: downloadUrl,
      });
      return;
    } catch (err: any) {
      if (err.name === 'AbortError') return;
    }
  }

  emit('openCreateShareDialog', {
    connectionId: activeConnectionId.value,
    path: item.path,
  });
  uiStore.closeContextMenu();
}

function handleProperties() {
  if (uiStore.contextMenu.item) {
    emit('openPropertiesDialog', {
      connectionId: activeConnectionId.value,
      path: uiStore.contextMenu.item.path,
    });
  }
  uiStore.closeContextMenu();
}

function handleCopy() {
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  workspaceStore.copySelection(panelId);
  uiStore.showToast('Copied to clipboard', 'info');
  uiStore.closeContextMenu();
}

function handleCut() {
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  workspaceStore.cutSelection(panelId);
  uiStore.showToast('Cut to clipboard', 'info');
  uiStore.closeContextMenu();
}

async function handlePaste() {
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  uiStore.closeContextMenu();
  await workspaceStore.paste(panelId);
}

function isArchive(name: string): boolean {
  const n = name.toLowerCase();
  return n.endsWith('.zip') || n.endsWith('.tar.gz') || n.endsWith('.tgz');
}

function handleOpen() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const connId = activeConnectionId.value;

  if (item.kind === 'directory') {
    workspaceStore.navigateTo(panelId, item.path);
  } else {
    const url = getDownloadUrl(connId, item.path);
    window.open(url, '_blank');
  }
  uiStore.closeContextMenu();
}

function handleOpenInOtherPanel() {
  const item = uiStore.contextMenu.item;
  if (!item || item.kind !== 'directory') return;
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  workspaceStore.openInOtherPanel(panelId, item.path);
  uiStore.closeContextMenu();
}

function handleCompress() {
  if (uiStore.contextMenu.item) {
    emit('openArchiveDialog', [uiStore.contextMenu.item.path]);
  }
  uiStore.closeContextMenu();
}

async function handleExtract() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const connId = activeConnectionId.value;
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  uiStore.closeContextMenu();

  try {
    const parentDir = item.path.substring(0, item.path.lastIndexOf('/')) || '/';
    await apiClient.post(`/connections/${connId}/archive/extract`, {
      archive_path: item.path,
      destination_dir: parentDir,
    });
    uiStore.showToast(`Extracted ${item.name} successfully`, 'success');
    await workspaceStore.fetchPanelEntries(panelId);
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Extraction failed', 'error');
  }
}

function handleRename() {
  if (uiStore.contextMenu.item) {
    uiStore.openRename(uiStore.contextMenu.item);
  }
  uiStore.closeContextMenu();
}

function handleDelete() {
  if (uiStore.contextMenu.item) {
    uiStore.openDelete([uiStore.contextMenu.item.path]);
  }
  uiStore.closeContextMenu();
}

function onWindowClick() {
  if (uiStore.contextMenu.visible) {
    uiStore.closeContextMenu();
  }
}

onMounted(() => {
  window.addEventListener('click', onWindowClick);
});

onUnmounted(() => {
  window.removeEventListener('click', onWindowClick);
});
</script>
