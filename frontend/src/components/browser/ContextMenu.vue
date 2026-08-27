<template>
  <div
    v-if="uiStore.contextMenu.visible"
    ref="menuRef"
    :style="computedStyle"
    class="fixed z-50 bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 shadow-2xl rounded-2xl py-1.5 w-56 text-xs text-gray-700 dark:text-slate-200 select-none animate-in fade-in zoom-in-95 duration-100 font-sans"
    @click.stop
  >
    <div v-if="uiStore.contextMenu.item">
      <!-- 1. Open / View / Edit in Code Editor -->
      <button
        v-if="uiStore.contextMenu.item.kind === 'directory'"
        @click="handleOpen"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>📁 Open Folder</span>
      </button>

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

      <!-- Extract Archive if .zip or .tar.gz -->
      <button
        v-if="isArchive(uiStore.contextMenu.item.name)"
        @click="handleExtract"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl text-amber-500 cursor-pointer"
      >
        <span>📦 Extract Archive Here</span>
      </button>

      <!-- Compress Selected -->
      <button
        @click="handleCompress"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>📦 Compress...</span>
      </button>

      <!-- Rename -->
      <button
        @click="handleRename"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>✏️ Rename</span>
      </button>

      <!-- Delete -->
      <button
        @click="handleDelete"
        class="w-full text-left px-3.5 py-2 hover:bg-red-600 hover:text-white text-red-500 flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>🗑️ Delete</span>
      </button>
    </div>

    <!-- Blank Area Context Menu -->
    <div v-else>
      <button
        @click="uiStore.openCreate('file'); uiStore.closeContextMenu()"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>+ New File</span>
      </button>
      <button
        @click="uiStore.openCreate('directory'); uiStore.closeContextMenu()"
        class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2 transition rounded-xl cursor-pointer"
      >
        <span>+ New Folder</span>
      </button>
      <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
      <button
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
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { apiClient } from '../../api/client';
import { useFileStore } from '../../stores/fileStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
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
const starredStore = useStarredStore();
const uiStore = useUiStore();

const menuRef = ref<HTMLElement | null>(null);
const posTop = ref(0);
const posLeft = ref(0);

const activeConnectionId = computed(() => {
  return uiStore.contextMenu.connectionId || fileStore.currentConnectionId;
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
      const menuHeight = menuRef.value.offsetHeight || 360;
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

function handleShare() {
  if (uiStore.contextMenu.item) {
    emit('openCreateShareDialog', {
      connectionId: activeConnectionId.value,
      path: uiStore.contextMenu.item.path,
    });
  }
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
    workspaceStore.navigatePanel(panelId, item.path);
  } else {
    const url = getDownloadUrl(connId, item.path);
    window.open(url, '_blank');
  }
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
