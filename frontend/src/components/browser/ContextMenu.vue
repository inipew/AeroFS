<template>
  <Transition :name="menuTransitionName">
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
            ? 'fixed inset-x-0 bottom-0 rounded-t-3xl border-t border-gray-200 dark:border-slate-800 shadow-2xl p-4 max-h-[85vh] overflow-y-auto pb-safe'
            : 'fixed border border-gray-200 dark:border-slate-700 shadow-2xl rounded-2xl py-1.5 w-60 text-xs'
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
        <FbIcon :name="uiStore.contextMenu.item.kind === 'directory' ? 'folder' : 'file'" size="1.5em" class="text-amber-500 shrink-0" />
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
          <FbIcon name="x" size="1.1em" />
        </button>
      </div>

      <!-- A. ITEM CONTEXT MENU (When an item is clicked) -->
      <div v-if="uiStore.contextMenu.item" :class="uiStore.isMobile ? 'space-y-1' : ''">
        <!-- 1. Open / View / Edit in Code Editor -->
        <template v-if="uiStore.contextMenu.item.kind === 'directory'">
          <button
            @click="handleOpen"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="folder" size="1.1em" class="text-amber-500 shrink-0" />
            <span>Open Folder</span>
          </button>

          <button
            @click="handleOpenInOtherPanel"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="panel-right" size="1.1em" class="shrink-0" />
              <span>Open in Other Panel</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">Ctrl+Enter</span>
          </button>

          <button
            @click="handleSyncFolder"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-indigo-600 dark:text-indigo-400"
          >
            <span class="text-sm shrink-0">🔄</span>
            <span>Sync Folder...</span>
          </button>
        </template>

        <template v-else>
          <!-- Edit in Code Editor -->
          <button
            @click="handleEditInEditor"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
          >
            <FbIcon name="code" size="1.1em" class="text-emerald-500 shrink-0" />
            <span>Edit in Code Editor</span>
          </button>

          <!-- Browse Archive Contents -->
          <button
            v-if="isArchive(uiStore.contextMenu.item.name)"
            @click="handleOpenArchiveViewer"
            class="w-full text-left px-3.5 py-2 hover:bg-amber-500 hover:text-white flex items-center space-x-2.5 transition rounded-xl text-amber-600 dark:text-amber-400 font-semibold cursor-pointer"
          >
            <FbIcon name="archive" size="1.1em" class="shrink-0" />
            <span>Browse Archive Contents</span>
          </button>

          <button
            @click="handleOpen"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="download" size="1.1em" class="text-blue-500 shrink-0" />
            <span>Download / Open</span>
          </button>
        </template>

        <!-- Cross-Pane Dual Pane Actions -->
        <template v-if="workspaceStore.isDualPane">
          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

          <button
            @click="handleCopyToOtherPane"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-indigo-600 dark:text-indigo-400 font-semibold"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="copy" size="1.1em" class="shrink-0" />
              <span>Copy to Other Pane</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">F5</span>
          </button>

          <button
            v-if="canWrite"
            @click="handleMoveToOtherPane"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-indigo-600 dark:text-indigo-400 font-semibold"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="move" size="1.1em" class="shrink-0" />
              <span>Move to Other Pane</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">F6</span>
          </button>
        </template>

        <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

        <!-- Toggle Star Bookmark -->
        <button
          @click="handleToggleStar"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
        >
          <FbIcon name="star" size="1.1em" :class="isItemStarred ? 'text-yellow-500' : 'text-gray-400'" class="shrink-0" />
          <span>{{ isItemStarred ? 'Remove from Starred' : 'Add to Starred' }}</span>
        </button>

        <!-- Share Link -->
        <button
          @click="handleShare"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
        >
          <FbIcon name="share" size="1.1em" class="text-blue-500 shrink-0" />
          <span>Share Link...</span>
        </button>

        <!-- Properties / Permissions -->
        <button
          @click="handleProperties"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
        >
          <FbIcon name="info" size="1.1em" class="text-cyan-500 shrink-0" />
          <span>Properties / Permissions</span>
        </button>

        <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

        <!-- Read-Only Badge -->
        <div v-if="!canWrite" class="px-3.5 py-1.5 mb-1 bg-amber-50 dark:bg-amber-950/40 border-b border-amber-200 dark:border-amber-800/50 text-[11px] font-semibold text-amber-700 dark:text-amber-400 flex items-center space-x-2 rounded-xl">
          <FbIcon name="shield" size="1.1em" class="text-amber-500 shrink-0" />
          <span>Read-Only Storage</span>
        </div>

        <!-- Copy & Cut Operations -->
        <button
          @click="handleCopy"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
        >
          <div class="flex items-center space-x-2.5">
            <FbIcon name="copy" size="1.1em" class="text-indigo-500 shrink-0" />
            <span>Copy</span>
          </div>
          <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+C</span>
        </button>

        <button
          v-if="canWrite"
          @click="handleCut"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
        >
          <div class="flex items-center space-x-2.5">
            <FbIcon name="move" size="1.1em" class="text-rose-500 shrink-0" />
            <span>Cut</span>
          </div>
          <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+X</span>
        </button>

        <button
          v-if="canWrite && workspaceStore.clipboard"
          @click="handlePaste"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
        >
          <div class="flex items-center space-x-2.5">
            <FbIcon name="save" size="1.1em" class="shrink-0" />
            <span>Paste</span>
          </div>
          <span class="text-[10px] opacity-75 font-mono">Ctrl+V</span>
        </button>

        <div v-if="canWrite" class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

        <!-- Extract Archive -->
        <button
          v-if="canWrite && isArchive(uiStore.contextMenu.item.name)"
          @click="handleExtract"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl text-amber-500 cursor-pointer"
        >
          <FbIcon name="archive" size="1.1em" class="shrink-0" />
          <span>Extract Archive Here</span>
        </button>

        <!-- Compress Selected -->
        <button
          v-if="canWrite"
          @click="handleCompress"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
        >
          <FbIcon name="archive" size="1.1em" class="text-cyan-500 shrink-0" />
          <span>Compress...</span>
        </button>

        <!-- Rename -->
        <button
          v-if="canWrite"
          @click="handleRename"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
        >
          <FbIcon name="rename" size="1.1em" class="text-purple-500 shrink-0" />
          <span>Rename</span>
          <span class="text-[10px] text-gray-400 opacity-75 font-mono ml-auto">F2</span>
        </button>

        <!-- Delete -->
        <button
          v-if="canWrite"
          @click="handleDelete"
          class="w-full text-left px-3.5 py-2 hover:bg-red-600 hover:text-white text-red-500 flex items-center justify-between transition rounded-xl cursor-pointer"
        >
          <div class="flex items-center space-x-2.5">
            <FbIcon name="delete" size="1.1em" class="shrink-0" />
            <span>Delete</span>
          </div>
          <span class="text-[10px] opacity-75 font-mono">Del</span>
        </button>
      </div>

      <!-- B. BACKGROUND CONTEXT MENU (When clicked on empty space) -->
      <div v-else class="space-y-0.5">
        <div v-if="!canWrite" class="px-3.5 py-1.5 mb-1 bg-amber-50 dark:bg-amber-950/40 border-b border-amber-200 dark:border-amber-800/50 text-[11px] font-semibold text-amber-700 dark:text-amber-400 flex items-center space-x-2 rounded-xl">
          <FbIcon name="shield" size="1.1em" class="text-amber-500 shrink-0" />
          <span>Read-Only Storage</span>
        </div>

        <button
          v-if="canWrite && workspaceStore.clipboard"
          @click="handlePaste"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400 font-semibold"
        >
          <div class="flex items-center space-x-2.5">
            <FbIcon name="save" size="1.1em" class="shrink-0" />
            <span>Paste Here</span>
          </div>
          <span class="text-[10px] opacity-75 font-mono">Ctrl+V</span>
        </button>

        <div v-if="canWrite && workspaceStore.clipboard" class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

        <button
          v-if="canWrite"
          @click="uiStore.openCreate('file'); uiStore.closeContextMenu()"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
        >
          <FbIcon name="new-file" size="1.1em" class="text-blue-500 shrink-0" />
          <span>+ New File</span>
        </button>

        <button
          v-if="canWrite"
          @click="uiStore.openCreate('directory'); uiStore.closeContextMenu()"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
        >
          <FbIcon name="new-folder" size="1.1em" class="text-amber-500 shrink-0" />
          <span>+ New Folder</span>
        </button>

        <button
          v-if="canWrite"
          @click="uiStore.openUpload(); uiStore.closeContextMenu()"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
        >
          <FbIcon name="upload" size="1.1em" class="text-emerald-500 shrink-0" />
          <span>Upload Files</span>
        </button>

        <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

        <button
          @click="handleSelectAll"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
        >
          <div class="flex items-center space-x-2.5">
            <FbIcon name="select-all" size="1.1em" class="text-indigo-500 shrink-0" />
            <span>Select All</span>
          </div>
          <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+A</span>
        </button>

        <button
          @click="handleToggleViewMode"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
        >
          <FbIcon name="grid" size="1.1em" class="text-gray-500 shrink-0" />
          <span>Toggle Grid / List View</span>
        </button>

        <button
          @click="workspaceStore.refreshAll(); uiStore.closeContextMenu()"
          class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
        >
          <div class="flex items-center space-x-2.5">
            <FbIcon name="refresh" size="1.1em" class="text-gray-500 shrink-0" />
            <span>Refresh Directory</span>
          </div>
          <span class="text-[10px] text-gray-400 opacity-75 font-mono">F5</span>
        </button>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useStarredStore } from '../../stores/starredStore';
import { useUiStore } from '../../stores/uiStore';
import { getDownloadUrl, readFileApi } from '../../api/files';
import { extractArchiveApi } from '../../api/archive';
import { isArchiveFile } from '../../domain/capabilities';
import { normalizeApiError } from '../../utils/errorNormalizer';

const emit = defineEmits<{
  (e: 'openArchiveDialog', paths: string[]): void;
  (e: 'openCreateShareDialog', payload: { connectionId: string; path: string }): void;
  (e: 'openPropertiesDialog', payload: { connectionId: string; path: string }): void;
  (e: 'openArchiveViewer', payload: { connectionId: string; path: string }): void;
}>();

const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();
const starredStore = useStarredStore();
const uiStore = useUiStore();

const menuRef = ref<HTMLElement | null>(null);
const posTop = ref(0);
const posLeft = ref(0);

const menuTransitionName = computed(() => (uiStore.isMobile ? 'ios-bottom-sheet' : 'ios-context-menu'));

const activeConnectionId = computed(() => {
  return uiStore.contextMenu.connectionId || workspaceStore.activePanel.location.connectionId;
});

const canWrite = computed(() => {
  return connStore.canWrite(activeConnectionId.value);
});

const isItemStarred = computed(() => {
  if (!uiStore.contextMenu.item) return false;
  return starredStore.isStarred(activeConnectionId.value, uiStore.contextMenu.item.path);
});

function isArchive(name: string): boolean {
  return isArchiveFile(name);
}

// Smart Viewport Clamping: Prevents menu from overflowing off-screen
watch(
  () => [uiStore.contextMenu.visible, uiStore.contextMenu.x, uiStore.contextMenu.y],
  async ([visible]) => {
    if (visible) {
      posTop.value = uiStore.contextMenu.y;
      posLeft.value = uiStore.contextMenu.x;

      await nextTick();
      if (!menuRef.value) return;

      const menuWidth = menuRef.value.offsetWidth || 240;
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
    const resp = await readFileApi(connId, item.path);
    uiStore.openEditor(item, resp.content, resp.etag, connId);
  } catch (err: unknown) {
    const norm = normalizeApiError(err);
    uiStore.showToast(norm.message || 'Failed to open file in editor', 'error');
  }
}

function handleOpenArchiveViewer() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const connId = activeConnectionId.value;
  uiStore.closeContextMenu();
  emit('openArchiveViewer', {
    connectionId: connId,
    path: item.path,
  });
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
    } catch (err: unknown) {
      // User cancelled share
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

function handleOpen() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;

  if (item.kind === 'directory') {
    workspaceStore.navigateTo(panelId, item.path);
  } else {
    const url = getDownloadUrl(activeConnectionId.value, item.path);
    window.open(url, '_blank');
  }
  uiStore.closeContextMenu();
}

function handleOpenInOtherPanel() {
  const item = uiStore.contextMenu.item;
  if (!item || item.kind !== 'directory') return;
  const sourcePanelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  workspaceStore.openInOtherPanel(sourcePanelId, item.path);
  uiStore.closeContextMenu();
}

function handleSyncFolder() {
  const item = uiStore.contextMenu.item;
  if (!item || item.kind !== 'directory') return;
  const sourcePanelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const panel = workspaceStore.getPanel(sourcePanelId);
  uiStore.openSync(panel.connectionId || 'local', item.path);
  uiStore.closeContextMenu();
}

async function handleCopyToOtherPane() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const sourcePanelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const destPanelId = sourcePanelId === 'left' ? 'right' : 'left';
  const p = workspaceStore.getPanel(sourcePanelId);
  const paths = p.selection.paths.length > 0 ? p.selection.paths : [item.path];

  uiStore.closeContextMenu();
  await workspaceStore.transferBetweenPanels(sourcePanelId, destPanelId, paths, false);
}

async function handleMoveToOtherPane() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const sourcePanelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const destPanelId = sourcePanelId === 'left' ? 'right' : 'left';
  const p = workspaceStore.getPanel(sourcePanelId);
  const paths = p.selection.paths.length > 0 ? p.selection.paths : [item.path];

  uiStore.closeContextMenu();
  await workspaceStore.transferBetweenPanels(sourcePanelId, destPanelId, paths, true);
}

async function handleExtract() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const p = workspaceStore.getPanel(panelId);
  const connId = activeConnectionId.value;

  uiStore.closeContextMenu();
  try {
    await extractArchiveApi(connId, item.path, p.location.path);
    uiStore.showToast(`Extracted ${item.name}`, 'success');
    await workspaceStore.refresh(panelId);
  } catch (err: unknown) {
    const norm = normalizeApiError(err);
    uiStore.showToast(norm.message || 'Failed to extract archive', 'error');
  }
}

function handleCompress() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const p = workspaceStore.getPanel(panelId);
  const paths = p.selection.paths.length > 0 ? p.selection.paths : [item.path];

  emit('openArchiveDialog', paths);
  uiStore.closeContextMenu();
}

function handleRename() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  uiStore.openRename(item);
  uiStore.closeContextMenu();
}

function handleDelete() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const p = workspaceStore.getPanel(panelId);
  const paths = p.selection.paths.length > 0 ? p.selection.paths : [item.path];

  uiStore.openDelete(paths);
  uiStore.closeContextMenu();
}

function handleSelectAll() {
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const p = workspaceStore.getPanel(panelId);
  p.selection.paths = p.runtime.entries.map((e) => e.path);
  uiStore.closeContextMenu();
}

function handleToggleViewMode() {
  const panelId = uiStore.contextMenu.panelId || workspaceStore.activePanelId;
  const p = workspaceStore.getPanel(panelId);
  p.view.viewMode = p.view.viewMode === 'grid' ? 'list' : 'grid';
  workspaceStore.saveState();
  uiStore.closeContextMenu();
}
</script>
